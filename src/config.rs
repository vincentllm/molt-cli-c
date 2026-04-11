use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoltConfig {
    pub backend: BackendConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BackendConfig {
    /// 调用飞书 ClawBot 提取/执行 Pipeline（Demo 主路径）
    FeishuBot {
        app_id: String,
        app_secret: String,
        /// 发消息的群/机器人 chat_id（oc_xxx 格式）
        chat_id: String,
        #[serde(default = "default_poll_timeout")]
        poll_timeout_secs: u64,
    },
    /// 直接调用 LLM API（Anthropic Claude / OpenAI 兼容）
    DirectLlm {
        api_key: String,
        #[serde(default = "default_base_url")]
        base_url: String,
        #[serde(default = "default_model")]
        model: String,
    },
}

fn default_poll_timeout() -> u64 { 90 }
fn default_base_url() -> String { "https://api.anthropic.com".to_string() }
fn default_model() -> String { "claude-opus-4-6".to_string() }

impl MoltConfig {
    pub fn load() -> Option<Self> {
        let path = config_path()?;
        let content = fs::read_to_string(path).ok()?;
        serde_yaml::from_str(&content).ok()
    }

    pub fn save(&self) -> Result<()> {
        let path = config_path().ok_or_else(|| anyhow::anyhow!("Cannot determine home dir"))?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = serde_yaml::to_string(self)?;
        fs::write(&path, content)?;
        Ok(())
    }
}

pub fn config_path() -> Option<PathBuf> {
    let mut p = dirs::home_dir()?;
    p.push(".molt");
    p.push("config.yaml");
    Some(p)
}

pub fn pipelines_dir() -> Option<PathBuf> {
    let mut p = dirs::home_dir()?;
    p.push(".molt");
    p.push("pipelines");
    Some(p)
}
