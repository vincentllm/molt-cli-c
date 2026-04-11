/// Feishu ClawBot 后端
///
/// 流程：
///   1. app_id + app_secret → tenant_access_token
///   2. 发消息到 chat_id，文本包含 [MOLT_REQUEST:<corr_id>] 作为关联 ID
///   3. 轮询消息列表，找到含 [MOLT_RESPONSE:<corr_id>] 的回复
///   4. 返回回复文本（其中包含 YAML）
///
/// ClawBot demo 架构：Bot 接收 MOLT_REQUEST，调用内部 LLM，回复带 MOLT_RESPONSE 的 YAML
use anyhow::{bail, Context, Result};
use colored::Colorize;
use reqwest::blocking::Client;
use serde_json::json;
use std::thread;
use std::time::Duration;
use uuid::Uuid;

use super::{AiBackend, build_extraction_prompt};
use crate::cast_parser::MarkSlice;

pub struct FeishuBotBackend {
    pub app_id: String,
    pub app_secret: String,
    pub chat_id: String,
    pub poll_timeout_secs: u64,
}

const FEISHU_API: &str = "https://open.feishu.cn/open-apis";

impl AiBackend for FeishuBotBackend {
    fn extract_pipeline(&self, slices: &[MarkSlice]) -> Result<String> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .context("Failed to build HTTP client")?;

        // 1. 获取 tenant_access_token
        let token = get_tenant_access_token(&client, &self.app_id, &self.app_secret)
            .context("Failed to get Feishu tenant_access_token")?;

        // 2. 生成关联 ID（8 位，便于肉眼识别）
        let corr_id = Uuid::new_v4().to_string()[..8].to_string();

        // 3. 构造消息文本
        let prompt = build_extraction_prompt(slices);
        let message = format!(
            "[MOLT_REQUEST:{}]\n\n{}\n\n请在回复末尾附上 [MOLT_RESPONSE:{}]",
            corr_id, prompt, corr_id
        );

        // 4. 发送消息
        send_text_message(&client, &token, &self.chat_id, &message)
            .context("Failed to send message to Feishu")?;

        println!(
            "   {} [MOLT_REQUEST:{}] 已发送到飞书",
            "→".cyan(),
            corr_id.yellow()
        );

        // 5. 轮询等待回复
        let response = poll_for_response(
            &client,
            &token,
            &self.chat_id,
            &corr_id,
            self.poll_timeout_secs,
        )
        .context("Timed out waiting for Feishu bot response")?;

        Ok(response)
    }
}

fn get_tenant_access_token(client: &Client, app_id: &str, app_secret: &str) -> Result<String> {
    let url = format!("{}/auth/v3/tenant_access_token/internal", FEISHU_API);
    let resp = client
        .post(&url)
        .json(&json!({ "app_id": app_id, "app_secret": app_secret }))
        .send()
        .context("Auth request failed")?;

    let json: serde_json::Value = resp.json().context("Failed to parse auth response")?;
    if json["code"].as_i64().unwrap_or(-1) != 0 {
        bail!(
            "Feishu auth error: {} — {}",
            json["code"],
            json["msg"].as_str().unwrap_or("unknown")
        );
    }
    json["tenant_access_token"]
        .as_str()
        .map(|s| s.to_string())
        .context("No tenant_access_token in response")
}

fn send_text_message(
    client: &Client,
    token: &str,
    chat_id: &str,
    text: &str,
) -> Result<()> {
    let url = format!("{}/im/v1/messages?receive_id_type=chat_id", FEISHU_API);
    let content = serde_json::to_string(&json!({ "text": text }))?;

    let resp = client
        .post(&url)
        .bearer_auth(token)
        .json(&json!({
            "receive_id": chat_id,
            "msg_type": "text",
            "content": content,
            "uuid": Uuid::new_v4().to_string(),
        }))
        .send()
        .context("Send message request failed")?;

    let json: serde_json::Value = resp.json().context("Failed to parse send message response")?;
    if json["code"].as_i64().unwrap_or(-1) != 0 {
        bail!(
            "Send message failed: {} — {}",
            json["code"],
            json["msg"].as_str().unwrap_or("unknown")
        );
    }
    Ok(())
}

fn poll_for_response(
    client: &Client,
    token: &str,
    chat_id: &str,
    corr_id: &str,
    timeout_secs: u64,
) -> Result<String> {
    let target = format!("[MOLT_RESPONSE:{}]", corr_id);
    let url = format!(
        "{}/im/v1/messages?container_id_type=chat&container_id={}&sort_type=ByCreateTimeDesc&page_size=10",
        FEISHU_API, chat_id
    );

    let deadline = std::time::Instant::now() + Duration::from_secs(timeout_secs);
    let mut dots = 0u32;

    while std::time::Instant::now() < deadline {
        thread::sleep(Duration::from_secs(3));
        dots += 1;
        print!("\r   {} 等待飞书 Bot 回复{}", "⏳".yellow(), ".".repeat((dots % 4) as usize));
        // flush stdout
        use std::io::Write;
        let _ = std::io::stdout().flush();

        let resp = client
            .get(&url)
            .bearer_auth(token)
            .send();

        let resp = match resp {
            Ok(r) => r,
            Err(_) => continue,
        };

        let json: serde_json::Value = match resp.json() {
            Ok(j) => j,
            Err(_) => continue,
        };

        if let Some(items) = json["data"]["items"].as_array() {
            for item in items {
                // 消息内容是 JSON string：{"text":"..."}
                if let Some(content_str) = item["body"]["content"].as_str() {
                    if let Ok(content_json) = serde_json::from_str::<serde_json::Value>(content_str) {
                        let text = content_json["text"].as_str().unwrap_or("");
                        if text.contains(&target) {
                            println!(); // 换行，清掉进度点
                            // 去掉关联 ID 标记，返回纯内容
                            let clean = text.replace(&target, "").trim().to_string();
                            return Ok(clean);
                        }
                    }
                }
            }
        }
    }

    println!();
    bail!("Timeout: no response from Feishu bot after {}s", timeout_secs)
}
