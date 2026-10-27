//! `ccl uninstall`: remove `~/.ccl` (binary, config, template, every saved
//! key) and the PATH entry the installer added. Projects are left alone:
//! their `.claude/settings.local.json` keeps working without ccl.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use ccl_core::{paths, settings};

use crate::term;

/// The comment install.sh writes above its PATH line.
const RC_MARKER: &str = "# ccl - Claude Code LiteLLM";
const RC_FILES: [&str; 3] = [".zshrc", ".bashrc", ".profile"];

pub fn run() -> Result<()> {
    let home = paths::app_dir()?;
    let bin = paths::bin_dir()?;
    // Read, not KeyStore::load(): that would import ccc's keys on the way out.
    let keys = settings::read_any(&paths::keys_file()?)
        .ok()
        .flatten()
        .and_then(|json| json["keys"].as_object().map(|keys| keys.len()))
        .unwrap_or(0);

    println!("This removes:");
    println!("  {}  (ccl, config.json, template.json)", home.display());
    println!("  {keys} saved key(s) in keys.json; they cannot be recovered");
    println!("  {} from your PATH", bin.display());
    println!("Your projects' .claude/settings.local.json files are not touched.");
    if !term::confirm("Uninstall ccl?")? {
        println!("Nothing removed.");
        return Ok(());
    }

    remove_path_entry(&bin);
    if home.exists() {
        remove_home(&home)?;
    }
    println!("ccl is uninstalled. Open a new terminal for PATH to update.");
    Ok(())
}

fn remove_path_entry(bin: &Path) {
    if cfg!(windows) {
        remove_windows_path_entry(bin);
    } else {
        remove_unix_path_entry(bin);
    }
}

fn remove_windows_path_entry(bin: &Path) {
    // Edit the registry value rather than [Environment]::SetEnvironmentVariable:
    // that API returns PATH expanded, so writing it back would freeze entries
    // like %JAVA_HOME%\bin into whatever they point at today. Single quotes
    // take the path literally; a doubled one escapes itself.
    let dir = bin.display().to_string().replace('\'', "''");
    let script = format!(
        "$key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('Environment', $true); \
         if (-not $key) {{ exit 3 }}; \
         $raw = $key.GetValue('Path', '', 'DoNotExpandEnvironmentNames'); \
         $dir = '{dir}'.TrimEnd('\\'); \
         $new = (($raw -split ';') | Where-Object {{ $_.TrimEnd('\\') -ne $dir }}) -join ';'; \
         if ($new -eq $raw) {{ exit 3 }}; \
         $key.SetValue('Path', $new, $key.GetValueKind('Path')); \
         [Environment]::SetEnvironmentVariable('CCL_PATH_REFRESH', $null, 'User')"
    );
    let code = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .status()
        .ok()
        .and_then(|s| s.code());
    match code {
        Some(0) => println!("Removed {} from your user PATH.", bin.display()),
        Some(3) => {}
        _ => println!(
            "Could not edit PATH; remove {} from it by hand.",
            bin.display()
        ),
    }
}

fn remove_unix_path_entry(bin: &Path) {
    let Ok(user_home) = paths::user_home() else {
        return;
    };
    let needle = bin.display().to_string();
    for rc in RC_FILES {
        let path = user_home.join(rc);
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        let kept: Vec<&str> = text
            .lines()
            .filter(|line| !is_installer_line(line, &needle))
            .collect();
        if kept.len() == text.lines().count() {
            continue;
        }
        let mut cleaned = kept.join("\n");
        cleaned.push('\n');
        if fs::write(&path, cleaned).is_ok() {
            println!("Removed the ccl PATH line from {}.", path.display());
        }
    }
}

/// Only the two lines install.sh writes, never someone's own PATH edit.
fn is_installer_line(line: &str, bin: &str) -> bool {
    let line = line.trim();
    line == RC_MARKER || (line.starts_with("export PATH=") && line.contains(bin))
}

fn remove_home(home: &Path) -> Result<()> {
    match running_binary_inside(home) {
        // Windows locks a running .exe: everything else goes now, the folder
        // itself a moment after this process exits.
        Some(exe) if cfg!(windows) => {
            remove_all_but(home, &exe);
            schedule_windows_cleanup(home);
            println!(
                "Removed {} (ccl.exe follows once it exits).",
                home.display()
            );
        }
        _ => {
            fs::remove_dir_all(home)
                .with_context(|| format!("cannot delete {}", home.display()))?;
            println!("Removed {}.", home.display());
        }
    }
    Ok(())
}

fn remove_all_but(dir: &Path, keep: &Path) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for path in entries.flatten().map(|e| e.path()) {
        if path.canonicalize().ok().as_deref() == Some(keep) {
            continue;
        }
        if path.is_dir() {
            if keep.starts_with(path.canonicalize().unwrap_or_default()) {
                remove_all_but(&path, keep);
            } else {
                let _ = fs::remove_dir_all(&path);
            }
        } else {
            let _ = fs::remove_file(&path);
        }
    }
}

/// The running binary, when it lives inside the folder being deleted. A
/// build run from `target/` is not affected.
fn running_binary_inside(home: &Path) -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?.canonicalize().ok()?;
    let home = home.canonicalize().ok()?;
    exe.starts_with(&home).then_some(exe)
}

fn schedule_windows_cleanup(home: &Path) {
    // One argument per token: Rust escapes inner quotes for the C runtime,
    // which cmd.exe does not understand. ping is the delay because timeout
    // wants a console of its own.
    let _ = Command::new("cmd")
        .args([
            "/C",
            "ping",
            "127.0.0.1",
            "-n",
            "3",
            ">nul",
            "&",
            "rmdir",
            "/s",
            "/q",
        ])
        .arg(home)
        .spawn();
}
