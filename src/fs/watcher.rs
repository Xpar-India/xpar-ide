use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::mpsc;

pub struct FileWatcher {
    _watcher: RecommendedWatcher,
    pub receiver: mpsc::Receiver<notify::Result<Event>>,
}

impl FileWatcher {
    pub fn new(path: &Path) -> notify::Result<Self> {
        let (tx, rx) = mpsc::channel();
        let mut watcher = notify::recommended_watcher(tx)?;
        watcher.watch(path, RecursiveMode::Recursive)?;
        Ok(Self {
            _watcher: watcher,
            receiver: rx,
        })
    }
}
