use axum::{extract::{Query, State}, http::StatusCode, Json};
use serde::{Deserialize, Serialize};

use crate::AppState;

#[derive(Deserialize)]
pub struct UsageQuery {
  user_id: Option<String>,
  guild_id: Option<String>,
}

#[derive(Serialize)]
pub struct UsageResponse {
  global: GlobalUsage,
  user: Option<UserInfo>,
  resets_at: u64,
  resets_in_seconds: u64,
}

#[derive(Serialize)]
pub struct GlobalUsage {
  pub tokens_used_today: u64,
  pub daily_limit: u64,
  pub tokens_remaining: u64,
  pub note: &'static str,
}

#[derive(Serialize)]
pub struct UserInfo {
  pub user_id: String,
  pub guild_id: Option<String>,
  pub history_messages: u64,
}

pub async fn usage_handler(
  State(state): State<AppState>,
  Query(params): Query<UsageQuery>,
) -> Result<Json<UsageResponse>, (StatusCode, Json<serde_json::Value>)> {
  let now_secs = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .unwrap_or_default()
    .as_secs();

  let next_reset = ((now_secs / 86400) + 1) * 86400;
  let resets_in = next_reset.saturating_sub(now_secs);

  // Global usage
  let global_used = state.redis.get_global_usage().await.unwrap_or(0);

  // Specific user information
  let user_info = if let Some(ref user_id) = params.user_id {
    let guild_id = params.guild_id.as_deref();
    let history = state.redis.get_history(user_id, guild_id).await.unwrap_or_default();

    Some(UserInfo {
      user_id: user_id.clone(),
      guild_id: params.guild_id.clone(),
      history_messages: history.len() as u64,
    })
  } else {
    None
  };

  Ok(Json(UsageResponse {
    global: GlobalUsage {
      tokens_used_today: global_used,
      daily_limit: state.config.llm_token_limit,
      tokens_remaining: state.config.llm_token_limit.saturating_sub(global_used),
      note: "Shared across all users. Resets daily at 00:00 UTC.",
    },
    user: user_info,
    resets_at: next_reset,
    resets_in_seconds: resets_in,
  }))
}