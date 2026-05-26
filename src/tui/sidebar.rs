use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

use crate::fs::tree::{EntryKind, FileTree};

pub struct SidebarView<'a> {
    pub file_tree: Option<&'a FileTree>,
    pub claude_entries: &'a [String],
    pub scroll_offset: usize,
}

impl Widget for SidebarView<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(area);

        self.render_file_tree(chunks[0], buf);
        self.render_claude_log(chunks[1], buf);
    }
}

impl SidebarView<'_> {
    fn render_file_tree(&self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .title(" Files ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray));

        let inner = block.inner(area);
        block.render(area, buf);

        if let Some(tree) = self.file_tree {
            let entries = tree.display_entries();
            let visible = inner.height as usize;
            let start = self.scroll_offset.min(entries.len().saturating_sub(visible));

            for (i, (display, _idx, kind, selected)) in
                entries.iter().skip(start).take(visible).enumerate()
            {
                let y = inner.y + i as u16;
                let color = match kind {
                    EntryKind::Directory => Color::Cyan,
                    EntryKind::File => Color::Gray,
                };
                let style = if *selected {
                    Style::default()
                        .fg(Color::White)
                        .bg(Color::Rgb(50, 50, 70))
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(color)
                };

                for (j, ch) in display.chars().enumerate() {
                    let x = inner.x + j as u16;
                    if x < inner.x + inner.width {
                        buf[(x, y)].set_char(ch).set_style(style);
                    }
                }
            }
        } else {
            let line = Line::from(Span::styled(
                "(no project open)",
                Style::default().fg(Color::DarkGray),
            ));
            Paragraph::new(vec![line]).render(inner, buf);
        }
    }

    fn render_claude_log(&self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .title(" Claude ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray));

        let lines: Vec<Line> = self
            .claude_entries
            .iter()
            .map(|entry| {
                let style = if entry.contains("Edit") {
                    Style::default().fg(Color::Green)
                } else if entry.contains("Write") {
                    Style::default().fg(Color::Green)
                } else if entry.contains("Bash") {
                    Style::default().fg(Color::Yellow)
                } else if entry.contains("Read") {
                    Style::default().fg(Color::Blue)
                } else if entry.contains("💭") {
                    Style::default().fg(Color::Rgb(100, 100, 120))
                } else {
                    Style::default().fg(Color::Gray)
                };
                Line::from(Span::styled(entry.as_str(), style))
            })
            .collect();

        let content = Paragraph::new(lines).block(block);
        content.render(area, buf);
    }
}
