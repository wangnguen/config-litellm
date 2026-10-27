//! `ccl show`: this folder's settings as Claude Code reads them, safe to put
//! on a shared screen.

use anyhow::Result;

use super::Ctx;
use crate::term::Paint;

pub fn run() -> Result<()> {
    let ctx = Ctx::load()?;
    println!("{}", ctx.settings.path().display().to_string().dark_grey());
    if !ctx.settings.exists() {
        println!("Not set up yet; run ccl here.");
        return Ok(());
    }
    println!("{}", serde_json::to_string_pretty(&ctx.settings.masked())?);
    if let Some(key) = ctx.key() {
        let source = if key.from_folder {
            "this folder"
        } else {
            "the default key"
        };
        println!(
            "{}",
            format!("key: {} (from {source})", key.label()).dark_grey()
        );
    }
    Ok(())
}
