use anyhow::{Context, Result};
use regex::Regex;
use serde_json;
use std::collections::HashMap;
use std::fs;
use std::sync::OnceLock;

use super::{molt_mark_re, strip_ansi};

static CMD_RE: OnceLock<Regex> = OnceLock::new();

fn cmd_re() -> &'static Regex {
    CMD_RE.get_or_init(|| {
        // bash: $ cmd, zsh: % cmd / ❯ cmd / ➜ cmd, root: # cmd
        Regex::new(r"(?:[$%#❯➜]\s+|>\s+)([a-zA-Z][a-zA-Z0-9_/-]*)").unwrap()
    })
}

// ── 公共类型 ──────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct CastStats {
    pub path: String,
    pub file_size_bytes: u64,
    pub total_duration_secs: f64,
    pub total_output_events: usize,
    pub total_input_events: usize,
    pub segments: Vec<SegmentStats>,
    /// (command_name, count)，降序，取前 10
    pub top_commands: Vec<(String, usize)>,
}

#[derive(Debug)]
pub struct SegmentStats {
    pub mark_index: u32,
    pub label: Option<String>,
    pub start_secs: f64,
    pub end_secs: f64,
    pub event_count: usize,
}

impl CastStats {
    pub fn total_events(&self) -> usize {
        self.total_output_events + self.total_input_events
    }
    pub fn duration_display(&self) -> String {
        fmt_duration(self.total_duration_secs)
    }
}

impl SegmentStats {
    pub fn duration_secs(&self) -> f64 { self.end_secs - self.start_secs }
    pub fn duration_display(&self) -> String { fmt_duration(self.duration_secs()) }
    pub fn label_display(&self) -> String {
        self.label.clone().unwrap_or_else(|| "—".to_string())
    }
    pub fn name_display(&self) -> String {
        if self.mark_index == 0 { "Intro".to_string() }
        else { format!("Mark {}", self.mark_index) }
    }
}

pub fn fmt_duration(secs: f64) -> String {
    let s = secs as u64;
    if s < 60 { format!("0:{:02}", s) }
    else { format!("{}:{:02}", s / 60, s % 60) }
}

// ── 解析 ──────────────────────────────────────────────────────────────────────

pub fn parse_cast_stats(path: &str) -> Result<CastStats> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("Cannot read cast file: {}", path))?;
    let file_size_bytes = fs::metadata(path).map(|m| m.len()).unwrap_or(0);

    let mut lines = raw.lines();
    lines.next().context("Cast file is empty")?;

    // (time, kind, data)
    let mut all: Vec<(f64, String, String)> = Vec::new();
    for line in lines {
        let line = line.trim();
        if line.is_empty() { continue; }
        if let Ok(arr) = serde_json::from_str::<serde_json::Value>(line) {
            if let (Some(t), Some(k), Some(d)) = (
                arr[0].as_f64(), arr[1].as_str(), arr[2].as_str(),
            ) {
                all.push((t, k.to_string(), d.to_string()));
            }
        }
    }

    let total_duration_secs = all.last().map(|(t, _, _)| *t).unwrap_or(0.0);
    let total_output_events = all.iter().filter(|(_, k, _)| k == "o").count();
    let total_input_events = all.iter().filter(|(_, k, _)| k == "i").count();
    let segments = build_segments(&all, total_duration_secs);

    let all_output: String = all.iter()
        .filter(|(_, k, _)| k == "o")
        .map(|(_, _, d)| strip_ansi(d))
        .collect::<Vec<_>>()
        .join("\n");
    let top_commands = detect_commands(&all_output);

    Ok(CastStats {
        path: path.to_string(),
        file_size_bytes,
        total_duration_secs,
        total_output_events,
        total_input_events,
        segments,
        top_commands,
    })
}

fn build_segments(events: &[(f64, String, String)], total: f64) -> Vec<SegmentStats> {
    let mut segments = Vec::new();
    let mut cur_mark = 0u32;
    let mut cur_label: Option<String> = None;
    let mut seg_start = 0.0f64;
    let mut seg_events = 0usize;

    for (time, kind, data) in events {
        if kind != "o" { continue; }
        let stripped = strip_ansi(data);
        if let Some(caps) = molt_mark_re().captures(&stripped) {
            segments.push(SegmentStats {
                mark_index: cur_mark,
                label: cur_label.clone(),
                start_secs: seg_start,
                end_secs: *time,
                event_count: seg_events,
            });
            cur_mark = caps[1].parse().unwrap_or(0);
            cur_label = caps.get(3)
                .map(|m| m.as_str().trim().to_string())
                .filter(|s| !s.is_empty());
            seg_start = *time;
            seg_events = 0;
        } else {
            seg_events += 1;
        }
    }
    segments.push(SegmentStats {
        mark_index: cur_mark,
        label: cur_label,
        start_secs: seg_start,
        end_secs: total,
        event_count: seg_events,
    });
    segments
}

fn detect_commands(text: &str) -> Vec<(String, usize)> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for caps in cmd_re().captures_iter(text) {
        let cmd = caps[1].to_lowercase();
        if cmd.len() <= 20 && !cmd.starts_with(|c: char| c.is_ascii_digit()) {
            *counts.entry(cmd).or_insert(0) += 1;
        }
    }
    let mut sorted: Vec<_> = counts.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));
    sorted.into_iter().take(10).collect()
}
