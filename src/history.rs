/// ~/.molt/history.jsonl  —  one RunRecord per line
use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StepRecord {
    pub name: String,
    pub executor: String,
    pub duration_ms: u64,
    pub status: String, // "success" | "failed" | "skipped"
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RunRecord {
    pub id: String,
    pub pipeline: String,
    pub started_at: String, // ISO-8601 UTC
    pub ended_at: String,
    pub duration_ms: u64,
    pub status: String,            // "success" | "failed" | "aborted"
    pub failed_step: Option<String>,
    pub trigger: String,           // "exact" | "fuzzy" | "intent"
    pub intent_query: Option<String>,
    pub intent_confidence: Option<f64>,
    pub dry_run: bool,
    pub steps: Vec<StepRecord>,
    pub clawbot_steps: usize,
    pub clawbot_duration_ms: u64,
}

pub fn history_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".molt")
        .join("history.jsonl")
}

pub fn append_run(record: &RunRecord) -> Result<()> {
    let path = history_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
    writeln!(file, "{}", serde_json::to_string(record)?)?;
    Ok(())
}

pub fn load_history(days: u32) -> Result<Vec<RunRecord>> {
    let path = history_path();
    if !path.exists() {
        return Ok(vec![]);
    }
    let cutoff = Utc::now() - chrono::Duration::days(days as i64);
    // ISO-8601 UTC strings sort lexicographically
    let cutoff_str = cutoff.to_rfc3339();

    let reader = BufReader::new(std::fs::File::open(&path)?);
    let mut records = Vec::new();
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() { continue; }
        if let Ok(rec) = serde_json::from_str::<RunRecord>(&line) {
            if rec.started_at >= cutoff_str {
                records.push(rec);
            }
        }
    }
    Ok(records)
}
