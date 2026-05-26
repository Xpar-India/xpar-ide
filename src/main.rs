mod app;
mod core;
mod fs;
mod integrations;
mod tui;

use std::path::PathBuf;

use app::App;
use integrations::claude::ClaudeSession;

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    let mut project_path: Option<String> = None;
    let mut watch_path: Option<PathBuf> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--watch" => {
                if i + 1 < args.len() {
                    watch_path = Some(PathBuf::from(&args[i + 1]));
                    i += 1;
                }
            }
            _ => {
                if project_path.is_none() {
                    project_path = Some(args[i].clone());
                }
            }
        }
        i += 1;
    }

    let project_dir = project_path.as_ref().map(|s| {
        let path = PathBuf::from(s);
        let path = if path.is_relative() {
            std::env::current_dir().unwrap_or_default().join(path)
        } else {
            path
        };
        let path = path.canonicalize().unwrap_or(path);
        if path.is_dir() {
            path
        } else if path.is_file() {
            path.parent().map(|p| p.to_path_buf()).unwrap_or(path)
        } else {
            path
        }
    });

    let claude_session = if let Some(wp) = watch_path {
        let mut session = ClaudeSession::new();
        session.watch(wp);
        Some(session)
    } else if let Some(dir) = &project_dir {
        ClaudeSession::auto_detect(dir)
    } else if let Ok(cwd) = std::env::current_dir() {
        ClaudeSession::auto_detect(&cwd)
    } else {
        None
    };

    let mut app = App::new(project_dir, claude_session);

    if let Some(s) = &project_path {
        let path = PathBuf::from(s);
        let path = if path.is_relative() {
            std::env::current_dir()?.join(path)
        } else {
            path
        };
        if path.is_file() {
            let contents = std::fs::read_to_string(&path)?;
            let buf = crate::core::buffer::Buffer::from_file(path, &contents);
            let sels = crate::core::selections::Selections::new(
                crate::core::selections::Position::zero(),
            );
            app.buffers.push(buf);
            app.selections.push(sels);
            app.scroll_offsets.push(0);
            app.active_buffer = app.buffers.len() - 1;
        }
    }

    crossterm::execute!(std::io::stdout(), crossterm::event::EnableMouseCapture)?;
    let terminal = ratatui::init();
    let result = app.run(terminal);
    ratatui::restore();
    crossterm::execute!(std::io::stdout(), crossterm::event::DisableMouseCapture)?;
    result
}
