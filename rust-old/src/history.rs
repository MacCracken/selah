//! Screenshot history — persisted as JSON-lines.

use crate::SelahError;
use crate::fsutil;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::cmp::{Ordering, Reverse};
use std::collections::BinaryHeap;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use uuid::Uuid;

/// How long writers wait for the history lock before giving up.
const LOCK_TIMEOUT: Duration = Duration::from_secs(5);
const LOCK_RETRY: Duration = Duration::from_millis(10);

/// Exclusive advisory lock on `<history>.lock`, held by writers.
///
/// Uses OS file locking (`flock` / `LockFileEx`), which the kernel releases
/// when the handle is dropped *or the process dies*, so a crash can never
/// leave a stale lock behind. Readers do not lock: rewrites are atomic
/// renames and appends are single writes, so a reader always sees a
/// consistent snapshot (at worst a torn final line, which is skipped).
struct HistoryLock {
    _file: std::fs::File,
}

impl HistoryLock {
    fn acquire(path: &Path) -> Result<Self, SelahError> {
        let file = fsutil::private_file_options()
            .create(true)
            .truncate(false)
            .open(path)?;
        let deadline = Instant::now() + LOCK_TIMEOUT;
        loop {
            match file.try_lock() {
                Ok(()) => return Ok(Self { _file: file }),
                Err(std::fs::TryLockError::WouldBlock) if Instant::now() < deadline => {
                    std::thread::sleep(LOCK_RETRY);
                }
                Err(std::fs::TryLockError::WouldBlock) => {
                    return Err(SelahError::Io(std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        "timed out waiting for history lock",
                    )));
                }
                Err(std::fs::TryLockError::Error(e)) => return Err(SelahError::Io(e)),
            }
        }
    }
}

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

/// Just the ID, for cheap scans that don't need the full entry.
#[derive(Deserialize)]
struct EntryId {
    id: Uuid,
}

/// Heap key for `list`: newest first, ties broken by file order.
struct Ranked {
    key: (DateTime<Utc>, Reverse<usize>),
    entry: HistoryEntry,
}

impl PartialEq for Ranked {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}
impl Eq for Ranked {}
impl PartialOrd for Ranked {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Ranked {
    fn cmp(&self, other: &Self) -> Ordering {
        self.key.cmp(&other.key)
    }
}

/// Persistent screenshot history store backed by a JSON-lines file.
#[derive(Debug, Clone)]
pub struct HistoryStore {
    path: PathBuf,
}

/// Resolve `$XDG_DATA_HOME/selah` or `$HOME/.local/share/selah`.
///
/// Per the XDG spec, empty or relative values are ignored.
fn default_data_dir(
    xdg_data_home: Option<std::ffi::OsString>,
    home: Option<std::ffi::OsString>,
) -> Option<PathBuf> {
    let absolute = |v: std::ffi::OsString| Some(PathBuf::from(v)).filter(|p| p.is_absolute());
    xdg_data_home
        .and_then(absolute)
        .or_else(|| home.and_then(absolute).map(|h| h.join(".local/share")))
        .map(|d| d.join("selah"))
}

impl HistoryStore {
    /// Open or create a history store at the default location.
    ///
    /// Uses `$XDG_DATA_HOME/selah/history.jsonl` or `~/.local/share/selah/history.jsonl`.
    /// On Unix the directory is created with mode `0o700` and the history
    /// file with `0o600`, since it records where screenshots are stored.
    pub fn open_default() -> Result<Self, SelahError> {
        let data_dir = default_data_dir(
            std::env::var_os("XDG_DATA_HOME"),
            std::env::var_os("HOME"),
        )
        .ok_or_else(|| {
            SelahError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "cannot locate data directory: neither XDG_DATA_HOME nor HOME is an absolute path",
            ))
        })?;
        fsutil::create_private_dir_all(&data_dir)?;
        Ok(Self {
            path: data_dir.join("history.jsonl"),
        })
    }

    /// Open a history store at a specific path (useful for testing).
    ///
    /// # Example
    ///
    /// ```
    /// use selah::HistoryStore;
    ///
    /// let store = HistoryStore::open(std::env::temp_dir().join("selah_doctest.jsonl"));
    /// let entries = store.list(10, None).unwrap();
    /// // Fresh store has no entries
    /// assert!(entries.is_empty());
    /// ```
    #[must_use]
    pub fn open(path: PathBuf) -> Self {
        Self { path }
    }

    /// Path of the backing JSON-lines file.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    fn lock_path(&self) -> PathBuf {
        let mut p = self.path.clone().into_os_string();
        p.push(".lock");
        PathBuf::from(p)
    }

    /// Open the history file for reading; `None` if it does not exist yet.
    fn open_reader(&self) -> Result<Option<BufReader<std::fs::File>>, SelahError> {
        match std::fs::File::open(&self.path) {
            Ok(f) => Ok(Some(BufReader::new(f))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(SelahError::Io(e)),
        }
    }

    /// Visit each non-empty line (as raw bytes). Lines that are not valid
    /// UTF-8 JSON are the caller's to skip, so one corrupt line never makes
    /// the whole history unreadable.
    fn for_each_line(
        &self,
        mut f: impl FnMut(&[u8]) -> std::ops::ControlFlow<()>,
    ) -> Result<(), SelahError> {
        let Some(mut reader) = self.open_reader()? else {
            return Ok(());
        };
        let mut line = Vec::new();
        loop {
            line.clear();
            if reader.read_until(b'\n', &mut line)? == 0 {
                return Ok(());
            }
            let trimmed = line.trim_ascii();
            if !trimmed.is_empty() && f(trimmed).is_break() {
                return Ok(());
            }
        }
    }

    /// Record a new capture in the history.
    pub fn record(&self, entry: HistoryEntry) -> Result<(), SelahError> {
        let mut line = serde_json::to_vec(&entry).map_err(|e| {
            SelahError::CaptureFailed(format!("failed to serialize history entry: {e}"))
        })?;
        line.push(b'\n');

        // Serialize with `delete` so an append can't land on a file that is
        // about to be replaced (which would silently drop the entry).
        let _lock = HistoryLock::acquire(&self.lock_path())?;
        let mut file = fsutil::private_file_options()
            .read(true)
            .append(true)
            .create(true)
            .open(&self.path)?;
        // If an earlier append was torn (crash mid-write, no trailing
        // newline), terminate it so this entry isn't glued onto it and lost.
        if file.metadata()?.len() > 0 {
            let mut last = [0u8; 1];
            file.seek(SeekFrom::End(-1))?;
            file.read_exact(&mut last)?;
            if last[0] != b'\n' {
                line.insert(0, b'\n');
            }
        }
        // Append mode: the write lands at the end regardless of the seek.
        file.write_all(&line)?;
        tracing::debug!(id = %entry.id, path = %entry.path, "recorded history entry");
        Ok(())
    }

    /// List recent captures, newest first.
    ///
    /// Streams the file and keeps only the `limit` newest entries in memory.
    /// Malformed lines are skipped.
    pub fn list(
        &self,
        limit: usize,
        since: Option<DateTime<Utc>>,
    ) -> Result<Vec<HistoryEntry>, SelahError> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        // Min-heap of the `limit` best entries seen so far.
        let mut heap: BinaryHeap<Reverse<Ranked>> = BinaryHeap::with_capacity(limit.min(1024) + 1);
        let mut seq = 0usize;
        let mut skipped = 0usize;

        self.for_each_line(|line| {
            seq += 1;
            match serde_json::from_slice::<HistoryEntry>(line) {
                Ok(entry) if since.is_none_or(|s| entry.timestamp >= s) => {
                    heap.push(Reverse(Ranked {
                        key: (entry.timestamp, Reverse(seq)),
                        entry,
                    }));
                    if heap.len() > limit {
                        heap.pop();
                    }
                }
                Ok(_) => {}
                Err(_) => skipped += 1,
            }
            std::ops::ControlFlow::Continue(())
        })?;

        if skipped > 0 {
            tracing::warn!(skipped, path = %self.path.display(), "skipped malformed history lines");
        }
        // Ascending order of Reverse<Ranked> = descending Ranked = newest first.
        Ok(heap
            .into_sorted_vec()
            .into_iter()
            .map(|Reverse(r)| r.entry)
            .collect())
    }

    /// Delete a history entry by ID. Returns true if the entry was found and removed.
    ///
    /// Holds the history lock and rewrites the file atomically (temp file +
    /// rename), so concurrent writers and crashes cannot corrupt it.
    /// Malformed lines are preserved untouched.
    pub fn delete(&self, id: Uuid) -> Result<bool, SelahError> {
        let _lock = HistoryLock::acquire(&self.lock_path())?;

        let content = match std::fs::read(&self.path) {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(e) => return Err(SelahError::Io(e)),
        };

        let mut found = false;
        let mut remaining = Vec::with_capacity(content.len());
        for line in content.split(|&b| b == b'\n') {
            if line.trim_ascii().is_empty() {
                continue;
            }
            if serde_json::from_slice::<EntryId>(line).is_ok_and(|e| e.id == id) {
                found = true;
                continue;
            }
            remaining.extend_from_slice(line);
            remaining.push(b'\n');
        }

        if found {
            fsutil::write_atomic(&self.path, &remaining)?;
            tracing::debug!(%id, "deleted history entry");
        }
        Ok(found)
    }

    /// Get a specific entry by ID.
    pub fn get(&self, id: Uuid) -> Result<Option<HistoryEntry>, SelahError> {
        let mut found = None;
        self.for_each_line(|line| {
            // Cheap ID check first; only fully parse the match.
            if serde_json::from_slice::<EntryId>(line).is_ok_and(|e| e.id == id)
                && let Ok(entry) = serde_json::from_slice::<HistoryEntry>(line)
            {
                found = Some(entry);
                return std::ops::ControlFlow::Break(());
            }
            std::ops::ControlFlow::Continue(())
        })?;
        Ok(found)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A store in its own temp directory, removed (with its lock file) on drop.
    struct TempStore {
        store: HistoryStore,
        dir: PathBuf,
    }

    impl std::ops::Deref for TempStore {
        type Target = HistoryStore;
        fn deref(&self) -> &HistoryStore {
            &self.store
        }
    }

    impl Drop for TempStore {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    fn temp_store() -> TempStore {
        let dir = std::env::temp_dir().join(format!("selah_test_history_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        TempStore {
            store: HistoryStore::open(dir.join("history.jsonl")),
            dir,
        }
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
    }

    #[test]
    fn test_delete_entry() {
        let store = temp_store();
        let entry = make_entry("to-delete");
        let id = entry.id;
        store.record(make_entry("keep")).unwrap();
        store.record(entry).unwrap();
        store.record(make_entry("also-keep")).unwrap();

        assert!(store.delete(id).unwrap());
        let entries = store.list(10, None).unwrap();
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().all(|e| e.id != id));

        // Deleting again returns false
        assert!(!store.delete(id).unwrap());
    }

    #[test]
    fn test_delete_nonexistent() {
        let store = temp_store();
        store.record(make_entry("test")).unwrap();
        assert!(!store.delete(Uuid::new_v4()).unwrap());
    }

    #[test]
    fn test_delete_from_empty() {
        let store = temp_store();
        assert!(!store.delete(Uuid::new_v4()).unwrap());
    }

    #[test]
    fn test_list_since() {
        let store = temp_store();
        let before = Utc::now();
        store.record(make_entry("old")).unwrap();
        // All entries created now should be >= before
        let entries = store.list(10, Some(before)).unwrap();
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn test_record_and_get_roundtrip() {
        let store = temp_store();
        let entry = make_entry("roundtrip");
        let id = entry.id;
        store.record(entry).unwrap();
        let got = store.get(id).unwrap().unwrap();
        assert_eq!(got.source, "roundtrip");
        assert_eq!(got.width, 1920);
        assert_eq!(got.height, 1080);
    }

    #[test]
    fn test_list_returns_newest_first() {
        let store = temp_store();
        for i in 0..3 {
            let mut entry = make_entry(&format!("entry_{i}"));
            entry.timestamp = Utc::now() + chrono::Duration::seconds(i as i64);
            store.record(entry).unwrap();
        }
        let entries = store.list(10, None).unwrap();
        assert!(entries[0].timestamp >= entries[1].timestamp);
        assert!(entries[1].timestamp >= entries[2].timestamp);
    }

    #[test]
    fn test_lock_file_is_separate_and_persistent() {
        let store = temp_store();
        store.record(make_entry("a")).unwrap();
        let lock = store.lock_path();
        assert!(lock.to_string_lossy().ends_with(".jsonl.lock"));
        // A leftover lock file (e.g. after a crash) must not block writers:
        // only a live OS lock does.
        assert!(lock.exists());
        store.record(make_entry("b")).unwrap();
        assert!(store.delete(store.list(1, None).unwrap()[0].id).unwrap());
    }

    #[test]
    fn test_concurrent_record_and_delete_lose_nothing() {
        let store = temp_store();
        let doomed: Vec<_> = (0..10)
            .map(|i| make_entry(&format!("doomed {i}")))
            .collect();
        for e in &doomed {
            store.record(e.clone()).unwrap();
        }

        std::thread::scope(|s| {
            for t in 0..4 {
                let store = store.clone();
                s.spawn(move || {
                    for i in 0..25 {
                        store.record(make_entry(&format!("t{t} #{i}"))).unwrap();
                    }
                });
            }
            let store = store.clone();
            let doomed = &doomed;
            s.spawn(move || {
                for e in doomed {
                    assert!(store.delete(e.id).unwrap());
                }
            });
        });

        let entries = store.list(usize::MAX, None).unwrap();
        assert_eq!(entries.len(), 100, "appends lost during concurrent deletes");
        assert!(entries.iter().all(|e| !e.source.starts_with("doomed")));
    }

    #[test]
    fn test_list_skips_malformed_and_invalid_utf8_lines() {
        let store = temp_store();
        let good = make_entry("good");
        let mut content = serde_json::to_vec(&good).unwrap();
        content.extend_from_slice(b"\n{not json}\n\xff\xfe garbage\n\n");
        std::fs::write(&store.path, &content).unwrap();

        let entries = store.list(10, None).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, good.id);
        assert_eq!(store.get(good.id).unwrap().unwrap().source, "good");

        // Delete keeps the malformed lines it can't interpret
        assert!(store.delete(good.id).unwrap());
        let after = std::fs::read(&store.path).unwrap();
        assert!(after.windows(10).any(|w| w == b"{not json}"));
        assert!(store.list(10, None).unwrap().is_empty());
    }

    #[test]
    fn test_list_limit_keeps_newest_with_stable_ties() {
        let store = temp_store();
        let base = Utc::now();
        // Insert out of order; two entries share a timestamp
        for (i, offset) in [3i64, 1, 5, 5, 2].into_iter().enumerate() {
            let mut e = make_entry(&format!("e{i}"));
            e.timestamp = base + chrono::Duration::seconds(offset);
            store.record(e).unwrap();
        }
        let top: Vec<_> = store
            .list(3, None)
            .unwrap()
            .into_iter()
            .map(|e| e.source)
            .collect();
        // Newest first; equal timestamps keep file order
        assert_eq!(top, ["e2", "e3", "e0"]);
        assert!(store.list(0, None).unwrap().is_empty());
    }

    #[test]
    fn test_list_since_filters_older() {
        let store = temp_store();
        let now = Utc::now();
        let mut old = make_entry("old");
        old.timestamp = now - chrono::Duration::hours(1);
        store.record(old).unwrap();
        store.record(make_entry("new")).unwrap();
        let entries = store
            .list(10, Some(now - chrono::Duration::minutes(1)))
            .unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].source, "new");
    }

    #[cfg(unix)]
    #[test]
    fn test_history_file_is_private() {
        use std::os::unix::fs::PermissionsExt;
        let store = temp_store();
        store.record(make_entry("x")).unwrap();
        let mode = std::fs::metadata(&store.path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        // Still private after an atomic rewrite
        let keep = make_entry("keep");
        store.record(keep).unwrap();
        let first = store.list(10, None).unwrap().pop().unwrap();
        assert!(store.delete(first.id).unwrap());
        let mode = std::fs::metadata(&store.path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }

    #[test]
    fn test_record_after_torn_line_is_not_lost() {
        let store = temp_store();
        let first = make_entry("first");
        let mut content = serde_json::to_vec(&first).unwrap();
        content.extend_from_slice(b"\n{\"id\":\"0000"); // torn append, no newline
        std::fs::write(&store.path, &content).unwrap();

        let next = make_entry("next");
        store.record(next.clone()).unwrap();
        let ids: Vec<_> = store
            .list(10, None)
            .unwrap()
            .into_iter()
            .map(|e| e.id)
            .collect();
        assert!(ids.contains(&first.id));
        assert!(
            ids.contains(&next.id),
            "entry appended after a torn line was lost"
        );
        assert!(store.get(next.id).unwrap().is_some());
    }

    #[test]
    fn test_default_data_dir_resolution() {
        use std::ffi::OsString;
        let xdg = Some(OsString::from("/xdg"));
        let home = Some(OsString::from("/home/u"));
        assert_eq!(
            default_data_dir(xdg.clone(), home.clone()),
            Some(PathBuf::from("/xdg/selah"))
        );
        assert_eq!(
            default_data_dir(None, home.clone()),
            Some(PathBuf::from("/home/u/.local/share/selah"))
        );
        // Empty or relative XDG_DATA_HOME is ignored per the XDG spec
        assert_eq!(
            default_data_dir(Some(OsString::new()), home.clone()),
            Some(PathBuf::from("/home/u/.local/share/selah"))
        );
        assert_eq!(
            default_data_dir(Some(OsString::from("rel/dir")), home),
            Some(PathBuf::from("/home/u/.local/share/selah"))
        );
        // No usable location at all
        assert_eq!(default_data_dir(None, Some(OsString::from("."))), None);
        assert_eq!(default_data_dir(None, None), None);
    }
}
