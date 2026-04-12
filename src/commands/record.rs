use colored::Colorize;
use std::fs;
use std::process::{Command, Stdio};

use crate::session::{CAST_FILE, MARK_COUNT_FILE, PID_FILE};

pub fn run() {
    // Reject double-start
    if let Ok(pid_str) = fs::read_to_string(PID_FILE) {
        let pid = pid_str.trim();
        let alive = Command::new("kill")
            .args(["-0", pid])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if alive {
            eprintln!("🦞 Already recording (pid {}). Run `molt stop` first.", pid);
            std::process::exit(1);
        }
    }

    // Clean slate
    let _ = fs::remove_file(CAST_FILE);
    fs::write(MARK_COUNT_FILE, "0").expect("Cannot reset mark count");

    print_recording_banner();

    #[cfg(unix)]
    start_native();

    #[cfg(not(unix))]
    start_asciinema();
}

// ── native PTY (Linux / WSL) ──────────────────────────────────────────────────

#[cfg(unix)]
fn start_native() {
    use crate::recording::pty_session;
    // pty_session::start() blocks until the shell exits.
    if let Err(e) = pty_session::start() {
        // Restore a newline after potential raw-mode artifacts
        println!();
        eprintln!("{} Recording error: {}", "❌".red(), e);
        std::process::exit(1);
    }
    // Print a clean newline after the PTY session ends
    println!();
    println!("{} Recording stopped. Run {} to extract pipeline.",
        "🦞".green(), "`molt stop`".cyan());
}

// ── asciinema subprocess fallback (Windows) ───────────────────────────────────

#[cfg(not(unix))]
fn start_asciinema() {
    let child = Command::new("asciinema")
        .args(["rec", "--quiet", "--stdin", CAST_FILE])
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn();

    match child {
        Ok(child) => {
            let pid = child.id();
            fs::write(PID_FILE, pid.to_string()).expect("Cannot write PID file");
            let _ = update_title(0);
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            eprintln!("❌ `asciinema` not found. On Windows use WSL, or install:");
            eprintln!("   pip install asciinema");
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("❌ Failed to start asciinema: {}", e);
            std::process::exit(1);
        }
    }
}

// ── shared helpers ────────────────────────────────────────────────────────────

fn print_recording_banner() {
    #[cfg(unix)]
    let extra = "  Shell launched inside PTY — type normally.  ";
    #[cfg(not(unix))]
    let extra = "  Recording via asciinema — type normally.    ";

    let border = "─".repeat(50);
    println!();
    println!("  ╭{}╮", border);
    println!("  │  {}{}│", "🦞 Recording started".green().bold(), " ".repeat(28));
    println!("  │{}│", " ".repeat(52));
    println!("  │  {}{}│", format!("File  {}", CAST_FILE).cyan(), " ".repeat(50 - 6 - CAST_FILE.len()));
    println!("  │{}│", " ".repeat(52));
    println!("  │  {}{}│", extra.dimmed(), " ".repeat(52_usize.saturating_sub(extra.len())));
    println!("  │  {}{}│", "molt mark -l <label>   drop an anchor".dimmed(), " ".repeat(13));
    println!("  │  {}{}│", "molt stop              finish + extract".dimmed(), " ".repeat(12));
    println!("  ╰{}╯", border);
    println!();
}

/// Update the terminal title bar with current mark count.
pub fn update_title(mark_count: u32) {
    print!("\x1b]0;🦞 recording... ({} marks)\x07", mark_count);
}

pub fn read_mark_count() -> u32 {
    fs::read_to_string(MARK_COUNT_FILE)
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0)
}

pub fn write_mark_count(count: u32) {
    let _ = fs::write(MARK_COUNT_FILE, count.to_string());
}
