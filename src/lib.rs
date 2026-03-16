mod schemas;
mod services;

use std::sync::Arc;

pub use schemas::chat::{ChatMessage, ChatRequest, ChatResponse, SemanticCacheEntry};
pub use schemas::config::Config;
pub use services::llm::LlmService;
pub use services::redis::{RedisService, RateLimitResult};

pub struct Chatbot {
  pub llm: Arc<LlmService>,
  pub redis: Arc<RedisService>,
  pub config: Arc<Config>,
}

impl Chatbot {
  /// Init dari config — redis_url diambil dari config.redis_url()
  pub async fn new() -> anyhow::Result<Self> {
    let config = Arc::new(Config::from_env()?);
    let redis = Arc::new(RedisService::new(&config.redis_url()).await?);
    let llm = Arc::new(LlmService::new(
      config.llm_api_key.clone(),
      config.llm_base_url.clone(),
      config.llm_aig_token.clone(),
      config.llm_model.clone(),
      config.llm_embed_api_key.clone(),
      config.llm_embed_base_url.clone(),
    ));

    Ok(Self { llm, redis, config })
  }

  pub async fn handle_message(
    &self,
    user_id: &str,
    guild_id: Option<&str>,
    message: &str,
    reset_context: bool,
  ) -> anyhow::Result<ChatResponse> {
    let message = message.trim().to_string();

    if message.is_empty() {
      anyhow::bail!("Message cannot be empty");
    }

    if reset_context {
      self.redis.clear_history(user_id, guild_id).await.ok();
    }

    // semantic cache
    let query_embedding = self.llm.embed(&message).await.unwrap_or_else(|_| vec![]);
    if !query_embedding.is_empty() {
      if let Ok(Some(cached)) = self.redis
        .find_similar_cache(&query_embedding, self.config.llm_similarity_threshold)
        .await
      {
        tracing::info!("Semantic cache HIT for user: {}", user_id);
        return Ok(ChatResponse { reply: cached, from_cache: true, tokens_used: None });
      }
    }

    // get token quota
    let quota = self.redis.check_token_quota(self.config.llm_token_limit).await?;
    if !quota.allowed {
      anyhow::bail!(
        "Daily token quota reached ({}/{}). Try again tomorrow.",
        quota.current, quota.limit
      );
    }

    // build message
    let mut history = self.redis.get_history(user_id, guild_id).await.unwrap_or_default();

    let mut messages = vec![ChatMessage {
      role: "system".into(),
      content: self.config.llm_system_prompt.clone(),
    }];

    messages.extend(history.clone());
    messages.push(ChatMessage { role: "user".into(), content: message.clone() });

    // call LLM
    let (reply, tokens_used) = self.llm.chat(messages, 1024).await?;

    tracing::info!("LLM response - user: {}, tokens: {:?}", user_id, tokens_used);

    // save new token usage info
    if let Some(tokens) = tokens_used {
      self.redis.add_token_usage(tokens as u64, self.config.llm_token_limit).await.ok();
    }

    // save conversation history
    history.push(ChatMessage { role: "user".into(), content: message.clone() });
    history.push(ChatMessage { role: "assistant".into(), content: reply.clone() });

    self.redis.save_history(
      user_id, guild_id, &history,
      self.config.llm_max_history_messages,
      self.config.llm_context_ttl_seconds,
    ).await.ok();

    // save semantic cache
    if !query_embedding.is_empty() {
      self.redis.save_semantic_cache(
        &SemanticCacheEntry {
          question_embedding: query_embedding,
          question: message,
          answer: reply.clone(),
        },
        self.config.llm_semantic_cache_ttl_seconds,
      ).await.ok();
    }

    Ok(ChatResponse { reply, from_cache: false, tokens_used })
  }

  pub async fn get_usage(&self) -> anyhow::Result<RateLimitResult> {
    self.redis.check_token_quota(self.config.llm_token_limit).await
  }
}
