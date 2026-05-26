# xpar.IDE

A lightweight Rust TUI code editor purpose-built for the Claude Code workflow.

## Quick Start

```bash
cargo run -- .                    # Open current directory
cargo run -- . --watch file.jsonl # Open with Claude session file
```

## Build & Test

```bash
cargo build          # Debug build
cargo test           # Run tests (35 tests)
cargo clippy         # Lint
cargo install --path . # Install to ~/.cargo/bin/xpar-ide
```

## Architecture

Layered monolith: `core/` (buffer, undo, selections) -> `tui/` (rendering, input) -> `integrations/` (claude, terminal, tree-sitter) -> `fs/` (file tree, watcher).

## Key Directories

- `src/core/` — Editor engine (no TUI dependency): buffer.rs, history.rs, selections.rs
- `src/tui/` — Rendering and input: layout.rs, editor_view.rs, sidebar.rs, bottom_panel.rs, input.rs, menu_bar.rs, tab_bar.rs, statusbar.rs
- `src/integrations/` — External adapters: claude.rs (stream-json parser), terminal.rs (PTY), treesitter.rs (syntax)
- `src/fs/` — File operations: tree.rs (directory model), loader.rs, watcher.rs
- `src/app.rs` — App state machine, event loop, all wiring

## Conventions

- Non-modal editing (Nano/Micro style, not Vim)
- Event-driven TUI loop with 100ms poll for Claude session updates
- Tree-sitter grammars compiled into binary (Rust, Go, JS/TS, Python, JSON, TOML)
- Claude session auto-detected from ~/.claude/projects/
