/// VTE terminal emulator — 2D virtual screen buffer
///
/// Implements enough VT100/ANSI to correctly render 95% of interactive shell
/// sessions: cursor movement, screen/line erase, scroll regions, character
/// insert/delete.  SGR (colours/bold) is silently ignored — we only care about
/// character positions for the AI-readable snapshot.
use vte::{Params, Perform};

pub struct VirtualScreen {
    pub cols: usize,
    pub rows: usize,
    cells: Vec<Vec<char>>,
    cursor_x: usize,
    cursor_y: usize,
    scroll_top: usize,
    scroll_bottom: usize,
}

impl VirtualScreen {
    pub fn new(cols: usize, rows: usize) -> Self {
        let scroll_bottom = rows.saturating_sub(1);
        Self {
            cols,
            rows,
            cells: vec![vec![' '; cols]; rows],
            cursor_x: 0,
            cursor_y: 0,
            scroll_top: 0,
            scroll_bottom,
        }
    }

    /// Render the visible screen as a compact string (empty lines stripped).
    pub fn snapshot(&self) -> String {
        self.cells
            .iter()
            .map(|row| row.iter().collect::<String>())
            .map(|s| s.trim_end().to_string())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    }

    // ── private scroll helpers ────────────────────────────────────────────────

    /// Remove one line from scroll_top, push blank at scroll_bottom.
    fn scroll_up_region(&mut self) {
        let top = self.scroll_top;
        let bottom = self.scroll_bottom;
        if top >= self.rows || bottom >= self.rows || top > bottom {
            return;
        }
        self.cells.remove(top);
        self.cells.insert(bottom, vec![' '; self.cols]);
        // keep total row count
        while self.cells.len() < self.rows {
            self.cells.push(vec![' '; self.cols]);
        }
        self.cells.truncate(self.rows);
    }

    /// Insert blank line at scroll_top, remove one from scroll_bottom.
    fn scroll_down_region(&mut self) {
        let top = self.scroll_top;
        let bottom = self.scroll_bottom;
        if top >= self.rows || bottom >= self.rows || top > bottom {
            return;
        }
        if bottom < self.cells.len() {
            self.cells.remove(bottom);
        } else if !self.cells.is_empty() {
            self.cells.pop();
        }
        self.cells.insert(top, vec![' '; self.cols]);
        while self.cells.len() < self.rows {
            self.cells.push(vec![' '; self.cols]);
        }
        self.cells.truncate(self.rows);
    }
}

// ── VTE Perform impl ──────────────────────────────────────────────────────────

impl Perform for VirtualScreen {
    fn print(&mut self, c: char) {
        // Line wrap
        if self.cursor_x >= self.cols {
            self.cursor_x = 0;
            self.cursor_y += 1;
        }
        // Scroll if past bottom of scroll region
        if self.cursor_y > self.scroll_bottom {
            self.scroll_up_region();
            self.cursor_y = self.scroll_bottom;
        }
        if self.cursor_y < self.rows && self.cursor_x < self.cols {
            self.cells[self.cursor_y][self.cursor_x] = c;
        }
        self.cursor_x += 1;
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            b'\r' => {
                self.cursor_x = 0;
            }
            b'\n' => {
                self.cursor_y += 1;
                if self.cursor_y > self.scroll_bottom {
                    self.scroll_up_region();
                    self.cursor_y = self.scroll_bottom;
                }
            }
            0x08 => {
                // backspace
                if self.cursor_x > 0 {
                    self.cursor_x -= 1;
                }
            }
            0x09 => {
                // horizontal tab — advance to next 8-column stop
                self.cursor_x = (self.cursor_x / 8 + 1) * 8;
                if self.cursor_x >= self.cols {
                    self.cursor_x = self.cols.saturating_sub(1);
                }
            }
            _ => {} // bell, SO/SI, etc. — ignored
        }
    }

    fn csi_dispatch(&mut self, params: &Params, _intermediates: &[u8], _ignore: bool, action: char) {
        // Collect params as u16; missing params default to 0
        let ps: Vec<u16> = params.iter().map(|p| p[0]).collect();
        let p0 = ps.first().copied().unwrap_or(0);
        let p1 = ps.get(1).copied().unwrap_or(0);

        match action {
            // ── cursor positioning ────────────────────────────────────────────
            'H' | 'f' => {
                // CUP / HVP: row;col (1-based, 0 treated as 1)
                self.cursor_y = (p0.max(1) as usize - 1).min(self.rows - 1);
                self.cursor_x = (p1.max(1) as usize - 1).min(self.cols - 1);
            }
            'A' => { self.cursor_y = self.cursor_y.saturating_sub(p0.max(1) as usize); }
            'B' => { self.cursor_y = (self.cursor_y + p0.max(1) as usize).min(self.rows - 1); }
            'C' => { self.cursor_x = (self.cursor_x + p0.max(1) as usize).min(self.cols - 1); }
            'D' => { self.cursor_x = self.cursor_x.saturating_sub(p0.max(1) as usize); }
            'G' => { self.cursor_x = (p0.max(1) as usize - 1).min(self.cols - 1); }
            'd' => { self.cursor_y = (p0.max(1) as usize - 1).min(self.rows - 1); }
            'E' => { // CNL — cursor next line
                self.cursor_x = 0;
                self.cursor_y = (self.cursor_y + p0.max(1) as usize).min(self.rows - 1);
            }
            'F' => { // CPL — cursor previous line
                self.cursor_x = 0;
                self.cursor_y = self.cursor_y.saturating_sub(p0.max(1) as usize);
            }

            // ── erase ─────────────────────────────────────────────────────────
            'J' => {
                match p0 {
                    0 => {
                        // from cursor to end of screen
                        if self.cursor_y < self.rows {
                            for x in self.cursor_x..self.cols {
                                self.cells[self.cursor_y][x] = ' ';
                            }
                        }
                        for y in (self.cursor_y + 1)..self.rows {
                            self.cells[y] = vec![' '; self.cols];
                        }
                    }
                    1 => {
                        // from start to cursor
                        for y in 0..self.cursor_y {
                            self.cells[y] = vec![' '; self.cols];
                        }
                        if self.cursor_y < self.rows {
                            for x in 0..=self.cursor_x.min(self.cols - 1) {
                                self.cells[self.cursor_y][x] = ' ';
                            }
                        }
                    }
                    2 | 3 => {
                        // entire screen
                        for row in &mut self.cells {
                            *row = vec![' '; self.cols];
                        }
                        self.cursor_x = 0;
                        self.cursor_y = 0;
                    }
                    _ => {}
                }
            }
            'K' => {
                if self.cursor_y >= self.rows { return; }
                match p0 {
                    0 => { for x in self.cursor_x..self.cols { self.cells[self.cursor_y][x] = ' '; } }
                    1 => { for x in 0..=self.cursor_x.min(self.cols - 1) { self.cells[self.cursor_y][x] = ' '; } }
                    2 => { self.cells[self.cursor_y] = vec![' '; self.cols]; }
                    _ => {}
                }
            }

            // ── insert/delete characters ──────────────────────────────────────
            '@' => {
                // ICH — insert character
                let n = p0.max(1) as usize;
                if self.cursor_y < self.rows {
                    let row = &mut self.cells[self.cursor_y];
                    let at = self.cursor_x.min(self.cols);
                    let end = (at + n).min(self.cols);
                    row.drain((self.cols - n.min(self.cols))..self.cols);
                    for _ in 0..n.min(self.cols - at) {
                        row.insert(at, ' ');
                    }
                    row.truncate(self.cols);
                    while row.len() < self.cols { row.push(' '); }
                }
            }
            'P' => {
                // DCH — delete character
                let n = p0.max(1) as usize;
                if self.cursor_y < self.rows {
                    let row = &mut self.cells[self.cursor_y];
                    let start = self.cursor_x.min(self.cols);
                    let count = n.min(self.cols - start);
                    row.drain(start..(start + count));
                    while row.len() < self.cols { row.push(' '); }
                }
            }

            // ── insert/delete lines ───────────────────────────────────────────
            'L' => { for _ in 0..p0.max(1) { self.scroll_down_region(); } }
            'M' => { for _ in 0..p0.max(1) { self.scroll_up_region(); } }

            // ── scroll ────────────────────────────────────────────────────────
            'S' => { for _ in 0..p0.max(1) { self.scroll_up_region(); } }
            'T' => { for _ in 0..p0.max(1) { self.scroll_down_region(); } }

            // ── scroll region ─────────────────────────────────────────────────
            'r' => {
                let top = (p0.max(1) as usize).saturating_sub(1);
                let bottom = if p1 == 0 { self.rows - 1 } else { (p1 as usize).saturating_sub(1) };
                if top < bottom && bottom < self.rows {
                    self.scroll_top = top;
                    self.scroll_bottom = bottom;
                }
            }

            // ── ignored ───────────────────────────────────────────────────────
            'm' | 'h' | 'l' | 'n' | 'c' | 'q' | 'X' | '`' => {}
            _ => {}
        }
    }

    fn esc_dispatch(&mut self, _intermediates: &[u8], _ignore: bool, byte: u8) {
        match byte {
            b'M' => {
                // RI — reverse index (scroll down at top margin)
                if self.cursor_y == self.scroll_top {
                    self.scroll_down_region();
                } else if self.cursor_y > 0 {
                    self.cursor_y -= 1;
                }
            }
            b'7' | b'8' => {} // save/restore cursor — simplified, no-op
            _ => {}
        }
    }

    fn hook(&mut self, _params: &Params, _intermediates: &[u8], _ignore: bool, _action: char) {}
    fn put(&mut self, _byte: u8) {}
    fn unhook(&mut self) {}
    fn osc_dispatch(&mut self, _params: &[&[u8]], _bell_terminated: bool) {}
}
