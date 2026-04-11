use colored::Colorize;
use std::fs;

use crate::ai::feishu_bot::FeishuBotBackend;
use crate::config::{pipelines_dir, BackendConfig, MoltConfig};
use crate::pipeline::{parse_pipeline_yaml, Pipeline};

pub fn run(name: &str) {
    let pipeline = load_pipeline(name);

    println!(
        "\n{} 执行 Pipeline: {}",
        "🚀".green(),
        pipeline.name.cyan().bold()
    );
    if let Some(desc) = &pipeline.description {
        println!("   {}", desc.dimmed());
    }
    println!("   {} 个步骤", pipeline.steps.len().to_string().yellow());
    println!("{}", "─".repeat(50).dimmed());

    // 获取 Feishu 配置（run_via_feishu_bot 需要）
    let feishu_backend = MoltConfig::load().and_then(|cfg| {
        if let BackendConfig::FeishuBot { app_id, app_secret, chat_id, poll_timeout_secs } = cfg.backend {
            Some(FeishuBotBackend { app_id, app_secret, chat_id, poll_timeout_secs })
        } else {
            None
        }
    });

    let total = pipeline.steps.len();
    for (i, step) in pipeline.steps.iter().enumerate() {
        println!(
            "\n{} [{}/{}] {}",
            "▶".cyan(),
            i + 1,
            total,
            step.name.yellow().bold()
        );
        if let Some(desc) = &step.description {
            println!("   {}", desc.dimmed());
        }
        println!("   $ {}", step.cmd.green());

        match step.executor.as_str() {
            "feishu_bot" => {
                match &feishu_backend {
                    Some(fb) => run_via_feishu_bot(fb, &pipeline.name, step, i + 1, total),
                    None => {
                        println!(
                            "   {} 未配置 Feishu 后端，降级为本地执行",
                            "⚠️".yellow()
                        );
                        run_local(&step.cmd);
                    }
                }
            }
            "ask" => run_with_confirmation(&step.cmd),
            _ => run_local(&step.cmd),
        }
    }

    println!("\n{} Pipeline 执行完毕！", "✅".green());
}

fn load_pipeline(name: &str) -> Pipeline {
    let dir = pipelines_dir().unwrap_or_default();
    let path = dir.join(format!("{}.yaml", name));

    let content = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => {
            eprintln!(
                "{} Pipeline '{}' 不存在: {}",
                "❌".red(),
                name,
                path.display()
            );
            // 显示已有的 pipelines
            if let Ok(entries) = fs::read_dir(&dir) {
                let names: Vec<_> = entries
                    .flatten()
                    .filter_map(|e| {
                        let n = e.file_name().to_string_lossy().to_string();
                        if n.ends_with(".yaml") { Some(n.replace(".yaml", "")) } else { None }
                    })
                    .collect();
                if names.is_empty() {
                    eprintln!("   (暂无已保存的 pipeline)");
                } else {
                    eprintln!("   可用: {}", names.join(", "));
                }
            }
            std::process::exit(1);
        }
    };

    match parse_pipeline_yaml(&content) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{} Pipeline YAML 解析失败: {}", "❌".red(), e);
            std::process::exit(1);
        }
    }
}

fn run_local(cmd: &str) {
    use std::process::Command;

    let parts: Vec<&str> = cmd.split_whitespace().collect();
    if parts.is_empty() { return; }

    let status = Command::new(parts[0]).args(&parts[1..]).status();

    match status {
        Ok(s) if s.success() => println!("   {}", "✓ 执行成功".green()),
        Ok(s) => eprintln!("   {} 退出码: {}", "✗".red(), s.code().unwrap_or(-1)),
        Err(e) => eprintln!("   {} {}", "✗ 启动失败:".red(), e),
    }
}

fn run_with_confirmation(cmd: &str) {
    use inquire::Confirm;

    let ok = Confirm::new(&format!("执行？\n  $ {}", cmd))
        .with_default(true)
        .prompt()
        .unwrap_or(false);

    if ok { run_local(cmd); } else { println!("   {} 已跳过", "—".yellow()); }
}

/// 发互动卡片给 ClawBot，等待 [MOLT_CALLBACK:<id>] 回调
///
/// ClawBot 收到按钮事件后：
///   - 执行: 在本机/云端执行命令，回复 "[MOLT_CALLBACK:<id>] result: <输出>"
///   - 分析: 内部 LLM 分析命令上下文，给出执行建议并回复
///   - 跳过: 回复 "[MOLT_CALLBACK:<id>] result: skipped"
fn run_via_feishu_bot(
    fb: &FeishuBotBackend,
    pipeline_name: &str,
    step: &crate::pipeline::PipelineStep,
    step_index: usize,
    total_steps: usize,
) {
    match fb.run_step(pipeline_name, &step.name, &step.cmd, step_index, total_steps) {
        Ok(result) => {
            if result == "skipped" {
                println!("   {} ClawBot 跳过了此步骤", "—".yellow());
            } else {
                println!("   {} ClawBot 执行结果:", "✓".green());
                for line in result.lines().take(20) {
                    println!("   {}", line);
                }
            }
        }
        Err(e) => {
            eprintln!("   {} ClawBot 回调失败: {}", "❌".red(), e);
            eprintln!("   降级为本地执行...");
            run_local(&step.cmd);
        }
    }
}
