//! Replacing the output file safely, as `_atomic_write` and `_create_temporary`
//! in `simulacra.py` do: write a new temporary file next to the destination,
//! then rename it over the destination, so a failed write never leaves a
//! partial image and an existing file stays untouched.
//!
//! The temporary file is created with mode 0o666 so the umask applies exactly
//! as for a normal file. (`tempfile::NamedTempFile` would create it with 0o600
//! and keep that on rename, leaving images readable only by their owner.)

use std::fs::{self, File, OpenOptions};
use std::hash::{BuildHasher, Hasher};
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};

/// Creates `.simulacra-<16 hex digits>.tmp` in `directory`, exclusively.
fn create_temporary(directory: &Path) -> io::Result<(PathBuf, File)> {
    for _ in 0..100 {
        let bits = std::collections::hash_map::RandomState::new()
            .build_hasher()
            .finish();
        let path = directory.join(format!(".simulacra-{bits:016x}.tmp"));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o666);
        match options.open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {}
            Err(err) => return Err(err),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        format!(
            "could not create a unique temporary file in {}",
            directory.display()
        ),
    ))
}

/// Writes `bytes` to `path` through a temporary file and a rename. On any
/// error the temporary file is removed and an existing `path` is unchanged.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let absolute = std::path::absolute(path)?;
    let directory = absolute.parent().unwrap_or(Path::new("."));
    let (temporary, mut file) = create_temporary(directory)?;
    let written = file.write_all(bytes).and_then(|()| file.flush());
    // Close the file before renaming or removing it: Windows refuses both while
    // a handle is open, which would leak the temporary file after a failure.
    drop(file);
    let result = written.and_then(|()| fs::rename(&temporary, &absolute));
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::atomic_write;
    use std::fs;

    fn scratch_dir(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("simulacra-cli-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn replaces_the_file_and_leaves_no_temporary() {
        let dir = scratch_dir("replace");
        let path = dir.join("out.png");
        fs::write(&path, b"old").unwrap();
        atomic_write(&path, b"new").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"new");
        let names: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, ["out.png"]);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_failed_write_leaves_the_destination_and_no_temporary() {
        let dir = scratch_dir("fail");
        // Renaming a file over a non-empty directory fails.
        let target = dir.join("busy");
        fs::create_dir(&target).unwrap();
        fs::write(target.join("keep"), b"x").unwrap();
        assert!(atomic_write(&target, b"data").is_err());
        assert_eq!(fs::read(target.join("keep")).unwrap(), b"x");
        let names: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, ["busy"]);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn new_files_get_the_umask_default_mode() {
        use std::os::unix::fs::PermissionsExt;
        let dir = scratch_dir("mode");
        let path = dir.join("out.png");
        atomic_write(&path, b"x").unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        // 0o666 with the process umask applied: under the usual umasks (0o022,
        // 0o002) group and others can read it, unlike tempfile's 0o600. (Reading
        // the umask itself would change it for every thread of the test run.)
        assert_eq!(mode & 0o111, 0, "mode {mode:o} is executable");
        assert_ne!(mode & 0o044, 0, "mode {mode:o} is owner-only");
        fs::remove_dir_all(&dir).unwrap();
    }
}
