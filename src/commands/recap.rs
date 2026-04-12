/// molt recap — usage analytics + OpenClaw lift visualization
use chrono::{NaiveDate, Utc};
use colored::Colorize;
use std::collections::BTreeMap;

use crate::history::{load_history, RunRecord};

const MANUAL_SECS_PER_STEP: u64 = 300; // 5 min conservative estimate per ClawBot delegation

pub fn run(days: u32, filter_pipeline: Option<&str>) {
    let records = match load_history(days) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{} 无法读取历史记录: {}", "❌".red(), e);
            return;
        }
    };

    let records: Vec<RunRecord> = if let Some(name) = filter_pipeline {
        records.into_iter().filter(|r| r.pipeline == name).collect()
    } else {
        records
    };

    let today = Utc::now().format("%d %b %Y").to_string();
    let sep = "━".repeat(66);

    println!("\n{}", sep.cyan().bold());
    println!("  {}  molt recap  ·  last {} days  ·  {}", "🦞", days, today.white().bold());
    println!("{}", sep.cyan().bold());

    if records.is_empty() {
        println!();
        println!("  No run history yet for this period.");
        println!("  Run {}  to start tracking.", "`molt run <pipeline>`".dimmed());
        println!("\n{}\n", sep.dimmed());
        return;
    }

    print_runs_section(&records);
    print_pipelines_section(&records);
    print_activity_section(&records);
    print_openclaw_section(&records);
    print_intent_section(&records);
    print_reliability_section(&records);
    print_footer(&records, &sep);
}

// ── section header helper ─────────────────────────────────────────────────────

fn section_line(title: &str) -> String {
    // title is ASCII — .len() equals display width
    let dashes = 60_usize.saturating_sub(title.len() + 1);
    format!("  {} {}", title.white().bold(), "─".repeat(dashes).dimmed())
}

// ── RUNS ──────────────────────────────────────────────────────────────────────

fn print_runs_section(records: &[RunRecord]) {
    let total = records.len();
    let success = records.iter().filter(|r| r.status == "success").count();
    let failed = total - success;
    let total_ms: u64 = records.iter().map(|r| r.duration_ms).sum();
    let avg_per_day = total as f64 / 30.0;

    println!();
    println!("{}", section_line("RUNS"));
    println!(
        "  Total  {}   Avg/day  {:.1}   Success  {} ({:.0}%)",
        total.to_string().yellow().bold(),
        avg_per_day,
        success.to_string().green(),
        if total > 0 { (success as f64 / total as f64) * 100.0 } else { 0.0 }
    );
    println!(
        "  Failed {}   Total time  {}",
        if failed > 0 { failed.to_string().red().bold() } else { "0".normal() },
        fmt_ms(total_ms).yellow()
    );
}

// ── PIPELINES ─────────────────────────────────────────────────────────────────

fn print_pipelines_section(records: &[RunRecord]) {
    let mut counts: BTreeMap<&str, (usize, u64, usize)> = BTreeMap::new();
    for r in records {
        let e = counts.entry(r.pipeline.as_str()).or_default();
        e.0 += 1;
        e.1 += r.duration_ms;
        if r.status == "success" { e.2 += 1; }
    }

    let mut entries: Vec<(&str, usize, u64, usize)> = counts
        .into_iter()
        .map(|(name, (runs, ms, ok))| (name, runs, ms, ok))
        .collect();
    entries.sort_by(|a, b| b.1.cmp(&a.1));
    entries.truncate(8);

    let max_runs = entries.first().map(|e| e.1).unwrap_or(1);

    println!();
    println!("{}", section_line(&format!("PIPELINES  (top {} by runs)", entries.len())));
    for &(name, runs, total_ms, ok) in &entries {
        let avg_ms = if runs > 0 { total_ms / runs as u64 } else { 0 };
        let ok_pct = if runs > 0 { (ok as f64 / runs as f64) * 100.0 } else { 0.0 };
        let bar_len = ((runs as f64 / max_runs as f64) * 24.0) as usize;
        let bar = "█".repeat(bar_len.max(1)) + &"░".repeat(24 - bar_len.max(1));
        println!(
            "  {:<22}  {}  {}  {}  {:.0}%{}",
            name.cyan(),
            bar.green(),
            runs.to_string().yellow(),
            fmt_ms(avg_ms).dimmed(),
            ok_pct,
            if ok_pct == 100.0 { " ✔".green().to_string() } else { String::new() }
        );
    }
}

// ── ACTIVITY ──────────────────────────────────────────────────────────────────

fn print_activity_section(records: &[RunRecord]) {
    let mut by_date: BTreeMap<NaiveDate, usize> = BTreeMap::new();
    for r in records {
        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&r.started_at) {
            *by_date.entry(dt.date_naive()).or_default() += 1;
        }
    }
    if by_date.is_empty() { return; }

    let mut days_sorted: Vec<(NaiveDate, usize)> = by_date.into_iter().collect();
    days_sorted.sort_by_key(|(d, _)| *d);
    let display: Vec<(NaiveDate, usize)> = days_sorted.into_iter().rev().take(14).rev().collect();

    let max_count = display.iter().map(|(_, c)| *c).max().unwrap_or(1);
    let today_date = Utc::now().date_naive();

    println!();
    println!("{}", section_line("ACTIVITY  (last 14 active days)"));
    for (date, count) in &display {
        let bar_len = ((*count as f64 / max_count as f64) * 20.0) as usize;
        let bar = "█".repeat(bar_len.max(1));
        let suffix = if *date == today_date { " ← today".dimmed().to_string() } else { String::new() };
        println!(
            "  {}  {}  {}{}",
            date.format("%b %d").to_string().dimmed(),
            bar.green(),
            count.to_string().yellow(),
            suffix
        );
    }
}

// ── OPENCLAW LIFT ─────────────────────────────────────────────────────────────

fn print_openclaw_section(records: &[RunRecord]) {
    let total_clawbot_steps: usize = records.iter().map(|r| r.clawbot_steps).sum();
    let total_clawbot_ms: u64 = records.iter().map(|r| r.clawbot_duration_ms).sum();
    let runs_with_clawbot = records.iter().filter(|r| r.clawbot_steps > 0).count();

    println!();
    println!("{}", section_line("OPENCLAW LIFT"));

    if total_clawbot_steps == 0 {
        println!("  No ClawBot steps recorded yet.");
        println!("  Set {}  on a pipeline step to delegate.", "executor: feishu_bot".cyan());
        return;
    }

    println!(
        "  {} steps delegated across {} pipeline runs",
        total_clawbot_steps.to_string().cyan().bold(),
        runs_with_clawbot.to_string().yellow()
    );
    println!();

    let manual_ms = total_clawbot_steps as u64 * MANUAL_SECS_PER_STEP * 1000;
    let actual_ms = total_clawbot_ms.max(1);
    let saved_ms = manual_ms.saturating_sub(actual_ms);
    let lift = manual_ms as f64 / actual_ms as f64;

    let bar_width = 30usize;
    let actual_bar = ((actual_ms as f64 / manual_ms as f64) * bar_width as f64) as usize;
    let actual_bar = actual_bar.max(1).min(bar_width);

    println!("  Estimated savings:");
    println!("  Manual   {:>8}  {}", fmt_ms(manual_ms), "░".repeat(bar_width).dimmed());
    println!(
        "  Actual   {:>8}  {}{}",
        fmt_ms(actual_ms),
        "█".repeat(actual_bar).cyan(),
        "░".repeat(bar_width - actual_bar).dimmed()
    );
    println!(
        "  Saved    {:>8}  Lift  {:.1}× via OpenClaw",
        fmt_ms(saved_ms).green(),
        lift
    );

    // Per-pipeline breakdown
    let mut seen: std::collections::HashMap<&str, (usize, usize)> = std::collections::HashMap::new();
    for r in records {
        if r.clawbot_steps > 0 {
            let e = seen.entry(r.pipeline.as_str()).or_default();
            e.0 += r.clawbot_steps;
            e.1 += 1;
        }
    }
    let mut by_pipeline: Vec<(&str, usize, usize)> =
        seen.into_iter().map(|(n, (s, r))| (n, s, r)).collect();
    by_pipeline.sort_by(|a, b| b.1.cmp(&a.1));

    if !by_pipeline.is_empty() {
        println!();
        println!("  Breakdown:");
        for (name, steps, runs) in &by_pipeline {
            println!(
                "  {} {:<22}  {} step × {} run = {} delegations",
                "✦".cyan(), name.cyan(), steps, runs, steps * runs
            );
        }
    }
}

// ── INTENT MODE ───────────────────────────────────────────────────────────────

fn print_intent_section(records: &[RunRecord]) {
    let intent_runs: Vec<&RunRecord> = records.iter().filter(|r| r.trigger == "intent").collect();
    if intent_runs.is_empty() { return; }

    let total = records.len();
    let intent_count = intent_runs.len();
    let auto_run = intent_runs.iter()
        .filter(|r| r.intent_confidence.map(|c| c >= 0.80).unwrap_or(false))
        .count();
    let avg_conf: f64 = {
        let vals: Vec<f64> = intent_runs.iter().filter_map(|r| r.intent_confidence).collect();
        if vals.is_empty() { 0.0 } else { vals.iter().sum::<f64>() / vals.len() as f64 }
    };

    println!();
    println!("{}", section_line("INTENT MATCHING"));
    println!(
        "  Runs via -v    {} ({:.0}%)   Avg confidence  {:.0}%",
        intent_count.to_string().yellow(),
        (intent_count as f64 / total as f64) * 100.0,
        avg_conf * 100.0
    );
    println!(
        "  Auto-run  {}   Confirmed  {}",
        auto_run.to_string().green(),
        (intent_count - auto_run).to_string().yellow()
    );
}

// ── RELIABILITY ───────────────────────────────────────────────────────────────

fn print_reliability_section(records: &[RunRecord]) {
    let failed: Vec<&RunRecord> = records.iter().filter(|r| r.status == "failed").collect();
    if failed.is_empty() { return; }

    println!();
    println!("{}", section_line("RELIABILITY"));
    println!(
        "  {} {} this period:",
        failed.len().to_string().red(),
        if failed.len() == 1 { "failure" } else { "failures" }
    );
    println!();
    for r in failed.iter().take(5) {
        let date = chrono::DateTime::parse_from_rfc3339(&r.started_at)
            .map(|dt| dt.format("%b %d").to_string())
            .unwrap_or_else(|_| "?".to_string());
        println!(
            "  {}  {:<22}  {}",
            date.dimmed(),
            r.pipeline.yellow(),
            r.failed_step.as_deref().unwrap_or("?").red()
        );
    }

    // Flaky pipeline: > 30% failure rate with >= 3 runs
    let mut stats: std::collections::HashMap<&str, (usize, usize)> = std::collections::HashMap::new();
    for r in records {
        let e = stats.entry(r.pipeline.as_str()).or_default();
        e.0 += 1;
        if r.status == "failed" { e.1 += 1; }
    }
    for (name, (runs, fails)) in &stats {
        if *runs >= 3 && (*fails as f64 / *runs as f64) > 0.30 {
            println!();
            println!(
                "  {} Flaky:  {}  ({}/{} runs failed)",
                "⚠".yellow(), name.yellow().bold(), fails, runs
            );
            println!("  Refresh:  {}", format!("molt record --name {}", name).dimmed());
        }
    }
}

// ── FOOTER ────────────────────────────────────────────────────────────────────

fn print_footer(records: &[RunRecord], sep: &str) {
    let total_clawbot_steps: usize = records.iter().map(|r| r.clawbot_steps).sum();
    let total_clawbot_ms: u64 = records.iter().map(|r| r.clawbot_duration_ms).sum();
    let manual_ms = total_clawbot_steps as u64 * MANUAL_SECS_PER_STEP * 1000;
    let saved_ms = manual_ms.saturating_sub(total_clawbot_ms);

    println!();
    println!("{}", sep.cyan().bold());
    if total_clawbot_steps > 0 {
        println!(
            "  OpenClaw handled {} steps — saving {} this period. That's the leverage.",
            total_clawbot_steps.to_string().cyan().bold(),
            fmt_ms(saved_ms).green().bold()
        );
    } else {
        println!(
            "  {} pipeline runs recorded.  Add {}  to see OpenClaw lift.",
            records.len().to_string().yellow(),
            "executor: feishu_bot".cyan()
        );
    }
    println!("{}\n", sep.cyan().bold());
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn fmt_ms(ms: u64) -> String {
    if ms == 0 { return "0s".to_string(); }
    if ms < 1_000 { return format!("{}ms", ms); }
    let s = ms / 1_000;
    if s < 60 { return format!("{}s", s); }
    let m = s / 60;
    if m < 60 { return format!("{}m {:02}s", m, s % 60); }
    format!("{}h {:02}m", m / 60, m % 60)
}
