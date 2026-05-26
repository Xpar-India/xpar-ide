use ratatui::layout::{Constraint, Direction, Layout, Rect};

pub struct PanelState {
    pub sidebar_visible: bool,
    pub bottom_panel_visible: bool,
    pub sidebar_width: u16,
    pub bottom_panel_height: u16,
}

impl Default for PanelState {
    fn default() -> Self {
        Self {
            sidebar_visible: true,
            bottom_panel_visible: true,
            sidebar_width: 25,
            bottom_panel_height: 10,
        }
    }
}

impl PanelState {
    pub fn toggle_sidebar(&mut self) {
        self.sidebar_visible = !self.sidebar_visible;
    }

    pub fn toggle_bottom_panel(&mut self) {
        self.bottom_panel_visible = !self.bottom_panel_visible;
    }
}

#[derive(Clone)]
pub struct AppLayout {
    pub menu_bar: Rect,
    pub sidebar: Option<Rect>,
    pub tab_bar: Rect,
    pub editor: Rect,
    pub bottom_panel: Option<Rect>,
    pub statusbar: Rect,
}

pub fn compute_layout(area: Rect, panel_state: &PanelState) -> AppLayout {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints(if panel_state.bottom_panel_visible {
            vec![
                Constraint::Length(1), // menu bar
                Constraint::Length(1), // tab bar
                Constraint::Min(5),    // editor + sidebar
                Constraint::Length(panel_state.bottom_panel_height),
                Constraint::Length(1), // statusbar
            ]
        } else {
            vec![
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Min(5),
                Constraint::Length(0),
                Constraint::Length(1),
            ]
        })
        .split(area);

    let menu_bar_area = vertical[0];
    let tab_bar_area = vertical[1];
    let main_area = vertical[2];
    let bottom_area = vertical[3];
    let statusbar_area = vertical[4];

    let (sidebar_area, editor_area) = if panel_state.sidebar_visible {
        let horizontal = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(panel_state.sidebar_width),
                Constraint::Min(20),
            ])
            .split(main_area);
        (Some(horizontal[0]), horizontal[1])
    } else {
        (None, main_area)
    };

    let bottom_panel_area = if panel_state.bottom_panel_visible && bottom_area.height > 0 {
        Some(bottom_area)
    } else {
        None
    };

    AppLayout {
        menu_bar: menu_bar_area,
        sidebar: sidebar_area,
        tab_bar: tab_bar_area,
        editor: editor_area,
        bottom_panel: bottom_panel_area,
        statusbar: statusbar_area,
    }
}
