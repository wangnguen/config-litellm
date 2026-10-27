//! JSON files that may not exist yet. A missing file reads as `None`, never
//! as an error, and a write lands whole or not at all: it goes to a sibling
//! `.tmp` file first and is renamed over the old one.

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::Path;

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::{Error, Result};

/// Read and parse `path`; `None` when there is no such file or it is empty.
pub fn read_json<T: DeserializeOwned>(path: &Path) -> Result<Option<T>> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(Error::io(path, e)),
    };
    // Notepad used to start UTF-8 files with a BOM, which is not JSON.
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    if text.trim().is_empty() {
        return Ok(None);
    }
    serde_json::from_str(text)
        .map(Some)
        .map_err(|source| Error::Json {
            path: path.to_path_buf(),
            source,
        })
}

/// Write `value` as pretty JSON. A `private` file (it holds a key) is made
/// readable by its owner only on Unix; on Windows the profile already is.
pub fn write_json<T: Serialize>(path: &Path, value: &T, private: bool) -> Result<()> {
    let mut text = serde_json::to_string_pretty(value).map_err(|source| Error::Json {
        path: path.to_path_buf(),
        source,
    })?;
    text.push('\n');
    write_atomic(path, text.as_bytes(), private)
}

fn write_atomic(path: &Path, bytes: &[u8], private: bool) -> Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| Error::io(dir, e))?;
    }
    let tmp = path.with_extension("tmp");
    // A stale one would keep whatever permissions it was created with.
    let _ = fs::remove_file(&tmp);
    let written = create(&tmp, private).and_then(|mut file| {
        file.write_all(bytes)?;
        file.sync_all()
    });
    if let Err(e) = written {
        let _ = fs::remove_file(&tmp);
        return Err(Error::io(&tmp, e));
    }
    fs::rename(&tmp, path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        Error::io(path, e)
    })
}

#[cfg(unix)]
fn create(path: &Path, private: bool) -> io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt;
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(if private { 0o600 } else { 0o644 })
        .open(path)
}

#[cfg(not(unix))]
fn create(path: &Path, _private: bool) -> io::Result<File> {
    File::create(path)
}
