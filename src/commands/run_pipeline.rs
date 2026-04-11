use colored::Colorize;
use std::fs;

use crate::config::pipelines_dir;
use crate::pipeline::{parse_pipeline_yaml, Pipeline};

pub fn run(name: &str) {
    // 1. 加载 pipeline YAML
    let pipeline = load_pipeline(name);

    println!(
        "\n{} 执行 Pipeline: {}",
        "🚀".green(),
        pipeline.name.cyan().bold()
    );
    if let Some(desc) = &pipeline.description {
        println!("   {}", desc.dimmed());
    }
    println!("{}", "─".repeat(50).dimmed());

    // 2. 按步骤执行
    for (i, step) in pipeline.steps.iter().enumerate() {
        println!(
            "\n{} 步骤 {}/{}: {}",
            "▶".cyan(),
            i + 1,
            pipeline.steps.len(),
            step.name.yellow()
        );
        if let Some(desc) = &step.description {
            println!("   {}", desc.dimmed());
        }
        println!("   $ {}", step.cmd.green());

        match step.executor.as_str() {
            "feishu_bot" => run_via_feishu_bot(&step.cmd, &step.name),
            "ask"        => run_with_confirmation(&step.cmd),
            _            => run_local(&step.cmd),
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
            eprintln!("   可用列表: molt run --list");
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

    let status = Command::new(parts[0])
        .args(&parts[1..])
        .status();

    match status {
        Ok(s) if s.success() => {
            println!("   {}", "✓ 执行成功".green());
        }
        Ok(s) => {
            eprintln!("   {} 退出码: {}", "✗ 执行失败".red(), s.code().unwrap_or(-1));
        }
        Err(e) => {
            eprintln!("   {} {}", "✗ 启动失败:".red(), e);
        }
    }
}

fn run_with_confirmation(cmd: &str) {
    use inquire::Confirm;

    let ok = Confirm::new(&format!("执行命令？\n  $ {}", cmd))
        .with_default(true)
        .prompt()
        .unwrap_or(false);

    if ok {
        run_local(cmd);
    } else {
        println!("   {} 已跳过", "—".yellow());
    }
}

/// Day 3: 发飞书授权卡片给 ClawBot，让它异步执行并回调结果
///
/// 架构：
///   molt run → 发 Feishu Interactive Card → ClawBot 判断执行路径
///     ├── 本地命令 → 通过回调触发本地执行
///     ├── 需要找人 → ClawBot 在飞书中 @相关人员
///     └── 需要 LLM 分析 → ClawBot 内部调用 LLM，返回执行建议
///   → 等待 [MOLT_CALLBACK:<corr_id>] 回调
fn run_via_feishu_bot(cmd: &str, _step_name: &str) {
    // TODO Day 3: 实现飞书互动卡片 + 回调等待
    // 临时降级为本地执行 + 提示
    println!(
        "   {} 该步骤标记为 feishu_bot，Day 3 将委托 ClawBot 执行",
        "🤖".cyan()
    );
    println!("   当前降级为本地执行...");
    run_local(cmd);
}
