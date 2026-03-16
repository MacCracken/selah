//! Screenshot history — persisted as JSON-lines.

use chrono::{DateTime, Utc};
use selah_core::SelahError;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

/// A single entry in the screenshot history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: Uuid,
    pub path: String,
    pub timestamp: DateTime<Utc>,
    pub source: String,
    pub width: u32,
    pub height: u32,
    pub format: String,
}

/// Persistent screenshot history store backed by a JSON-lines file.
#[derive(Debug, Clone)]
pub struct HistoryStore {
    path: PathBuf,
}

impl HistoryStore {
    /// Open or create a history store at the default location.
    ///
    /// Uses `$XDG_DATA_HOME/selah/history.jsonl` or `~/.local/share/selah/history.jsonl`.
    pub fn open_default() -> Result<Self, SelahError> {
        let data_dir = std::env::var("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
                PathBuf::from(home).join(".local/share")
            })
            .join("selah");

        std::fs::create_dir_all(&data_dir)?;
        let path = data_dir.join("history.jsonl");
        Ok(Self { path })
    }

    /// Open a history store at a specific path (useful for testing).
    pub fn open(path: PathBuf) -> Self {
        Self { path }
    }

    /// Record a new capture in the history.
    pub fn record(&self, entry: HistoryEntry) -> Result<(), SelahError> {
        use std::io::Write;
        let line = serde_json::to_string(&entry)
            .map_err(|e| SelahError::CaptureFailed(format!("failed to serialize history entry: {e}")))?;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        writeln!(file, "{line}")?;
        Ok(())
    }

    /// List recent captures, newest first.
    pub fn list(&self, limit: usize, since: Option<DateTime<Utc>>) -> Result<Vec<HistoryEntry>, SelahError> {
        let content = match std::fs::read_to_string(&self.path) {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(SelahError::Io(e)),
        };

        let mut entries: Vec<HistoryEntry> = content
            .lines()
            .filter(|l| !l.trim().is_empty())
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect();

        if let Some(since) = since {
            entries.retain(|e| e.timestamp >= since);
        }

        // Newest first
        entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        entries.truncate(limit);
        Ok(entries)
    }

    /// Get a specific entry by ID.
    pub fn get(&self, id: Uuid) -> Result<Option<HistoryEntry>, SelahError> {
        let content = match std::fs::read_to_string(&self.path) {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(SelahError::Io(e)),
        };

        let entry = content
            .lines()
            .filter(|l| !l.trim().is_empty())
            .filter_map(|l| serde_json::from_str::<HistoryEntry>(l).ok())
            .find(|e| e.id == id);

        Ok(entry)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store() -> HistoryStore {
        let path = std::env::temp_dir().join(format!("selah_test_history_{}.jsonl", Uuid::new_v4()));
        HistoryStore::open(path)
    }

    fn make_entry(source: &str) -> HistoryEntry {
        HistoryEntry {
            id: Uuid::new_v4(),
            path: "/tmp/test.png".to_string(),
            timestamp: Utc::now(),
            source: source.to_string(),
            width: 1920,
            height: 1080,
            format: "png".to_string(),
        }
    }

    #[test]
    fn test_record_and_list() {
        let store = temp_store();
        store.record(make_entry("full screen")).unwrap();
        store.record(make_entry("region")).unwrap();

        let entries = store.list(10, None).unwrap();
        assert_eq!(entries.len(), 2);
        // Cleanup
        std::fs::remove_file(&store.path).ok();
    }

    #[test]
    fn test_list_empty() {
        let store = temp_store();
        let entries = store.list(10, None).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_list_with_limit() {
        let store = temp_store();
        for i in 0..5 {
            store.record(make_entry(&format!("capture {i}"))).unwrap();
        }
        let entries = store.list(3, None).unwrap();
        assert_eq!(entries.len(), 3);
        std::fs::remove_file(&store.path).ok();
    }

    #[test]
    fn test_get_by_id() {
        let store = temp_store();
        let entry = make_entry("test");
        let id = entry.id;
        store.record(entry).unwrap();

        let found = store.get(id).unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, id);

        let not_found = store.get(Uuid::new_v4()).unwrap();
        assert!(not_found.is_none());
        std::fs::remove_file(&store.path).ok();
    }

    #[test]
    fn test_list_since() {
        let store = temp_store();
        let before = Utc::now();
        store.record(make_entry("old")).unwrap();
        // All entries created now should be >= before
        let entries = store.list(10, Some(before)).unwrap();
        assert_eq!(entries.len(), 1);
        std::fs::remove_file(&store.path).ok();
    }
}
