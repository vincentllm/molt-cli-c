use chrono::Utc;
use colored::Colorize;
use std::fs;
use std::time::Instant;
use uuid::Uuid;

use crate::backends::feishu::FeishuBotBackend;
use crate::config::{pipelines_dir, BackendConfig, MoltConfig};
use crate::history::{append_run, RunRecord, StepRecord};
use crate::pipeline::{parse_pipeline_yaml, Pipeline, PipelineStep};

const INTENT_THRESHOLD: f64 = 0.80;

// ── entry point ───────────────────────────────────────────────────────────────

pub fn run(name: Option<&str>, intent: Option<&str>, auto_yes: bool, dry_run: bool) {
    let selection = select_pipeline(name, intent, auto_yes);
    let (pipeline, trigger, intent_query, intent_confidence) = match selection {
        Some(s) => s,
        None => return,
    };
    execute_pipeline(&pipeline, &trigger, intent_query.as_deref(), intent_confidence, dry_run);
}

// ── pipeline selection ────────────────────────────────────────────────────────

fn select_pipeline(
    name: Option<&str>,
    intent: Option<&str>,
    auto_yes: bool,
) -> Option<(Pipeline, String, Option<String>, Option<f64>)> {
    let all = load_all_pipelines();

    if all.is_empty() {
        println!("{} 暂无已保存的 Pipeline。先运行 `molt stop` 提取一个。", "🦞".yellow());
        return None;
    }

    if let Some(query) = intent {
        return select_by_intent(all, query, auto_yes);
    }

    if let Some(exact) = name {
        match all.into_iter().find(|p| p.name == exact) {
            Some(p) => return Some((p, "exact".to_string(), None, None)),
            None => {
                eprintln!("{} Pipeline '{}' 不存在", "❌".red(), exact);
                return None;
            }
        }
    }

    select_interactive(all)
}

fn select_by_intent(
    pipelines: Vec<Pipeline>,
    query: &str,
    auto_yes: bool,
) -> Option<(Pipeline, String, Option<String>, Option<f64>)> {
    let mut scored: Vec<(f64, Pipeline)> = pipelines
        .into_iter()
        .map(|p| {
            let s = score_intent(query, &p.name, p.description.as_deref().unwrap_or(""));
            (s, p)
        })
        .filter(|(s, _)| *s > 0.05)
        .collect();
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(3);

    let sep = "─".repeat(66);
    println!("\n  {} intent   \"{}\"", "✦".cyan(), query.yellow());
    println!("  {}", sep.dimmed());

    if scored.is_empty() {
        println!("  No pipelines matched \"{}\".", query);
        println!();
        println!("  Record one?  {}", format!("molt record --name {}", query.replace(' ', "-")).dimmed());
        return None;
    }

    if scored.len() == 1 {
        let (score, pipeline) = scored.remove(0);
        let desc = pipeline.description.as_deref().unwrap_or("");
        println!(
            "  {}  {}  {:.0}%   {}",
            "▶".yellow(),
            pipeline.name.cyan().bold(),
            score * 100.0,
            desc.dimmed()
        );
        println!("  {}", sep.dimmed());

        if score >= INTENT_THRESHOLD && auto_yes {
            println!("  {} auto-run ({:.0}% confidence)\n", "✦".cyan(), score * 100.0);
        } else {
            let ok = inquire::Confirm::new("Run this pipeline?")
                .with_default(true)
                .prompt()
                .unwrap_or(false);
            if !ok { return None; }
        }
        return Some((pipeline, "intent".to_string(), Some(query.to_string()), Some(score)));
    }

    // Multiple candidates
    for (i, (score, p)) in scored.iter().enumerate() {
        let desc = p.description.as_deref().unwrap_or("");
        let desc = if desc.len() > 38 { &desc[..38] } else { desc };
        println!(
            "  [{}]  {:<22}  {:.0}%   {}",
            i + 1,
            p.name.cyan(),
            score * 100.0,
            desc.dimmed()
        );
    }
    println!("  {}", sep.dimmed());

    let choices: Vec<String> = scored.iter().map(|(_, p)| p.name.clone()).collect();
    let selection = inquire::Select::new("Pick a pipeline:", choices)
        .prompt()
        .ok()?;

    let (score, pipeline) = scored.into_iter().find(|(_, p)| p.name == selection)?;
    Some((pipeline, "intent".to_string(), Some(query.to_string()), Some(score)))
}

fn select_interactive(
    pipelines: Vec<Pipeline>,
) -> Option<(Pipeline, String, Option<String>, Option<f64>)> {
    let options: Vec<String> = pipelines
        .iter()
        .map(|p| {
            let desc = p.description.as_deref().unwrap_or("");
            let desc = if desc.len() > 42 { &desc[..42] } else { desc };
            format!("{} | {}", p.name, desc)
        })
        .collect();

    let selection = inquire::Select::new("Pick a pipeline:", options.clone())
        .prompt()
        .ok()?;

    let idx = options.iter().position(|o| o == &selection)?;
    Some((pipelines.into_iter().nth(idx)?, "fuzzy".to_string(), None, None))
}

fn score_intent(query: &str, name: &str, description: &str) -> f64 {
    let target = format!("{} {}", name, description).to_lowercase();
    let query_lower = query.to_lowercase();
    let words: Vec<&str> = query_lower.split_whitespace().collect();
    if words.is_empty() { return 0.0; }

    let mut hits = 0.0f64;
    for word in words.iter().copied() {
        if target.contains(word) {
            hits += 1.0;
        } else {
            let partial = target
                .split_whitespace()
                .any(|tw| tw.starts_with(word) || word.starts_with(tw));
            if partial { hits += 0.6; }
        }
    }
    hits / words.len() as f64
}

fn load_all_pipelines() -> Vec<Pipeline> {
    let dir = match pipelines_dir() {
        Some(d) => d,
        None => return vec![],
    };
    let entries = match fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return vec![],
    };
    let mut pipelines: Vec<Pipeline> = entries
        .flatten()
        .filter_map(|e| {
            let path = e.path();
            if path.extension()?.to_str()? != "yaml" { return None; }
            let content = fs::read_to_string(&path).ok()?;
            parse_pipeline_yaml(&content).ok()
        })
        .collect();
    pipelines.sort_by(|a, b| a.name.cmp(&b.name));
    pipelines
}

// ── execution ─────────────────────────────────────────────────────────────────

fn execute_pipeline(
    pipeline: &Pipeline,
    trigger: &str,
    intent_query: Option<&str>,
    intent_confidence: Option<f64>,
    dry_run: bool,
) {
    let run_id = Uuid::new_v4().to_string();
    let started_at = Utc::now().to_rfc3339();
    let run_start = Instant::now();

    // Header
    let sep = "━".repeat(66);
    println!("\n{}", sep.dimmed());
    let local_count = pipeline.steps.iter().filter(|s| s.executor == "local" || s.executor == "ask").count();
    let clawbot_count = pipeline.steps.iter().filter(|s| s.executor == "feishu_bot").count();
    println!(
        "  {} {}  {}  {}",
        "molt ▶".dimmed(),
        pipeline.name.cyan().bold(),
        if dry_run { "[dry-run]".yellow().to_string() } else { String::new() },
        format!("{} steps · local({}) · ✦ clawbot({})", pipeline.steps.len(), local_count, clawbot_count).dimmed()
    );
    if let Some(desc) = &pipeline.description {
        println!("  {}", desc.dimmed());
    }
    println!("{}", sep.dimmed());

    // Load Feishu backend once
    let feishu_backend = MoltConfig::load().and_then(|cfg| {
        if let BackendConfig::FeishuBot { app_id, app_secret, chat_id, poll_timeout_secs } = cfg.backend {
            Some(FeishuBotBackend { app_id, app_secret, chat_id, poll_timeout_secs })
        } else {
            None
        }
    });

    let total = pipeline.steps.len();
    let mut step_records: Vec<StepRecord> = Vec::new();
    let mut failed_step: Option<String> = None;
    let mut pipeline_status = "success".to_string();
    let mut total_clawbot_steps = 0usize;
    let mut total_clawbot_ms = 0u64;

    for (i, step) in pipeline.steps.iter().enumerate() {
        let executor_label = match step.executor.as_str() {
            "feishu_bot" => format!("  {} ClawBot", "✦".cyan()),
            "ask" => format!("  {} ask", "?".yellow()),
            _ => "  local".dimmed().to_string(),
        };

        println!(
            "\n  [{}/{}]  {}  {}",
            i + 1, total,
            step.name.yellow().bold(),
            executor_label
        );
        if let Some(desc) = &step.description {
            println!("           {}", desc.dimmed());
        }
        println!("           {} {}", "$".dimmed(), step.cmd.green());

        if dry_run {
            println!("           {} dry-run, skipped", "—".dimmed());
            step_records.push(StepRecord {
                name: step.name.clone(),
                executor: step.executor.clone(),
                duration_ms: 0,
                status: "skipped".to_string(),
            });
            continue;
        }

        let step_start = Instant::now();
        let step_status = execute_step(step, &feishu_backend, pipeline, i + 1, total);
        let step_ms = step_start.elapsed().as_millis() as u64;

        if step.executor == "feishu_bot" {
            total_clawbot_steps += 1;
            total_clawbot_ms += step_ms;
        }

        let is_failed = step_status == "failed";
        step_records.push(StepRecord {
            name: step.name.clone(),
            executor: step.executor.clone(),
            duration_ms: step_ms,
            status: step_status,
        });

        if is_failed {
            failed_step = Some(step.name.clone());
            pipeline_status = "failed".to_string();
            let next = i + 1;
            println!(
                "\n  {} Pipeline halted at step {}/{}.",
                "⚠️".yellow(), next, total
            );
            if next < total {
                println!(
                    "  Fix the issue then:  {}",
                    format!("molt run {} --resume {}", pipeline.name, next + 1).dimmed()
                );
            }
            break;
        }
    }

    let duration_ms = run_start.elapsed().as_millis() as u64;
    let ended_at = Utc::now().to_rfc3339();

    // Footer
    println!("\n{}", sep.dimmed());
    if pipeline_status == "success" {
        println!(
            "  {}  {} complete   {}   {} via ✦ OpenClaw",
            "✔".green(),
            pipeline.name.cyan(),
            fmt_ms(duration_ms).dimmed(),
            format!("{} step(s)", total_clawbot_steps).cyan()
        );
    } else {
        println!(
            "  {}  {} failed at: {}   {}",
            "✗".red(),
            pipeline.name.cyan(),
            failed_step.as_deref().unwrap_or("?").yellow(),
            fmt_ms(duration_ms).dimmed()
        );
    }
    println!("{}\n", sep.dimmed());

    // Write history (non-fatal)
    let record = RunRecord {
        id: run_id,
        pipeline: pipeline.name.clone(),
        started_at,
        ended_at,
        duration_ms,
        status: pipeline_status,
        failed_step,
        trigger: trigger.to_string(),
        intent_query: intent_query.map(|s| s.to_string()),
        intent_confidence,
        dry_run,
        steps: step_records,
        clawbot_steps: total_clawbot_steps,
        clawbot_duration_ms: total_clawbot_ms,
    };
    let _ = append_run(&record);
}

fn execute_step(
    step: &PipelineStep,
    feishu_backend: &Option<FeishuBotBackend>,
    pipeline: &Pipeline,
    step_index: usize,
    total_steps: usize,
) -> String {
    match step.executor.as_str() {
        "feishu_bot" => match feishu_backend {
            Some(fb) => run_via_clawbot(fb, &pipeline.name, step, step_index, total_steps),
            None => {
                println!("   {} 未配置 Feishu 后端，降级为本地执行", "⚠️".yellow());
                run_local_step(&step.cmd)
            }
        },
        "ask" => run_ask_step(&step.cmd),
        _ => run_local_step(&step.cmd),
    }
}

fn run_local_step(cmd: &str) -> String {
    use std::process::Command;
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    if parts.is_empty() { return "skipped".to_string(); }

    match Command::new(parts[0]).args(&parts[1..]).status() {
        Ok(s) if s.success() => {
            println!("           {} done", "✔".green());
            "success".to_string()
        }
        Ok(s) => {
            eprintln!("           {} exit {}", "✗".red(), s.code().unwrap_or(-1));
            "failed".to_string()
        }
        Err(e) => {
            eprintln!("           {} {}", "✗".red(), e);
            "failed".to_string()
        }
    }
}

fn run_ask_step(cmd: &str) -> String {
    let ok = inquire::Confirm::new(&format!("执行此步骤?"))
        .with_default(true)
        .prompt()
        .unwrap_or(false);

    if ok { run_local_step(cmd) } else {
        println!("           {} 已跳过", "—".yellow());
        "skipped".to_string()
    }
}

fn run_via_clawbot(
    fb: &FeishuBotBackend,
    pipeline_name: &str,
    step: &PipelineStep,
    step_index: usize,
    total_steps: usize,
) -> String {
    match fb.run_step(pipeline_name, &step.name, &step.cmd, step_index, total_steps) {
        Ok(result) => {
            if result == "skipped" {
                println!("           {} OpenClaw 跳过了此步骤", "—".yellow());
                "skipped".to_string()
            } else {
                println!("           {} OpenClaw 已响应:", "✦".cyan());
                for line in result.lines().take(10) {
                    println!("           {}", line.dimmed());
                }
                "success".to_string()
            }
        }
        Err(e) => {
            eprintln!("           {} ClawBot 回调失败: {}", "❌".red(), e);
            eprintln!("           降级为本地执行...");
            run_local_step(&step.cmd)
        }
    }
}

// ── formatting helpers ────────────────────────────────────────────────────────

fn fmt_ms(ms: u64) -> String {
    if ms < 1_000 {
        format!("{}ms", ms)
    } else if ms < 60_000 {
        format!("{:.1}s", ms as f64 / 1_000.0)
    } else {
        let s = ms / 1_000;
        format!("{}m {:02}s", s / 60, s % 60)
    }
}

// keep inquire accessible without full path
use inquire;
