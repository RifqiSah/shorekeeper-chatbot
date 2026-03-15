use anyhow::Result;
use redis::{AsyncCommands, Client, aio::ConnectionManager};
use serde_json;

use crate::models::chat::{ChatMessage, SemanticCacheEntry};

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

  // Conv history
  fn history_key(user_id: &str, guild_id: Option<&str>) -> String {
    match guild_id {
      Some(gid) => format!("sk_ai:chat:{}:{}", gid, user_id),
      None => format!("sk_ai:chat:dm:{}", user_id),
    }
  }

  /// Get user conv history
  pub async fn get_history(
    &self,
    user_id: &str,
    guild_id: Option<&str>,
  ) -> Result<Vec<ChatMessage>> {
    let key = Self::history_key(user_id, guild_id);
    let mut conn = self.conn.clone();

    let raw: Option<String> = conn.get(&key).await?;
    match raw {
      Some(s) => Ok(serde_json::from_str(&s)?),
      None => Ok(vec![]),
    }
  }

  /// Save conv history, trim if too long
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

    // Ambil hanya N pesan terakhir (sliding window)
    let trimmed = if messages.len() > max_messages {
      &messages[messages.len() - max_messages..]
    } else {
      messages
    };

    let serialized = serde_json::to_string(trimmed)?;
    conn.set_ex::<_, _, ()>(&key, serialized, ttl_seconds).await?;

    Ok(())
  }

  /// Hapus history (reset context)
  pub async fn clear_history(&self, user_id: &str, guild_id: Option<&str>) -> Result<()> {
    let key = Self::history_key(user_id, guild_id);
    let mut conn = self.conn.clone();

    conn.del::<_, ()>(&key).await?;
    
    Ok(())
  }

  /// Save LLM response to semantic cache (with embedding)
  pub async fn save_semantic_cache(
    &self,
    entry: &SemanticCacheEntry,
    ttl_seconds: u64,
  ) -> Result<()> {
    let mut conn = self.conn.clone();

    // Key from question hash
    let key = format!("sk_ai:semcache:{}", hash_question(&entry.question));
    let serialized = serde_json::to_string(entry)?;
    conn.set_ex::<_, _, ()>(&key, serialized, ttl_seconds).await?;

    // Add to index list for scanning
    let index_key = "sk_ai:semcache:index";
    conn.lpush::<_, _, ()>(index_key, &key).await?;
    conn.ltrim::<_, ()>(index_key, 0, 999).await?; // max 1000 cache entries

    Ok(())
  }

  /// Search semantic similarity cache, returns (answer, similarity_score) if exceeds the threshold
  pub async fn find_similar_cache(
    &self,
    query_embedding: &[f32],
    threshold: f32,
  ) -> Result<Option<String>> {
    let mut conn = self.conn.clone();

    // Get all keys from index list
    let keys: Vec<String> = conn.lrange("sk_ai:semcache:index", 0, -1).await?;

    let mut best_score = 0.0f32;
    let mut best_answer: Option<String> = None;

    for key in keys {
      let raw: Option<String> = conn.get(&key).await?;
      if let Some(s) = raw {
        if let Ok(entry) = serde_json::from_str::<SemanticCacheEntry>(&s) {
          let score = cosine_similarity(query_embedding, &entry.question_embedding);
          if score > best_score && score >= threshold {
            best_score = score;
            best_answer = Some(entry.answer.clone());
          }
        }
      }
    }

    if best_answer.is_some() {
      tracing::debug!("Semantic cache HIT (score: {:.3})", best_score);
    }

    Ok(best_answer)
  }

  /// Rate limit
  pub async fn check_rate_limit(
    &self,
    user_id: &str,
    guild_id: Option<&str>,
    daily_limit: u64,
  ) -> Result<RateLimitResult> {
    let mut conn = self.conn.clone();

    // Reset every day, use UTC for suffix
    let today = {
      use std::time::{SystemTime, UNIX_EPOCH};
      let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
      secs / 86400 // hari ke-N sejak epoch
    };

    let key = match guild_id {
      Some(gid) => format!("sk_ai:ratelimit:{}:{}:{}", gid, user_id, today),
      None => format!("sk_ai:ratelimit:dm:{}:{}", user_id, today),
    };

    // INCR atomic
    let count: u64 = conn.incr::<_, _, u64>(&key, 1u64).await?;

    // Set TTL only for first request
    if count == 1 {
      conn.expire::<_, ()>(&key, 86400).await?;
    }

    Ok(RateLimitResult {
      allowed: count <= daily_limit,
      current: count,
      limit: daily_limit,
      remaining: daily_limit.saturating_sub(count),
    })
  }

  /// Ping for health check
  pub async fn ping(&self) -> bool {
    let mut conn = self.conn.clone();
    redis::cmd("PING")
      .query_async::<_, String>(&mut conn)
      .await
      .is_ok()
  }
}

/// Cosine similarity between two vectors embedding
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

/// Simple hash for Redis, using uuid based on content
fn hash_question(q: &str) -> String {
  use std::collections::hash_map::DefaultHasher;
  use std::hash::{Hash, Hasher};
  
  let mut hasher = DefaultHasher::new();
  q.hash(&mut hasher);
  format!("{:x}", hasher.finish())
}