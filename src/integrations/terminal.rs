use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::thread;

pub struct EmbeddedTerminal {
    writer: Box<dyn Write + Send>,
    screen: Arc<Mutex<vt100::Parser>>,
    cols: u16,
    rows: u16,
    alive: Arc<Mutex<bool>>,
}

impl EmbeddedTerminal {
    pub fn new(cols: u16, rows: u16) -> Result<Self, Box<dyn std::error::Error>> {
        let pty_system = native_pty_system();

        let pair = pty_system.openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;

        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
        let mut cmd = CommandBuilder::new(&shell);
        cmd.env("TERM", "xterm-256color");

        let _child = pair.slave.spawn_command(cmd)?;
        drop(pair.slave);

        let mut reader = pair.master.try_clone_reader()?;
        let writer = pair.master.take_writer()?;

        let screen = Arc::new(Mutex::new(vt100::Parser::new(rows, cols, 200)));
        let screen_clone = Arc::clone(&screen);
        let alive = Arc::new(Mutex::new(true));
        let alive_clone = Arc::clone(&alive);

        thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => {
                        *alive_clone.lock().unwrap() = false;
                        break;
                    }
                    Ok(n) => {
                        screen_clone.lock().unwrap().process(&buf[..n]);
                    }
                    Err(_) => {
                        *alive_clone.lock().unwrap() = false;
                        break;
                    }
                }
            }
        });

        Ok(Self {
            writer,
            screen,
            cols,
            rows,
            alive,
        })
    }

    pub fn write_input(&mut self, data: &[u8]) {
        let _ = self.writer.write_all(data);
        let _ = self.writer.flush();
    }

    pub fn is_alive(&self) -> bool {
        *self.alive.lock().unwrap()
    }

    pub fn screen_lines(&self) -> Vec<String> {
        let screen = self.screen.lock().unwrap();
        let s = screen.screen();
        let mut lines: Vec<String> = s.rows(0, self.cols).collect();

        while lines.last().map_or(false, |l| l.is_empty()) {
            lines.pop();
        }

        lines
    }

    pub fn cursor_position(&self) -> (u16, u16) {
        let screen = self.screen.lock().unwrap();
        let s = screen.screen();
        s.cursor_position()
    }

    pub fn resize(&mut self, cols: u16, rows: u16) {
        self.cols = cols;
        self.rows = rows;
        self.screen.lock().unwrap().set_size(rows, cols);
    }
}
