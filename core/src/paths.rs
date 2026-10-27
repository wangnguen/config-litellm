//! Where ccl keeps its own files, and where Claude Code reads its settings.

use std::env;
use std::path::{Path, PathBuf};

use crate::{Error, Result};

/// Replaces `~/.ccl` when set; the tests point it at a scratch folder.
pub const HOME_OVERRIDE: &str = "CCL_HOME";

pub fn user_home() -> Result<PathBuf> {
    let var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    env::var_os(var)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .ok_or(Error::NoHome)
}

/// `~/.ccl`: config, keys, the settings template and the installed binary.
pub fn app_dir() -> Result<PathBuf> {
    if let Some(dir) = env::var_os(HOME_OVERRIDE).filter(|v| !v.is_empty()) {
        return Ok(PathBuf::from(dir));
    }
    Ok(user_home()?.join(".ccl"))
}

pub fn config_file() -> Result<PathBuf> {
    Ok(app_dir()?.join("config.json"))
}

pub fn keys_file() -> Result<PathBuf> {
    Ok(app_dir()?.join("keys.json"))
}

pub fn template_file() -> Result<PathBuf> {
    Ok(app_dir()?.join("template.json"))
}

/// Where the install scripts put the binary, and so the PATH entry.
pub fn bin_dir() -> Result<PathBuf> {
    Ok(app_dir()?.join("bin"))
}

/// ccc's key store, read once so its keys come over on the first run.
pub fn ccc_keys_file() -> Result<PathBuf> {
    Ok(user_home()?.join(".ccc").join("keys.json"))
}

/// The file ccl writes: Claude Code's per-folder settings, never committed.
pub fn local_settings(dir: &Path) -> PathBuf {
    dir.join(".claude").join("settings.local.json")
}

/// The folder's shared settings; below `settings.local.json` in priority.
pub fn project_settings(dir: &Path) -> PathBuf {
    dir.join(".claude").join("settings.json")
}

/// `~/.claude/settings.json`, where `/model` saves a choice.
pub fn user_settings() -> Result<PathBuf> {
    Ok(user_home()?.join(".claude").join("settings.json"))
}

/// Policy pushed by IT. It beats every other settings file.
pub fn managed_settings() -> PathBuf {
    PathBuf::from(if cfg!(windows) {
        r"C:\ProgramData\ClaudeCode\managed-settings.json"
    } else if cfg!(target_os = "macos") {
        "/Library/Application Support/ClaudeCode/managed-settings.json"
    } else {
        "/etc/claude-code/managed-settings.json"
    })
}
