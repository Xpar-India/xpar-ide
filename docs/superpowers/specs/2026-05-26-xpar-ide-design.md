# xpar.IDE — Design Spec

## Context

Claude Code runs in the terminal and makes file edits, runs commands, and spawns processes — but there's no efficient way to review those changes without opening a heavyweight editor like VS Code. xpar.IDE is a lightweight Rust TUI code editor purpose-built for the Claude Code workflow. It opens files at the points where Claude made changes, highlights diffs inline, shows a log of every command Claude ran, and provides a built-in editor and terminal so you never leave the terminal.

Target: M1 Mac, 8GB RAM. Must stay under 50MB RSS.

---

## Architecture: Layered Monolith

Single binary (`xpar-ide`), internally organized as four clean module layers:

```
src/
├── main.rs                  # Entry point, arg parsing, app lifecycle
├── app.rs                   # App state machine, event dispatch
│
├── core/                    # Layer 1: Editor engine (no TUI dependency)
│   ├── buffer.rs            # Rope-backed text buffer (ropey)
│   ├── selections.rs        # Cursor positions, multi-cursor
│   ├── history.rs           # Undo/redo (operation-based, transaction grouping)
│   ├── diff.rs              # Diff computation & hunk tracking (similar crate)
│   └── syntax.rs            # Tree-sitter integration, highlight queries
│
├── tui/                     # Layer 2: Rendering & input
│   ├── layout.rs            # Panel geometry, resize logic
│   ├── editor_view.rs       # Main editor widget (renders buffer + diff)
│   ├── sidebar.rs           # File tree + Claude log panel
│   ├── bottom_panel.rs      # Tabbed panel (terminal/build/claude)
│   ├── statusbar.rs         # Bottom status line
│   ├── tab_bar.rs           # Open file tabs
│   └── input.rs             # Keymap dispatch (Ctrl+shortcuts)
│
├── integrations/            # Layer 3: External system adapters
│   ├── claude.rs            # Parse stream-json, build command log
│   ├── terminal.rs          # PTY management, ANSI → screen buffer
│   ├── treesitter.rs        # Grammar loading, incremental parse
│   └── git.rs               # Optional: git status for gutter markers
│
└── fs/                      # Layer 4: Filesystem operations
    ├── watcher.rs           # File change notifications (notify crate)
    ├── loader.rs            # Async file read/write
    └── tree.rs              # Directory tree model for sidebar
```

**Dependency rule:** `core` depends on nothing in-project. `tui` depends on `core`. `integrations` depends on `core`. `fs` is standalone. `app.rs` wires them together.

**Async model:** `tokio` runtime. File I/O, Claude stream parsing, PTY reads, and file watching run as spawned tasks. Communication via `tokio::sync::mpsc` channels to the main event loop. TUI renders on the main thread, event-driven (not polling).

---

## Layout: Two-Column + Tabbed Bottom Panel

```
┌─────────┬──────────────────────────────────────┐
│ Files    │ src/main.rs [C]          src/lib.rs  │
│ ├ src/   │                                      │
│ │ main.rs│  fn main() {                         │
│ │ lib.rs │+   let x = new();                    │
│ ├ tests/ │-   let x = old();                    │
│──────────│    println!("{}", x);                 │
│ Claude   │                                      │
│ ▸ Edit   ├──────────────────────────────────────│
│   main.rs│ [Terminal] [Build] [Claude Log]      │
│ ▸ Bash   │ $ cargo build                        │
│   cargo t│ Compiling xpar-ide v0.1.0            │
│          │ Finished dev [unoptimized + debug]    │
└─────────┴──────────────────────────────────────┘
```

### Left sidebar (toggleable with Ctrl+B)

- **Top:** File tree. Lazy-loaded directories. Change markers from Claude stream: `[M]` modified (yellow), `[N]` new (green), `[D]` deleted (red).
- **Bottom:** Claude command log (compact). One-liners: `▸ Edit main.rs`, `▸ Bash cargo test`. Enter jumps to full detail in bottom panel.
- Resizable via mouse drag or `Ctrl+{` / `Ctrl+}`.

### Center: Editor

- Tab bar for open files with state indicators: `[C]` Claude-modified, `[N]` Claude-created, `[+]` user-modified, `[C+]` both.
- Diff gutter: `+` green (added), `-` red dimmed (removed), `~` blue (modified), `●` yellow (your unsaved edits).
- Ghost lines: deleted lines shown inline as dimmed red text, toggleable with `Ctrl+Shift+D`.
- Hunk navigation: `Alt+]` next change, `Alt+[` previous change.
- Diff reset: `Ctrl+Shift+R` clears Claude markers on current file (marks as "reviewed").

### Bottom panel (toggleable with Ctrl+`)

Three tabs, switchable with `Ctrl+J` then `1`/`2`/`3`:

1. **Terminal** — PTY-backed real shell. Full ANSI color. Resizes with panel.
2. **Build** — convenience launcher. `Ctrl+Shift+B` opens command palette with detected build commands (from Cargo.toml, go.mod, package.json). Runs in terminal tab.
3. **Claude Log** — full command log. Scrollable, `Enter` to expand/collapse. Color-coded: green (edits), yellow (bash), blue (reads), dim (thinking).

---

## Claude Code Integration

### Data flow

Claude Code runs in a separate terminal pane. Its output is redirected to a session file:
```bash
claude --output-format stream-json | tee ~/.xpar/sessions/current.jsonl
```

xpar.IDE watches that file with `notify` crate, parses new JSONL lines incrementally, and updates the CommandLog and diff state.

### Parsed events

| Stream event | Extracted data | Destination |
|---|---|---|
| `tool_use` name=`Edit` | File path, old/new content | Diff highlights + Claude Log |
| `tool_use` name=`Write` | File path, content | Diff highlights (new file) + Claude Log |
| `tool_use` name=`Bash` | Command string | Claude Log |
| `tool_result` | Output, success/failure | Expandable detail in Claude Log |
| `assistant` message | Reasoning text | Claude Log (collapsible) |

### CommandLog model

```rust
pub struct CommandLog {
    entries: Vec<CommandEntry>,
}

pub struct CommandEntry {
    timestamp: Instant,
    kind: CommandKind,          // Edit, Write, Bash, Read, Think
    summary: String,            // "Edit src/main.rs"
    detail: String,             // Full input/output
    status: EntryStatus,        // Running, Success, Failed
    affected_files: Vec<PathBuf>,
}
```

---

## Editor Core

### Text buffer

`ropey::Rope` — O(log n) inserts/deletes, efficient line indexing.

### Undo/redo

Operation-based. Each edit: `(position, deleted_text, inserted_text)`. Grouped by transaction (typing a word = one undo step).

### Selections

```rust
pub struct Selection {
    anchor: Position,
    head: Position,
}
pub struct Position {
    line: usize,
    col: usize,
}
```

Multi-cursor from day one.

### Keybindings (non-modal)

| Key | Action |
|---|---|
| `Ctrl+S` | Save |
| `Ctrl+Q` | Quit |
| `Ctrl+F` | Find |
| `Ctrl+G` | Go to line |
| `Ctrl+Z` / `Ctrl+Y` | Undo / Redo |
| `Ctrl+D` | Select next occurrence (multi-cursor) |
| `Ctrl+P` | Fuzzy file picker |
| `Ctrl+Shift+P` | Command palette |
| `Ctrl+\`` | Toggle bottom panel |
| `Ctrl+B` | Toggle sidebar |
| `Ctrl+J` | Focus terminal tab |
| `Ctrl+L` | Focus Claude log tab |
| `Ctrl+Tab` | Next open file tab |
| `Alt+↑/↓` | Move line up/down |
| `Ctrl+/` | Toggle comment |
| `Ctrl+Shift+D` | Toggle ghost lines (deleted) |
| `Ctrl+Shift+R` | Clear Claude diff markers (reviewed) |
| `Alt+]` / `Alt+[` | Next/prev Claude change hunk |
| `Ctrl+Shift+B` | Build command palette |
| `Ctrl+?` / `F1` | Keybinding cheatsheet overlay |

### Syntax highlighting

Tree-sitter on a background tokio task. Incremental re-parse on buffer edits. Rust and Go grammars compiled in. Others loaded from `~/.xpar/grammars/`.

---

## Embedded Terminal

```rust
pub struct TerminalPane {
    pty: portable_pty::Child,
    parser: vt100::Parser,
    screen: Vec<Vec<Cell>>,
    scroll_offset: usize,
    size: (u16, u16),
}
```

- Spawns `$SHELL` on startup, stays alive for the session.
- ANSI escape parsing via `vt100` crate.
- Resizes with panel. Mouse support for scrollback.
- On shell exit: "Shell exited. Press Enter to restart."
- On editor quit: sends SIGHUP to child.

---

## CLI

```bash
xpar-ide .                                          # Open directory
xpar-ide . --watch ~/.xpar/sessions/current.jsonl   # Open + watch Claude stream
xpar-ide src/main.rs                                # Open specific file
```

---

## Dependencies

| Crate | Purpose |
|---|---|
| `ratatui` + `crossterm` | TUI rendering + terminal backend |
| `ropey` | Rope text buffer |
| `tree-sitter` + `tree-sitter-rust` + `tree-sitter-go` | Syntax highlighting |
| `tokio` | Async runtime |
| `portable-pty` | PTY for embedded terminal |
| `vt100` | ANSI escape parsing |
| `notify` | Filesystem watcher |
| `serde` + `serde_json` | Parse Claude stream-json |
| `similar` | Diff algorithm (Myers) |
| `dirs` | Platform config paths |

---

## Config

Location: `~/.config/xpar-ide/config.toml`

Covers: theme, keybinding overrides, default shell, grammar paths.

---

## Verification

1. **Unit tests** for `core` — buffer ops, undo/redo, diff computation
2. **Integration tests** for `integrations::claude` — feed recorded `.jsonl`, assert CommandLog state
3. **Integration tests** for `integrations::terminal` — spawn PTY, send commands, verify screen buffer
4. **Manual TUI testing** — launch editor, verify rendering, keybindings, panel switching
5. **Memory profiling** — `heaptrack` or `dhat`, verify <50MB RSS under typical workload (5-10 files, terminal, Claude stream)
6. **Syntax highlighting benchmarks** — <16ms per incremental re-parse


-- this is for the test to see if the editor works 
