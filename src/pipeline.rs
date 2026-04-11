use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::config::pipelines_dir;

#[derive(Debug, Serialize, Deserialize)]
pub struct Pipeline {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub created_at: String,
    pub steps: Vec<PipelineStep>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PipelineStep {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mark: Option<u32>,
    pub cmd: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// 执行目标: "local" | "feishu_bot" | "ask"
    #[serde(default = "default_executor")]
    pub executor: String,
}

fn default_executor() -> String { "local".to_string() }

/// 从 AI 返回的文本中提取 YAML 代码块
pub fn extract_yaml_from_response(text: &str) -> String {
    // 尝试提取 ```yaml ... ``` 代码块
    if let Some(start) = text.find("```yaml") {
        let inner = &text[start + 7..];
        if let Some(end) = inner.find("```") {
            return inner[..end].trim().to_string();
        }
    }
    // 回退：提取 ``` ... ``` 代码块
    if let Some(start) = text.find("```") {
        let inner = &text[start + 3..];
        if let Some(end) = inner.find("```") {
            return inner[..end].trim().to_string();
        }
    }
    // 回退：原样返回（可能就是纯 YAML）
    text.trim().to_string()
}

/// 将 YAML 文本解析为 Pipeline 结构体并验证
pub fn parse_pipeline_yaml(yaml: &str) -> Result<Pipeline> {
    serde_yaml::from_str(yaml).context("Failed to parse pipeline YAML")
}

/// 保存 pipeline 到 ~/.molt/pipelines/<name>.yaml
pub fn save_pipeline(pipeline: &Pipeline) -> Result<PathBuf> {
    let dir = pipelines_dir().context("Cannot determine home dir")?;
    fs::create_dir_all(&dir)?;

    let filename = format!("{}.yaml", sanitize_name(&pipeline.name));
    let path = dir.join(&filename);

    let content = serde_yaml::to_string(pipeline)?;
    fs::write(&path, content)?;

    Ok(path)
}

fn sanitize_name(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect::<String>()
        .to_lowercase()
}
