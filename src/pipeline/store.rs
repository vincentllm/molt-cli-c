use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

use crate::config::pipelines_dir;
use super::schema::Pipeline;

/// 从 AI 返回文本中提取 YAML 代码块
pub fn extract_yaml_from_response(text: &str) -> String {
    if let Some(start) = text.find("```yaml") {
        let inner = &text[start + 7..];
        if let Some(end) = inner.find("```") {
            return inner[..end].trim().to_string();
        }
    }
    if let Some(start) = text.find("```") {
        let inner = &text[start + 3..];
        if let Some(end) = inner.find("```") {
            return inner[..end].trim().to_string();
        }
    }
    text.trim().to_string()
}

pub fn parse_pipeline_yaml(yaml: &str) -> Result<Pipeline> {
    serde_yaml::from_str(yaml).context("Failed to parse pipeline YAML")
}

/// 保存到 ~/.molt/pipelines/<name>.yaml，返回保存路径
pub fn save_pipeline(pipeline: &Pipeline) -> Result<PathBuf> {
    let dir = pipelines_dir().context("Cannot determine home dir")?;
    fs::create_dir_all(&dir)?;

    let path = dir.join(format!("{}.yaml", sanitize_name(&pipeline.name)));
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
