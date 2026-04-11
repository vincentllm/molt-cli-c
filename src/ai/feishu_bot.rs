/// Feishu ClawBot 后端
///
/// 消息协议：
///   - 提取请求:  [MOLT_REQUEST:<8位id>]  + prompt → Bot 回 [MOLT_RESPONSE:<id>] + YAML
///   - 执行步骤:  Interactive Card (按钮) → Bot 回 [MOLT_CALLBACK:<id>] result: <输出>
///   - 通知卡片:  保存 Pipeline 后发一张展示卡片到群
///
/// 注意：body.content 是 JSON string，需二次解析；create_time 是毫秒时间戳
use anyhow::{bail, Context, Result};
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::blocking::Client;
use serde_json::{json, Value};
use std::thread;
use std::time::Duration;
use uuid::Uuid;

use super::{AiBackend, build_extraction_prompt};
use crate::cast_parser::MarkSlice;

pub const FEISHU_API: &str = "https://open.feishu.cn/open-apis";

pub struct FeishuBotBackend {
    pub app_id: String,
    pub app_secret: String,
    pub chat_id: String,
    pub poll_timeout_secs: u64,
}

impl FeishuBotBackend {
    fn client(&self) -> Result<Client> {
        Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .context("Failed to build HTTP client")
    }

    /// 发通知卡片到群，告知 Pipeline 已保存（molt stop 成功后调用）
    pub fn notify_pipeline_saved(
        &self,
        pipeline_name: &str,
        step_count: usize,
        path: &str,
    ) -> Result<()> {
        let client = self.client()?;
        let token = get_tenant_access_token(&client, &self.app_id, &self.app_secret)?;

        let card = json!({
            "config": { "wide_screen_mode": true },
            "header": {
                "title": { "content": "🦞 Molt Pipeline 已就绪", "tag": "plain_text" },
                "template": "green"
            },
            "elements": [
                {
                    "tag": "div",
                    "fields": [
                        { "is_short": true, "text": { "tag": "lark_md", "content": format!("**Pipeline**\n{}", pipeline_name) } },
                        { "is_short": true, "text": { "tag": "lark_md", "content": format!("**步骤数**\n{} 个", step_count) } }
                    ]
                },
                { "tag": "div", "text": { "tag": "lark_md", "content": format!("📁 `{}`", path) } },
                { "tag": "hr" },
                {
                    "tag": "div",
                    "text": { "tag": "lark_md",
                    "content": format!("运行: `molt run {}`", pipeline_name) }
                }
            ]
        });

        send_card_message(&client, &token, &self.chat_id, card)
    }

    /// 发互动卡片让 ClawBot 执行某个步骤，等待 [MOLT_CALLBACK:<id>] 回复
    pub fn run_step(
        &self,
        pipeline_name: &str,
        step_name: &str,
        cmd: &str,
        step_index: usize,
        total_steps: usize,
    ) -> Result<String> {
        let client = self.client()?;
        let token = get_tenant_access_token(&client, &self.app_id, &self.app_secret)?;

        let corr_id = &Uuid::new_v4().to_string()[..8];
        let send_time_ms = now_ms();

        let card = json!({
            "config": { "wide_screen_mode": true },
            "header": {
                "title": {
                    "content": format!("🦞 Molt 执行请求 [{}/{}]", step_index, total_steps),
                    "tag": "plain_text"
                },
                "template": "blue"
            },
            "elements": [
                {
                    "tag": "div",
                    "fields": [
                        { "is_short": true, "text": { "tag": "lark_md", "content": format!("**Pipeline**\n{}", pipeline_name) } },
                        { "is_short": true, "text": { "tag": "lark_md", "content": format!("**步骤**\n{}", step_name) } }
                    ]
                },
                {
                    "tag": "div",
                    "text": { "tag": "lark_md", "content": format!("**命令**\n```\n{}\n```", cmd) }
                },
                { "tag": "hr" },
                {
                    "tag": "action",
                    "actions": [
                        {
                            "tag": "button",
                            "text": { "content": "✅ 立即执行", "tag": "lark_md" },
                            "type": "primary",
                            "value": { "action": "execute", "cmd": cmd, "corr_id": corr_id }
                        },
                        {
                            "tag": "button",
                            "text": { "content": "🤔 分析再执行", "tag": "lark_md" },
                            "type": "default",
                            "value": { "action": "analyze", "cmd": cmd, "corr_id": corr_id }
                        },
                        {
                            "tag": "button",
                            "text": { "content": "⏭ 跳过", "tag": "lark_md" },
                            "type": "danger",
                            "value": { "action": "skip", "cmd": cmd, "corr_id": corr_id }
                        }
                    ]
                },
                {
                    "tag": "note",
                    "elements": [
                        { "tag": "plain_text", "content": format!("关联 ID: {}", corr_id) }
                    ]
                }
            ]
        });

        send_card_message(&client, &token, &self.chat_id, card)?;

        println!(
            "   {} 已发送执行卡片 [{}] → 等待 ClawBot 回调...",
            "🤖".cyan(),
            corr_id.yellow()
        );

        poll_for_callback(
            &client,
            &token,
            &self.chat_id,
            corr_id,
            self.poll_timeout_secs,
            send_time_ms,
        )
    }
}

impl AiBackend for FeishuBotBackend {
    fn extract_pipeline(&self, slices: &[MarkSlice]) -> Result<String> {
        let client = self.client()?;
        let token = get_tenant_access_token(&client, &self.app_id, &self.app_secret)?;

        let corr_id = &Uuid::new_v4().to_string()[..8];
        let send_time_ms = now_ms();

        let prompt = build_extraction_prompt(slices);
        let message = format!(
            "[MOLT_REQUEST:{}]\n\n{}\n\n请在回复末尾附上 [MOLT_RESPONSE:{}]",
            corr_id, prompt, corr_id
        );

        send_text_message(&client, &token, &self.chat_id, &message)?;

        println!(
            "   {} [MOLT_REQUEST:{}] 已发送到飞书",
            "→".cyan(),
            corr_id.yellow()
        );

        poll_for_marker(
            &client,
            &token,
            &self.chat_id,
            &format!("[MOLT_RESPONSE:{}]", corr_id),
            self.poll_timeout_secs,
            send_time_ms,
        )
    }
}

// ── 通用飞书 API 封装 ──────────────────────────────────────────────────────────

pub fn get_tenant_access_token(client: &Client, app_id: &str, app_secret: &str) -> Result<String> {
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

pub fn send_text_message(client: &Client, token: &str, chat_id: &str, text: &str) -> Result<()> {
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
        .context("Send text message failed")?;

    check_feishu_response(resp)
}

pub fn send_card_message(client: &Client, token: &str, chat_id: &str, card: Value) -> Result<()> {
    let url = format!("{}/im/v1/messages?receive_id_type=chat_id", FEISHU_API);
    // 卡片 content 是 stringified JSON（与文本消息相同规则）
    let content = serde_json::to_string(&card)?;

    let resp = client
        .post(&url)
        .bearer_auth(token)
        .json(&json!({
            "receive_id": chat_id,
            "msg_type": "interactive",
            "content": content,
            "uuid": Uuid::new_v4().to_string(),
        }))
        .send()
        .context("Send card message failed")?;

    check_feishu_response(resp)
}

fn check_feishu_response(resp: reqwest::blocking::Response) -> Result<()> {
    let j: Value = resp.json().context("Failed to parse Feishu response")?;
    let code = j["code"].as_i64().unwrap_or(-1);
    if code != 0 {
        bail!("Feishu API error {}: {}", code, j["msg"].as_str().unwrap_or("?"));
    }
    Ok(())
}

fn make_spinner(msg: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.enable_steady_tick(Duration::from_millis(100));
    pb.set_style(
        ProgressStyle::with_template("   {spinner:.cyan} {msg}")
            .unwrap()
            .tick_strings(&["⠋","⠙","⠹","⠸","⠼","⠴","⠦","⠧","⠇","⠏"]),
    );
    pb.set_message(msg.to_string());
    pb
}

/// 轮询消息列表，找到含 marker 字符串的回复
fn poll_for_marker(
    client: &Client,
    token: &str,
    chat_id: &str,
    marker: &str,
    timeout_secs: u64,
    send_time_ms: u64,
) -> Result<String> {
    let url = build_message_list_url(chat_id, send_time_ms);
    let spinner = make_spinner("等待飞书 Bot 回复...");
    let deadline = std::time::Instant::now() + Duration::from_secs(timeout_secs);

    while std::time::Instant::now() < deadline {
        thread::sleep(Duration::from_secs(3));
        let elapsed = (std::time::Instant::now().elapsed().as_secs() % timeout_secs) as u64;
        spinner.set_message(format!("等待飞书 Bot 回复... ({}s)", elapsed));

        if let Some(text) = find_text_containing(client, token, &url, marker, send_time_ms) {
            spinner.finish_with_message(format!("{} 收到回复！", "✅".green()));
            return Ok(text.replace(marker, "").trim().to_string());
        }
    }

    spinner.finish_with_message(format!("{} 超时", "❌".red()));
    bail!("Timeout ({}s): no [MOLT_RESPONSE] from Feishu bot", timeout_secs)
}

/// 轮询回调：找 [MOLT_CALLBACK:<id>] result: ...
fn poll_for_callback(
    client: &Client,
    token: &str,
    chat_id: &str,
    corr_id: &str,
    timeout_secs: u64,
    send_time_ms: u64,
) -> Result<String> {
    let marker = format!("[MOLT_CALLBACK:{}]", corr_id);
    let url = build_message_list_url(chat_id, send_time_ms);
    let spinner = make_spinner("等待 ClawBot 回调...");
    let deadline = std::time::Instant::now() + Duration::from_secs(timeout_secs);

    while std::time::Instant::now() < deadline {
        thread::sleep(Duration::from_secs(3));

        if let Some(text) = find_text_containing(client, token, &url, &marker, send_time_ms) {
            spinner.finish_with_message(format!("{} ClawBot 已响应！", "✅".green()));
            let result = text
                .replace(&marker, "")
                .trim()
                .trim_start_matches("result:")
                .trim()
                .to_string();
            return Ok(result);
        }
    }

    spinner.finish_with_message(format!("{} 超时", "❌".red()));
    bail!("Timeout ({}s): no [MOLT_CALLBACK] from ClawBot", timeout_secs)
}

fn build_message_list_url(chat_id: &str, since_ms: u64) -> String {
    // start_time 为秒级时间戳
    let since_sec = since_ms / 1000;
    format!(
        "{}/im/v1/messages?container_id_type=chat&container_id={}&sort_type=ByCreateTimeDesc&page_size=20&start_time={}",
        FEISHU_API, chat_id, since_sec
    )
}

fn find_text_containing(
    client: &Client,
    token: &str,
    url: &str,
    needle: &str,
    since_ms: u64,
) -> Option<String> {
    let resp = client.get(url).bearer_auth(token).send().ok()?;
    let j: Value = resp.json().ok()?;
    let items = j["data"]["items"].as_array()?;

    for item in items {
        // 过滤时间（双重保险：API start_time + 本地过滤）
        let create_ms: u64 = item["create_time"]
            .as_str()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        if create_ms < since_ms { continue; }

        // 只处理文本消息
        if item["msg_type"].as_str() != Some("text") { continue; }

        // body.content 是 JSON string：{"text":"..."}
        let content_str = item["body"]["content"].as_str()?;
        let inner: Value = serde_json::from_str(content_str).ok()?;
        let text = inner["text"].as_str()?;

        if text.contains(needle) {
            return Some(text.to_string());
        }
    }
    None
}

fn now_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
