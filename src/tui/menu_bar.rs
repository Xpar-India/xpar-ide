use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::Widget;

pub struct MenuItem {
    pub label: &'static str,
    pub x_start: u16,
    pub x_end: u16,
}

pub struct MenuBar {
    pub items: Vec<MenuItem>,
}

impl MenuBar {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
        }
    }

    pub fn hit_test(&self, x: u16) -> Option<&'static str> {
        for item in &self.items {
            if x >= item.x_start && x < item.x_end {
                return Some(item.label);
            }
        }
        None
    }
}

impl Widget for &mut MenuBar {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 {
            return;
        }

        let bg = Style::default().bg(Color::Rgb(40, 40, 60)).fg(Color::Gray);
        for x in area.x..area.x + area.width {
            buf[(x, area.y)].set_style(bg);
        }

        let labels = [
            " xpar.IDE ",
            " File ",
            " Edit ",
            " View ",
            " Exit ",
        ];

        self.items.clear();
        let mut x_offset = area.x;

        for (i, label) in labels.iter().enumerate() {
            let style = if i == 0 {
                Style::default()
                    .bg(Color::Rgb(80, 80, 120))
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else if *label == " Exit " {
                Style::default()
                    .bg(Color::Rgb(40, 40, 60))
                    .fg(Color::Rgb(255, 100, 100))
            } else {
                Style::default()
                    .bg(Color::Rgb(40, 40, 60))
                    .fg(Color::Gray)
            };

            let x_start = x_offset;
            for ch in label.chars() {
                if x_offset < area.x + area.width {
                    buf[(x_offset, area.y)].set_char(ch).set_style(style);
                    x_offset += 1;
                }
            }

            self.items.push(MenuItem {
                label: labels[i],
                x_start,
                x_end: x_offset,
            });
        }
    }
}
