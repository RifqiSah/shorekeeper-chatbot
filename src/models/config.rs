use anyhow::Context;

#[derive(Debug, Clone)]
pub struct Config {
  // LLM backend
  pub llm_api_key: String,
  pub llm_model: String,
  pub llm_base_url: String,

  // Embedding backend
  pub embed_api_key: String,
  pub embed_base_url: String,
  
  // Redis
  pub redis_url: String,

  // API
  pub port: u16,
  pub api_secret_key: String,

  // LLM behavior
  pub max_history_messages: usize,
  pub context_ttl_seconds: u64,
  pub semantic_cache_ttl_seconds: u64,
  pub similarity_threshold: f32,
  pub system_prompt: String,
}

impl Config {
  pub fn from_env() -> anyhow::Result<Self> {
    let llm_api_key = std::env::var("LLM_API_KEY")
      .context("LLM_API_KEY must be set")?;

    let embed_api_key = std::env::var("EMBED_API_KEY")
      .unwrap_or_else(|_| llm_api_key.clone());

    Ok(Self {
      llm_api_key,
      llm_model: std::env::var("LLM_MODEL")
        .unwrap_or_else(|_| "@cf/meta/llama-3.3-70b-instruct-fp8-fast".into()),
      llm_base_url: std::env::var("LLM_BASE_URL")
        .context("LLM_BASE_URL must be set")?,

      embed_api_key,
      embed_base_url: std::env::var("EMBED_BASE_URL")
        .context("EMBED_BASE_URL must be set")?,
          
      redis_url: std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://localhost:6379".into()),

      port: std::env::var("PORT")
        .unwrap_or_else(|_| "3000".into())
        .parse()
        .context("PORT must be a valid number")?,
      api_secret_key: std::env::var("API_SECRET_KEY")
        .context("API_SECRET_KEY must be set")?,

      max_history_messages: std::env::var("MAX_HISTORY_MESSAGES")
        .unwrap_or_else(|_| "15".into())
        .parse()
        .unwrap_or(15),
      context_ttl_seconds: std::env::var("CONTEXT_TTL_SECONDS")
        .unwrap_or_else(|_| "1800".into())
        .parse()
        .unwrap_or(1800),
      semantic_cache_ttl_seconds: std::env::var("SEMANTIC_CACHE_TTL_SECONDS")
        .unwrap_or_else(|_| "86400".into())
        .parse()
        .unwrap_or(86400),
      similarity_threshold: std::env::var("SIMILARITY_THRESHOLD")
        .unwrap_or_else(|_| "0.92".into())
        .parse()
        .unwrap_or(0.92),
      system_prompt: std::env::var("SYSTEM_PROMPT")
        .unwrap_or_else(|_| "Kamu adalah asisten AI yang ramah dan helpful.".into()),
    })
  }
}