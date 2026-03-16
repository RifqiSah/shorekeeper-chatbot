use serde::{Deserialize, Serialize};

/// Request DTO
#[derive(Debug, Deserialize)]
pub struct ChatRequest {
  pub user_id: String,
  pub guild_id: Option<String>,
  pub message: String,

  #[serde(default)]
  pub reset_context: bool,
}

/// Response DTO
#[derive(Debug, Serialize)]
pub struct ChatResponse {
  pub reply: String,
  pub from_cache: bool,
  pub tokens_used: Option<u32>,
}

/// conv history
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatMessage {
  pub role: String, // "user" | "assistant" | "system"
  pub content: String,
}

/// Semantic entry cache, saved in Redis
#[derive(Debug, Serialize, Deserialize)]
pub struct SemanticCacheEntry {
  pub question_embedding: Vec<f32>,
  pub question: String,
  pub answer: String,
}