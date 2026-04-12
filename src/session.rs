/// 录制会话运行时状态文件路径
/// 集中管理，方便未来支持多路录制或自定义路径
pub const CAST_FILE: &str = "/tmp/molt_session.cast";
pub const PID_FILE: &str = "/tmp/molt_session.pid";
pub const MARK_COUNT_FILE: &str = "/tmp/molt_mark_count";
/// Per-mark VTE screen snapshots written by the native PTY recorder.
/// Format: one JSON object per line — { mark_index, timestamp, label?, screen }.
pub const SNAPSHOTS_FILE: &str = "/tmp/molt_snapshots.jsonl";
