/// Feishu 执行层
///
/// 职责：
///   - run_step: 发 Interactive Card 请求 ClawBot 执行某个步骤，
///     等待 [MOLT_CALLBACK:<id>] 回复
///   - notify_pipeline_saved: 保存成功后发飞书通知卡片
use anyhow::{bail, Result};
use colored::Colorize;
use serde_json::json;
use std::thread;
use std::time::Duration;
use uuid::Uuid;

use super::client::FeishuClient;
use super::extractor::{make_spinner, FeishuBotBackend};

impl FeishuBotBackend {
    /// 发互动卡片让 ClawBot 执行某个步骤，等待 [MOLT_CALLBACK:<id>] 回复
    pub fn run_step(
        &self,
        pipeline_name: &str,
        step_name: &str,
        cmd: &str,
        step_index: usize,
        total_steps: usize,
    ) -> Result<String> {
        let client = self.make_client()?;
        let corr_id = &Uuid::new_v4().to_string()[..8];
        let send_time_ms = FeishuClient::now_ms();

        let card = build_execution_card(pipeline_name, step_name, cmd, step_index, total_steps, corr_id);
        client.send_card(&card)?;

        println!(
            "   {} 已发送执行卡片 [{}] → 等待 ClawBot 回调...",
            "🤖".cyan(), corr_id.yellow()
        );

        poll_for_callback(&client, corr_id, send_time_ms)
    }

    /// 发飞书通知卡片，告知 Pipeline 已保存
    pub fn notify_pipeline_saved(
        &self,
        pipeline_name: &str,
        step_count: usize,
        path: &str,
    ) -> Result<()> {
        let client = self.make_client()?;
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
                        { "is_short": true, "text": { "tag": "lark_md",
                          "content": format!("**Pipeline**\n{}", pipeline_name) } },
                        { "is_short": true, "text": { "tag": "lark_md",
                          "content": format!("**步骤数**\n{} 个", step_count) } }
                    ]
                },
                { "tag": "div", "text": { "tag": "lark_md",
                  "content": format!("📁 `{}`", path) } },
                { "tag": "hr" },
                { "tag": "div", "text": { "tag": "lark_md",
                  "content": format!("运行: `molt run {}`", pipeline_name) } }
            ]
        });
        client.send_card(&card)
    }
}

// ── 内部构建函数 ──────────────────────────────────────────────────────────────

fn build_execution_card(
    pipeline_name: &str,
    step_name: &str,
    cmd: &str,
    step_index: usize,
    total_steps: usize,
    corr_id: &str,
) -> serde_json::Value {
    json!({
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
                    { "is_short": true, "text": { "tag": "lark_md",
                      "content": format!("**Pipeline**\n{}", pipeline_name) } },
                    { "is_short": true, "text": { "tag": "lark_md",
                      "content": format!("**步骤**\n{}", step_name) } }
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
                        "tag": "button", "type": "primary",
                        "text": { "content": "✅ 立即执行", "tag": "lark_md" },
                        "value": { "action": "execute", "cmd": cmd, "corr_id": corr_id }
                    },
                    {
                        "tag": "button", "type": "default",
                        "text": { "content": "🤔 分析再执行", "tag": "lark_md" },
                        "value": { "action": "analyze", "cmd": cmd, "corr_id": corr_id }
                    },
                    {
                        "tag": "button", "type": "danger",
                        "text": { "content": "⏭ 跳过", "tag": "lark_md" },
                        "value": { "action": "skip", "cmd": cmd, "corr_id": corr_id }
                    }
                ]
            },
            {
                "tag": "note",
                "elements": [{ "tag": "plain_text",
                  "content": format!("关联 ID: {}", corr_id) }]
            }
        ]
    })
}

fn poll_for_callback(
    client: &FeishuClient,
    corr_id: &str,
    send_time_ms: u64,
) -> Result<String> {
    let marker = format!("[MOLT_CALLBACK:{}]", corr_id);
    let spinner = make_spinner("等待 ClawBot 回调...");
    let deadline = std::time::Instant::now() + Duration::from_secs(client.poll_timeout_secs);

    while std::time::Instant::now() < deadline {
        thread::sleep(Duration::from_secs(3));
        if let Some(text) = client.find_text_after(&marker, send_time_ms) {
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
    bail!("Timeout ({}s): no [MOLT_CALLBACK] from ClawBot", client.poll_timeout_secs)
}
