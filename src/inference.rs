use anyhow::{anyhow, Context, Result};
use serde_json::{json, Value};
use std::time::Duration;

#[derive(Clone)]
pub struct InferenceClient {
    http: reqwest::Client,
    base_url: String,
    api_key: String,
}

#[derive(Debug, Clone)]
pub struct Completion {
    pub text: String,
    pub model: String,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
}

impl InferenceClient {
    pub fn new(base_url: String, api_key: String) -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(240))
            .pool_idle_timeout(Duration::from_secs(60))
            .build()
            .expect("http client");
        Self {
            http,
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
        }
    }

    pub async fn chat(
        &self,
        model: &str,
        system: &str,
        user: &str,
        temperature: f32,
        max_tokens: u32,
    ) -> Result<Completion> {
        let payload = json!({
            "model": model,
            "temperature": temperature,
            "max_tokens": max_tokens,
            // Vultr's OpenAI-compatible endpoint forwards this to the model's chat
            // template. Reasoning can be disabled for task-shaped generation, which
            // cuts latency from ~60s to ~6s without changing the artifact contract.
            "chat_template_kwargs": { "enable_thinking": false },
            "messages": [
                {"role": "system", "content": system},
                {"role": "user", "content": user}
            ]
        });

        let mut last_err: Option<anyhow::Error> = None;
        for attempt in 0..3u32 {
            if attempt > 0 {
                tokio::time::sleep(Duration::from_millis(900 * attempt as u64)).await;
            }
            let res = self
                .http
                .post(format!("{}/chat/completions", self.base_url))
                .bearer_auth(&self.api_key)
                .json(&payload)
                .send()
                .await;

            match res {
                Ok(resp) => {
                    let status = resp.status();
                    let body: Value = match resp.json().await {
                        Ok(v) => v,
                        Err(e) => {
                            last_err = Some(anyhow!("invalid JSON from inference API: {e}"));
                            continue;
                        }
                    };
                    if !status.is_success() {
                        let msg = body
                            .get("error")
                            .and_then(|e| e.get("message"))
                            .and_then(|m| m.as_str())
                            .unwrap_or("unknown inference error");
                        last_err = Some(anyhow!("inference HTTP {status}: {msg}"));
                        if status.as_u16() == 429 || status.is_server_error() {
                            continue;
                        }
                        return Err(last_err.unwrap());
                    }
                    let text = body
                        .pointer("/choices/0/message/content")
                        .and_then(|c| c.as_str())
                        .unwrap_or("")
                        .to_string();
                    if text.trim().is_empty() {
                        last_err = Some(anyhow!("model returned an empty completion"));
                        continue;
                    }
                    let prompt_tokens = body
                        .pointer("/usage/prompt_tokens")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0);
                    let completion_tokens = body
                        .pointer("/usage/completion_tokens")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0);
                    let used_model = body
                        .get("model")
                        .and_then(|v| v.as_str())
                        .unwrap_or(model)
                        .to_string();
                    return Ok(Completion {
                        text,
                        model: used_model,
                        prompt_tokens,
                        completion_tokens,
                    });
                }
                Err(e) => {
                    last_err = Some(anyhow!("inference request failed: {e}"));
                }
            }
        }
        Err(last_err.unwrap_or_else(|| anyhow!("inference failed")))
    }

    /// Extract a JSON object from a model response that may contain prose or code fences.
    pub fn extract_json(text: &str) -> Option<Value> {
        let trimmed = text.trim();
        let candidate = if let Some(start) = trimmed.find('{') {
            if let Some(end) = trimmed.rfind('}') {
                &trimmed[start..=end]
            } else {
                trimmed
            }
        } else {
            trimmed
        };
        serde_json::from_str::<Value>(candidate).ok().or_else(|| {
            let stripped = strip_fences(trimmed);
            serde_json::from_str::<Value>(stripped.trim()).ok()
        })
    }

    /// Extract a python code block from a model response.
    pub fn extract_python(text: &str) -> Option<String> {
        if let Some(start) = text.find("```python") {
            let rest = &text[start + 9..];
            if let Some(end) = rest.find("```") {
                return Some(rest[..end].trim().to_string());
            }
        }
        if let Some(start) = text.find("```") {
            let rest = &text[start + 3..];
            if let Some(end) = rest.find("```") {
                let block = rest[..end].trim();
                let block = block.strip_prefix("py\n").unwrap_or(block);
                return Some(block.to_string());
            }
        }
        if text.contains("import ") || text.contains("def ") {
            return Some(text.trim().to_string());
        }
        None
    }
}

fn strip_fences(s: &str) -> String {
    let mut out = s.trim().to_string();
    if let Some(rest) = out.strip_prefix("```json") {
        out = rest.to_string();
    } else if let Some(rest) = out.strip_prefix("```") {
        out = rest.to_string();
    }
    if let Some(rest) = out.strip_suffix("```") {
        out = rest.to_string();
    }
    out.trim().to_string()
}

pub fn require_json(text: &str) -> Result<Value> {
    InferenceClient::extract_json(text)
        .with_context(|| format!("could not parse JSON from model output: {}", truncate(text, 300)))
}

pub fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        s.chars().take(max).collect::<String>() + "…"
    }
}
