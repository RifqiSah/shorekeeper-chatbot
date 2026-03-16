mod chat;
mod health;
mod usage;

use axum::{routing::{get, post}, Router};
use crate::AppState;

pub fn build_routes() -> Router<AppState> {
  Router::new()
    .route("/health", get(health::health_handler))
    .route("/chat", post(chat::chat_handler))
    .route("/usage", get(usage::usage_handler))
}