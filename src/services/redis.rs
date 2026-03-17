use anyhow::Result;
use redis::{AsyncCommands, Client, aio::ConnectionManager};
use serde_json;

use crate::schemas::chat::{ChatMessage, SemanticCacheEntry};

pub struct RedisService {
  conn: ConnectionManager,
}

#[derive(Debug)]
pub struct RateLimitResult {
  pub allowed: bool,
  pub current: u64,
  pub limit: u64,
  pub remaining: u64,
}

impl RedisService {
  pub async fn new(redis_url: &str) -> Result<Self> {
    let client = Client::open(redis_url)?;
    let conn = ConnectionManager::new(client).await?;
    Ok(Self { conn })
  }

  fn history_key(user_id: &str, guild_id: Option<&str>) -> String {
    match guild_id {
      Some(gid) => format!("sk_ai:chat:{}:{}", gid, user_id),
      None => format!("sk_ai:chat:dm:{}", user_id),
    }
  }

  pub async fn get_history(&self, user_id: &str, guild_id: Option<&str>) -> Result<Vec<ChatMessage>> {
    let key = Self::history_key(user_id, guild_id);
    let mut conn = self.conn.clone();

    let raw: Option<String> = conn.get::<_, Option<String>>(&key).await?;
    match raw {
      Some(s) => Ok(serde_json::from_str(&s)?),
      None => Ok(vec![]),
    }
  }

  pub async fn save_history(
    &self,
    user_id: &str,
    guild_id: Option<&str>,
    messages: &[ChatMessage],
    max_messages: usize,
    ttl_seconds: u64,
  ) -> Result<()> {
    let key = Self::history_key(user_id, guild_id);
    let mut conn = self.conn.clone();

    let trimmed = if messages.len() > max_messages {
      &messages[messages.len() - max_messages..]
    } else {
      messages
    };

    let serialized = serde_json::to_string(trimmed)?;

    conn.set_ex::<_, _, ()>(&key, serialized, ttl_seconds).await?;

    Ok(())
  }

  pub async fn clear_history(&self, user_id: &str, guild_id: Option<&str>) -> Result<()> {
    let key = Self::history_key(user_id, guild_id);
    let mut conn = self.conn.clone();

    conn.del::<_, ()>(&key).await?;

    Ok(())
  }

  pub async fn save_semantic_cache(&self, entry: &SemanticCacheEntry, ttl_seconds: u64) -> Result<()> {
    let mut conn = self.conn.clone();
    let key = format!("sk_ai:semcache:{}", hash_question(&entry.question));
    let serialized = serde_json::to_string(entry)?;

    conn.set_ex::<_, _, ()>(&key, serialized, ttl_seconds).await?;
    conn.lpush::<_, _, ()>("sk_ai:semcache:index", &key).await?;
    conn.ltrim::<_, ()>("sk_ai:semcache:index", 0, 999).await?;

    Ok(())
  }

  pub async fn find_similar_cache(&self, query_embedding: &[f32], threshold: f32) -> Result<Option<String>> {
    let mut conn = self.conn.clone();
    let keys: Vec<String> = conn.lrange::<_, Vec<String>>("sk_ai:semcache:index", 0, -1).await?;

    let mut best_score = 0.0f32;
    let mut best_answer: Option<String> = None;
    let mut best_question = String::new();

    for key in keys {
      let raw: Option<String> = conn.get::<_, Option<String>>(&key).await?;
      if let Some(s) = raw {
        if let Ok(entry) = serde_json::from_str::<SemanticCacheEntry>(&s) {
          let score = cosine_similarity(query_embedding, &entry.question_embedding);
          tracing::debug!(
            "Cache candidate: score={:.3} threshold={:.2} q={:?}",
            score, threshold,
            &entry.question.chars().take(60).collect::<String>()
          );

          if score > best_score && score >= threshold {
            best_score = score;
            best_answer = Some(entry.answer.clone());
            best_question = entry.question.clone();
          }
        }
      }
    }

    if best_answer.is_some() {
      tracing::info!(
        "Cache HIT score={:.3} q={:?}",
        best_score,
        &best_question.chars().take(60).collect::<String>()
      );
    } else {
      tracing::debug!("Cache MISS (best score below threshold {:.2})", threshold);
    }

    Ok(best_answer)
  }

  fn global_rate_limit_key() -> String {
    let today = std::time::SystemTime::now()
      .duration_since(std::time::UNIX_EPOCH)
      .unwrap_or_default()
      .as_secs() / 86400;
    format!("sk_ai:ratelimit:global:{}", today)
  }

  pub async fn get_global_usage(&self) -> Result<u64> {
    let mut conn = self.conn.clone();
    let key = Self::global_rate_limit_key();

    let count: Option<u64> = conn.get::<_, Option<u64>>(&key).await?;

    Ok(count.unwrap_or(0))
  }

  pub async fn add_token_usage(&self, tokens_used: u64, daily_token_limit: u64) -> Result<RateLimitResult> {
    let mut conn = self.conn.clone();
    let key = Self::global_rate_limit_key();
    let total: u64 = conn.incr::<_, _, u64>(&key, tokens_used).await?;

    if total == tokens_used {
      conn.expire::<_, ()>(&key, 86400).await?;
    }

    Ok(RateLimitResult {
      allowed: true,
      current: total,
      limit: daily_token_limit,
      remaining: daily_token_limit.saturating_sub(total),
    })
  }

  pub async fn check_token_quota(&self, daily_token_limit: u64) -> Result<RateLimitResult> {
    let used = self.get_global_usage().await?;

    Ok(RateLimitResult {
      allowed: used < daily_token_limit,
      current: used,
      limit: daily_token_limit,
      remaining: daily_token_limit.saturating_sub(used),
    })
  }

  pub async fn ping(&self) -> bool {
    let mut conn = self.conn.clone();
    redis::cmd("PING")
      .query_async::<String>(&mut conn)
      .await
      .is_ok()
  }
}


pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
  if a.len() != b.len() || a.is_empty() {
    return 0.0;
  }

  let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
  let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
  let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

  if norm_a == 0.0 || norm_b == 0.0 {
    return 0.0;
  }

  dot / (norm_a * norm_b)
}

fn hash_question(q: &str) -> String {
  use std::collections::hash_map::DefaultHasher;
  use std::hash::{Hash, Hasher};
  
  let mut hasher = DefaultHasher::new();
  q.hash(&mut hasher);
  format!("{:x}", hasher.finish())
}
