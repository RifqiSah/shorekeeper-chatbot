use axum::{
  body::Body,
  extract::State,
  http::{Request, StatusCode},
  middleware::Next,
  response::Response,
  Json,
};
use serde_json::json;

use crate::AppState;

pub async fn auth_middleware(
  State(state): State<AppState>,
  req: Request<Body>,
  next: Next,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
  // Skip auth for health check
  if req.uri().path() == "/api/health" {
    return Ok(next.run(req).await);
  }

  let auth_header = req
    .headers()
    .get("Authorization")
    .and_then(|v| v.to_str().ok())
    .and_then(|v| v.strip_prefix("Bearer "));

  match auth_header {
    Some(token) if token == state.config.api_secret_key => Ok(next.run(req).await),
    _ => Err((
      StatusCode::UNAUTHORIZED,
      Json(json!({ "error": "Unauthorized. Provide valid Bearer token." })),
    )),
  }
}