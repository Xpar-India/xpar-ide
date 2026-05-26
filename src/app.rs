use std::io;
use std::path::PathBuf;

use crossterm::event::{self, Event, KeyCode, MouseButton, MouseEventKind};
use ratatui::layout::Rect;
use ratatui::DefaultTerminal;

use crate::core::buffer::Buffer;
use crate::core::selections::{Position, Selection, Selections};
use crate::fs::loader;
use crate::fs::tree::{EntryKind, FileTree};
use crate::integrations::claude::ClaudeSession;
use crate::integrations::terminal::EmbeddedTerminal;
use crate::integrations::treesitter::Highlighter;
use crate::tui::bottom_panel::BottomPanel;
use crate::tui::editor_view::EditorView;
use crate::tui::input::{map_key, Action};
use crate::tui::layout::{compute_layout, AppLayout, PanelState};
use crate::tui::menu_bar::MenuBar;
use crate::tui::sidebar::SidebarView;
use crate::tui::statusbar::StatusBar;
use crate::tui::tab_bar::{TabBar, TabBarState, TabBarWidget, TabEntry, TabHitResult};

const GUTTER_WIDTH: u16 = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusTarget {
    Editor,
    Sidebar,
    BottomPanel,
}

pub struct App {
    pub buffers: Vec<Buffer>,
    pub active_buffer: usize,
    pub selections: Vec<Selections>,
    pub scroll_offsets: Vec<usize>,
    pub panel_state: PanelState,
    pub file_tree: Option<FileTree>,
    pub claude_session: ClaudeSession,
    pub bottom_panel: BottomPanel,
    pub menu_bar: MenuBar,
    pub focus: FocusTarget,
    pub show_cheatsheet: bool,
    pub running: bool,
    pub sidebar_scroll: usize,
    pub highlighter: Highlighter,
    highlight_dirty: bool,
    pub tab_bar_state: TabBarState,
    pub terminal: Option<EmbeddedTerminal>,
    last_layout: Option<AppLayout>,
}

impl App {
    pub fn new(project_dir: Option<PathBuf>, claude_session: Option<ClaudeSession>) -> Self {
        let file_tree = project_dir.as_ref().map(|dir| FileTree::new(dir.clone()));

        let buf = Buffer::from_str("// Welcome to xpar.IDE\n// Open a file or select one from the sidebar.\n");
        let sels = Selections::new(Position::zero());

        let claude_session = claude_session.unwrap_or_else(ClaudeSession::new);

        Self {
            buffers: vec![buf],
            active_buffer: 0,
            selections: vec![sels],
            scroll_offsets: vec![0],
            panel_state: PanelState::default(),
            file_tree,
            claude_session,
            bottom_panel: BottomPanel::default(),
            menu_bar: MenuBar::new(),
            focus: FocusTarget::Editor,
            show_cheatsheet: false,
            running: true,
            sidebar_scroll: 0,
            highlighter: Highlighter::new(),
            highlight_dirty: true,
            tab_bar_state: TabBarState::new(),
            terminal: EmbeddedTerminal::new(80, 10).ok(),
            last_layout: None,
        }
    }

    pub fn buffer(&self) -> &Buffer {
        &self.buffers[self.active_buffer]
    }

    pub fn buffer_mut(&mut self) -> &mut Buffer {
        &mut self.buffers[self.active_buffer]
    }

    pub fn selections(&self) -> &Selections {
        &self.selections[self.active_buffer]
    }

    pub fn selections_mut(&mut self) -> &mut Selections {
        &mut self.selections[self.active_buffer]
    }

    pub fn scroll_offset(&self) -> usize {
        self.scroll_offsets[self.active_buffer]
    }

    pub fn run(mut self, mut terminal: DefaultTerminal) -> io::Result<()> {
        while self.running {
            let _ = self.claude_session.poll();
            self.sync_claude_to_ui();

            terminal.draw(|frame| self.draw(frame))?;

            if event::poll(std::time::Duration::from_millis(100))? {
                match event::read()? {
                    Event::Key(key) => {
                        if self.focus == FocusTarget::BottomPanel
                            && self.bottom_panel.active_tab == crate::tui::bottom_panel::BottomTab::Terminal
                        {
                            self.handle_terminal_key(key);
                        } else if self.focus == FocusTarget::Sidebar {
                            self.handle_sidebar_key(key.code);
                        } else {
                            let action = map_key(key);
                            self.handle_action(action);
                        }
                    }
                    Event::Mouse(mouse) => {
                        self.handle_mouse(mouse);
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }

    fn sync_claude_to_ui(&mut self) {
        self.bottom_panel.claude_log_lines = self.claude_session.log_lines();
        if let Some(term) = &self.terminal {
            self.bottom_panel.terminal_lines = term.screen_lines();
            if !term.is_alive() {
                self.bottom_panel.terminal_lines.push("Shell exited. Press Enter to restart.".to_string());
            }
        }
    }

    fn refresh_highlights(&mut self) {
        if self.highlight_dirty {
            if let Some(path) = self.buffer().path().cloned() {
                self.highlighter.set_language_for_file(&path);
            }
            let source = self.buffer().contents();
            self.highlighter.parse(&source);
            self.highlight_dirty = false;
        }
    }

    fn draw(&mut self, frame: &mut ratatui::Frame) {
        self.refresh_highlights();

        let area = frame.area();
        let layout = compute_layout(area, &self.panel_state);

        frame.render_widget(&mut self.menu_bar, layout.menu_bar);

        let tabs: Vec<TabEntry> = self
            .buffers
            .iter()
            .map(|b| {
                let name = b
                    .path()
                    .and_then(|p| p.file_name())
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "[scratch]".to_string());
                let claude_modified = b
                    .path()
                    .map(|p| self.claude_session.is_file_claude_modified(p))
                    .unwrap_or(false);
                TabEntry {
                    name,
                    dirty: b.is_dirty(),
                    claude_modified,
                }
            })
            .collect();

        let tab_bar = TabBar {
            tabs: &tabs,
            active: self.active_buffer,
        };
        frame.render_widget(TabBarWidget { bar: &tab_bar, state: &mut self.tab_bar_state }, layout.tab_bar);

        if let Some(sidebar_area) = layout.sidebar {
            let claude_entries = self.claude_session.sidebar_entries();
            let sidebar_view = SidebarView {
                file_tree: self.file_tree.as_ref(),
                claude_entries: &claude_entries,
                scroll_offset: self.sidebar_scroll,
            };
            frame.render_widget(sidebar_view, sidebar_area);
        }

        let current_path = self.buffer().path().cloned();
        let diff_fn = |line: usize| -> Option<crate::integrations::claude::DiffMarker> {
            current_path
                .as_ref()
                .and_then(|p| self.claude_session.get_line_diff_marker(p, line))
        };
        let editor_view = EditorView::new(
            self.buffer(),
            self.selections(),
            self.scroll_offset(),
        )
        .with_diff_markers(&diff_fn)
        .with_highlighter(&self.highlighter);
        frame.render_widget(editor_view, layout.editor);

        if let Some(bottom_area) = layout.bottom_panel {
            frame.render_widget(&self.bottom_panel, bottom_area);
        }

        let filename = self
            .buffer()
            .path()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "[scratch]".to_string());

        let primary = self.selections().primary();
        let claude_status = self.claude_session.status_text();
        let statusbar = StatusBar {
            filename: &filename,
            line: primary.head.line,
            col: primary.head.col,
            dirty: self.buffer().is_dirty(),
            total_lines: self.buffer().line_count(),
            claude_status: &claude_status,
        };
        frame.render_widget(statusbar, layout.statusbar);

        self.last_layout = Some(layout);

        if self.show_cheatsheet {
            self.draw_cheatsheet(frame, area);
        }
    }

    fn draw_cheatsheet(&self, frame: &mut ratatui::Frame, area: Rect) {
        use ratatui::style::{Color, Modifier, Style};
        use ratatui::text::{Line, Span};
        use ratatui::widgets::{Block, Borders, Clear, Paragraph};

        let width = 52u16.min(area.width.saturating_sub(4));
        let height = 24u16.min(area.height.saturating_sub(4));
        let x = (area.width.saturating_sub(width)) / 2;
        let y = (area.height.saturating_sub(height)) / 2;
        let popup_area = Rect::new(x, y, width, height);

        frame.render_widget(Clear, popup_area);

        let entries = [
            ("Ctrl+S", "Save"),
            ("Ctrl+Q", "Quit"),
            ("Ctrl+Z / Ctrl+Y", "Undo / Redo"),
            ("Ctrl+B", "Toggle sidebar"),
            ("Ctrl+`", "Toggle bottom panel"),
            ("Ctrl+J", "Focus terminal"),
            ("Ctrl+L", "Focus Claude log"),
            ("Ctrl+E", "Focus sidebar"),
            ("Ctrl+Tab", "Next file tab"),
            ("Ctrl+←/→", "Word jump"),
            ("Tab", "Insert indent / cycle tabs"),
            ("Ctrl+?/F1", "This cheatsheet"),
            ("Mouse click", "Position cursor / select"),
            ("Arrows", "Move cursor / navigate tree"),
            ("Enter", "New line / open file"),
            ("Home/End", "Line start/end"),
            ("PgUp/PgDn", "Scroll page"),
            ("Backspace/Del", "Delete"),
            ("Esc", "Return to editor"),
            ("", ""),
            ("Click Exit", "Quit (menu bar)"),
        ];

        let lines: Vec<Line> = entries
            .iter()
            .filter(|(k, _)| !k.is_empty())
            .map(|(key, desc)| {
                Line::from(vec![
                    Span::styled(
                        format!("{:<18}", key),
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(*desc, Style::default().fg(Color::Gray)),
                ])
            })
            .collect();

        let block = Block::default()
            .title(" Keybindings — Ctrl+? to dismiss ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));

        let paragraph = Paragraph::new(lines).block(block);
        frame.render_widget(paragraph, popup_area);
    }

    fn handle_mouse(&mut self, mouse: crossterm::event::MouseEvent) {
        let Some(layout) = &self.last_layout else {
            return;
        };

        let x = mouse.column;
        let y = mouse.row;

        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if self.show_cheatsheet {
                    self.show_cheatsheet = false;
                    return;
                }

                if rect_contains(layout.menu_bar, x, y) {
                    self.handle_menu_click(x);
                    return;
                }

                if rect_contains(layout.tab_bar, x, y) {
                    self.handle_tab_click(x);
                    return;
                }

                if let Some(sidebar) = layout.sidebar {
                    if rect_contains(sidebar, x, y) {
                        self.handle_sidebar_click(x, y, sidebar);
                        return;
                    }
                }

                if rect_contains(layout.editor, x, y) {
                    self.handle_editor_click(x, y, layout.editor);
                    return;
                }

                if let Some(bottom) = layout.bottom_panel {
                    if rect_contains(bottom, x, y) {
                        self.focus = FocusTarget::BottomPanel;
                        return;
                    }
                }
            }
            MouseEventKind::ScrollUp => {
                if let Some(sidebar) = layout.sidebar {
                    if rect_contains(sidebar, x, y) {
                        self.sidebar_scroll = self.sidebar_scroll.saturating_sub(3);
                        return;
                    }
                }
                if rect_contains(layout.editor, x, y) {
                    let offset = &mut self.scroll_offsets[self.active_buffer];
                    *offset = offset.saturating_sub(3);
                }
            }
            MouseEventKind::ScrollDown => {
                if let Some(sidebar) = layout.sidebar {
                    if rect_contains(sidebar, x, y) {
                        self.sidebar_scroll += 3;
                        return;
                    }
                }
                if rect_contains(layout.editor, x, y) {
                    let offset = &mut self.scroll_offsets[self.active_buffer];
                    let max = self.buffers[self.active_buffer]
                        .line_count()
                        .saturating_sub(1);
                    *offset = (*offset + 3).min(max);
                }
            }
            _ => {}
        }
    }

    fn handle_menu_click(&mut self, x: u16) {
        if let Some(label) = self.menu_bar.hit_test(x) {
            match label.trim() {
                "Exit" => self.running = false,
                "File" => {
                    // Future: file menu dropdown
                }
                "Edit" => {
                    // Future: edit menu dropdown
                }
                "View" => {
                    // Future: view menu dropdown
                }
                _ => {}
            }
        }
    }

    fn handle_tab_click(&mut self, x: u16) {
        if let Some(result) = self.tab_bar_state.hit_test(x) {
            match result {
                TabHitResult::Select(i) => {
                    if i < self.buffers.len() {
                        self.active_buffer = i;
                        self.focus = FocusTarget::Editor;
                        self.highlight_dirty = true;
                    }
                }
                TabHitResult::Close(i) => {
                    self.close_tab(i);
                }
            }
        }
    }

    fn close_tab(&mut self, idx: usize) {
        if self.buffers.len() <= 1 {
            return;
        }
        self.buffers.remove(idx);
        self.selections.remove(idx);
        self.scroll_offsets.remove(idx);

        if self.active_buffer >= self.buffers.len() {
            self.active_buffer = self.buffers.len() - 1;
        } else if self.active_buffer > idx {
            self.active_buffer -= 1;
        }
        self.highlight_dirty = true;
    }

    fn handle_sidebar_click(&mut self, _x: u16, y: u16, sidebar_area: Rect) {
        self.focus = FocusTarget::Sidebar;

        let inner_y = y.saturating_sub(sidebar_area.y + 1);
        let click_idx = self.sidebar_scroll + inner_y as usize;

        if let Some(tree) = &mut self.file_tree {
            if click_idx < tree.entries.len() {
                tree.selected = click_idx;
                let entry = tree.entries[click_idx].clone();
                match entry.kind {
                    EntryKind::Directory => {
                        tree.toggle_expand();
                    }
                    EntryKind::File => {
                        self.open_file(&entry.path);
                        self.focus = FocusTarget::Editor;
                    }
                }
            }
        }
    }

    fn handle_editor_click(&mut self, x: u16, y: u16, editor_area: Rect) {
        self.focus = FocusTarget::Editor;

        let rel_y = (y - editor_area.y) as usize;
        let rel_x = x.saturating_sub(editor_area.x + GUTTER_WIDTH) as usize;

        let line = self.scroll_offset() + rel_y;
        if line >= self.buffer().line_count() {
            return;
        }

        let line_len = self.buffer().line_len(line);
        let col = rel_x.min(line_len);

        self.selections_mut()
            .set_primary(Selection::cursor(Position::new(line, col)));
    }

    fn handle_terminal_key(&mut self, key: crossterm::event::KeyEvent) {
        use crossterm::event::{KeyModifiers};

        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

        if key.code == KeyCode::Esc {
            self.focus = FocusTarget::Editor;
            return;
        }
        if ctrl && key.code == KeyCode::Char('q') {
            self.running = false;
            return;
        }

        let Some(term) = &mut self.terminal else { return };

        if !term.is_alive() {
            if key.code == KeyCode::Enter {
                self.terminal = EmbeddedTerminal::new(80, 10).ok();
            }
            return;
        }

        let bytes: Vec<u8> = match key.code {
            KeyCode::Char(c) if ctrl => vec![(c as u8) & 0x1f],
            KeyCode::Char(c) => {
                let mut buf = [0u8; 4];
                c.encode_utf8(&mut buf);
                buf[..c.len_utf8()].to_vec()
            }
            KeyCode::Enter => vec![b'\r'],
            KeyCode::Backspace => vec![0x7f],
            KeyCode::Delete => vec![0x1b, b'[', b'3', b'~'],
            KeyCode::Tab => vec![b'\t'],
            KeyCode::Up => vec![0x1b, b'[', b'A'],
            KeyCode::Down => vec![0x1b, b'[', b'B'],
            KeyCode::Right => vec![0x1b, b'[', b'C'],
            KeyCode::Left => vec![0x1b, b'[', b'D'],
            KeyCode::Home => vec![0x1b, b'[', b'H'],
            KeyCode::End => vec![0x1b, b'[', b'F'],
            _ => vec![],
        };

        if !bytes.is_empty() {
            term.write_input(&bytes);
        }
    }

    fn handle_sidebar_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Up => {
                if let Some(tree) = &mut self.file_tree {
                    tree.move_selection(-1);
                }
            }
            KeyCode::Down => {
                if let Some(tree) = &mut self.file_tree {
                    tree.move_selection(1);
                }
            }
            KeyCode::Enter => {
                if let Some(tree) = &mut self.file_tree {
                    let entry = tree.selected_entry().cloned();
                    if let Some(entry) = entry {
                        match entry.kind {
                            EntryKind::Directory => {
                                tree.toggle_expand();
                            }
                            EntryKind::File => {
                                self.open_file(&entry.path);
                                self.focus = FocusTarget::Editor;
                            }
                        }
                    }
                }
            }
            KeyCode::Esc => {
                self.focus = FocusTarget::Editor;
            }
            _ => {}
        }
    }

    fn open_file(&mut self, path: &PathBuf) {
        for (i, buf) in self.buffers.iter().enumerate() {
            if buf.path() == Some(path) {
                self.active_buffer = i;
                return;
            }
        }

        match loader::load_file(path) {
            Ok(contents) => {
                let buf = Buffer::from_file(path.clone(), &contents);
                let sels = Selections::new(Position::zero());
                self.buffers.push(buf);
                self.selections.push(sels);
                self.scroll_offsets.push(0);
                self.active_buffer = self.buffers.len() - 1;
            }
            Err(e) => {
                self.bottom_panel
                    .terminal_lines
                    .push(format!("Error opening {}: {}", path.display(), e));
            }
        }
    }

    fn handle_action(&mut self, action: Action) {
        if self.show_cheatsheet {
            match action {
                Action::ShowCheatsheet | Action::Noop => {}
                _ => {
                    self.show_cheatsheet = false;
                    if matches!(action, Action::Quit) {
                        return;
                    }
                    return;
                }
            }
            if action == Action::ShowCheatsheet {
                self.show_cheatsheet = false;
            }
            return;
        }

        match action {
            Action::Quit => self.running = false,
            Action::Save => self.save_current_buffer(),
            Action::CloseTab => self.close_tab(self.active_buffer),
            Action::Undo if self.focus == FocusTarget::Editor => {
                if let Some(pos) = self.buffer_mut().undo() {
                    self.selections_mut()
                        .set_primary(Selection::cursor(pos));
                }
            }
            Action::Redo if self.focus == FocusTarget::Editor => {
                if let Some(pos) = self.buffer_mut().redo() {
                    self.selections_mut()
                        .set_primary(Selection::cursor(pos));
                }
            }
            Action::ToggleSidebar => self.panel_state.toggle_sidebar(),
            Action::ToggleBottomPanel => self.panel_state.toggle_bottom_panel(),
            Action::FocusSidebar => {
                if self.panel_state.sidebar_visible && self.file_tree.is_some() {
                    self.focus = FocusTarget::Sidebar;
                }
            }
            Action::FocusTerminal => {
                self.panel_state.bottom_panel_visible = true;
                self.bottom_panel.active_tab = crate::tui::bottom_panel::BottomTab::Terminal;
                self.focus = FocusTarget::BottomPanel;
            }
            Action::FocusClaudeLog => {
                self.panel_state.bottom_panel_visible = true;
                self.bottom_panel.active_tab = crate::tui::bottom_panel::BottomTab::ClaudeLog;
                self.focus = FocusTarget::BottomPanel;
            }
            Action::NextTab => {
                if self.buffers.len() > 1 {
                    self.active_buffer = (self.active_buffer + 1) % self.buffers.len();
                    self.highlight_dirty = true;
                }
            }
            Action::CycleBottomTab => {
                if self.focus == FocusTarget::BottomPanel {
                    self.bottom_panel.active_tab = self.bottom_panel.active_tab.next();
                }
            }
            Action::ShowCheatsheet => {
                self.show_cheatsheet = true;
            }
            Action::Escape => {
                self.focus = FocusTarget::Editor;
            }
            Action::InsertChar(c) if self.focus == FocusTarget::Editor => {
                let pos = self.selections().primary().head;
                self.buffer_mut().insert(pos, &c.to_string());
                let new_col = pos.col + 1;
                self.selections_mut()
                    .set_primary(Selection::cursor(Position::new(pos.line, new_col)));
                self.highlight_dirty = true;
            }
            Action::InsertTab if self.focus == FocusTarget::Editor => {
                let pos = self.selections().primary().head;
                self.buffer_mut().insert(pos, "    ");
                self.selections_mut()
                    .set_primary(Selection::cursor(Position::new(pos.line, pos.col + 4)));
                self.highlight_dirty = true;
            }
            Action::Backspace if self.focus == FocusTarget::Editor => {
                let pos = self.selections().primary().head;
                if let Some(new_pos) = self.buffer_mut().delete_char_before(pos) {
                    self.selections_mut()
                        .set_primary(Selection::cursor(new_pos));
                }
                self.highlight_dirty = true;
            }
            Action::Delete if self.focus == FocusTarget::Editor => {
                let pos = self.selections().primary().head;
                self.buffer_mut().delete_char_at(pos);
                self.highlight_dirty = true;
            }
            Action::Enter if self.focus == FocusTarget::Editor => {
                let pos = self.selections().primary().head;
                self.buffer_mut().insert(pos, "\n");
                self.selections_mut()
                    .set_primary(Selection::cursor(Position::new(pos.line + 1, 0)));
                self.highlight_dirty = true;
            }
            Action::CursorUp => self.move_cursor_vertical(-1),
            Action::CursorDown => self.move_cursor_vertical(1),
            Action::CursorLeft => self.move_cursor_horizontal(-1),
            Action::CursorRight => self.move_cursor_horizontal(1),
            Action::WordLeft if self.focus == FocusTarget::Editor => {
                self.move_word_left();
            }
            Action::WordRight if self.focus == FocusTarget::Editor => {
                self.move_word_right();
            }
            Action::Home => {
                let pos = self.selections().primary().head;
                self.selections_mut()
                    .set_primary(Selection::cursor(Position::new(pos.line, 0)));
            }
            Action::End => {
                let pos = self.selections().primary().head;
                let line_len = self.buffer().line_len(pos.line);
                self.selections_mut()
                    .set_primary(Selection::cursor(Position::new(pos.line, line_len)));
            }
            Action::SelectAll if self.focus == FocusTarget::Editor => {
                let last_line = self.buffer().line_count().saturating_sub(1);
                let last_col = self.buffer().line_len(last_line);
                self.selections_mut().set_primary(Selection::new(
                    Position::zero(),
                    Position::new(last_line, last_col),
                ));
            }
            Action::PageUp => {
                let offset = &mut self.scroll_offsets[self.active_buffer];
                *offset = offset.saturating_sub(20);
            }
            Action::PageDown => {
                let offset = &mut self.scroll_offsets[self.active_buffer];
                let max = self.buffers[self.active_buffer]
                    .line_count()
                    .saturating_sub(1);
                *offset = (*offset + 20).min(max);
            }
            _ => {}
        }

        self.ensure_cursor_visible();
    }

    fn move_word_left(&mut self) {
        let pos = self.selections().primary().head;
        if let Some(line_slice) = self.buffer().line(pos.line) {
            let line_str: String = line_slice.chars().collect();
            let mut col = pos.col;
            while col > 0 && !line_str.as_bytes().get(col - 1).map_or(false, |b| b.is_ascii_alphanumeric() || *b == b'_') {
                col -= 1;
            }
            while col > 0 && line_str.as_bytes().get(col - 1).map_or(false, |b| b.is_ascii_alphanumeric() || *b == b'_') {
                col -= 1;
            }
            self.selections_mut()
                .set_primary(Selection::cursor(Position::new(pos.line, col)));
        }
    }

    fn move_word_right(&mut self) {
        let pos = self.selections().primary().head;
        let line_len = self.buffer().line_len(pos.line);
        if let Some(line_slice) = self.buffer().line(pos.line) {
            let line_str: String = line_slice.chars().collect();
            let mut col = pos.col;
            while col < line_len && line_str.as_bytes().get(col).map_or(false, |b| b.is_ascii_alphanumeric() || *b == b'_') {
                col += 1;
            }
            while col < line_len && !line_str.as_bytes().get(col).map_or(false, |b| b.is_ascii_alphanumeric() || *b == b'_') {
                col += 1;
            }
            self.selections_mut()
                .set_primary(Selection::cursor(Position::new(pos.line, col)));
        }
    }

    fn move_cursor_vertical(&mut self, delta: i32) {
        let pos = self.selections().primary().head;
        let new_line = if delta < 0 {
            pos.line.saturating_sub((-delta) as usize)
        } else {
            (pos.line + delta as usize).min(self.buffer().line_count().saturating_sub(1))
        };
        let line_len = self.buffer().line_len(new_line);
        let new_col = pos.col.min(line_len);
        self.selections_mut()
            .set_primary(Selection::cursor(Position::new(new_line, new_col)));
    }

    fn move_cursor_horizontal(&mut self, delta: i32) {
        let pos = self.selections().primary().head;
        if delta < 0 {
            if pos.col > 0 {
                self.selections_mut()
                    .set_primary(Selection::cursor(Position::new(pos.line, pos.col - 1)));
            } else if pos.line > 0 {
                let prev_len = self.buffer().line_len(pos.line - 1);
                self.selections_mut()
                    .set_primary(Selection::cursor(Position::new(pos.line - 1, prev_len)));
            }
        } else {
            let line_len = self.buffer().line_len(pos.line);
            if pos.col < line_len {
                self.selections_mut()
                    .set_primary(Selection::cursor(Position::new(pos.line, pos.col + 1)));
            } else if pos.line + 1 < self.buffer().line_count() {
                self.selections_mut()
                    .set_primary(Selection::cursor(Position::new(pos.line + 1, 0)));
            }
        }
    }

    fn ensure_cursor_visible(&mut self) {
        let cursor_line = self.selections().primary().head.line;
        let offset = &mut self.scroll_offsets[self.active_buffer];
        if cursor_line < *offset {
            *offset = cursor_line;
        }
        if cursor_line >= *offset + 40 {
            *offset = cursor_line.saturating_sub(39);
        }
    }

    fn save_current_buffer(&mut self) {
        if let Some(path) = self.buffer().path().cloned() {
            let contents = self.buffer().contents();
            if let Err(e) = std::fs::write(&path, &contents) {
                self.bottom_panel
                    .terminal_lines
                    .push(format!("Error saving: {}", e));
            } else {
                self.buffer_mut().mark_clean();
                self.bottom_panel
                    .terminal_lines
                    .push(format!("Saved: {}", path.display()));
            }
        }
    }
}

fn rect_contains(rect: Rect, x: u16, y: u16) -> bool {
    x >= rect.x && x < rect.x + rect.width && y >= rect.y && y < rect.y + rect.height
}
