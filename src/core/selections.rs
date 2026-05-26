#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub line: usize,
    pub col: usize,
}

impl Position {
    pub fn new(line: usize, col: usize) -> Self {
        Self { line, col }
    }

    pub fn zero() -> Self {
        Self { line: 0, col: 0 }
    }
}

impl PartialOrd for Position {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Position {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.line.cmp(&other.line).then(self.col.cmp(&other.col))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    pub anchor: Position,
    pub head: Position,
}

impl Selection {
    pub fn cursor(pos: Position) -> Self {
        Self {
            anchor: pos,
            head: pos,
        }
    }

    pub fn new(anchor: Position, head: Position) -> Self {
        Self { anchor, head }
    }

    pub fn is_cursor(&self) -> bool {
        self.anchor == self.head
    }

    pub fn start(&self) -> Position {
        if self.anchor <= self.head {
            self.anchor
        } else {
            self.head
        }
    }

    pub fn end(&self) -> Position {
        if self.anchor <= self.head {
            self.head
        } else {
            self.anchor
        }
    }

    pub fn collapse_to_head(&self) -> Self {
        Self::cursor(self.head)
    }
}

pub struct Selections {
    primary: usize,
    selections: Vec<Selection>,
}

impl Selections {
    pub fn new(pos: Position) -> Self {
        Self {
            primary: 0,
            selections: vec![Selection::cursor(pos)],
        }
    }

    pub fn primary(&self) -> &Selection {
        &self.selections[self.primary]
    }

    pub fn primary_mut(&mut self) -> &mut Selection {
        &mut self.selections[self.primary]
    }

    pub fn all(&self) -> &[Selection] {
        &self.selections
    }

    pub fn set_primary(&mut self, sel: Selection) {
        self.selections[self.primary] = sel;
    }

    pub fn add(&mut self, sel: Selection) {
        self.selections.push(sel);
        self.primary = self.selections.len() - 1;
    }

    pub fn count(&self) -> usize {
        self.selections.len()
    }

    pub fn collapse_to_primary(&mut self) {
        let primary = self.selections[self.primary];
        self.selections = vec![primary];
        self.primary = 0;
    }

    pub fn clear_to_cursor(&mut self, pos: Position) {
        self.selections = vec![Selection::cursor(pos)];
        self.primary = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn position_ordering() {
        assert!(Position::new(0, 0) < Position::new(0, 1));
        assert!(Position::new(0, 5) < Position::new(1, 0));
        assert!(Position::new(1, 3) == Position::new(1, 3));
    }

    #[test]
    fn cursor_selection() {
        let sel = Selection::cursor(Position::new(3, 5));
        assert!(sel.is_cursor());
        assert_eq!(sel.start(), sel.end());
    }

    #[test]
    fn range_selection_forward() {
        let sel = Selection::new(Position::new(0, 0), Position::new(0, 5));
        assert!(!sel.is_cursor());
        assert_eq!(sel.start(), Position::new(0, 0));
        assert_eq!(sel.end(), Position::new(0, 5));
    }

    #[test]
    fn range_selection_backward() {
        let sel = Selection::new(Position::new(0, 5), Position::new(0, 0));
        assert_eq!(sel.start(), Position::new(0, 0));
        assert_eq!(sel.end(), Position::new(0, 5));
    }

    #[test]
    fn multi_cursor() {
        let mut sels = Selections::new(Position::new(0, 0));
        assert_eq!(sels.count(), 1);

        sels.add(Selection::cursor(Position::new(1, 0)));
        assert_eq!(sels.count(), 2);
        assert_eq!(sels.primary().head, Position::new(1, 0));
    }

    #[test]
    fn collapse_to_primary() {
        let mut sels = Selections::new(Position::new(0, 0));
        sels.add(Selection::cursor(Position::new(1, 0)));
        sels.add(Selection::cursor(Position::new(2, 0)));
        assert_eq!(sels.count(), 3);

        sels.collapse_to_primary();
        assert_eq!(sels.count(), 1);
        assert_eq!(sels.primary().head, Position::new(2, 0));
    }

    #[test]
    fn clear_to_cursor() {
        let mut sels = Selections::new(Position::new(0, 0));
        sels.add(Selection::cursor(Position::new(1, 0)));
        sels.clear_to_cursor(Position::new(5, 3));
        assert_eq!(sels.count(), 1);
        assert_eq!(sels.primary().head, Position::new(5, 3));
    }
}
