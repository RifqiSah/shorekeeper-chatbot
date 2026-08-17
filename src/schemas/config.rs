use std::{fmt, path::{Path, PathBuf}};

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
  format!("{}...{}", &s[..3], &s[n - 3..])
}

#[derive(Deserialize)]
struct RawConfig {
  #[serde(default)]
  redis: RawRedis,
  llm: RawLlm,
  #[serde(default)]
  backends: Vec<LlmBackend>,
}

#[derive(Deserialize)]
struct RawRedis {
  #[serde(default = "default_redis_host")]
  host: String,
  #[serde(default = "default_redis_port")]
  port: u32,
}
impl Default for RawRedis {
  fn default() -> Self { Self { host: default_redis_host(), port: default_redis_port() } }
}
fn default_redis_host() -> String { "localhost".into() }
fn default_redis_port() -> u32 { 6379 }

#[derive(Deserialize)]
struct RawLlm {
  #[serde(default = "default_token_limit")]
  token_limit: u64,
  embed: RawEmbed,
  #[serde(default = "default_max_history")]
  max_history_messages: usize,
  #[serde(default = "default_context_ttl")]
  context_ttl_seconds: u64,
  #[serde(default = "default_semantic_ttl")]
  semantic_cache_ttl_seconds: u64,
  #[serde(default = "default_similarity")]
  similarity_threshold: f32,
  #[serde(default = "default_system_prompt")]
  system_prompt: String,
}

#[derive(Deserialize)]
struct RawEmbed {
  api_key: String,
  base_url: String,
}

fn default_token_limit() -> u64 { 5000 }
fn default_max_history() -> usize { 15 }
fn default_context_ttl() -> u64 { 1800 }
fn default_semantic_ttl() -> u64 { 86400 }
fn default_similarity() -> f32 { 0.92 }
fn default_system_prompt() -> String { "Kamu adalah asisten AI yang ramah dan helpful.".into() }

#[derive(Clone)]
pub struct Config {
  pub config_path: PathBuf,

  // Redis
  pub redis_host: String,
  pub redis_port: u32,
  
  // Embedding backend
  pub llm_embed_api_key: String,
  pub llm_embed_base_url: String,
  
  // LLM behavior
  pub llm_max_history_messages: usize,
  pub llm_context_ttl_seconds: u64,
  pub llm_semantic_cache_ttl_seconds: u64,
  pub llm_similarity_threshold: f32,
  pub llm_system_prompt: String,
  pub llm_token_limit: u64,
}

impl fmt::Debug for Config {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("LlmBackend")
      .field("config_path", &self.config_path)
      .field("redis_host", &self.redis_host)
      .field("llm_max_history_messages", &self.llm_max_history_messages)
      .field("llm_context_ttl_seconds", &self.llm_context_ttl_seconds)
      .field("llm_semantic_cache_ttl_seconds", &self.llm_semantic_cache_ttl_seconds)
      .field("llm_similarity_threshold", &self.llm_similarity_threshold)
      .field("llm_system_prompt", &self.llm_system_prompt)
      .field("llm_token_limit", &self.llm_token_limit)

      // redacted value
      .field("llm_embed_api_key", &redact(&self.llm_embed_api_key))
      .field("llm_embed_base_url", &redact(&self.llm_embed_base_url))
      .finish()
  }
}

impl Config {
  pub fn from_file(path: impl AsRef<Path>) -> anyhow::Result<(Self, Vec<LlmBackend>)> {
    let path = path.as_ref();
    let raw = std::fs::read_to_string(path)
      .with_context(|| format!("Failed to read {}", path.display()))?;

    let parsed: RawConfig = toml::from_str(&raw)
      .with_context(|| format!("Invalid TOML in {}", path.display()))?;

    anyhow::ensure!(!parsed.backends.is_empty(), "{} must contain at least 1 [[backends]] entry", path.display());

    let config = Self {
      config_path: path.to_path_buf(),
      redis_host: parsed.redis.host,
      redis_port: parsed.redis.port,
      llm_embed_api_key: parsed.llm.embed.api_key,
      llm_embed_base_url: parsed.llm.embed.base_url,
      llm_max_history_messages: parsed.llm.max_history_messages,
      llm_context_ttl_seconds: parsed.llm.context_ttl_seconds,
      llm_semantic_cache_ttl_seconds: parsed.llm.semantic_cache_ttl_seconds,
      llm_similarity_threshold: parsed.llm.similarity_threshold,
      llm_system_prompt: parsed.llm.system_prompt,
      llm_token_limit: parsed.llm.token_limit,
    };

    Ok((config, parsed.backends))
  }

  pub fn redis_url(&self) -> String {
    format!("redis://{}:{}", self.redis_host, self.redis_port)
  }
}