//! In-memory command history ring, with optional file persistence.

use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

const DEFAULT_CAPACITY: usize = 1000;

#[derive(Debug, Default)]
pub struct History {
    entries: Vec<String>,
    capacity: usize,
}

impl History {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            capacity: DEFAULT_CAPACITY,
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Oldest = 0, newest = len-1.
    pub fn get(&self, index: usize) -> Option<&str> {
        self.entries.get(index).map(String::as_str)
    }

    /// Push a non-empty line; skip if it duplicates the newest entry.
    pub fn push(&mut self, line: impl Into<String>) {
        let line = line.into();
        if line.trim().is_empty() {
            return;
        }
        if self.entries.last().is_some_and(|last| last == &line) {
            return;
        }
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push(line);
    }

    pub fn load_from_path(&mut self, path: &Path) -> io::Result<()> {
        let file = match fs::File::open(path) {
            Ok(f) => f,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(err) => return Err(err),
        };
        for line in io::BufReader::new(file).lines() {
            let line = line?;
            self.push(line);
        }
        Ok(())
    }

    pub fn append_to_path(&self, path: &Path, line: &str) -> io::Result<()> {
        if line.trim().is_empty() {
            return Ok(());
        }
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let mut file = OpenOptions::new().create(true).append(true).open(path)?;
        writeln!(file, "{line}")?;
        Ok(())
    }
}

pub fn default_history_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join(".0shell_history"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_skips_empty_and_duplicates() {
        let mut h = History::new();
        h.push("echo a");
        h.push("echo a");
        h.push("   ");
        h.push("echo b");
        assert_eq!(h.len(), 2);
        assert_eq!(h.get(0), Some("echo a"));
        assert_eq!(h.get(1), Some("echo b"));
    }

    #[test]
    fn ring_evicts_oldest_when_full() {
        let mut h = History {
            entries: Vec::new(),
            capacity: 2,
        };
        h.push("one");
        h.push("two");
        h.push("three");
        assert_eq!(h.len(), 2);
        assert_eq!(h.get(0), Some("two"));
        assert_eq!(h.get(1), Some("three"));
    }

    #[test]
    fn load_and_append_round_trip() {
        let dir = std::env::temp_dir().join(format!("0-shell-history-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("hist");

        let mut h = History::new();
        h.push("first");
        h.append_to_path(&path, "first").unwrap();
        h.append_to_path(&path, "second").unwrap();

        let mut loaded = History::new();
        loaded.load_from_path(&path).unwrap();
        assert_eq!(loaded.get(0), Some("first"));
        assert_eq!(loaded.get(1), Some("second"));

        let _ = fs::remove_dir_all(&dir);
    }
}
