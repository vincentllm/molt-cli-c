use anyhow::{Context, Result};
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::sync::OnceLock;

pub const CAST_FILE: &str = "/tmp/molt_session.cast";

static ANSI_RE: OnceLock<Regex> = OnceLock::new();
static MOLT_MARK_RE: OnceLock<Regex> = OnceLock::new();
/// 匹配 bash/zsh/fish 常见 prompt 后的第一个词作为命令
/// 支持: $ cmd, % cmd, ❯ cmd, ➜ cmd, # cmd
static CMD_RE: OnceLock<Regex> = OnceLock::new();

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

fn cmd_re() -> &'static Regex {
    CMD_RE.get_or_init(|| {
        // 匹配常见 shell prompt 字符后面的命令名
        // bash: "$ cmd", zsh: "% cmd" / "❯ cmd" / "➜ cmd", root: "# cmd"
        Regex::new(r"(?:[$%#❯➜]\s+|>\s+)([a-zA-Z][a-zA-Z0-9_/-]*)").unwrap()
    })
}

// ── 公共数据类型 ──────────────────────────────────────────────────────────────

/// 按 MOLT_MARK 切分的一段录制内容（供 AI 提取使用）
#[derive(Debug, Clone)]
pub struct MarkSlice {
    pub mark_index: u32,
    pub _timestamp: String,
    pub label: Option<String>,
    /// ANSI stripped + truncated (≤2000 chars)
    pub content: String,
}

/// 完整录制统计信息（供 molt stats 展示）
#[derive(Debug)]
pub struct CastStats {
    pub path: String,
    pub file_size_bytes: u64,
    pub total_duration_secs: f64,
    pub total_output_events: usize,
    pub total_input_events: usize,
    pub segments: Vec<SegmentStats>,
    /// (command_name, count)，按 count 降序，取前 10
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

// ── 解析函数 ──────────────────────────────────────────────────────────────────

/// 解析 .cast → MarkSlice 列表（供 molt stop AI 提取使用）
pub fn parse_cast(path: &str) -> Result<Vec<MarkSlice>> {
    let events = read_events(path)?;
    let slices = build_slices(&events);
    Ok(slices)
}

/// 解析 .cast → 完整统计信息（供 molt stats 使用）
pub fn parse_cast_stats(path: &str) -> Result<CastStats> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("Cannot read cast file: {}", path))?;
    let file_size_bytes = fs::metadata(path).map(|m| m.len()).unwrap_or(0);

    let mut lines = raw.lines();
    lines.next().context("Cast file is empty")?; // skip header

    // 解析所有事件，保留时间戳
    let mut all_events: Vec<(f64, String, String)> = Vec::new(); // (time, kind, data)
    for line in lines {
        let line = line.trim();
        if line.is_empty() { continue; }
        if let Ok(arr) = serde_json::from_str::<serde_json::Value>(line) {
            if let (Some(t), Some(k), Some(d)) = (
                arr[0].as_f64(),
                arr[1].as_str(),
                arr[2].as_str(),
            ) {
                all_events.push((t, k.to_string(), d.to_string()));
            }
        }
    }

    let total_duration_secs = all_events.last().map(|(t, _, _)| *t).unwrap_or(0.0);
    let total_output_events = all_events.iter().filter(|(_, k, _)| k == "o").count();
    let total_input_events = all_events.iter().filter(|(_, k, _)| k == "i").count();

    // 按 MOLT_MARK 分段，记录时间和事件数
    let segments = build_segments(&all_events, total_duration_secs);

    // 命令检测：从所有 output 事件提取
    let all_output: String = all_events.iter()
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

// ── 内部函数 ──────────────────────────────────────────────────────────────────

struct RawEvent {
    _time: f64,
    kind: String,
    data: String,
}

fn read_events(path: &str) -> Result<Vec<RawEvent>> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("Cannot read cast file: {}", path))?;
    let mut lines = raw.lines();
    lines.next().context("Cast file is empty")?;

    let mut events = Vec::new();
    for line in lines {
        let line = line.trim();
        if line.is_empty() { continue; }
        if let Ok(arr) = serde_json::from_str::<serde_json::Value>(line) {
            if let (Some(t), Some(k), Some(d)) = (
                arr[0].as_f64(), arr[1].as_str(), arr[2].as_str(),
            ) {
                events.push(RawEvent { _time: t, kind: k.to_string(), data: d.to_string() });
            }
        }
    }
    Ok(events)
}

fn build_slices(events: &[RawEvent]) -> Vec<MarkSlice> {
    let mut slices = Vec::new();
    let mut current_mark_index = 0u32;
    let mut current_ts = "start".to_string();
    let mut current_label: Option<String> = None;
    let mut buf = String::new();

    for ev in events {
        if ev.kind != "o" { continue; }
        let stripped = strip_ansi(&ev.data);

        if let Some(caps) = molt_mark_re().captures(&stripped) {
            if current_mark_index > 0 || !buf.trim().is_empty() {
                slices.push(MarkSlice {
                    mark_index: current_mark_index,
                    _timestamp: current_ts.clone(),
                    label: current_label.clone(),
                    content: truncate(&buf, 2000),
                });
            }
            current_mark_index = caps[1].parse().unwrap_or(0);
            current_ts = caps[2].to_string();
            current_label = caps.get(3).map(|m| m.as_str().trim().to_string()).filter(|s| !s.is_empty());
            buf.clear();
        } else {
            buf.push_str(&stripped);
        }
    }

    if !buf.trim().is_empty() {
        slices.push(MarkSlice {
            mark_index: current_mark_index,
            _timestamp: current_ts,
            label: current_label,
            content: truncate(&buf, 2000),
        });
    }
    slices
}

fn build_segments(events: &[(f64, String, String)], total_duration: f64) -> Vec<SegmentStats> {
    let mut segments: Vec<SegmentStats> = Vec::new();
    let mut current_mark: u32 = 0;
    let mut current_label: Option<String> = None;
    let mut seg_start: f64 = 0.0;
    let mut seg_events: usize = 0;

    for (time, kind, data) in events {
        if kind != "o" { continue; }
        let stripped = strip_ansi(data);

        if let Some(caps) = molt_mark_re().captures(&stripped) {
            // 保存当前段
            segments.push(SegmentStats {
                mark_index: current_mark,
                label: current_label.clone(),
                start_secs: seg_start,
                end_secs: *time,
                event_count: seg_events,
            });
            current_mark = caps[1].parse().unwrap_or(0);
            current_label = caps.get(3).map(|m| m.as_str().trim().to_string()).filter(|s| !s.is_empty());
            seg_start = *time;
            seg_events = 0;
        } else {
            seg_events += 1;
        }
    }

    // 最后一段
    segments.push(SegmentStats {
        mark_index: current_mark,
        label: current_label,
        start_secs: seg_start,
        end_secs: total_duration,
        event_count: seg_events,
    });

    segments
}

fn detect_commands(text: &str) -> Vec<(String, usize)> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for caps in cmd_re().captures_iter(text) {
        let cmd = caps[1].to_lowercase();
        // 过滤掉太长的或数字开头（误匹配）
        if cmd.len() <= 20 && !cmd.starts_with(|c: char| c.is_ascii_digit()) {
            *counts.entry(cmd).or_insert(0) += 1;
        }
    }
    let mut sorted: Vec<_> = counts.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));
    sorted.into_iter().take(10).collect()
}

pub fn strip_ansi(s: &str) -> String {
    ansi_re().replace_all(s, "").to_string()
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
