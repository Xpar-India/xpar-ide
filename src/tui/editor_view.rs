use ratatui::buffer::Buffer as RatBuffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::Widget;

use crate::core::buffer::Buffer;
use crate::core::selections::Selections;
use crate::integrations::claude::DiffMarker;

pub struct EditorView<'a> {
    buffer: &'a Buffer,
    selections: &'a Selections,
    scroll_offset: usize,
    diff_markers: Option<&'a dyn Fn(usize) -> Option<DiffMarker>>,
}

impl<'a> EditorView<'a> {
    pub fn new(buffer: &'a Buffer, selections: &'a Selections, scroll_offset: usize) -> Self {
        Self {
            buffer,
            selections,
            scroll_offset,
            diff_markers: None,
        }
    }

    pub fn with_diff_markers(mut self, f: &'a dyn Fn(usize) -> Option<DiffMarker>) -> Self {
        self.diff_markers = Some(f);
        self
    }
}

const GUTTER_WIDTH: u16 = 6;

impl Widget for EditorView<'_> {
    fn render(self, area: Rect, buf: &mut RatBuffer) {
        if area.width < GUTTER_WIDTH + 2 || area.height == 0 {
            return;
        }

        let visible_lines = area.height as usize;

        for row in 0..visible_lines {
            let line_idx = self.scroll_offset + row;
            let y = area.y + row as u16;

            if line_idx < self.buffer.line_count() {
                let diff_marker = self
                    .diff_markers
                    .and_then(|f| f(line_idx));

                let (marker_char, marker_color) = match diff_marker {
                    Some(DiffMarker::Added) => ('+', Color::Green),
                    Some(DiffMarker::Removed) => ('-', Color::Red),
                    Some(DiffMarker::Modified) => ('~', Color::Blue),
                    None => (' ', Color::DarkGray),
                };

                buf[(area.x, y)]
                    .set_char(marker_char)
                    .set_style(Style::default().fg(marker_color));

                let line_num = format!("{:>4} ", line_idx + 1);
                let gutter_style = Style::default().fg(Color::DarkGray);
                for (i, ch) in line_num.chars().enumerate() {
                    let x = area.x + 1 + i as u16;
                    if x < area.x + GUTTER_WIDTH {
                        buf[(x, y)].set_char(ch).set_style(gutter_style);
                    }
                }

                let line_bg = match diff_marker {
                    Some(DiffMarker::Added) => Some(Color::Rgb(20, 40, 20)),
                    Some(DiffMarker::Modified) => Some(Color::Rgb(20, 20, 45)),
                    _ => None,
                };

                if let Some(line) = self.buffer.line(line_idx) {
                    let line_str: String = line.chars().take_while(|c| *c != '\n').collect();
                    for (i, ch) in line_str.chars().enumerate() {
                        let x = area.x + GUTTER_WIDTH + i as u16;
                        if x < area.x + area.width {
                            let style = if let Some(bg) = line_bg {
                                Style::default().bg(bg)
                            } else {
                                Style::default()
                            };
                            buf[(x, y)].set_char(ch).set_style(style);
                        }
                    }

                    if let Some(bg) = line_bg {
                        let text_end = area.x + GUTTER_WIDTH + line_str.len() as u16;
                        for x in text_end..area.x + area.width {
                            buf[(x, y)].set_style(Style::default().bg(bg));
                        }
                    }
                }
            } else {
                let tilde_style = Style::default().fg(Color::DarkGray);
                buf[(area.x + GUTTER_WIDTH - 2, y)]
                    .set_char('~')
                    .set_style(tilde_style);
            }
        }

        let primary = self.selections.primary();
        let cursor_line = primary.head.line;
        let cursor_col = primary.head.col;
        if cursor_line >= self.scroll_offset
            && cursor_line < self.scroll_offset + visible_lines
        {
            let screen_row = (cursor_line - self.scroll_offset) as u16;
            let screen_col = GUTTER_WIDTH + cursor_col as u16;
            let x = area.x + screen_col;
            let y = area.y + screen_row;
            if x < area.x + area.width {
                buf[(x, y)].set_style(
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::White),
                );
            }
        }
    }
}
