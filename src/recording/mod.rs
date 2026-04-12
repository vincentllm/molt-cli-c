pub mod cast;
pub mod stats;

pub use cast::{parse_cast, MarkSlice};
pub use stats::{parse_cast_stats, CastStats, fmt_duration};

use regex::Regex;
use std::sync::OnceLock;

static ANSI_RE: OnceLock<Regex> = OnceLock::new();
static MOLT_MARK_RE: OnceLock<Regex> = OnceLock::new();

pub(super) fn ansi_re() -> &'static Regex {
    ANSI_RE.get_or_init(|| {
        Regex::new(r"\x1b(?:[@-Z\\-_]|\[[0-?]*[ -/]*[@-~])").unwrap()
    })
}

pub(super) fn molt_mark_re() -> &'static Regex {
    MOLT_MARK_RE.get_or_init(|| {
        Regex::new(r"MOLT_MARK\s+(\d+)\s+(\S+)(?:\s+(.+))?").unwrap()
    })
}

pub(super) fn strip_ansi(s: &str) -> String {
    ansi_re().replace_all(s, "").to_string()
}
