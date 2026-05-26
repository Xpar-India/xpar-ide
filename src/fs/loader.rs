use std::fs;
use std::io;
use std::path::Path;

pub fn load_file(path: &Path) -> io::Result<String> {
    fs::read_to_string(path)
}

pub fn save_file(path: &Path, contents: &str) -> io::Result<()> {
    fs::write(path, contents)
}
