use anyhow::Context;

#[derive(Debug, Clone)]
pub struct Config {
  // Redis
  pub redis_host: String,
  pub redis_port: u32,

  // LLM backend
  pub llm_api_key: String,
  pub llm_model: String,
  pub llm_base_url: String,
  pub llm_aig_token: Option<String>,
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
    let llm_api_key = std::env::var("LLM_API_KEY")
      .context("LLM_API_KEY must be set")?;

    let llm_embed_api_key = std::env::var("LLM_EMBED_API_KEY")
      .unwrap_or_else(|_| llm_api_key.clone());

    Ok(Self {
      redis_host: std::env::var("REDIS_HOST").unwrap_or_else(|_| "localhost".into()),
      redis_port: std::env::var("REDIS_PORT")
        .unwrap_or_else(|_| "6379".into())
        .parse()
        .unwrap_or(6379),
      
      llm_api_key,
      llm_model: std::env::var("LLM_MODEL").unwrap_or_else(|_| "@cf/meta/llama-3.3-70b-instruct-fp8-fast".into()),
      llm_base_url: std::env::var("LLM_BASE_URL").context("LLM_BASE_URL must be set")?,
      llm_aig_token: std::env::var("LLM_AIG_TOKEN").ok(),
      llm_token_limit: std::env::var("LLM_TOKEN_LIMIT")
        .unwrap_or_else(|_| "5000".into())
        .parse()
        .unwrap_or(5000),
      
      llm_embed_api_key,
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