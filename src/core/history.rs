use super::selections::Position;

#[derive(Debug, Clone, PartialEq)]
pub enum EditKind {
    Insert,
    Delete,
}

#[derive(Debug, Clone)]
pub struct Edit {
    pub kind: EditKind,
    pub position: Position,
    pub text: String,
}

pub struct History {
    undo_stack: Vec<Edit>,
    redo_stack: Vec<Edit>,
}

impl History {
    pub fn new() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    pub fn push_edit(&mut self, edit: Edit) {
        self.undo_stack.push(edit);
        self.redo_stack.clear();
    }

    pub fn undo(&mut self) -> Option<Edit> {
        let edit = self.undo_stack.pop()?;
        self.redo_stack.push(edit.clone());
        Some(edit)
    }

    pub fn redo(&mut self) -> Option<Edit> {
        let edit = self.redo_stack.pop()?;
        self.undo_stack.push(edit.clone());
        Some(edit)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_insert(line: usize, col: usize, text: &str) -> Edit {
        Edit {
            kind: EditKind::Insert,
            position: Position::new(line, col),
            text: text.to_string(),
        }
    }

    fn make_delete(line: usize, col: usize, text: &str) -> Edit {
        Edit {
            kind: EditKind::Delete,
            position: Position::new(line, col),
            text: text.to_string(),
        }
    }

    #[test]
    fn empty_history() {
        let history = History::new();
        assert!(!history.can_undo());
        assert!(!history.can_redo());
    }

    #[test]
    fn push_and_undo() {
        let mut history = History::new();
        history.push_edit(make_insert(0, 0, "hello"));
        assert!(history.can_undo());

        let edit = history.undo().unwrap();
        assert_eq!(edit.kind, EditKind::Insert);
        assert_eq!(edit.text, "hello");
        assert!(!history.can_undo());
        assert!(history.can_redo());
    }

    #[test]
    fn undo_then_redo() {
        let mut history = History::new();
        history.push_edit(make_insert(0, 0, "hello"));
        history.undo();

        let edit = history.redo().unwrap();
        assert_eq!(edit.text, "hello");
        assert!(history.can_undo());
        assert!(!history.can_redo());
    }

    #[test]
    fn new_edit_clears_redo() {
        let mut history = History::new();
        history.push_edit(make_insert(0, 0, "hello"));
        history.undo();
        assert!(history.can_redo());

        history.push_edit(make_delete(0, 0, "h"));
        assert!(!history.can_redo());
    }

    #[test]
    fn multiple_undos() {
        let mut history = History::new();
        history.push_edit(make_insert(0, 0, "a"));
        history.push_edit(make_insert(0, 1, "b"));
        history.push_edit(make_insert(0, 2, "c"));

        let c = history.undo().unwrap();
        assert_eq!(c.text, "c");
        let b = history.undo().unwrap();
        assert_eq!(b.text, "b");
        let a = history.undo().unwrap();
        assert_eq!(a.text, "a");
        assert!(history.undo().is_none());
    }
}
