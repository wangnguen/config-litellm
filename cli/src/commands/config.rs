//! `ccl config`: the one file to edit when the gateway changes. Printing it
//! also creates it, so `config edit` always has something to open.

use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};
use ccl_core::config::Config;
use ccl_core::{paths, settings};

use crate::ConfigAction;
use crate::term::Paint;

pub fn run(action: Option<ConfigAction>) -> Result<()> {
    Config::load()?;
    settings::ensure_template()?;
    let config = paths::config_file()?;
    match action {
        None => {
            println!("{}", config.display().to_string().dark_grey());
            print!("{}", fs::read_to_string(&config)?);
            println!("{}", "Edit with: ccl config edit".dark_grey());
        }
        Some(ConfigAction::Edit) => open(&config)?,
        Some(ConfigAction::Path) => {
            println!("config    {}", config.display());
            println!("template  {}", paths::template_file()?.display());
            println!("keys      {}", paths::keys_file()?.display());
            println!("binary    {}", env::current_exe()?.display());
        }
    }
    Ok(())
}

/// `$VISUAL`, then `$EDITOR`, then whatever the system opens text files with.
fn open(path: &Path) -> Result<()> {
    let editor = env::var("VISUAL")
        .ok()
        .or_else(|| env::var("EDITOR").ok())
        .filter(|e| !e.trim().is_empty());
    let mut command = match editor {
        // `code --wait` and the like: the program, then its own arguments.
        Some(editor) => {
            let mut parts = editor.split_whitespace();
            let mut command = Command::new(parts.next().unwrap_or("vi"));
            command.args(parts);
            command
        }
        None if cfg!(windows) => Command::new("notepad"),
        None if cfg!(target_os = "macos") => {
            let mut command = Command::new("open");
            command.arg("-t");
            command
        }
        None => Command::new("xdg-open"),
    };
    let status = command
        .arg(path)
        .status()
        .context("could not start an editor; set EDITOR, or open the file yourself")?;
    if !status.success() {
        bail!("the editor exited with {status}");
    }
    Ok(())
}
