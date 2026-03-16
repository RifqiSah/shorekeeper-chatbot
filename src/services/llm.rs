use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::models::chat::ChatMessage;

pub struct LlmService {
  client: Client,

  // LLM
  llm_api_key: String,
  llm_base_url: String,
  llm_aig_token: Option<String>,
  pub model: String,

  // Embedding, independent
  embed_api_key: String,
  embed_base_url: String,
}

// Chat request
#[derive(Serialize)]
struct ChatCompletionRequest {
  model: String,
  messages: Vec<ChatMessage>,
  max_tokens: u32,
  temperature: f32,
}

#[derive(Deserialize)]
struct ChatCompletionResponse {
  choices: Vec<Choice>,
  usage: Option<Usage>,
}

#[derive(Deserialize)]
struct Choice {
  message: ChatMessage,
}

#[derive(Deserialize)]
struct Usage {
  total_tokens: u32,
}

// CF Workers AI Embedding types
#[derive(Serialize)]
struct CfEmbeddingRequest {
  text: Vec<String>,
}

#[derive(Deserialize)]
struct CfEmbeddingResponse {
  result: CfEmbeddingResult,
}

#[derive(Deserialize)]
struct CfEmbeddingResult {
  data: Vec<Vec<f32>>,
}

/// Services
impl LlmService {
  pub fn new(
    llm_api_key: String,
    llm_base_url: String,
    llm_aig_token: Option<String>,
    model: String,
    embed_api_key: String,
    embed_base_url: String,
  ) -> Self {
    let client = Client::builder()
      .timeout(std::time::Duration::from_secs(30))
      .build()
      .expect("Failed to build HTTP client");

    Self { client, llm_api_key, llm_base_url, llm_aig_token, model, embed_api_key, embed_base_url }
  }

  /// Chat completion — OpenAI-compatible
  pub async fn chat(
    &self,
    messages: Vec<ChatMessage>,
    max_tokens: u32,
  ) -> Result<(String, Option<u32>)> {
    let url = format!("{}/chat/completions", self.llm_base_url);

    let body = ChatCompletionRequest {
      model: self.model.clone(),
      messages,
      max_tokens,
      temperature: 0.7,
    };

    let mut req = self
      .client
      .post(&url)
      .bearer_auth(&self.llm_api_key)
      .json(&body);

    if let Some(ref aig_token) = self.llm_aig_token {
      req = req.header("cf-aig-authorization", format!("Bearer {}", aig_token));
    }

    let response = req.send().await.context("Failed to send request to LLM")?;
    if !response.status().is_success() {
      let status = response.status();
      let text = response.text().await.unwrap_or_default();
      anyhow::bail!("LLM API error {}: {}", status, text);
    }

    let data: ChatCompletionResponse = response
      .json()
      .await
      .context("Failed to parse LLM response")?;

    let reply = data
      .choices
      .into_iter()
      .next()
      .map(|c| c.message.content)
      .unwrap_or_else(|| "Maaf, tidak ada respon.".into());

    let tokens = data.usage.map(|u| u.total_tokens);

    Ok((reply, tokens))
  }

  /// Generate embedding with EMBED_BASE_URL, independen from LLM backend
  /// Default: CF Workers AI @cf/baai/bge-small-en-v1.5
  pub async fn embed(&self, text: &str) -> Result<Vec<f32>> {
    let body = CfEmbeddingRequest {
      text: vec![text.to_string()],
    };

    let response = self
      .client
      .post(&self.embed_base_url)
      .bearer_auth(&self.embed_api_key)
      .json(&body)
      .send()
      .await
      .context("Failed to send embedding request")?;

    if !response.status().is_success() {
      let status = response.status();
      let err_text = response.text().await.unwrap_or_default();

      tracing::warn!("Embedding error {} — {}, using fallback", status, err_text);
      
      return Ok(simple_embedding(text));
    }

    let data: CfEmbeddingResponse = response
      .json()
      .await
      .context("Failed to parse embedding response")?;

    data.result
      .data
      .into_iter()
      .next()
      .context("No embedding returned")
  }
}

/// Fallback if embedding not available
fn simple_embedding(text: &str) -> Vec<f32> {
  let mut vec = vec![0.0f32; 384];
  let text_lower = text.to_lowercase();
  for (i, ch) in text_lower.chars().enumerate() {
    let idx = (ch as usize + i) % 384;
    vec[idx] += 1.0;
  }

  let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
  if norm > 0.0 {
    vec.iter_mut().for_each(|x| *x /= norm);
  }
  
  vec
}
