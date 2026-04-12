/// Feishu HTTP 客户端层
///
/// 职责：获取 token、发文本消息、发互动卡片、轮询消息列表
/// 不含业务逻辑（提取 / 执行），只负责 Feishu Open Platform API 的 HTTP 通信
use anyhow::{bail, Context, Result};
use reqwest::blocking::Client;
use serde_json::{json, Value};
use std::time::Duration;
use uuid::Uuid;

pub const FEISHU_API: &str = "https://open.feishu.cn/open-apis";

pub struct FeishuClient {
    inner: Client,
    pub token: String,
    pub chat_id: String,
    pub poll_timeout_secs: u64,
}

impl FeishuClient {
    /// 使用 app_id + app_secret 创建客户端（自动获取 tenant_access_token）
    pub fn new(
        app_id: &str,
        app_secret: &str,
        chat_id: &str,
        poll_timeout_secs: u64,
    ) -> Result<Self> {
        let inner = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .context("Failed to build HTTP client")?;
        let token = Self::fetch_token(&inner, app_id, app_secret)?;
        Ok(Self { inner, token, chat_id: chat_id.to_string(), poll_timeout_secs })
    }

    fn fetch_token(client: &Client, app_id: &str, app_secret: &str) -> Result<String> {
        let url = format!("{}/auth/v3/tenant_access_token/internal", FEISHU_API);
        let resp = client
            .post(&url)
            .json(&json!({ "app_id": app_id, "app_secret": app_secret }))
            .send()
            .context("Auth request failed")?;
        let j: Value = resp.json().context("Failed to parse auth response")?;
        if j["code"].as_i64().unwrap_or(-1) != 0 {
            bail!("Feishu auth error {}: {}", j["code"], j["msg"].as_str().unwrap_or("?"));
        }
        j["tenant_access_token"]
            .as_str()
            .map(|s| s.to_string())
            .context("No tenant_access_token in response")
    }

    pub fn send_text(&self, text: &str) -> Result<()> {
        let url = format!("{}/im/v1/messages?receive_id_type=chat_id", FEISHU_API);
        let content = serde_json::to_string(&json!({ "text": text }))?;
        let resp = self.inner
            .post(&url)
            .bearer_auth(&self.token)
            .json(&json!({
                "receive_id": self.chat_id,
                "msg_type": "text",
                "content": content,
                "uuid": Uuid::new_v4().to_string(),
            }))
            .send()
            .context("Send text message failed")?;
        Self::check(resp)
    }

    pub fn send_card(&self, card: &Value) -> Result<()> {
        let url = format!("{}/im/v1/messages?receive_id_type=chat_id", FEISHU_API);
        let content = serde_json::to_string(card)?;
        let resp = self.inner
            .post(&url)
            .bearer_auth(&self.token)
            .json(&json!({
                "receive_id": self.chat_id,
                "msg_type": "interactive",
                "content": content,
                "uuid": Uuid::new_v4().to_string(),
            }))
            .send()
            .context("Send card message failed")?;
        Self::check(resp)
    }

    /// 在 since_ms 之后的消息中，查找含 needle 的文本，返回完整消息文本
    /// 每次调用都发一次请求（由上层的 spinner 循环驱动）
    pub fn find_text_after(&self, needle: &str, since_ms: u64) -> Option<String> {
        let since_sec = since_ms / 1000;
        let url = format!(
            "{}/im/v1/messages?container_id_type=chat&container_id={}\
             &sort_type=ByCreateTimeDesc&page_size=20&start_time={}",
            FEISHU_API, self.chat_id, since_sec
        );

        let resp = self.inner.get(&url).bearer_auth(&self.token).send().ok()?;
        let j: Value = resp.json().ok()?;
        let items = j["data"]["items"].as_array()?;

        for item in items {
            // create_time 是毫秒字符串，二次校验排除历史消息
            let create_ms: u64 = item["create_time"]
                .as_str()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            if create_ms < since_ms { continue; }

            if item["msg_type"].as_str() != Some("text") { continue; }

            // body.content 是 JSON string: {"text":"..."}
            let content_str = item["body"]["content"].as_str()?;
            let inner: Value = serde_json::from_str(content_str).ok()?;
            let text = inner["text"].as_str()?;

            if text.contains(needle) {
                return Some(text.to_string());
            }
        }
        None
    }

    pub fn now_ms() -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    fn check(resp: reqwest::blocking::Response) -> Result<()> {
        let j: Value = resp.json().context("Failed to parse Feishu response")?;
        let code = j["code"].as_i64().unwrap_or(-1);
        if code != 0 {
            bail!("Feishu API error {}: {}", code, j["msg"].as_str().unwrap_or("?"));
        }
        Ok(())
    }
}
