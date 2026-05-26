use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::Widget;

pub struct StatusBar<'a> {
    pub filename: &'a str,
    pub line: usize,
    pub col: usize,
    pub dirty: bool,
    pub total_lines: usize,
    pub claude_status: &'a str,
}

impl Widget for StatusBar<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 {
            return;
        }

        let bg = Style::default().bg(Color::DarkGray).fg(Color::White);
        for x in area.x..area.x + area.width {
            buf[(x, area.y)].set_style(bg);
        }

        let dirty_marker = if self.dirty { " [+]" } else { "" };
        let left = format!(" {}{}", self.filename, dirty_marker);
        let right = format!(
            "{} | {}:{} / {} lines  ",
            self.claude_status,
            self.line + 1,
            self.col + 1,
            self.total_lines
        );

        for (i, ch) in left.chars().enumerate() {
            let x = area.x + i as u16;
            if x < area.x + area.width {
                buf[(x, area.y)].set_char(ch).set_style(bg);
            }
        }

        let right_start = area.x + area.width.saturating_sub(right.len() as u16);
        for (i, ch) in right.chars().enumerate() {
            let x = right_start + i as u16;
            if x < area.x + area.width && x >= area.x {
                let style = if self.claude_status.contains("Claude") {
                    Style::default().bg(Color::DarkGray).fg(Color::Green)
                } else {
                    bg
                };
                buf[(x, area.y)].set_char(ch).set_style(style);
            }
        }
    }
}
