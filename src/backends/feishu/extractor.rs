/// Feishu 提取后端
///
/// 实现 AiBackend trait：把终端录制内容发给 ClawBot，
/// 通过 [MOLT_REQUEST/RESPONSE:<id>] 关联 ID 协议等待 YAML 回复
use anyhow::{bail, Result};
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use std::thread;
use std::time::Duration;
use uuid::Uuid;

use crate::backends::{AiBackend, build_extraction_prompt};
use crate::recording::MarkSlice;
use super::client::FeishuClient;

pub struct FeishuBotBackend {
    pub app_id: String,
    pub app_secret: String,
    pub chat_id: String,
    pub poll_timeout_secs: u64,
}

impl FeishuBotBackend {
    pub fn make_client(&self) -> Result<FeishuClient> {
        FeishuClient::new(&self.app_id, &self.app_secret, &self.chat_id, self.poll_timeout_secs)
    }
}

impl AiBackend for FeishuBotBackend {
    fn extract_pipeline(&self, slices: &[MarkSlice]) -> Result<String> {
        let client = self.make_client()?;
        let corr_id = &Uuid::new_v4().to_string()[..8];
        let send_time_ms = FeishuClient::now_ms();

        let prompt = build_extraction_prompt(slices);
        let message = format!(
            "[MOLT_REQUEST:{}]\n\n{}\n\n请在回复末尾附上 [MOLT_RESPONSE:{}]",
            corr_id, prompt, corr_id
        );

        client.send_text(&message)?;
        println!(
            "   {} [MOLT_REQUEST:{}] 已发送到飞书",
            "→".cyan(), corr_id.yellow()
        );

        poll_for_marker(
            &client,
            &format!("[MOLT_RESPONSE:{}]", corr_id),
            send_time_ms,
        )
    }
}

pub fn poll_for_marker(
    client: &FeishuClient,
    marker: &str,
    send_time_ms: u64,
) -> Result<String> {
    let spinner = make_spinner("等待飞书 Bot 回复...");
    let deadline = std::time::Instant::now() + Duration::from_secs(client.poll_timeout_secs);

    while std::time::Instant::now() < deadline {
        thread::sleep(Duration::from_secs(3));

        if let Some(text) = client.find_text_after(marker, send_time_ms) {
            spinner.finish_with_message(format!("{} 收到回复！", "✅".green()));
            return Ok(text.replace(marker, "").trim().to_string());
        }
    }

    spinner.finish_with_message(format!("{} 超时", "❌".red()));
    bail!("Timeout ({}s): no response from Feishu bot", client.poll_timeout_secs)
}

pub fn make_spinner(msg: &str) -> ProgressBar {
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
