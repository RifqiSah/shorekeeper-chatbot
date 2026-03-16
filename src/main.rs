mod models;
mod routes;
mod services;
mod middleware;

use axum::{
  Router,
  middleware as axum_middleware,
};
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use std::sync::Arc;

use services::{llm::LlmService, redis::RedisService};
use middleware::auth::auth_middleware;

#[derive(Clone)]
pub struct AppState {
  pub llm: Arc<LlmService>,
  pub redis: Arc<RedisService>,
  pub config: Arc<models::config::Config>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
  // Load .env
  dotenvy::dotenv().ok();

  // Init tracing
  tracing_subscriber::registry()
    .with(tracing_subscriber::EnvFilter::new(
      std::env::var("RUST_LOG").unwrap_or_else(|_| "shorekeeper_ai=debug,tower_http=debug".into()),
    ))
    .with(tracing_subscriber::fmt::layer())
    .init();

  // Load config
  let config = Arc::new(models::config::Config::from_env()?);
  tracing::info!("Config loaded. Port: {}", config.port);

  // Init services
  let redis = Arc::new(RedisService::new(&config.redis_url).await?);
  tracing::info!("Redis connected");

  let llm = Arc::new(LlmService::new(
    config.llm_api_key.clone(),
    config.llm_base_url.clone(),
    config.llm_aig_token.clone(),
    config.llm_model.clone(),
    config.embed_api_key.clone(),
    config.embed_base_url.clone(),
  ));

  tracing::info!("LLM service initialized. Model: {}", config.llm_model);
  tracing::info!("SYSTEM_PROMPT = {:?}", std::env::var("SYSTEM_PROMPT"));

  let state = AppState { llm, redis, config: config.clone() };

  // Build router
  let app = Router::new()
    .nest("/api", routes::build_routes())
    .layer(axum_middleware::from_fn_with_state(state.clone(), auth_middleware))
    .layer(CorsLayer::permissive())
    .layer(TraceLayer::new_for_http())
    .with_state(state);

  let addr = format!("0.0.0.0:{}", config.port);
  let listener = tokio::net::TcpListener::bind(&addr).await?;
  tracing::info!("Server running on http://{}", addr);

  axum::serve(listener, app).await?;
  Ok(())
}