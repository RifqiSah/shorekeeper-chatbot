use thiserror::Error;

#[derive(Debug, Error)]
pub enum ChatbotError {
  #[error("Message cannot be empty")]
  EmptyMessage,

  #[error("Daily token quota reached ({current}/{limit}). Try again tomorrow.")]
  QuotaExceeded { current: u64, limit: u64 },

  #[error("LLM service error: {0}")]
  LlmError(String),

  #[error("Redis error: {0}")]
  RedisError(String),

  #[error("Unexpected error: {0}")]
  Other(String),
}

impl From<anyhow::Error> for ChatbotError {
  fn from(err: anyhow::Error) -> Self {
    ChatbotError::Other(err.to_string())
  }
}
