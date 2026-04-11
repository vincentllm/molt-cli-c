use colored::Colorize;
use inquire::{Confirm, Select, Text};
use std::fs;
use std::process::{Command, Stdio};

use crate::ai::build_backend;
use crate::ai::feishu_bot::FeishuBotBackend;
use crate::cast_parser::{parse_cast, MarkSlice, CAST_FILE};
use crate::config::{BackendConfig, MoltConfig};
use crate::pipeline::{extract_yaml_from_response, parse_pipeline_yaml, save_pipeline};

const PID_FILE: &str = "/tmp/molt_session.pid";

pub fn run() {
    // ── 1. 停止 asciinema ──────────────────────────────────────────────
    let cast_path = stop_recording();

    // ── 2. 解析 .cast 文件 ────────────────────────────────────────────
    println!("\n{} 解析录制文件...", "📼".cyan());
    let slices = match parse_cast(&cast_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{} {}", "❌ 解析失败:".red(), e);
            std::process::exit(1);
        }
    };

    if slices.is_empty() {
        println!(
            "{} 未找到任何 MOLT_MARK 分段。录制中使用 `molt mark` 打标记可提升提取质量。",
            "⚠️".yellow()
        );
    }

    show_slices_summary(&slices);

    // ── 3. 确定 AI 后端 ───────────────────────────────────────────────
    let config = load_or_prompt_config();
    let backend = build_backend(&config.backend);

    // ── 4. 调用 AI 提取 ───────────────────────────────────────────────
    let backend_name = backend_display_name(&config.backend);
    println!("\n{} 正在通过 {} 提取 Pipeline...", "🤖".cyan(), backend_name.yellow());

    let raw_response = match backend.extract_pipeline(&slices) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{} AI 提取失败: {}", "❌".red(), e);
            eprintln!("   提示：可检查 ~/.molt/config.yaml 中的配置");
            std::process::exit(1);
        }
    };

    // ── 5. 解析 YAML ──────────────────────────────────────────────────
    let yaml_text = extract_yaml_from_response(&raw_response);

    let mut pipeline = match parse_pipeline_yaml(&yaml_text) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{} 无法解析 YAML: {}", "❌".red(), e);
            eprintln!("--- AI 原始输出 ---\n{}", raw_response);
            std::process::exit(1);
        }
    };

    // ── 6. 展示提取结果 ───────────────────────────────────────────────
    println!("\n{}", "✅ 提取成功！Pipeline 预览：".green().bold());
    println!("{}", "─".repeat(50).dimmed());
    for step in &pipeline.steps {
        let exec_icon = match step.executor.as_str() {
            "feishu_bot" => "🤖",
            "ask"        => "❓",
            _            => "💻",
        };
        println!(
            "  {} {} {}",
            exec_icon,
            format!("[MARK {}]", step.mark.unwrap_or(0)).dimmed(),
            step.name.cyan()
        );
        println!("     $ {}", step.cmd.green());
        if let Some(desc) = &step.description {
            println!("     {}", desc.dimmed());
        }
    }
    println!("{}", "─".repeat(50).dimmed());

    // ── 7. 命名并保存 ─────────────────────────────────────────────────
    let default_name = pipeline.name.clone();
    let name = Text::new("Pipeline 名称:")
        .with_default(&default_name)
        .prompt()
        .unwrap_or(default_name);

    let confirm = Confirm::new(&format!("保存到 ~/.molt/pipelines/{}.yaml ?", name))
        .with_default(true)
        .prompt()
        .unwrap_or(false);

    if !confirm {
        println!("{}", "已取消，Pipeline 未保存。".yellow());
        return;
    }

    pipeline.name = name.clone();
    pipeline.created_at = chrono::Utc::now().to_rfc3339();

    let step_count = pipeline.steps.len();
    match save_pipeline(&pipeline) {
        Ok(path) => {
            let path_str = path.display().to_string();
            println!(
                "\n{} 已保存: {}\n",
                "✅".green(),
                path_str.cyan()
            );
            println!("   运行: {}", format!("molt run {}", name).green().bold());

            // 如果配置的是 DirectLlm，但用户也有 Feishu 配置，发通知卡片
            // 如果本来就是 Feishu 后端，也发通知卡片（通知群组）
            send_feishu_notification_if_configured(&name, step_count, &path_str, &config);
        }
        Err(e) => {
            eprintln!("{} 保存失败: {}", "❌".red(), e);
            std::process::exit(1);
        }
    }
}

fn send_feishu_notification_if_configured(
    pipeline_name: &str,
    step_count: usize,
    path: &str,
    config: &MoltConfig,
) {
    if let BackendConfig::FeishuBot { app_id, app_secret, chat_id, poll_timeout_secs } = &config.backend {
        let fb = FeishuBotBackend {
            app_id: app_id.clone(),
            app_secret: app_secret.clone(),
            chat_id: chat_id.clone(),
            poll_timeout_secs: *poll_timeout_secs,
        };
        match fb.notify_pipeline_saved(pipeline_name, step_count, path) {
            Ok(_) => println!("   {} 飞书通知已发送", "🔔".cyan()),
            Err(e) => println!("   {} 飞书通知发送失败（不影响结果）: {}", "⚠️".yellow(), e),
        }
    }
}

// ── helpers ──────────────────────────────────────────────────────────────────

fn stop_recording() -> String {
    let pid_str = match fs::read_to_string(PID_FILE) {
        Ok(s) => s,
        Err(_) => {
            eprintln!("{} 没有正在进行的录制。先运行 `molt record`。", "🦞".red());
            std::process::exit(1);
        }
    };
    let pid = pid_str.trim().to_string();

    let status = Command::new("kill")
        .args(["-TERM", &pid])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    match status {
        Ok(s) if s.success() => {
            // 等 asciinema 写完 trailer
            std::thread::sleep(std::time::Duration::from_millis(800));
            let _ = fs::remove_file(PID_FILE);
            println!("{} 录制已停止。Cast 文件: {}", "🦞".green(), CAST_FILE.cyan());
        }
        _ => {
            eprintln!("{} 无法终止进程 {}（可能已退出）", "⚠️".yellow(), pid);
            let _ = fs::remove_file(PID_FILE);
        }
    }

    CAST_FILE.to_string()
}

fn show_slices_summary(slices: &[MarkSlice]) {
    let mark_count = slices.iter().filter(|s| s.mark_index > 0).count();
    println!(
        "   {} 个分段，{} 个 MOLT_MARK",
        slices.len().to_string().yellow(),
        mark_count.to_string().yellow()
    );
    for s in slices {
        if s.mark_index > 0 {
            let label = s.label.as_deref().unwrap_or("-");
            println!(
                "   {} MARK {} ({}) — {} chars",
                "◆".cyan(),
                s.mark_index.to_string().yellow(),
                label,
                s.content.len().to_string().dimmed()
            );
        }
    }
}

fn backend_display_name(cfg: &BackendConfig) -> String {
    match cfg {
        BackendConfig::FeishuBot { chat_id, .. } => format!("飞书 ClawBot ({})", &chat_id[..8.min(chat_id.len())]),
        BackendConfig::DirectLlm { model, .. } => format!("Direct LLM ({})", model),
    }
}

fn load_or_prompt_config() -> MoltConfig {
    if let Some(cfg) = MoltConfig::load() {
        return cfg;
    }

    println!(
        "\n{} 未找到 ~/.molt/config.yaml，请选择 AI 后端：",
        "⚙️".yellow()
    );

    let choices = vec!["飞书 ClawBot（Demo 推荐）", "直接调用 LLM API"];
    let choice = Select::new("选择后端:", choices)
        .prompt()
        .unwrap_or("直接调用 LLM API");

    let backend = if choice.contains("飞书") {
        let app_id = Text::new("Feishu App ID (cli_xxx):").prompt().unwrap_or_default();
        let app_secret = Text::new("Feishu App Secret:").prompt().unwrap_or_default();
        let chat_id = Text::new("Chat ID (oc_xxx):").prompt().unwrap_or_default();
        BackendConfig::FeishuBot {
            app_id,
            app_secret,
            chat_id,
            poll_timeout_secs: 90,
        }
    } else {
        let api_key = Text::new("Anthropic API Key (sk-ant-xxx):").prompt().unwrap_or_default();
        BackendConfig::DirectLlm {
            api_key,
            base_url: "https://api.anthropic.com".to_string(),
            model: "claude-opus-4-6".to_string(),
        }
    };

    let cfg = MoltConfig { backend };
    match cfg.save() {
        Ok(_) => println!("{} 配置已保存到 ~/.molt/config.yaml", "✅".green()),
        Err(e) => eprintln!("{} 配置保存失败（仅此次生效）: {}", "⚠️".yellow(), e),
    }
    cfg
}
