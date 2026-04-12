/// Writes asciinema v2 (.cast) files
///
/// Format:
///   Line 1 — JSON header
///   Line N — [elapsed_secs, "o"|"i", data]  (one event per line)
use anyhow::Result;
use serde_json::json;
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::time::Instant;

pub struct CastWriter {
    writer: BufWriter<File>,
    start: Instant,
}

impl CastWriter {
    /// Create (or truncate) a .cast file and write the v2 header.
    pub fn new(path: &str, cols: u16, rows: u16) -> Result<Self> {
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)?;
        let mut writer = BufWriter::new(file);
        let header = json!({
            "version": 2,
            "width": cols,
            "height": rows,
            "timestamp": chrono::Utc::now().timestamp(),
            "env": {
                "TERM": std::env::var("TERM").unwrap_or_else(|_| "xterm-256color".into()),
                "SHELL": std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into()),
            }
        });
        writeln!(writer, "{}", header)?;
        Ok(Self { writer, start: Instant::now() })
    }

    /// Append an output event (type "o") with the current elapsed time.
    pub fn write_output(&mut self, data: &[u8]) -> Result<()> {
        let t = self.start.elapsed().as_secs_f64();
        let text = String::from_utf8_lossy(data);
        let event = json!([t, "o", text.as_ref()]);
        writeln!(self.writer, "{}", event)?;
        Ok(())
    }

    pub fn flush(&mut self) -> Result<()> {
        Ok(self.writer.flush()?)
    }
}
