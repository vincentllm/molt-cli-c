/// Native PTY recording session
///
/// Replaces the external `asciinema` subprocess on Unix.
///
/// Lifecycle:
///   1. Create PTY pair (portable-pty), spawn user's $SHELL in the slave.
///   2. Write RECORDER's own PID to PID_FILE (not the shell's PID).
///      `molt stop` sends SIGTERM to this process; a signal handler then
///      sends SIGHUP to the child shell to trigger a clean PTY hangup.
///      (Interactive bash/zsh ignore SIGTERM; they respond to SIGHUP.)
///   3. Set controlling terminal to raw mode (no buffering, no echo).
///   4. Two threads:
///      – reader: PTY master → stdout + .cast file + VTE parser
///      – writer: stdin → PTY master (keyboard forwarding)
///   5. Reader detects MOLT_MARK in the output and saves a VTE screen
///      snapshot to SNAPSHOTS_FILE (one JSON line per mark).
///   6. When the shell exits (PTY master returns EIO/EOF), restore terminal
///      mode, flush the cast file, clean up PID_FILE.
///
/// SIGWINCH is forwarded to the PTY so the shell resizes correctly.

#[cfg(unix)]
use anyhow::{Context, Result};
#[cfg(unix)]
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
#[cfg(unix)]
use serde_json::json;
#[cfg(unix)]
use std::io::{Read, Write};
#[cfg(unix)]
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
#[cfg(unix)]
use std::sync::Arc;
#[cfg(unix)]
use vte::Parser as VteParser;

#[cfg(unix)]
use crate::recording::{molt_mark_re, strip_ansi};
#[cfg(unix)]
use crate::session::{CAST_FILE, MARK_COUNT_FILE, PID_FILE, SNAPSHOTS_FILE};
#[cfg(unix)]
use super::cast_writer::CastWriter;
#[cfg(unix)]
use super::virtual_screen::VirtualScreen;

// ── global signal state ───────────────────────────────────────────────────────

#[cfg(unix)]
static RESIZE_FLAG: AtomicBool = AtomicBool::new(false);

/// Set by SIGTERM handler → main loop sends SIGHUP to child and exits.
#[cfg(unix)]
static STOP_FLAG: AtomicBool = AtomicBool::new(false);

/// Child shell PID, stored globally so the async-signal-safe handler can reach it.
#[cfg(unix)]
static CHILD_PID: AtomicU32 = AtomicU32::new(0);

#[cfg(unix)]
extern "C" fn on_sigwinch(_: libc::c_int) {
    RESIZE_FLAG.store(true, Ordering::Relaxed);
}

/// On SIGTERM: tell the main loop to end the session gracefully.
#[cfg(unix)]
extern "C" fn on_sigterm(_: libc::c_int) {
    STOP_FLAG.store(true, Ordering::Relaxed);
}

// ── public entry point ────────────────────────────────────────────────────────

#[cfg(unix)]
pub fn start() -> Result<()> {
    // Reset globals (in case start() is ever called more than once)
    STOP_FLAG.store(false, Ordering::Relaxed);
    RESIZE_FLAG.store(false, Ordering::Relaxed);

    let (cols, rows) = terminal_size();

    // ── PTY + child shell ─────────────────────────────────────────────────────
    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })
        .context("Failed to open PTY pair")?;

    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
    let mut cmd = CommandBuilder::new(&shell);
    cmd.env("TERM", std::env::var("TERM").unwrap_or_else(|_| "xterm-256color".to_string()));

    let mut child = pair.slave.spawn_command(cmd).context("Failed to spawn shell")?;
    drop(pair.slave); // close slave fd in parent process

    let child_pid = child.process_id().unwrap_or(0);
    CHILD_PID.store(child_pid, Ordering::Relaxed);

    // Write OWN PID — `molt stop` will send SIGTERM here, NOT to the shell
    std::fs::write(PID_FILE, std::process::id().to_string())?;
    std::fs::write(MARK_COUNT_FILE, "0")?;
    let _ = std::fs::remove_file(SNAPSHOTS_FILE); // fresh per session

    // ── Cast writer + VTE state ───────────────────────────────────────────────
    let mut cast_writer = CastWriter::new(CAST_FILE, cols, rows)?;
    let mut screen = VirtualScreen::new(cols as usize, rows as usize);
    let mut vte_parser = VteParser::new();

    // ── Terminal raw mode ─────────────────────────────────────────────────────
    let saved_termios = unsafe { set_raw_mode() };

    // ── Signal handlers ───────────────────────────────────────────────────────
    unsafe {
        libc::signal(libc::SIGWINCH, on_sigwinch as libc::sighandler_t);
        libc::signal(libc::SIGTERM, on_sigterm as libc::sighandler_t);
    }

    // ── Keyboard forwarding thread (stdin → PTY master) ───────────────────────
    let done = Arc::new(AtomicBool::new(false));
    let done_clone = done.clone();
    let mut master_write = pair.master.take_writer().context("Cannot get PTY writer")?;
    let fwd_thread = std::thread::spawn(move || {
        let mut buf = [0u8; 256];
        loop {
            if done_clone.load(Ordering::Relaxed) { break; }
            match std::io::stdin().lock().read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => { if master_write.write_all(&buf[..n]).is_err() { break; } }
            }
        }
    });

    // ── Main read loop: PTY master → stdout + cast + VTE ─────────────────────
    let mut reader = pair.master.try_clone_reader().context("Cannot get PTY reader")?;
    let stdout = std::io::stdout();
    let mut buf = [0u8; 4096];
    let mut line_buf = String::new();

    loop {
        // SIGTERM received → send SIGHUP to child shell and exit
        if STOP_FLAG.load(Ordering::Relaxed) {
            let cpid = CHILD_PID.load(Ordering::Relaxed);
            if cpid > 0 {
                unsafe { libc::kill(cpid as libc::pid_t, libc::SIGHUP); }
            }
            // Small drain window so the shell's exit output makes it into .cast
            std::thread::sleep(std::time::Duration::from_millis(150));
            break;
        }

        // SIGWINCH → update PTY size and reset VirtualScreen
        if RESIZE_FLAG.swap(false, Ordering::Relaxed) {
            let (nc, nr) = terminal_size();
            pair.master.resize(PtySize { rows: nr, cols: nc, pixel_width: 0, pixel_height: 0 }).ok();
            screen = VirtualScreen::new(nc as usize, nr as usize);
        }

        let n = match reader.read(&mut buf) {
            Ok(0) => break,
            Err(ref e) if is_pty_eof(e) => break,
            Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => break,
            Ok(n) => n,
        };

        let data = &buf[..n];

        // 1 — echo to user's terminal
        { let mut out = stdout.lock(); out.write_all(data).ok(); out.flush().ok(); }

        // 2 — append to .cast file
        cast_writer.write_output(data).ok();

        // 3 — feed VTE parser → update virtual screen
        for &byte in data { vte_parser.advance(&mut screen, byte); }

        // 4 — scan for MOLT_MARK (accumulate until newline)
        for ch in String::from_utf8_lossy(data).chars() {
            if ch == '\n' {
                let stripped = strip_ansi(&line_buf);
                if let Some(caps) = molt_mark_re().captures(&stripped) {
                    let idx: u32 = caps[1].parse().unwrap_or(0);
                    let label = caps.get(3)
                        .map(|m| m.as_str().trim().to_string())
                        .filter(|s| !s.is_empty());
                    save_snapshot(idx, label.as_deref(), &screen.snapshot());
                }
                line_buf.clear();
            } else {
                line_buf.push(ch);
            }
        }

        // 5 — natural shell exit (try_wait is non-blocking)
        if matches!(child.try_wait(), Ok(Some(_))) {
            // Drain any final PTY output
            std::thread::sleep(std::time::Duration::from_millis(50));
            if let Ok(n2) = reader.read(&mut buf) {
                if n2 > 0 {
                    let d2 = &buf[..n2];
                    { let mut out = stdout.lock(); out.write_all(d2).ok(); out.flush().ok(); }
                    cast_writer.write_output(d2).ok();
                    for &byte in d2 { vte_parser.advance(&mut screen, byte); }
                }
            }
            break;
        }
    }

    // ── Cleanup ───────────────────────────────────────────────────────────────
    done.store(true, Ordering::Relaxed);
    cast_writer.flush().ok();

    unsafe {
        restore_raw_mode(saved_termios);
        libc::signal(libc::SIGWINCH, libc::SIG_DFL);
        libc::signal(libc::SIGTERM, libc::SIG_DFL);
    }

    let _ = std::fs::remove_file(PID_FILE);
    drop(fwd_thread);

    Ok(())
}

// ── helpers ───────────────────────────────────────────────────────────────────

#[cfg(unix)]
fn save_snapshot(mark_idx: u32, label: Option<&str>, snapshot: &str) {
    use std::fs::OpenOptions;
    use std::io::Write as _;
    let entry = json!({
        "mark_index": mark_idx,
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "label": label,
        "screen": snapshot,
    });
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(SNAPSHOTS_FILE) {
        let _ = writeln!(f, "{}", entry);
    }
}

#[cfg(unix)]
fn is_pty_eof(e: &std::io::Error) -> bool {
    use std::io::ErrorKind;
    matches!(e.kind(), ErrorKind::BrokenPipe | ErrorKind::ConnectionReset)
        || e.raw_os_error() == Some(libc::EIO)
        || e.raw_os_error() == Some(libc::ENXIO)
}

#[cfg(unix)]
pub fn terminal_size() -> (u16, u16) {
    unsafe {
        let mut ws: libc::winsize = std::mem::zeroed();
        if libc::ioctl(libc::STDOUT_FILENO, libc::TIOCGWINSZ, &mut ws) == 0
            && ws.ws_col > 0 && ws.ws_row > 0
        {
            return (ws.ws_col, ws.ws_row);
        }
    }
    (220, 50)
}

#[cfg(unix)]
unsafe fn set_raw_mode() -> libc::termios {
    let mut t: libc::termios = std::mem::zeroed();
    libc::tcgetattr(libc::STDIN_FILENO, &mut t);
    let saved = t;
    libc::cfmakeraw(&mut t);
    libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &t);
    saved
}

#[cfg(unix)]
unsafe fn restore_raw_mode(saved: libc::termios) {
    libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &saved);
}
