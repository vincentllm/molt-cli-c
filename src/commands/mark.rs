use super::record::{read_mark_count, update_title, write_mark_count};
use chrono::Utc;

/// MOLT_MARK 前缀是后续 LLM 解析的语义锚点标识
const MARK_PREFIX: &str = "MOLT_MARK";

pub fn run(label: Option<String>) {
    let count = read_mark_count() + 1;
    write_mark_count(count);

    let timestamp = Utc::now().format("%Y-%m-%dT%H:%M:%SZ");
    let label_str = label.as_deref().unwrap_or("");

    // 输出到终端 — asciinema 会把这行录制进 .cast 文件
    // 格式: MOLT_MARK <index> <timestamp> [label]
    // LLM 后续解析时以这个前缀为分段边界
    println!(
        "{} {} {} {}",
        MARK_PREFIX, count, timestamp, label_str
    );

    // 同步更新标题栏
    update_title(count);

    eprintln!("🦞 Mark {} dropped", count);
}
