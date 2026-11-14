//! Filesystem helpers: bounded reads and atomic writes.

use crate::error::SelahError;
use std::io::{Read, Write};
use std::path::Path;

/// Read a regular file, refusing anything larger than `max_bytes`.
///
/// Rejects directories, FIFOs and device files *before* opening them, so a
/// path like `/dev/zero` or a named pipe cannot exhaust memory or block the
/// caller indefinitely. The read itself is also capped, in case the file
/// grows between the size check and the read.
pub(crate) fn read_limited(path: &Path, max_bytes: u64) -> Result<Vec<u8>, SelahError> {
    let meta = std::fs::metadata(path)?;
    if !meta.is_file() {
        return Err(SelahError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("{} is not a regular file", path.display()),
        )));
    }
    if meta.len() > max_bytes {
        return Err(too_large(path, meta.len(), max_bytes));
    }

    let file = std::fs::File::open(path)?;
    // Capacity hint is bounded by `max_bytes`, which the caller controls.
    let mut buf = Vec::with_capacity(usize::try_from(meta.len()).unwrap_or(0));
    file.take(max_bytes.saturating_add(1))
        .read_to_end(&mut buf)?;
    if buf.len() as u64 > max_bytes {
        return Err(too_large(path, buf.len() as u64, max_bytes));
    }
    Ok(buf)
}

fn too_large(path: &Path, len: u64, max: u64) -> SelahError {
    SelahError::Io(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        format!(
            "{} is too large ({len} bytes, limit {max} bytes)",
            path.display()
        ),
    ))
}

/// Atomically replace `path` with `data`.
///
/// Writes to a uniquely-named temporary file in the same directory, flushes
/// it to disk, then renames it over the target, so readers never observe a
/// truncated or half-written file and a crash mid-write leaves the original
/// intact. On Unix the file is created with mode `0o600`.
pub(crate) fn write_atomic(path: &Path, data: &[u8]) -> Result<(), SelahError> {
    let dir = match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    };
    let file_name = path.file_name().ok_or_else(|| {
        SelahError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("{} has no file name", path.display()),
        ))
    })?;
    let mut tmp_name = std::ffi::OsString::from(".");
    tmp_name.push(file_name);
    tmp_name.push(format!(".tmp-{}", uuid::Uuid::new_v4().simple()));
    let tmp_path = dir.join(tmp_name);

    let result = (|| {
        let mut file = private_file_options().create_new(true).open(&tmp_path)?;
        file.write_all(data)?;
        file.sync_all()?;
        std::fs::rename(&tmp_path, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp_path);
    }
    result.map_err(SelahError::Io)
}

/// `OpenOptions` for write access that create files readable only by the
/// current user on Unix (`0o600`). Screenshot metadata can be sensitive.
pub(crate) fn private_file_options() -> std::fs::OpenOptions {
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    opts
}

/// Create `dir` (and parents), restricting the leaf directory to the
/// current user on Unix (`0o700`) when it is newly created.
pub(crate) fn create_private_dir_all(dir: &Path) -> Result<(), SelahError> {
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(dir).map_err(SelahError::Io)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("selah_fsutil_{name}_{}", uuid::Uuid::new_v4()))
    }

    #[test]
    fn test_read_limited_ok() {
        let p = temp_path("ok");
        std::fs::write(&p, b"hello").unwrap();
        assert_eq!(read_limited(&p, 5).unwrap(), b"hello");
        std::fs::remove_file(&p).ok();
    }

    #[test]
    fn test_read_limited_too_large() {
        let p = temp_path("big");
        std::fs::write(&p, b"hello world").unwrap();
        let err = read_limited(&p, 4).unwrap_err().to_string();
        assert!(err.contains("too large"), "{err}");
        std::fs::remove_file(&p).ok();
    }

    #[test]
    fn test_read_limited_rejects_directory() {
        let err = read_limited(&std::env::temp_dir(), 1 << 20)
            .unwrap_err()
            .to_string();
        assert!(err.contains("not a regular file"), "{err}");
    }

    #[cfg(unix)]
    #[test]
    fn test_read_limited_rejects_device() {
        // /dev/zero would otherwise be read forever
        let err = read_limited(Path::new("/dev/zero"), 1 << 20)
            .unwrap_err()
            .to_string();
        assert!(err.contains("not a regular file"), "{err}");
    }

    #[test]
    fn test_read_limited_missing() {
        assert!(read_limited(&temp_path("missing"), 10).is_err());
    }

    #[test]
    fn test_write_atomic_replaces_and_cleans_up() {
        let dir = temp_path("atomic_dir");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("file.txt");
        std::fs::write(&p, b"old contents").unwrap();
        write_atomic(&p, b"new").unwrap();
        assert_eq!(std::fs::read(&p).unwrap(), b"new");
        // No temp files left behind
        let entries: Vec<_> = std::fs::read_dir(&dir).unwrap().collect();
        assert_eq!(entries.len(), 1);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[cfg(unix)]
    #[test]
    fn test_write_atomic_private_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let p = temp_path("perm");
        write_atomic(&p, b"x").unwrap();
        let mode = std::fs::metadata(&p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        std::fs::remove_file(&p).ok();
    }
}
