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

#[cfg(test)]
mod tests {
    use super::*;

    // ── extract_yaml_from_response ─────────────────────────────────────────────

    #[test]
    fn extracts_yaml_fenced_block() {
        let input = "Some text\n```yaml\nname: test\n```\ntrailing";
        assert_eq!(extract_yaml_from_response(input), "name: test");
    }

    #[test]
    fn extracts_generic_fenced_block_when_no_yaml_fence() {
        let input = "```\nname: fallback\n```";
        assert_eq!(extract_yaml_from_response(input), "name: fallback");
    }

    #[test]
    fn returns_trimmed_raw_text_when_no_fence() {
        let input = "  name: bare  ";
        assert_eq!(extract_yaml_from_response(input), "name: bare");
    }

    #[test]
    fn prefers_yaml_fence_over_generic_fence() {
        let input = "```\nwrong\n```\n```yaml\nright: true\n```";
        assert_eq!(extract_yaml_from_response(input), "right: true");
    }

    // ── parse_pipeline_yaml ────────────────────────────────────────────────────

    #[test]
    fn parses_minimal_pipeline() {
        let yaml = r#"
name: deploy
steps:
  - name: Build
    cmd: cargo build --release
    executor: local
"#;
        let p = parse_pipeline_yaml(yaml).unwrap();
        assert_eq!(p.name, "deploy");
        assert_eq!(p.steps.len(), 1);
        assert_eq!(p.steps[0].cmd, "cargo build --release");
        assert_eq!(p.steps[0].executor, "local");
    }

    #[test]
    fn parse_error_on_invalid_yaml() {
        let err = parse_pipeline_yaml("not: valid: yaml: :").unwrap_err();
        assert!(err.to_string().contains("parse") || err.to_string().contains("YAML")
            || err.to_string().contains("yaml"), "got: {err}");
    }

    #[test]
    fn accepts_command_alias_for_cmd() {
        let yaml = r#"
name: alias-test
steps:
  - name: Step
    command: echo hello
    executor: local
"#;
        let p = parse_pipeline_yaml(yaml).unwrap();
        assert_eq!(p.steps[0].cmd, "echo hello");
    }

    // ── sanitize_name ──────────────────────────────────────────────────────────

    #[test]
    fn sanitize_replaces_spaces_and_dots() {
        assert_eq!(sanitize_name("My Pipeline 1.0"), "my-pipeline-1-0");
    }

    #[test]
    fn sanitize_keeps_alphanumeric_dash_underscore() {
        assert_eq!(sanitize_name("deploy_v2-final"), "deploy_v2-final");
    }
}
