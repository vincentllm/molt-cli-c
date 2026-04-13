/// molt list — show all saved pipelines
use colored::Colorize;
use std::fs;

use crate::config::pipelines_dir;
use crate::pipeline::parse_pipeline_yaml;

pub fn run() {
    let dir = match pipelines_dir() {
        Some(d) => d,
        None => {
            eprintln!("No pipelines directory found.");
            return;
        }
    };

    let entries = match fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => {
            println!("\n  No saved pipelines yet.");
            println!("  Run {} to record and extract your first pipeline.\n", "`molt record`".cyan());
            return;
        }
    };

    let mut pipelines: Vec<_> = entries
        .flatten()
        .filter_map(|e| {
            let path = e.path();
            if path.extension()?.to_str()? != "yaml" { return None; }
            let content = fs::read_to_string(&path).ok()?;
            parse_pipeline_yaml(&content).ok()
        })
        .collect();

    if pipelines.is_empty() {
        println!("\n  No saved pipelines yet.");
        println!("  Run {} to record and extract your first pipeline.\n", "`molt record`".cyan());
        return;
    }

    pipelines.sort_by(|a, b| a.name.cmp(&b.name));

    let sep = "─".repeat(66);
    println!("\n  🦞 Saved Pipelines  ({})", pipelines.len().to_string().yellow());
    println!("  {}", sep.dimmed());

    for p in &pipelines {
        let total = p.steps.len();
        let clawbot = p.steps.iter().filter(|s| s.executor == "feishu_bot").count();
        let local = total - clawbot;

        let steps_tag = if clawbot > 0 {
            format!(
                "{} step(s)  local {} · ✦ clawbot {}",
                total, local, clawbot
            )
        } else {
            format!("{} step(s)  local {}", total, local)
        };

        let desc = p.description.as_deref().unwrap_or("");
        let desc_display = if desc.len() > 44 { &desc[..44] } else { desc };

        println!(
            "  {:<24}  {}",
            p.name.cyan().bold(),
            steps_tag.dimmed()
        );
        if !desc.is_empty() {
            println!("  {:<24}  {}", "", desc_display.dimmed());
        }
    }

    println!("  {}", sep.dimmed());
    println!(
        "  Run:  {}    Intent:  {}\n",
        "molt run <name>".green(),
        "molt run -v \"<query>\"".green()
    );
}
