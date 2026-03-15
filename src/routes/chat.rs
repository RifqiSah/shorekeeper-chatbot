use axum::{extract::State, http::StatusCode, Json};
use serde_json::json;

use crate::{
  AppState, models::chat::{ChatMessage, ChatRequest, ChatResponse, SemanticCacheEntry}, services::redis::RateLimitResult
};

pub async fn chat_handler(
  State(state): State<AppState>,
  Json(payload): Json<ChatRequest>,
) -> Result<Json<ChatResponse>, (StatusCode, Json<serde_json::Value>)> {
  let user_id = &payload.user_id;
  let guild_id = payload.guild_id.as_deref();
  let user_message = payload.message.trim().to_string();

  if user_message.is_empty() {
    return Err((
      StatusCode::BAD_REQUEST,
      Json(json!({ "error": "Message cannot be empty" })),
    ));
  }

  // Reset context if needed
  if payload.reset_context {
    state.redis.clear_history(user_id, guild_id).await.ok();
    tracing::info!("Context reset for user: {}", user_id);
  }

  tracing::debug!("Chat request - user: {}, guild: {:?}", user_id, guild_id);

  // Check Semantic Cache
  let query_embedding = state
    .llm
    .embed(&user_message)
    .await
    .unwrap_or_else(|_| vec![]);

  if !query_embedding.is_empty() {
    match state
      .redis
      .find_similar_cache(&query_embedding, state.config.similarity_threshold)
      .await
    {
      Ok(Some(cached_answer)) => {
        tracing::info!("Semantic cache HIT for user: {}", user_id);
        return Ok(Json(ChatResponse {
          reply: cached_answer,
          from_cache: true,
          tokens_used: None,
        }));
      }
      Ok(None) => tracing::debug!("Semantic cache MISS"),
      Err(e) => tracing::warn!("Semantic cache error: {}", e),
    }
  }

  // Check rate limit
  match state
    .redis
    .check_rate_limit(user_id, guild_id, state.config.daily_request_limit)
    .await {
      Ok(RateLimitResult { allowed: false, current, limit, .. }) => {
        tracing::warn!("Rate limit exceeded - user: {}, count: {}/{}", user_id, current, limit);
        return Err((
          StatusCode::TOO_MANY_REQUESTS,
          Json(json!({
            "error": "Daily request limit reached. Try again tomorrow.",
            "limit": limit,
            "used": current,
          })),
        ));
      },
      Ok(r) => tracing::debug!("Rate limit ok - user: {}, {}/{}", user_id, r.current, r.limit),
      Err(e) => tracing::warn!("Rate limit check failed ({}), allowing request", e),
    }

  // Get conv history
  let mut history = state
    .redis
    .get_history(user_id, guild_id)
    .await
    .unwrap_or_default();

  // Build message for LLM
  let mut messages: Vec<ChatMessage> = vec![ChatMessage {
    role: "system".into(),
    content: state.config.system_prompt.clone(),
  }];
  messages.extend(history.clone());
  messages.push(ChatMessage {
    role: "user".into(),
    content: user_message.clone(),
  });

  // Call LLM
  let (reply, tokens_used) = state
    .llm
    .chat(messages, 1024)
    .await
    .map_err(|e| {
      tracing::error!("API error: {}", e);
      (
        StatusCode::BAD_GATEWAY,
        Json(json!({ "error": "LLM service error", "detail": e.to_string() })),
      )
    })?;

  tracing::info!(
    "LLM response - user: {}, tokens: {:?}",
    user_id,
    tokens_used
  );

  // Update conv history
  history.push(ChatMessage {
    role: "user".into(),
    content: user_message.clone(),
  });
  history.push(ChatMessage {
    role: "assistant".into(),
    content: reply.clone(),
  });

  state
    .redis
    .save_history(
      user_id,
      guild_id,
      &history,
      state.config.max_history_messages,
      state.config.context_ttl_seconds,
    )
    .await
    .ok();

  // Save to semantic cache
  if !query_embedding.is_empty() {
    let cache_entry = SemanticCacheEntry {
      question_embedding: query_embedding,
      question: user_message.clone(),
      answer: reply.clone(),
    };
    state
      .redis
      .save_semantic_cache(&cache_entry, state.config.semantic_cache_ttl_seconds)
      .await
      .ok();
  }

  Ok(Json(ChatResponse {
    reply,
    from_cache: false,
    tokens_used,
  }))
}