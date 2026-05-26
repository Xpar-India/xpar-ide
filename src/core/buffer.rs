use ropey::Rope;
use std::path::PathBuf;

use super::history::{Edit, EditKind, History};
use super::selections::Position;

pub struct Buffer {
    rope: Rope,
    path: Option<PathBuf>,
    history: History,
    dirty: bool,
}

impl Buffer {
    pub fn new() -> Self {
        Self {
            rope: Rope::new(),
            path: None,
            history: History::new(),
            dirty: false,
        }
    }

    pub fn from_str(text: &str) -> Self {
        Self {
            rope: Rope::from_str(text),
            path: None,
            history: History::new(),
            dirty: false,
        }
    }

    pub fn from_file(path: PathBuf, contents: &str) -> Self {
        Self {
            rope: Rope::from_str(contents),
            path: Some(path),
            history: History::new(),
            dirty: false,
        }
    }

    pub fn path(&self) -> Option<&PathBuf> {
        self.path.as_ref()
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn mark_clean(&mut self) {
        self.dirty = false;
    }

    pub fn text(&self) -> &Rope {
        &self.rope
    }

    pub fn line_count(&self) -> usize {
        self.rope.len_lines()
    }

    pub fn line(&self, idx: usize) -> Option<ropey::RopeSlice<'_>> {
        if idx < self.rope.len_lines() {
            Some(self.rope.line(idx))
        } else {
            None
        }
    }

    pub fn len_chars(&self) -> usize {
        self.rope.len_chars()
    }

    fn pos_to_char_idx(&self, pos: Position) -> usize {
        let line_start = self.rope.line_to_char(pos.line);
        let line_len = self.rope.line(pos.line).len_chars();
        line_start + pos.col.min(line_len)
    }

    pub fn insert(&mut self, pos: Position, text: &str) {
        let char_idx = self.pos_to_char_idx(pos);
        self.rope.insert(char_idx, text);
        self.dirty = true;

        self.history.push_edit(Edit {
            kind: EditKind::Insert,
            position: pos,
            text: text.to_string(),
        });
    }

    pub fn delete_range(&mut self, from: Position, to: Position) -> String {
        let from_idx = self.pos_to_char_idx(from);
        let to_idx = self.pos_to_char_idx(to);

        let (start, end) = if from_idx <= to_idx {
            (from_idx, to_idx)
        } else {
            (to_idx, from_idx)
        };

        let deleted: String = self.rope.slice(start..end).into();
        self.rope.remove(start..end);
        self.dirty = true;

        let del_pos = if from_idx <= to_idx { from } else { to };
        self.history.push_edit(Edit {
            kind: EditKind::Delete,
            position: del_pos,
            text: deleted.clone(),
        });

        deleted
    }

    pub fn delete_char_before(&mut self, pos: Position) -> Option<Position> {
        if pos.col == 0 && pos.line == 0 {
            return None;
        }

        let new_pos = if pos.col == 0 {
            let prev_line = pos.line - 1;
            let prev_line_len = self.rope.line(prev_line).len_chars();
            let col = if prev_line_len > 0 && self.rope.line(prev_line).char(prev_line_len - 1) == '\n' {
                prev_line_len - 1
            } else {
                prev_line_len
            };
            Position::new(prev_line, col)
        } else {
            Position::new(pos.line, pos.col - 1)
        };

        self.delete_range(new_pos, pos);
        Some(new_pos)
    }

    pub fn delete_char_at(&mut self, pos: Position) {
        let char_idx = self.pos_to_char_idx(pos);
        if char_idx < self.rope.len_chars() {
            let end_pos = self.char_idx_to_pos(char_idx + 1);
            self.delete_range(pos, end_pos);
        }
    }

    fn char_idx_to_pos(&self, char_idx: usize) -> Position {
        let line = self.rope.char_to_line(char_idx);
        let line_start = self.rope.line_to_char(line);
        Position::new(line, char_idx - line_start)
    }

    pub fn line_len(&self, line: usize) -> usize {
        if line >= self.rope.len_lines() {
            return 0;
        }
        let line_slice = self.rope.line(line);
        let len = line_slice.len_chars();
        if len > 0 && line_slice.char(len - 1) == '\n' {
            len - 1
        } else {
            len
        }
    }

    pub fn contents(&self) -> String {
        self.rope.to_string()
    }

    pub fn undo(&mut self) -> Option<Position> {
        if let Some(edit) = self.history.undo() {
            self.apply_inverse(&edit);
            self.dirty = true;
            Some(edit.position)
        } else {
            None
        }
    }

    pub fn redo(&mut self) -> Option<Position> {
        if let Some(edit) = self.history.redo() {
            self.apply_forward(&edit);
            self.dirty = true;
            Some(edit.position)
        } else {
            None
        }
    }

    fn apply_inverse(&mut self, edit: &Edit) {
        match edit.kind {
            EditKind::Insert => {
                let start = self.pos_to_char_idx(edit.position);
                let end = start + edit.text.len();
                self.rope.remove(start..end);
            }
            EditKind::Delete => {
                let idx = self.pos_to_char_idx(edit.position);
                self.rope.insert(idx, &edit.text);
            }
        }
    }

    fn apply_forward(&mut self, edit: &Edit) {
        match edit.kind {
            EditKind::Insert => {
                let idx = self.pos_to_char_idx(edit.position);
                self.rope.insert(idx, &edit.text);
            }
            EditKind::Delete => {
                let start = self.pos_to_char_idx(edit.position);
                let end = start + edit.text.len();
                self.rope.remove(start..end);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_buffer_is_empty() {
        let buf = Buffer::new();
        assert_eq!(buf.len_chars(), 0);
        assert_eq!(buf.line_count(), 1); // ropey always has at least 1 line
        assert!(!buf.is_dirty());
    }

    #[test]
    fn from_str_preserves_content() {
        let buf = Buffer::from_str("hello\nworld\n");
        assert_eq!(buf.line_count(), 3);
        assert_eq!(buf.contents(), "hello\nworld\n");
    }

    #[test]
    fn insert_at_start() {
        let mut buf = Buffer::new();
        buf.insert(Position::new(0, 0), "hello");
        assert_eq!(buf.contents(), "hello");
        assert!(buf.is_dirty());
    }

    #[test]
    fn insert_in_middle() {
        let mut buf = Buffer::from_str("helo");
        buf.insert(Position::new(0, 2), "l");
        assert_eq!(buf.contents(), "hello");
    }

    #[test]
    fn insert_newline() {
        let mut buf = Buffer::from_str("hello world");
        buf.insert(Position::new(0, 5), "\n");
        assert_eq!(buf.line_count(), 2);
        assert_eq!(buf.contents(), "hello\n world");
    }

    #[test]
    fn delete_range() {
        let mut buf = Buffer::from_str("hello world");
        let deleted = buf.delete_range(Position::new(0, 5), Position::new(0, 11));
        assert_eq!(deleted, " world");
        assert_eq!(buf.contents(), "hello");
    }

    #[test]
    fn delete_char_before() {
        let mut buf = Buffer::from_str("hello");
        let new_pos = buf.delete_char_before(Position::new(0, 5));
        assert_eq!(new_pos, Some(Position::new(0, 4)));
        assert_eq!(buf.contents(), "hell");
    }

    #[test]
    fn delete_char_before_at_start_returns_none() {
        let mut buf = Buffer::from_str("hello");
        let new_pos = buf.delete_char_before(Position::new(0, 0));
        assert_eq!(new_pos, None);
        assert_eq!(buf.contents(), "hello");
    }

    #[test]
    fn delete_char_before_joins_lines() {
        let mut buf = Buffer::from_str("hello\nworld");
        let new_pos = buf.delete_char_before(Position::new(1, 0));
        assert_eq!(new_pos, Some(Position::new(0, 5)));
        assert_eq!(buf.contents(), "helloworld");
    }

    #[test]
    fn undo_insert() {
        let mut buf = Buffer::from_str("hello");
        buf.insert(Position::new(0, 5), " world");
        assert_eq!(buf.contents(), "hello world");

        buf.undo();
        assert_eq!(buf.contents(), "hello");
    }

    #[test]
    fn undo_delete() {
        let mut buf = Buffer::from_str("hello world");
        buf.delete_range(Position::new(0, 5), Position::new(0, 11));
        assert_eq!(buf.contents(), "hello");

        buf.undo();
        assert_eq!(buf.contents(), "hello world");
    }

    #[test]
    fn redo_after_undo() {
        let mut buf = Buffer::from_str("hello");
        buf.insert(Position::new(0, 5), " world");
        buf.undo();
        assert_eq!(buf.contents(), "hello");

        buf.redo();
        assert_eq!(buf.contents(), "hello world");
    }

    #[test]
    fn new_edit_clears_redo_stack() {
        let mut buf = Buffer::from_str("hello");
        buf.insert(Position::new(0, 5), " world");
        buf.undo();
        buf.insert(Position::new(0, 5), "!");
        assert_eq!(buf.contents(), "hello!");
        assert!(buf.redo().is_none());
    }

    #[test]
    fn line_len_excludes_newline() {
        let buf = Buffer::from_str("hello\nworld\n");
        assert_eq!(buf.line_len(0), 5);
        assert_eq!(buf.line_len(1), 5);
    }

    #[test]
    fn mark_clean() {
        let mut buf = Buffer::from_str("hello");
        buf.insert(Position::new(0, 5), "!");
        assert!(buf.is_dirty());
        buf.mark_clean();
        assert!(!buf.is_dirty());
    }
}
