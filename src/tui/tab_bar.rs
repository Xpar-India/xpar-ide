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

pub struct TabHitZone {
    pub tab_index: usize,
    pub x_start: u16,
    pub x_end: u16,
    pub close_x: u16,
}

pub struct TabBarState {
    pub zones: Vec<TabHitZone>,
}

impl TabBarState {
    pub fn new() -> Self {
        Self { zones: Vec::new() }
    }

    pub fn hit_test(&self, x: u16) -> Option<TabHitResult> {
        for zone in &self.zones {
            if x == zone.close_x {
                return Some(TabHitResult::Close(zone.tab_index));
            }
            if x >= zone.x_start && x < zone.x_end {
                return Some(TabHitResult::Select(zone.tab_index));
            }
        }
        None
    }
}

pub enum TabHitResult {
    Select(usize),
    Close(usize),
}

pub struct TabBarWidget<'a> {
    pub bar: &'a TabBar<'a>,
    pub state: &'a mut TabBarState,
}

impl Widget for TabBarWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let (tab_bar, state) = (self.bar, self.state);
        if area.height == 0 {
            return;
        }

        state.zones.clear();

        let bg = Style::default().bg(Color::Rgb(30, 30, 46)).fg(Color::DarkGray);
        for x in area.x..area.x + area.width {
            buf[(x, area.y)].set_style(bg);
        }

        let mut x_offset = area.x;
        for (i, tab) in tab_bar.tabs.iter().enumerate() {
            let is_active = i == tab_bar.active;

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

            let x_start = x_offset;

            for ch in label.chars() {
                if x_offset < area.x + area.width {
                    buf[(x_offset, area.y)].set_char(ch).set_style(style);
                    x_offset += 1;
                }
            }

            let close_x = x_offset;
            if x_offset < area.x + area.width {
                let close_style = Style::default()
                    .fg(Color::Rgb(180, 80, 80))
                    .bg(if is_active {
                        Color::Rgb(50, 50, 70)
                    } else {
                        Color::Rgb(30, 30, 46)
                    });
                buf[(x_offset, area.y)].set_char('x').set_style(close_style);
                x_offset += 1;
            }

            if x_offset < area.x + area.width {
                buf[(x_offset, area.y)]
                    .set_char('│')
                    .set_style(Style::default().fg(Color::DarkGray));
                x_offset += 1;
            }

            state.zones.push(TabHitZone {
                tab_index: i,
                x_start,
                x_end: close_x,
                close_x,
            });
        }
    }
}
