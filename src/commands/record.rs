use colored::Colorize;
use std::fs;
use std::process::{Command, Stdio};

use crate::session::{CAST_FILE, MARK_COUNT_FILE, PID_FILE};

pub fn run() {
    // 检查是否已有录制在运行
    if let Ok(pid_str) = fs::read_to_string(PID_FILE) {
        let pid = pid_str.trim();
        // 检查进程是否仍在运行
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

    // 清理旧的录制文件
    let _ = fs::remove_file(CAST_FILE);
    // 重置 mark 计数
    fs::write(MARK_COUNT_FILE, "0").expect("Cannot write mark count file");

    // 后台启动 asciinema rec
    // --stdin: 也录制标准输入（保留原始按键）
    // --quiet: 不输出额外信息
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
            print_recording_banner();
            let _ = update_title(0);
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            eprintln!("❌ `asciinema` not found. Install it first:");
            eprintln!("   pip install asciinema");
            eprintln!("   # or: brew install asciinema");
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("❌ Failed to start asciinema: {}", e);
            std::process::exit(1);
        }
    }
}

fn print_recording_banner() {
    let border = "─".repeat(50);
    println!();
    println!("  ╭{}╮", border);
    println!("  │  {}{}│", "🦞 Recording started".green().bold(), " ".repeat(28));
    println!("  │{}│", " ".repeat(52));
    println!("  │  {}{}│", format!("File  {}", CAST_FILE).cyan(), " ".repeat(50 - 6 - CAST_FILE.len()));
    println!("  │{}│", " ".repeat(52));
    println!("  │  {}{}│", "molt mark -l <label>   drop an anchor".dimmed(), " ".repeat(13));
    println!("  │  {}{}│", "molt stop              finish + extract".dimmed(), " ".repeat(12));
    println!("  ╰{}╯", border);
    println!();
}

/// 更新终端标题栏显示录制状态
pub fn update_title(mark_count: u32) {
    // OSC 0 是设置窗口标题的标准转义序列
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
