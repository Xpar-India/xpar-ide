use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct TreeEntry {
    pub path: PathBuf,
    pub name: String,
    pub kind: EntryKind,
    pub depth: usize,
    pub expanded: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryKind {
    Directory,
    File,
}

pub struct FileTree {
    pub root: PathBuf,
    pub entries: Vec<TreeEntry>,
    pub selected: usize,
}

impl FileTree {
    pub fn new(root: PathBuf) -> Self {
        let mut tree = Self {
            root: root.clone(),
            entries: Vec::new(),
            selected: 0,
        };
        tree.load_directory(&root, 0);
        tree
    }

    fn load_directory(&mut self, dir: &Path, depth: usize) {
        let mut entries: Vec<(String, PathBuf, bool)> = Vec::new();

        let walker = ignore::WalkBuilder::new(dir)
            .max_depth(Some(1))
            .hidden(false)
            .sort_by_file_name(|a, b| {
                let a_is_dir = a.to_str().map(|s| !s.contains('.')).unwrap_or(false);
                let b_is_dir = b.to_str().map(|s| !s.contains('.')).unwrap_or(false);
                b_is_dir.cmp(&a_is_dir).then(a.cmp(b))
            })
            .build();

        for result in walker {
            let Ok(entry) = result else { continue };
            let path = entry.path().to_path_buf();
            if path == dir {
                continue;
            }
            let name = entry
                .file_name()
                .to_string_lossy()
                .to_string();
            let is_dir = path.is_dir();
            entries.push((name, path, is_dir));
        }

        entries.sort_by(|a, b| b.2.cmp(&a.2).then(a.0.cmp(&b.0)));

        for (name, path, is_dir) in entries {
            self.entries.push(TreeEntry {
                path,
                name,
                kind: if is_dir {
                    EntryKind::Directory
                } else {
                    EntryKind::File
                },
                depth,
                expanded: false,
            });
        }
    }

    pub fn toggle_expand(&mut self) {
        if self.entries.is_empty() {
            return;
        }

        let entry = &self.entries[self.selected];
        if entry.kind != EntryKind::Directory {
            return;
        }

        let path = entry.path.clone();
        let depth = entry.depth;

        if entry.expanded {
            self.entries[self.selected].expanded = false;
            let mut remove_end = self.selected + 1;
            while remove_end < self.entries.len() && self.entries[remove_end].depth > depth {
                remove_end += 1;
            }
            self.entries.drain(self.selected + 1..remove_end);
        } else {
            self.entries[self.selected].expanded = true;
            let insert_pos = self.selected + 1;
            let mut children = Vec::new();

            let walker = ignore::WalkBuilder::new(&path)
                .max_depth(Some(1))
                .hidden(false)
                .sort_by_file_name(|a, b| a.cmp(b))
                .build();

            let mut raw: Vec<(String, PathBuf, bool)> = Vec::new();
            for result in walker {
                let Ok(e) = result else { continue };
                let p = e.path().to_path_buf();
                if p == path {
                    continue;
                }
                let name = e.file_name().to_string_lossy().to_string();
                let is_dir = p.is_dir();
                raw.push((name, p, is_dir));
            }
            raw.sort_by(|a, b| b.2.cmp(&a.2).then(a.0.cmp(&b.0)));

            for (name, p, is_dir) in raw {
                children.push(TreeEntry {
                    path: p,
                    name,
                    kind: if is_dir {
                        EntryKind::Directory
                    } else {
                        EntryKind::File
                    },
                    depth: depth + 1,
                    expanded: false,
                });
            }

            for (i, child) in children.into_iter().enumerate() {
                self.entries.insert(insert_pos + i, child);
            }
        }
    }

    pub fn move_selection(&mut self, delta: i32) {
        if self.entries.is_empty() {
            return;
        }
        if delta < 0 {
            self.selected = self.selected.saturating_sub((-delta) as usize);
        } else {
            self.selected = (self.selected + delta as usize).min(self.entries.len() - 1);
        }
    }

    pub fn selected_entry(&self) -> Option<&TreeEntry> {
        self.entries.get(self.selected)
    }

    pub fn display_entries(&self) -> Vec<(String, usize, &EntryKind, bool)> {
        self.entries
            .iter()
            .enumerate()
            .map(|(i, e)| {
                let prefix = "  ".repeat(e.depth);
                let icon = match e.kind {
                    EntryKind::Directory if e.expanded => "▾ ",
                    EntryKind::Directory => "▸ ",
                    EntryKind::File => "  ",
                };
                let display = format!("{}{}{}", prefix, icon, e.name);
                (display, i, &e.kind, i == self.selected)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn tree_from_directory() {
        let dir = std::env::temp_dir().join("xpar_test_tree");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::write(dir.join("src/main.rs"), "fn main() {}").unwrap();
        fs::write(dir.join("Cargo.toml"), "[package]").unwrap();

        let tree = FileTree::new(dir.clone());
        assert!(!tree.entries.is_empty());

        let names: Vec<&str> = tree.entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"src"));
        assert!(names.contains(&"Cargo.toml"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn tree_expand_collapse() {
        let dir = std::env::temp_dir().join("xpar_test_expand");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("sub")).unwrap();
        fs::write(dir.join("sub/file.txt"), "hello").unwrap();
        fs::write(dir.join("root.txt"), "root").unwrap();

        let mut tree = FileTree::new(dir.clone());
        let initial_count = tree.entries.len();

        let dir_idx = tree
            .entries
            .iter()
            .position(|e| e.kind == EntryKind::Directory)
            .unwrap();
        tree.selected = dir_idx;
        tree.toggle_expand();
        assert!(tree.entries.len() > initial_count);

        tree.toggle_expand();
        assert_eq!(tree.entries.len(), initial_count);

        let _ = fs::remove_dir_all(&dir);
    }
}
