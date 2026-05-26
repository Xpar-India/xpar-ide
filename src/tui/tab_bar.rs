use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::Widget;

pub struct TabEntry {
    pub name: String,
    pub dirty: bool,
    pub claude_modified: bool,
}

pub struct TabBar<'a> {
    pub tabs: &'a [TabEntry],
    pub active: usize,
}

impl Widget for TabBar<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 {
            return;
        }

        let bg = Style::default().bg(Color::Rgb(30, 30, 46)).fg(Color::DarkGray);
        for x in area.x..area.x + area.width {
            buf[(x, area.y)].set_style(bg);
        }

        let mut x_offset = area.x;
        for (i, tab) in self.tabs.iter().enumerate() {
            let is_active = i == self.active;

            let indicator = if tab.claude_modified && tab.dirty {
                "[C+] "
            } else if tab.claude_modified {
                "[C] "
            } else if tab.dirty {
                "[+] "
            } else {
                ""
            };

            let label = format!(" {}{} ", indicator, tab.name);
            let style = if is_active {
                Style::default()
                    .fg(Color::White)
                    .bg(Color::Rgb(50, 50, 70))
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
                    .fg(Color::Gray)
                    .bg(Color::Rgb(30, 30, 46))
            };

            for ch in label.chars() {
                if x_offset < area.x + area.width {
                    buf[(x_offset, area.y)].set_char(ch).set_style(style);
                    x_offset += 1;
                }
            }

            if x_offset < area.x + area.width {
                buf[(x_offset, area.y)]
                    .set_char('│')
                    .set_style(Style::default().fg(Color::DarkGray));
                x_offset += 1;
            }
        }
    }
}
