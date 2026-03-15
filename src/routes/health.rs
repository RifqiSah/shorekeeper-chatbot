use axum::{extract::State, Json};
use crate::{models::chat::HealthResponse, AppState};

pub async fn health_handler(State(state): State<AppState>) -> Json<HealthResponse> {
  let redis_ok = state.redis.ping().await;
  Json(HealthResponse {
    status: if redis_ok { "ok".into() } else { "degraded".into() },
    redis: redis_ok,
    version: env!("CARGO_PKG_VERSION").into(),
  })
}