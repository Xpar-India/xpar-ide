use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Tabs, Widget};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BottomTab {
    Terminal,
    Build,
    ClaudeLog,
}

impl BottomTab {
    pub fn titles() -> Vec<&'static str> {
        vec!["Terminal", "Build", "Claude Log"]
    }

    pub fn index(&self) -> usize {
        match self {
            BottomTab::Terminal => 0,
            BottomTab::Build => 1,
            BottomTab::ClaudeLog => 2,
        }
    }

    pub fn next(&self) -> Self {
        match self {
            BottomTab::Terminal => BottomTab::Build,
            BottomTab::Build => BottomTab::ClaudeLog,
            BottomTab::ClaudeLog => BottomTab::Terminal,
        }
    }
}

pub struct BottomPanel {
    pub active_tab: BottomTab,
    pub terminal_lines: Vec<String>,
    pub build_lines: Vec<String>,
    pub claude_log_lines: Vec<String>,
}

impl Default for BottomPanel {
    fn default() -> Self {
        Self {
            active_tab: BottomTab::Terminal,
            terminal_lines: vec!["$ ".to_string()],
            build_lines: vec!["No build output yet.".to_string()],
            claude_log_lines: vec!["No Claude session active.".to_string()],
        }
    }
}

impl Widget for &BottomPanel {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height < 3 {
            return;
        }

        let block = Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(Color::DarkGray));

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height < 2 {
            return;
        }

        let tab_area = Rect::new(inner.x, inner.y, inner.width, 1);
        let content_area = Rect::new(inner.x, inner.y + 1, inner.width, inner.height - 1);

        let titles: Vec<Line> = BottomTab::titles()
            .iter()
            .map(|t| Line::from(*t))
            .collect();

        let tabs = Tabs::new(titles)
            .select(self.active_tab.index())
            .style(Style::default().fg(Color::DarkGray))
            .highlight_style(
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )
            .divider("│");
        tabs.render(tab_area, buf);

        let lines = match self.active_tab {
            BottomTab::Terminal => &self.terminal_lines,
            BottomTab::Build => &self.build_lines,
            BottomTab::ClaudeLog => &self.claude_log_lines,
        };

        let visible_lines = content_area.height as usize;
        let start = lines.len().saturating_sub(visible_lines);
        let display_lines: Vec<Line> = lines[start..]
            .iter()
            .map(|l| {
                let color = match self.active_tab {
                    BottomTab::Terminal => Color::Gray,
                    BottomTab::Build => Color::Gray,
                    BottomTab::ClaudeLog => {
                        if l.starts_with("▸ Edit") {
                            Color::Green
                        } else if l.starts_with("▸ Bash") {
                            Color::Yellow
                        } else if l.starts_with("▸ Read") {
                            Color::Blue
                        } else {
                            Color::DarkGray
                        }
                    }
                };
                Line::from(Span::styled(l.as_str(), Style::default().fg(color)))
            })
            .collect();

        let content = Paragraph::new(display_lines);
        content.render(content_area, buf);
    }
}
