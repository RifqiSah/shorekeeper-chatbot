use anyhow::{Context, Result};
use reqwest::Client;
use scraper::{Html, Selector};

pub struct Fetcher {
  client: Client,
}

impl Fetcher {
  pub fn new() -> Self {
    let client = Client::builder()
      .timeout(std::time::Duration::from_secs(15))
      .build()
      .expect("Failed to build fetcher client");

    Self { client }
  }

  pub async fn fetch_url(&self, url: &str) -> Result<FetchResult> {
    let response = self
      .client
      .get(url)
      .send()
      .await
      .context("Failed to fetch URL")?;

    if !response.status().is_success() {
      anyhow::bail!("URL returned status: {}", response.status());
    }

    let content_type = response
      .headers()
      .get("content-type")
      .and_then(|v| v.to_str().ok())
      .unwrap_or("")
      .to_string();

    if !content_type.contains("text/html") && !content_type.contains("text/plain") {
      anyhow::bail!("Unsupported content type: {}", content_type);
    }

    let html = response.text().await.context("Failed to read response body")?;
    let result = parse_html(url, &html);

    if result.text.is_empty() {
      anyhow::bail!("No readable content found at URL");
    }

    Ok(result)
  }
}

#[derive(Debug)]
pub struct FetchResult {
  pub url: String,
  pub title: Option<String>,
  pub text: String,
}

impl FetchResult {
  /// Truncate content to max_chars
  pub fn truncate(&self, max_chars: usize) -> String {
    let header = match &self.title {
      Some(t) => format!("Judul: {}\n\n", t),
      None => String::new(),
    };

    let available = max_chars.saturating_sub(header.len());

    if self.text.chars().count() <= available {
      format!("{}{}", header, self.text)
    } else {
      let truncated: String = self.text.chars().take(available).collect();
      format!("{}{}... [truncated]", header, truncated)
    }
  }
}

/// HTML Parser
fn parse_html(url: &str, html: &str) -> FetchResult {
  let document = Html::parse_document(html);

  // Get title
  let title = Selector::parse("title").ok()
    .and_then(|sel| document.select(&sel).next())
    .map(|el| el.text().collect::<String>().trim().to_string())
    .filter(|t| !t.is_empty());

  // Tag
  let noise_tags = [
    "script", "style", "nav", "header", "footer",
    "aside", "noscript", "iframe", "form", "button",
    "svg", "img", "figure", "figcaption",
  ];

  // Priority selector
  let content_selectors = [
    "article",
    "main",
    "[role='main']",
    ".content",
    ".post-content",
    ".entry-content",
    ".article-body",
    ".post-body",
    "#content",
    "#main",
    "body",
  ];

  let mut text = String::new();

  'outer: for selector_str in &content_selectors {
    if let Ok(sel) = Selector::parse(selector_str) {
      if let Some(element) = document.select(&sel).next() {
        for node in element.descendants() {
          // Skip noise tags
          if let Some(el) = node.value().as_element() {
            if noise_tags.contains(&el.name()) {
              continue;
            }
          }

          // Get teks
          if let Some(t) = node.value().as_text() {
            let t = t.trim();
            if !t.is_empty() {
              text.push_str(t);
              text.push(' ');
            }
          }
        }

        if !text.trim().is_empty() {
          break 'outer;
        }
      }
    }
  }

  // Bersihkan whitespace berlebih
  let text = text.split_whitespace().collect::<Vec<_>>().join(" ");

  FetchResult { url: url.to_string(), title, text }
}

/// URL detection
pub fn extract_urls(text: &str) -> Vec<String> {
  let re = regex::Regex::new(r#"https?://[^\s<>\"]+"#).unwrap();
  re.find_iter(text)
    .map(|m| {
      let url = m.as_str();
      let trimmed = url.trim_end_matches(|c: char| matches!(c, '.' | ',' | ')' | ']' | '!' | '?'));
      trimmed.to_string()
    })
    .filter(|u| u.len() > 10)
    .collect()
}