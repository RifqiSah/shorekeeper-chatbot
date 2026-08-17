use std::{fmt, path::PathBuf};

use anyhow::Context;
use serde::Deserialize;

#[derive(Clone, Deserialize)]
pub struct LlmBackend {
  pub name: String,
  pub api_key: String,
  pub base_url: String,
  pub model: String,

  #[serde(default)]
  pub aig_token: Option<String>, // for CF AI Gateway
}

impl fmt::Debug for LlmBackend {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("LlmBackend")
      .field("name", &self.name)
      .field("base_url", &self.base_url)
      .field("model", &self.model)

      // redacted value
      .field("api_key", &redact(&self.api_key))
      .field("aig_token", &self.aig_token.as_deref().map(redact))
      .finish()
  }
}

fn redact(s: &str) -> String {
  let n = s.len();
  if n <= 8 {
    return "****".into();
  }
  format!("{}...{}", &s[..4], &s[n - 4..])
}

#[derive(Debug, Clone)]
pub struct Config {
  // Redis
  pub redis_host: String,
  pub redis_port: u32,

  // LLM backend
  pub backend_path: PathBuf,
  pub llm_token_limit: u64,

  // Embedding backend
  pub llm_embed_api_key: String,
  pub llm_embed_base_url: String,
  
  // LLM behavior
  pub llm_max_history_messages: usize,
  pub llm_context_ttl_seconds: u64,
  pub llm_semantic_cache_ttl_seconds: u64,
  pub llm_similarity_threshold: f32,
  pub llm_system_prompt: String,
}

impl Config {
  pub fn from_env() -> anyhow::Result<Self> {
    Ok(Self {
      redis_host: std::env::var("REDIS_HOST").unwrap_or_else(|_| "localhost".into()),
      redis_port: std::env::var("REDIS_PORT")
        .unwrap_or_else(|_| "6379".into())
        .parse()
        .unwrap_or(6379),

      backend_path: std::env::var("LLM_BACKENDS_FILE")
        .unwrap_or_else(|_| "config_llm_backends.json".into())
        .into(),
      llm_token_limit: std::env::var("LLM_TOKEN_LIMIT")
        .unwrap_or_else(|_| "5000".into())
        .parse()
        .unwrap_or(5000),
      
      llm_embed_api_key: std::env::var("LLM_EMBED_API_KEY").context("LLM_EMBED_API_KEY must be set")?,
      llm_embed_base_url: std::env::var("LLM_EMBED_BASE_URL").context("EMBED_BASE_URL must be set")?,
      
      llm_max_history_messages: std::env::var("LLM_MAX_HISTORY_MESSAGES")
        .unwrap_or_else(|_| "15".into())
        .parse()
        .unwrap_or(15),
      llm_context_ttl_seconds: std::env::var("LLM_CONTEXT_TTL_SECONDS")
        .unwrap_or_else(|_| "1800".into())
        .parse()
        .unwrap_or(1800),
      llm_semantic_cache_ttl_seconds: std::env::var("LLM_SEMANTIC_CACHE_TTL_SECONDS")
        .unwrap_or_else(|_| "86400".into())
        .parse()
        .unwrap_or(86400),
      llm_similarity_threshold: std::env::var("LLM_IMILARITY_THRESHOLD")
        .unwrap_or_else(|_| "0.92".into())
        .parse()
        .unwrap_or(0.92),
      llm_system_prompt: std::env::var("LLM_SYSTEM_PROMPT")
        .unwrap_or_else(|_| "Kamu adalah asisten AI yang ramah dan helpful.".into())
    })
  }

  pub fn redis_url(&self) -> String {
    format!("redis://{}:{}", self.redis_host, self.redis_port)
  }
}