use anyhow::{Context, Result, bail};
use reqwest::blocking::Client;
use serde_json::json;
use std::time::Duration;

use super::{AiBackend, build_extraction_prompt};
use crate::cast_parser::MarkSlice;

pub struct DirectLlmBackend {
    pub api_key: String,
    pub base_url: String,
    pub model: String,
}

impl AiBackend for DirectLlmBackend {
    fn extract_pipeline(&self, slices: &[MarkSlice]) -> Result<String> {
        let prompt = build_extraction_prompt(slices);

        let client = Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .context("Failed to build HTTP client")?;

        // 支持 Anthropic API 和 OpenAI 兼容接口
        let is_anthropic = self.base_url.contains("anthropic.com")
            || !self.base_url.contains("openai");

        if is_anthropic {
            call_anthropic(&client, &self.api_key, &self.base_url, &self.model, &prompt)
        } else {
            call_openai_compat(&client, &self.api_key, &self.base_url, &self.model, &prompt)
        }
    }
}

fn call_anthropic(
    client: &Client,
    api_key: &str,
    base_url: &str,
    model: &str,
    prompt: &str,
) -> Result<String> {
    let url = format!("{}/v1/messages", base_url.trim_end_matches('/'));

    let body = json!({
        "model": model,
        "max_tokens": 4096,
        "thinking": { "type": "adaptive" },
        "messages": [
            { "role": "user", "content": prompt }
        ]
    });

    let resp = client
        .post(&url)
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .context("HTTP request to Anthropic failed")?;

    let status = resp.status();
    let text = resp.text().context("Failed to read response body")?;

    if !status.is_success() {
        bail!("Anthropic API error {}: {}", status, text);
    }

    let json: serde_json::Value = serde_json::from_str(&text)
        .context("Failed to parse Anthropic response")?;

    // 从 content 数组中找 type=text 的块
    if let Some(content) = json["content"].as_array() {
        for block in content {
            if block["type"].as_str() == Some("text") {
                if let Some(t) = block["text"].as_str() {
                    return Ok(t.to_string());
                }
            }
        }
    }

    bail!("No text content in Anthropic response: {}", text)
}

fn call_openai_compat(
    client: &Client,
    api_key: &str,
    base_url: &str,
    model: &str,
    prompt: &str,
) -> Result<String> {
    let url = format!("{}/v1/chat/completions", base_url.trim_end_matches('/'));

    let body = json!({
        "model": model,
        "messages": [
            { "role": "user", "content": prompt }
        ]
    });

    let resp = client
        .post(&url)
        .bearer_auth(api_key)
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .context("HTTP request failed")?;

    let status = resp.status();
    let text = resp.text().context("Failed to read response body")?;

    if !status.is_success() {
        bail!("LLM API error {}: {}", status, text);
    }

    let json: serde_json::Value = serde_json::from_str(&text)?;
    let content = json["choices"][0]["message"]["content"]
        .as_str()
        .context("No content in LLM response")?;

    Ok(content.to_string())
}
