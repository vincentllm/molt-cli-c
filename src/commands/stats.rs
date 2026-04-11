use colored::Colorize;
use comfy_table::{Attribute, Cell, Color, ContentArrangement, Table, presets};

use crate::cast_parser::{fmt_duration, parse_cast_stats, CastStats, CAST_FILE};

pub fn run(path: Option<&str>) {
    let cast_path = path.unwrap_or(CAST_FILE);

    let stats = match parse_cast_stats(cast_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{} {}", "❌ 无法读取录制文件:".red(), e);
            eprintln!("   先运行 `molt record` 开始录制");
            std::process::exit(1);
        }
    };

    print_header();
    print_summary(&stats);
    print_timeline(&stats);
    print_segments_table(&stats);
    print_commands(&stats);
    println!();
}

// ── 各块渲染 ──────────────────────────────────────────────────────────────────

fn print_header() {
    println!();
    println!("{}", "  🦞  Molt Recording Stats".cyan().bold());
    println!("  {}", "─".repeat(54).dimmed());
}

fn print_summary(s: &CastStats) {
    let size_kb = s.file_size_bytes as f64 / 1024.0;
    let mark_count = s.segments.iter().filter(|seg| seg.mark_index > 0).count();

    println!();
    println!("  {}  {}  {}", "📁".dimmed(), s.path.cyan(), format!("({:.1} KB)", size_kb).dimmed());
    println!("  {}  Duration   {}", "⏱ ".dimmed(),  s.duration_display().yellow().bold());
    println!(
        "  {}  Events     {}  {}",
        "📊".dimmed(),
        s.total_events().to_string().yellow().bold(),
        format!("(output {} / input {})", s.total_output_events, s.total_input_events).dimmed()
    );
    println!("  {}  Marks      {}", "🔖".dimmed(), mark_count.to_string().yellow().bold());
}

fn print_timeline(s: &CastStats) {
    let total = s.total_duration_secs;
    if total < 0.1 { return; }

    let width: usize = 54;
    println!();
    println!("  {} {}", "Timeline".white().bold(), "─".repeat(46).dimmed());
    println!();

    // 顶部时间轴标注
    let end_label = fmt_duration(total);
    println!(
        "  {:<width$}{}",
        format!("0:00"),
        end_label,
        width = width.saturating_sub(end_label.len())
    );

    // 画轴 + mark 位置
    let marks: Vec<_> = s.segments.iter().filter(|seg| seg.mark_index > 0).collect();
    let mut axis: Vec<char> = std::iter::repeat('─').take(width).collect();

    // 标出 mark 位置
    for seg in &marks {
        let pos = ((seg.start_secs / total) * width as f64) as usize;
        let pos = pos.min(width - 1);
        axis[pos] = '●';
    }
    // 起点和终点
    axis[0] = '●';
    if let Some(last) = axis.last_mut() { *last = '●'; }

    let axis_str: String = axis.iter().collect();
    println!("  {}", colorize_axis(&axis_str));

    // 下方 mark 标注（多行，每行显示不同 mark 的标签，避免重叠）
    let mark_labels: Vec<(usize, String)> = marks.iter().map(|seg| {
        let pos = ((seg.start_secs / total) * width as f64) as usize;
        let label = format!(
            "M{}{}",
            seg.mark_index,
            seg.label.as_ref().map(|l| format!(":{}", l)).unwrap_or_default()
        );
        (pos.min(width - 1), label)
    }).collect();

    // 第一行：mark 名称
    if !mark_labels.is_empty() {
        let mut line = " ".repeat(width + 2);
        for (pos, label) in &mark_labels {
            let start = pos + 2;
            let end = (start + label.len()).min(line.len());
            if start < line.len() {
                line.replace_range(start..end, &label[..end - start]);
            }
        }
        println!("{}", line.cyan());
    }

    // 第二行：时间标注
    let time_labels: Vec<(usize, String)> = marks.iter().map(|seg| {
        let pos = ((seg.start_secs / total) * width as f64) as usize;
        (pos.min(width - 1), fmt_duration(seg.start_secs))
    }).collect();

    if !time_labels.is_empty() {
        let mut line = " ".repeat(width + 2);
        for (pos, label) in &time_labels {
            let start = pos + 2;
            let end = (start + label.len()).min(line.len());
            if start < line.len() {
                line.replace_range(start..end, &label[..end - start]);
            }
        }
        println!("{}", line.dimmed());
    }
}

fn colorize_axis(s: &str) -> String {
    s.chars().map(|c| match c {
        '●' => "●".yellow().to_string(),
        '─' => "─".dimmed().to_string(),
        other => other.to_string(),
    }).collect()
}

fn print_segments_table(s: &CastStats) {
    if s.segments.is_empty() { return; }

    println!();
    println!("  {} {}", "Segments".white().bold(), "─".repeat(46).dimmed());
    println!();

    let mut table = Table::new();
    table.load_preset(presets::UTF8_BORDERS_ONLY)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            Cell::new("Segment").add_attribute(Attribute::Bold).fg(Color::Cyan),
            Cell::new("Label").add_attribute(Attribute::Bold).fg(Color::Cyan),
            Cell::new("Start").add_attribute(Attribute::Bold).fg(Color::Cyan),
            Cell::new("Duration").add_attribute(Attribute::Bold).fg(Color::Cyan),
            Cell::new("Events").add_attribute(Attribute::Bold).fg(Color::Cyan),
        ]);

    for seg in &s.segments {
        table.add_row(vec![
            Cell::new(seg.name_display()).fg(Color::Yellow),
            Cell::new(seg.label_display()),
            Cell::new(fmt_duration(seg.start_secs)).fg(Color::DarkGrey),
            Cell::new(seg.duration_display()),
            Cell::new(seg.event_count.to_string()).fg(Color::DarkGrey),
        ]);
    }

    // 缩进 2 格
    for line in table.to_string().lines() {
        println!("  {}", line);
    }
}

fn print_commands(s: &CastStats) {
    if s.top_commands.is_empty() { return; }

    println!();
    println!("  {} {}", "Commands".white().bold(), "─".repeat(46).dimmed());
    println!();

    let max_count = s.top_commands.first().map(|(_, c)| *c).unwrap_or(1);
    let bar_width = 24usize;

    for (cmd, count) in &s.top_commands {
        let filled = ((*count as f64 / max_count as f64) * bar_width as f64) as usize;
        let bar: String = "█".repeat(filled) + &"░".repeat(bar_width - filled);
        println!(
            "  {:<12} {} {}",
            cmd.cyan(),
            bar.green(),
            count.to_string().yellow()
        );
    }
}
