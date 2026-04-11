use anyhow::{Context, Result};
use regex::Regex;
use std::fs;
use std::sync::OnceLock;

pub const CAST_FILE: &str = "/tmp/molt_session.cast";

static ANSI_RE: OnceLock<Regex> = OnceLock::new();
static MOLT_MARK_RE: OnceLock<Regex> = OnceLock::new();

fn ansi_re() -> &'static Regex {
    ANSI_RE.get_or_init(|| {
        Regex::new(r"\x1b(?:[@-Z\\-_]|\[[0-?]*[ -/]*[@-~])").unwrap()
    })
}

fn molt_mark_re() -> &'static Regex {
    MOLT_MARK_RE.get_or_init(|| {
        Regex::new(r"MOLT_MARK\s+(\d+)\s+(\S+)(?:\s+(.+))?").unwrap()
    })
}

/// 一个 MOLT_MARK 分段：index 0 表示 mark 之前的内容（序言）
#[derive(Debug, Clone)]
pub struct MarkSlice {
    pub mark_index: u32,
    pub _timestamp: String,
    pub label: Option<String>,
    /// ANSI stripped + truncated terminal output between marks
    pub content: String,
}

/// asciinema v2 事件行
#[derive(Debug)]
struct CastEvent {
    _time: f64,
    kind: String, // "o", "i", "r", "e"
    data: String,
}

pub fn parse_cast(path: &str) -> Result<Vec<MarkSlice>> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("Cannot read cast file: {}", path))?;

    let mut lines = raw.lines();

    // 第一行是 header JSON，跳过
    lines.next().context("Cast file is empty")?;

    let mut events: Vec<CastEvent> = Vec::new();
    for line in lines {
        let line = line.trim();
        if line.is_empty() { continue; }
        if let Ok(arr) = serde_json::from_str::<serde_json::Value>(line) {
            if let (Some(t), Some(k), Some(d)) = (
                arr[0].as_f64(),
                arr[1].as_str(),
                arr[2].as_str(),
            ) {
                events.push(CastEvent {
                    _time: t,
                    kind: k.to_string(),
                    data: d.to_string(),
                });
            }
        }
    }

    // 把所有 "o" 事件按 MOLT_MARK 分段
    let mut slices: Vec<MarkSlice> = Vec::new();
    let mut current_mark_index = 0u32;
    let mut current_ts = "start".to_string();
    let mut current_label: Option<String> = None;
    let mut buf = String::new();

    for ev in &events {
        if ev.kind != "o" { continue; }

        let stripped = strip_ansi(&ev.data);

        // 检测 MOLT_MARK
        if let Some(caps) = molt_mark_re().captures(&stripped) {
            // 把之前的内容作为一段
            if current_mark_index > 0 || !buf.trim().is_empty() {
                slices.push(MarkSlice {
                    mark_index: current_mark_index,
                    _timestamp: current_ts.clone(),
                    label: current_label.clone(),
                    content: truncate(&buf, 2000),
                });
            }
            // 开新段
            current_mark_index = caps[1].parse().unwrap_or(0);
            current_ts = caps[2].to_string(); // stored in _timestamp
            current_label = caps.get(3).map(|m| m.as_str().trim().to_string()).filter(|s| !s.is_empty());
            buf.clear();
        } else {
            buf.push_str(&stripped);
        }
    }

    // 最后一段
    if !buf.trim().is_empty() {
        slices.push(MarkSlice {
            mark_index: current_mark_index,
            _timestamp: current_ts,
            label: current_label,
            content: truncate(&buf, 2000),
        });
    }

    Ok(slices)
}

fn strip_ansi(s: &str) -> String {
    ansi_re().replace_all(s, "").to_string()
}

fn truncate(s: &str, max_chars: usize) -> String {
    let clean: String = s.chars()
        .filter(|c| c.is_ascii_graphic() || *c == ' ' || *c == '\n' || *c == '\t')
        .collect();
    if clean.chars().count() <= max_chars {
        clean
    } else {
        let truncated: String = clean.chars().take(max_chars).collect();
        format!("{}\n[... truncated]", truncated)
    }
}
