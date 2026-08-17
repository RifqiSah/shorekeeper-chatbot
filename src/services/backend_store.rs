use std::path::PathBuf;
use std::sync::Arc;
use anyhow::{Context, Result};
use notify::{RecursiveMode, Watcher, Event};
use tokio::sync::mpsc;

use crate::schemas::config::LlmBackend;
use crate::services::llm::LlmService;

pub fn load_backends_from_file(path: &PathBuf) -> Result<Vec<LlmBackend>> {
  let raw = std::fs::read_to_string(path).with_context(|| format!("Failed to read {}", path.display()))?;
  let backends: Vec<LlmBackend> = serde_json::from_str(&raw).with_context(|| format!("Invalid JSON in {}", path.display()))?;

  anyhow::ensure!(!backends.is_empty(), "{} must contain at least 1 backend", path.display());

  Ok(backends)
}

pub fn spawn_watcher(path: &PathBuf, llm: Arc<LlmService>) -> Result<notify::RecommendedWatcher> {
  let (tx, mut rx) = mpsc::unbounded_channel::<()>();
  let path = path.clone();

  let mut watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
    if res.is_ok() {
      let _ = tx.send(());
    }
  })?;

  watcher.watch(&path, RecursiveMode::NonRecursive)?;

  tokio::spawn(async move {
    while rx.recv().await.is_some() {
      tokio::time::sleep(std::time::Duration::from_millis(300)).await;

      match load_backends_from_file(&path) {
        Ok(backends) => {
          tracing::info!("Backend config changed, reloaded {} backend(s)", backends.len());
          llm.set_backends(backends).await;
        }
        Err(e) => tracing::warn!("Failed to reload {}: {e}, keeping old backends", path.display()),
      }
    }
  });

  Ok(watcher)
}