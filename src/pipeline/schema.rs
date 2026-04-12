use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Pipeline {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub created_at: String,
    pub steps: Vec<PipelineStep>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PipelineStep {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mark: Option<u32>,
    #[serde(alias = "command")]
    pub cmd: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// "local" | "feishu_bot" | "ask"
    #[serde(default = "default_executor")]
    pub executor: String,
}

fn default_executor() -> String { "local".to_string() }
