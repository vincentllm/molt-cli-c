use anyhow::{Context, Result};
use serde_json;
use std::fs;

use super::{molt_mark_re, strip_ansi};

/// 按 MOLT_MARK 切分的一段录制内容（供 AI 提取使用）
#[derive(Debug, Clone)]
pub struct MarkSlice {
    pub mark_index: u32,
    pub label: Option<String>,
    /// ANSI stripped + truncated (≤2000 chars)
    pub content: String,
}

pub fn parse_cast(path: &str) -> Result<Vec<MarkSlice>> {
    let events = read_events(path)?;
    Ok(build_slices(&events))
}

// ── 内部 ──────────────────────────────────────────────────────────────────────

struct RawEvent {
    kind: String,
    data: String,
}

fn read_events(path: &str) -> Result<Vec<RawEvent>> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("Cannot read cast file: {}", path))?;
    let mut lines = raw.lines();
    lines.next().context("Cast file is empty")?; // skip header

    let mut events = Vec::new();
    for line in lines {
        let line = line.trim();
        if line.is_empty() { continue; }
        if let Ok(arr) = serde_json::from_str::<serde_json::Value>(line) {
            if let (Some(k), Some(d)) = (arr[1].as_str(), arr[2].as_str()) {
                events.push(RawEvent { kind: k.to_string(), data: d.to_string() });
            }
        }
    }
    Ok(events)
}

fn build_slices(events: &[RawEvent]) -> Vec<MarkSlice> {
    let mut slices = Vec::new();
    let mut current_mark_index = 0u32;
    let mut current_label: Option<String> = None;
    let mut buf = String::new();

    for ev in events {
        if ev.kind != "o" { continue; }
        let stripped = strip_ansi(&ev.data);

        if let Some(caps) = molt_mark_re().captures(&stripped) {
            if current_mark_index > 0 || !buf.trim().is_empty() {
                slices.push(MarkSlice {
                    mark_index: current_mark_index,
                    label: current_label.clone(),
                    content: truncate(&buf, 2000),
                });
            }
            current_mark_index = caps[1].parse().unwrap_or(0);
            current_label = caps.get(3)
                .map(|m| m.as_str().trim().to_string())
                .filter(|s| !s.is_empty());
            buf.clear();
        } else {
            buf.push_str(&stripped);
        }
    }

    if !buf.trim().is_empty() {
        slices.push(MarkSlice {
            mark_index: current_mark_index,
            label: current_label,
            content: truncate(&buf, 2000),
        });
    }
    slices
}

fn truncate(s: &str, max_chars: usize) -> String {
    let clean: String = s.chars()
        .filter(|c| c.is_ascii_graphic() || *c == ' ' || *c == '\n' || *c == '\t')
        .collect();
    if clean.chars().count() <= max_chars { clean }
    else {
        let t: String = clean.chars().take(max_chars).collect();
        format!("{}\n[... truncated]", t)
    }
}
