use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::io::{self, BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandKind {
    Edit,
    Write,
    Bash,
    Read,
    Think,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryStatus {
    Running,
    Success,
    Failed,
}

#[derive(Debug, Clone)]
pub struct CommandEntry {
    pub kind: CommandKind,
    pub summary: String,
    pub detail: String,
    pub status: EntryStatus,
    pub affected_files: Vec<PathBuf>,
    pub expanded: bool,
}

#[derive(Debug, Clone)]
pub struct FileDiff {
    pub path: PathBuf,
    pub is_new: bool,
    pub hunks: Vec<DiffHunk>,
}

#[derive(Debug, Clone)]
pub enum DiffHunk {
    Added { line: usize },
    Removed { line: usize, text: String },
    Modified { line: usize },
}

pub struct ClaudeSession {
    pub entries: Vec<CommandEntry>,
    pub file_diffs: HashMap<PathBuf, FileDiff>,
    pub modified_files: Vec<PathBuf>,
    file_offset: u64,
    stream_path: Option<PathBuf>,
}

impl ClaudeSession {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            file_diffs: HashMap::new(),
            modified_files: Vec::new(),
            file_offset: 0,
            stream_path: None,
        }
    }

    pub fn watch(&mut self, path: PathBuf) {
        self.stream_path = Some(path);
    }

    pub fn auto_detect(project_dir: &Path) -> Option<Self> {
        let home = dirs_next::home_dir()?;
        let claude_projects = home.join(".claude").join("projects");

        let dir_str = project_dir.to_string_lossy();
        let encoded = dir_str.replace('/', "-");

        let project_session_dir = claude_projects.join(&encoded);
        if !project_session_dir.is_dir() {
            return None;
        }

        let mut newest: Option<(PathBuf, std::time::SystemTime)> = None;
        if let Ok(entries) = fs::read_dir(&project_session_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map_or(false, |e| e == "jsonl") {
                    if let Ok(meta) = path.metadata() {
                        if let Ok(modified) = meta.modified() {
                            if newest.as_ref().map_or(true, |(_, t)| modified > *t) {
                                newest = Some((path, modified));
                            }
                        }
                    }
                }
            }
        }

        let (session_path, _) = newest?;
        let mut session = Self::new();
        session.watch(session_path);
        let _ = session.poll();
        Some(session)
    }

    pub fn poll(&mut self) -> io::Result<bool> {
        let Some(path) = &self.stream_path else {
            return Ok(false);
        };

        if !path.exists() {
            return Ok(false);
        }

        let file = fs::File::open(path)?;
        let metadata = file.metadata()?;
        let file_len = metadata.len();

        if file_len <= self.file_offset {
            return Ok(false);
        }

        let mut reader = BufReader::new(file);
        reader.seek(SeekFrom::Start(self.file_offset))?;

        let mut had_updates = false;
        let mut line = String::new();
        while reader.read_line(&mut line)? > 0 {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
                    self.process_event(&value);
                    had_updates = true;
                }
            }
            line.clear();
        }

        self.file_offset = file_len;
        Ok(had_updates)
    }

    fn process_event(&mut self, event: &Value) {
        let event_type = event.get("type").and_then(|t| t.as_str()).unwrap_or("");

        match event_type {
            "assistant" => self.process_assistant(event),
            "result" => self.process_result(event),
            _ => {}
        }
    }

    fn process_assistant(&mut self, event: &Value) {
        let Some(message) = event.get("message") else {
            return;
        };
        let Some(content) = message.get("content").and_then(|c| c.as_array()) else {
            return;
        };

        for block in content {
            let block_type = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
            match block_type {
                "text" => {
                    if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                        let preview = text.chars().take(80).collect::<String>();
                        if !preview.trim().is_empty() {
                            self.entries.push(CommandEntry {
                                kind: CommandKind::Think,
                                summary: format!("💭 {}", truncate(&preview, 60)),
                                detail: text.to_string(),
                                status: EntryStatus::Success,
                                affected_files: Vec::new(),
                                expanded: false,
                            });
                        }
                    }
                }
                "tool_use" => {
                    self.process_tool_use(block);
                }
                _ => {}
            }
        }
    }

    fn process_tool_use(&mut self, block: &Value) {
        let name = block.get("name").and_then(|n| n.as_str()).unwrap_or("");
        let input = block.get("input").unwrap_or(&Value::Null);

        match name {
            "Edit" => {
                let file_path = input
                    .get("file_path")
                    .and_then(|p| p.as_str())
                    .unwrap_or("");
                let old_string = input
                    .get("old_string")
                    .and_then(|s| s.as_str())
                    .unwrap_or("");
                let new_string = input
                    .get("new_string")
                    .and_then(|s| s.as_str())
                    .unwrap_or("");

                let path = PathBuf::from(file_path);
                let filename = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| file_path.to_string());

                self.entries.push(CommandEntry {
                    kind: CommandKind::Edit,
                    summary: format!("▸ Edit {}", filename),
                    detail: format!(
                        "File: {}\nOld: {}\nNew: {}",
                        file_path,
                        truncate(old_string, 100),
                        truncate(new_string, 100)
                    ),
                    status: EntryStatus::Success,
                    affected_files: vec![path.clone()],
                    expanded: false,
                });

                self.compute_edit_diff(&path, old_string, new_string);

                if !self.modified_files.contains(&path) {
                    self.modified_files.push(path);
                }
            }
            "Write" => {
                let file_path = input
                    .get("file_path")
                    .and_then(|p| p.as_str())
                    .unwrap_or("");
                let content = input
                    .get("content")
                    .and_then(|c| c.as_str())
                    .unwrap_or("");

                let path = PathBuf::from(file_path);
                let filename = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| file_path.to_string());

                let line_count = content.lines().count();

                self.entries.push(CommandEntry {
                    kind: CommandKind::Write,
                    summary: format!("▸ Write {} ({} lines)", filename, line_count),
                    detail: format!("File: {}\n{}", file_path, truncate(content, 200)),
                    status: EntryStatus::Success,
                    affected_files: vec![path.clone()],
                    expanded: false,
                });

                let hunks: Vec<DiffHunk> = (0..line_count)
                    .map(|i| DiffHunk::Added { line: i })
                    .collect();

                self.file_diffs.insert(
                    path.clone(),
                    FileDiff {
                        path: path.clone(),
                        is_new: true,
                        hunks,
                    },
                );

                if !self.modified_files.contains(&path) {
                    self.modified_files.push(path);
                }
            }
            "Bash" => {
                let command = input
                    .get("command")
                    .and_then(|c| c.as_str())
                    .unwrap_or("");

                self.entries.push(CommandEntry {
                    kind: CommandKind::Bash,
                    summary: format!("▸ Bash: {}", truncate(command, 50)),
                    detail: format!("Command: {}", command),
                    status: EntryStatus::Running,
                    affected_files: Vec::new(),
                    expanded: false,
                });
            }
            "Read" => {
                let file_path = input
                    .get("file_path")
                    .and_then(|p| p.as_str())
                    .unwrap_or("");

                self.entries.push(CommandEntry {
                    kind: CommandKind::Read,
                    summary: format!("▸ Read {}", truncate(file_path, 50)),
                    detail: format!("File: {}", file_path),
                    status: EntryStatus::Success,
                    affected_files: vec![PathBuf::from(file_path)],
                    expanded: false,
                });
            }
            _ => {
                self.entries.push(CommandEntry {
                    kind: CommandKind::Bash,
                    summary: format!("▸ {} ", truncate(name, 40)),
                    detail: format!("Tool: {}\nInput: {}", name, input),
                    status: EntryStatus::Running,
                    affected_files: Vec::new(),
                    expanded: false,
                });
            }
        }
    }

    fn compute_edit_diff(&mut self, path: &PathBuf, old_str: &str, new_str: &str) {
        let mut hunks = self
            .file_diffs
            .get(path)
            .map(|d| d.hunks.clone())
            .unwrap_or_default();

        use similar::{ChangeTag, TextDiff};
        let diff = TextDiff::from_lines(old_str, new_str);
        let mut line_num = 0;
        for change in diff.iter_all_changes() {
            match change.tag() {
                ChangeTag::Insert => {
                    hunks.push(DiffHunk::Added { line: line_num });
                    line_num += 1;
                }
                ChangeTag::Delete => {
                    hunks.push(DiffHunk::Removed {
                        line: line_num,
                        text: change.value().to_string(),
                    });
                }
                ChangeTag::Equal => {
                    line_num += 1;
                }
            }
        }

        self.file_diffs.insert(
            path.clone(),
            FileDiff {
                path: path.clone(),
                is_new: false,
                hunks,
            },
        );
    }

    fn process_result(&mut self, event: &Value) {
        if let Some(last) = self.entries.last_mut() {
            if last.status == EntryStatus::Running {
                let is_error = event
                    .get("is_error")
                    .and_then(|e| e.as_bool())
                    .unwrap_or(false);
                last.status = if is_error {
                    EntryStatus::Failed
                } else {
                    EntryStatus::Success
                };
            }
        }
    }

    pub fn status_text(&self) -> String {
        if self.stream_path.is_none() {
            return "No Claude session".to_string();
        }
        let count = self.entries.iter().filter(|e| matches!(e.kind, CommandKind::Edit | CommandKind::Write | CommandKind::Bash)).count();
        if count == 0 {
            "Claude: connected (0 actions)".to_string()
        } else {
            format!("Claude: {} actions", count)
        }
    }

    pub fn sidebar_entries(&self) -> Vec<String> {
        if self.entries.is_empty() {
            return vec!["(no session)".to_string()];
        }
        self.entries
            .iter()
            .rev()
            .take(20)
            .map(|e| e.summary.clone())
            .collect()
    }

    pub fn log_lines(&self) -> Vec<String> {
        if self.entries.is_empty() {
            return vec!["No Claude session active.".to_string()];
        }
        let mut lines = Vec::new();
        for entry in &self.entries {
            let status_icon = match entry.status {
                EntryStatus::Running => "⟳",
                EntryStatus::Success => "✓",
                EntryStatus::Failed => "✗",
            };
            lines.push(format!("{} {}", status_icon, entry.summary));
            if entry.expanded {
                for detail_line in entry.detail.lines() {
                    lines.push(format!("    {}", detail_line));
                }
            }
        }
        lines
    }

    pub fn is_file_claude_modified(&self, path: &Path) -> bool {
        self.modified_files.iter().any(|p| p == path)
    }

    pub fn is_file_claude_created(&self, path: &Path) -> bool {
        self.file_diffs
            .get(path)
            .map(|d| d.is_new)
            .unwrap_or(false)
    }

    pub fn get_line_diff_marker(&self, path: &Path, line: usize) -> Option<DiffMarker> {
        let diff = self.file_diffs.get(path)?;
        for hunk in &diff.hunks {
            match hunk {
                DiffHunk::Added { line: l } if *l == line => return Some(DiffMarker::Added),
                DiffHunk::Modified { line: l } if *l == line => return Some(DiffMarker::Modified),
                _ => {}
            }
        }
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffMarker {
    Added,
    Removed,
    Modified,
}

fn truncate(s: &str, max: usize) -> String {
    let first_line = s.lines().next().unwrap_or(s);
    if first_line.len() > max {
        format!("{}...", &first_line[..max])
    } else {
        first_line.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_edit_event(file: &str, old: &str, new: &str) -> String {
        serde_json::json!({
            "type": "assistant",
            "message": {
                "content": [{
                    "type": "tool_use",
                    "id": "test_1",
                    "name": "Edit",
                    "input": {
                        "file_path": file,
                        "old_string": old,
                        "new_string": new
                    }
                }]
            },
            "session_id": "test"
        })
        .to_string()
    }

    fn make_bash_event(command: &str) -> String {
        serde_json::json!({
            "type": "assistant",
            "message": {
                "content": [{
                    "type": "tool_use",
                    "id": "test_2",
                    "name": "Bash",
                    "input": {
                        "command": command
                    }
                }]
            },
            "session_id": "test"
        })
        .to_string()
    }

    fn make_write_event(file: &str, content: &str) -> String {
        serde_json::json!({
            "type": "assistant",
            "message": {
                "content": [{
                    "type": "tool_use",
                    "id": "test_3",
                    "name": "Write",
                    "input": {
                        "file_path": file,
                        "content": content
                    }
                }]
            },
            "session_id": "test"
        })
        .to_string()
    }

    #[test]
    fn parse_edit_event() {
        let mut session = ClaudeSession::new();
        let json = make_edit_event("/tmp/test.rs", "let x = old();", "let x = new();");
        let value: Value = serde_json::from_str(&json).unwrap();
        session.process_event(&value);

        assert_eq!(session.entries.len(), 1);
        assert_eq!(session.entries[0].kind, CommandKind::Edit);
        assert!(session.entries[0].summary.contains("Edit"));
        assert!(session.modified_files.contains(&PathBuf::from("/tmp/test.rs")));
    }

    #[test]
    fn parse_bash_event() {
        let mut session = ClaudeSession::new();
        let json = make_bash_event("cargo test");
        let value: Value = serde_json::from_str(&json).unwrap();
        session.process_event(&value);

        assert_eq!(session.entries.len(), 1);
        assert_eq!(session.entries[0].kind, CommandKind::Bash);
        assert!(session.entries[0].summary.contains("cargo test"));
    }

    #[test]
    fn parse_write_event_marks_new_file() {
        let mut session = ClaudeSession::new();
        let json = make_write_event("/tmp/new_file.rs", "fn main() {}\n");
        let value: Value = serde_json::from_str(&json).unwrap();
        session.process_event(&value);

        assert_eq!(session.entries.len(), 1);
        assert_eq!(session.entries[0].kind, CommandKind::Write);
        assert!(session.is_file_claude_created(&PathBuf::from("/tmp/new_file.rs")));
    }

    #[test]
    fn edit_diff_tracks_added_lines() {
        let mut session = ClaudeSession::new();
        let json = make_edit_event("/tmp/test.rs", "line1\nline2", "line1\nnew_line\nline2");
        let value: Value = serde_json::from_str(&json).unwrap();
        session.process_event(&value);

        let diff = session.file_diffs.get(&PathBuf::from("/tmp/test.rs")).unwrap();
        assert!(!diff.hunks.is_empty());
    }

    #[test]
    fn poll_reads_from_file() {
        let dir = std::env::temp_dir().join("xpar_claude_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let stream_file = dir.join("session.jsonl");

        let events = format!(
            "{}\n{}\n",
            make_bash_event("cargo build"),
            make_edit_event("/tmp/main.rs", "old", "new")
        );
        std::fs::write(&stream_file, &events).unwrap();

        let mut session = ClaudeSession::new();
        session.watch(stream_file);
        let had_updates = session.poll().unwrap();

        assert!(had_updates);
        assert_eq!(session.entries.len(), 2);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn sidebar_entries_most_recent_first() {
        let mut session = ClaudeSession::new();
        for event_json in [
            make_bash_event("first"),
            make_bash_event("second"),
            make_bash_event("third"),
        ] {
            let value: Value = serde_json::from_str(&event_json).unwrap();
            session.process_event(&value);
        }

        let sidebar = session.sidebar_entries();
        assert!(sidebar[0].contains("third"));
        assert!(sidebar[2].contains("first"));
    }
}
