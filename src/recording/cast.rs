use anyhow::{Context, Result};
use serde_json;
use std::fs;

use super::{molt_mark_re, strip_ansi};
use crate::session::SNAPSHOTS_FILE;

/// 按 MOLT_MARK 切分的一段录制内容（供 AI 提取使用）
#[derive(Debug, Clone)]
pub struct MarkSlice {
    pub mark_index: u32,
    pub label: Option<String>,
    /// ANSI-stripped raw terminal output, truncated to ≤2000 chars.
    pub content: String,
    /// Clean VTE screen snapshot taken at this mark (from native PTY recorder).
    /// None when recording was done via asciinema subprocess.
    pub screen_snapshot: Option<String>,
}

pub fn parse_cast(path: &str) -> Result<Vec<MarkSlice>> {
    let events = read_events(path)?;
    let mut slices = build_slices(&events);
    merge_snapshots(&mut slices);
    Ok(slices)
}

/// Load per-mark VTE screen snapshots (written by native PTY recorder) and
/// attach them to the matching MarkSlice entries.  No-op if the file is absent.
fn merge_snapshots(slices: &mut [MarkSlice]) {
    let raw = match fs::read_to_string(SNAPSHOTS_FILE) {
        Ok(r) => r,
        Err(_) => return,
    };
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() { continue; }
        if let Ok(obj) = serde_json::from_str::<serde_json::Value>(line) {
            let idx = obj["mark_index"].as_u64().unwrap_or(0) as u32;
            if let Some(screen) = obj["screen"].as_str() {
                if let Some(slice) = slices.iter_mut().find(|s| s.mark_index == idx) {
                    slice.screen_snapshot = Some(screen.to_string());
                }
            }
        }
    }
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
                    screen_snapshot: None,
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
            screen_snapshot: None,
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;

    /// Write cast content to a tempfile and return its path.
    fn write_cast(content: &str) -> tempfile::NamedTempFile {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(content.as_bytes()).unwrap();
        f
    }

    fn cast_header() -> &'static str {
        r#"{"version":2,"width":220,"height":50,"timestamp":1700000000}"#
    }

    fn cast_event(t: f64, data: &str) -> String {
        format!("[{}, \"o\", {}]\n", t, serde_json::to_string(data).unwrap())
    }

    // ── error paths ────────────────────────────────────────────────────────────

    #[test]
    fn empty_file_returns_error() {
        let f = write_cast("");
        let err = parse_cast(f.path().to_str().unwrap()).unwrap_err();
        assert!(err.to_string().contains("empty"), "got: {err}");
    }

    #[test]
    fn header_only_no_marks_returns_empty_slices() {
        let f = write_cast(cast_header());
        let slices = parse_cast(f.path().to_str().unwrap()).unwrap();
        assert!(slices.is_empty());
    }

    #[test]
    fn missing_file_returns_error() {
        let err = parse_cast("/tmp/__molt_does_not_exist_xyz.cast").unwrap_err();
        assert!(err.to_string().contains("Cannot read cast file"));
    }

    // ── happy paths ────────────────────────────────────────────────────────────

    #[test]
    fn events_before_first_mark_become_slice_0() {
        let mut cast = cast_header().to_string();
        cast.push('\n');
        cast.push_str(&cast_event(0.1, "hello\n"));
        cast.push_str(&cast_event(0.2, "world\n"));

        let f = write_cast(&cast);
        let slices = parse_cast(f.path().to_str().unwrap()).unwrap();
        assert_eq!(slices.len(), 1);
        assert_eq!(slices[0].mark_index, 0);
        assert!(slices[0].content.contains("hello"));
    }

    #[test]
    fn single_mark_splits_into_two_slices() {
        let mut cast = cast_header().to_string();
        cast.push('\n');
        cast.push_str(&cast_event(0.1, "before\n"));
        cast.push_str(&cast_event(0.5, "MOLT_MARK 1 2024-01-01T00:00:00Z setup\n"));
        cast.push_str(&cast_event(1.0, "after\n"));

        let f = write_cast(&cast);
        let slices = parse_cast(f.path().to_str().unwrap()).unwrap();
        assert_eq!(slices.len(), 2);
        assert_eq!(slices[0].mark_index, 0);
        assert!(slices[0].content.contains("before"));
        assert_eq!(slices[1].mark_index, 1);
        assert_eq!(slices[1].label.as_deref(), Some("setup"));
        assert!(slices[1].content.contains("after"));
    }

    #[test]
    fn two_marks_produce_three_slices() {
        let mut cast = cast_header().to_string();
        cast.push('\n');
        cast.push_str(&cast_event(0.1, "preamble\n"));
        cast.push_str(&cast_event(0.5, "MOLT_MARK 1 2024-01-01T00:00:00Z build\n"));
        cast.push_str(&cast_event(1.0, "building\n"));
        cast.push_str(&cast_event(1.5, "MOLT_MARK 2 2024-01-01T00:00:01Z test\n"));
        cast.push_str(&cast_event(2.0, "testing\n"));

        let f = write_cast(&cast);
        let slices = parse_cast(f.path().to_str().unwrap()).unwrap();
        assert_eq!(slices.len(), 3);
        assert_eq!(slices[1].label.as_deref(), Some("build"));
        assert_eq!(slices[2].label.as_deref(), Some("test"));
    }

    #[test]
    fn mark_without_label_has_none_label() {
        let mut cast = cast_header().to_string();
        cast.push('\n');
        cast.push_str(&cast_event(0.1, "MOLT_MARK 1 2024-01-01T00:00:00Z\n"));
        cast.push_str(&cast_event(0.2, "body\n"));

        let f = write_cast(&cast);
        let slices = parse_cast(f.path().to_str().unwrap()).unwrap();
        let marked = slices.iter().find(|s| s.mark_index == 1).unwrap();
        assert!(marked.label.is_none());
    }

    #[test]
    fn ansi_codes_are_stripped_from_content() {
        let mut cast = cast_header().to_string();
        cast.push('\n');
        cast.push_str(&cast_event(0.1, "\x1b[32mgreen text\x1b[0m\n"));

        let f = write_cast(&cast);
        let slices = parse_cast(f.path().to_str().unwrap()).unwrap();
        assert_eq!(slices.len(), 1);
        assert!(slices[0].content.contains("green text"));
        assert!(!slices[0].content.contains("\x1b"));
    }

    #[test]
    fn content_truncated_at_2000_chars() {
        let long_line = "x".repeat(3000);
        let mut cast = cast_header().to_string();
        cast.push('\n');
        cast.push_str(&cast_event(0.1, &format!("{}\n", long_line)));

        let f = write_cast(&cast);
        let slices = parse_cast(f.path().to_str().unwrap()).unwrap();
        assert!(slices[0].content.len() < 2100); // 2000 + "[... truncated]" overhead
        assert!(slices[0].content.contains("[... truncated]"));
    }
}
