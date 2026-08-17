use std::{path::PathBuf, sync::Arc};
use anyhow::{Context, Result};
use notify::{RecursiveMode, Watcher, Event};
use tokio::sync::mpsc;
use serde::Deserialize;

use crate::schemas::config::LlmBackend;
use crate::services::llm::LlmService;

#[derive(Deserialize)]
struct BackendsOnly {
  #[serde(default)]
  backends: Vec<LlmBackend>,
}

fn reload_backends(path: &PathBuf) -> Result<Vec<LlmBackend>> {
  let raw = std::fs::read_to_string(path)
    .with_context(|| format!("Failed to read {}", path.display()))?;
  let parsed: BackendsOnly = toml::from_str(&raw)
    .with_context(|| format!("Invalid TOML in {}", path.display()))?;
  anyhow::ensure!(!parsed.backends.is_empty(), "{} must contain at least 1 [[backends]] entry", path.display());
  Ok(parsed.backends)
}

pub fn spawn_watcher(path: PathBuf, llm: Arc<LlmService>) -> Result<notify::RecommendedWatcher> {
  let (tx, mut rx) = mpsc::unbounded_channel::<()>();
  let mut watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
    if res.is_ok() { let _ = tx.send(()); }
  })?;
  
  watcher.watch(&path, RecursiveMode::NonRecursive)?;

  tokio::spawn(async move {
    while rx.recv().await.is_some() {
      tokio::time::sleep(std::time::Duration::from_millis(300)).await;
      match reload_backends(&path) {
        Ok(backends) => {
          tracing::info!("{} changed, reloaded {} backend(s)", path.display(), backends.len());
          llm.set_backends(backends).await;
        }
        Err(e) => tracing::warn!("Failed to reload {}: {e}, keeping old backends", path.display()),
      }
    }
  });

  Ok(watcher)
}