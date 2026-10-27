//! `ccl key`: saved keys. `use` also refits the folder's models to what the
//! new key may use, so switching keys never leaves a slot on a 403.

use anyhow::{Result, bail};
use ccl_core::Error;
use ccl_core::gateway;
use ccl_core::keys::{self, mask};
use ccl_core::models;

use super::Ctx;
use crate::KeyAction;
use crate::term::{self, Paint};

pub fn run(action: Option<KeyAction>) -> Result<()> {
    match action.unwrap_or(KeyAction::Ls) {
        KeyAction::Ls => list(),
        KeyAction::Add { name } => add(&name),
        KeyAction::Use { name } => use_here(&name),
        KeyAction::Rm { name } => remove(&name),
    }
}

fn list() -> Result<()> {
    let ctx = Ctx::load()?;
    if ctx.keys.keys.is_empty() {
        println!("No saved keys. Add one with: ccl key add <name>");
        return Ok(());
    }
    let here = ctx.settings.key();
    let width = ctx.keys.keys.keys().map(String::len).max().unwrap_or(0);
    for (name, value) in &ctx.keys.keys {
        let default = if ctx.keys.active.as_deref() == Some(name) {
            "★".yellow()
        } else {
            " ".to_string()
        };
        let used = if here == Some(value.as_str()) {
            "●".green()
        } else {
            " ".to_string()
        };
        println!(
            "  {default} {used} {name:<width$}  {}",
            mask(value).dark_grey()
        );
    }
    if here.is_some_and(|key| ctx.keys.name_of(key).is_none()) {
        println!(
            "      {}",
            "this folder uses a key that is not saved".dark_grey()
        );
    }
    println!(
        "{}",
        "  ★ default for new folders   ● used in this folder".dark_grey()
    );
    Ok(())
}

fn add(name: &str) -> Result<()> {
    let mut ctx = Ctx::load()?;
    let Some(value) = term::secret(&format!("Key for '{name}' (hidden): "))? else {
        bail!("cancelled");
    };
    keys::validate(name, &value)?;
    match term::waiting("Checking the key…", || {
        gateway::list_models(&ctx.config.base_url, &value)
    }) {
        Ok(listed) => println!(
            "{} The gateway accepts it: {} usable model(s).",
            "✓".green(),
            models::usable(&listed, &ctx.config).len()
        ),
        Err(err @ Error::Rejected(_)) => bail!("{err}; not saved"),
        Err(err) => println!("{} Saved without checking: {err}", "!".yellow()),
    }
    let replaced = ctx.keys.add(name, &value);
    ctx.keys.save()?;
    let verb = if replaced { "Replaced" } else { "Saved" };
    if ctx.keys.active.as_deref() == Some(name) {
        println!("{verb} key '{name}'. It is the default for new folders.");
    } else {
        println!("{verb} key '{name}'. Use it here with: ccl key use {name}");
    }
    Ok(())
}

fn use_here(name: &str) -> Result<()> {
    let mut ctx = Ctx::load()?;
    let Some(value) = ctx.keys.get(name).map(str::to_string) else {
        bail!("no saved key named '{name}' (see: ccl key ls)");
    };
    let listed = ctx.list_models(&value)?;
    let usable = models::usable(&listed, &ctx.config);
    let current = ctx.settings.main_model().map(str::to_string);
    let Some(main) = models::keep_main(current.as_deref(), &usable) else {
        bail!(
            "key '{name}' cannot use any model ending in '{}'",
            ctx.config.model_suffix
        );
    };
    let long_context = current
        .as_deref()
        .map_or(ctx.config.long_context, models::has_long_context);
    let slots = models::fill_slots(&main, &usable, &ctx.config, long_context);
    let effort = ctx.settings.effort().map(str::to_string).or_else(|| {
        (!ctx.config.effort_levels.is_empty()).then(|| ctx.config.default_effort.clone())
    });

    ctx.keys.set_active(name)?;
    ctx.keys.save()?;
    println!("Using key '{name}' here; it is also the default for new folders.");
    ctx.save(&value, &slots, effort.as_deref())
}

fn remove(name: &str) -> Result<()> {
    let mut ctx = Ctx::load()?;
    let Some(value) = ctx.keys.get(name).map(str::to_string) else {
        bail!("no saved key named '{name}' (see: ccl key ls)");
    };
    if !term::confirm(&format!("Delete key '{name}'? It cannot be recovered."))? {
        println!("Kept.");
        return Ok(());
    }
    ctx.keys.remove(name)?;
    ctx.keys.save()?;
    println!("Deleted key '{name}'.");
    if ctx.settings.key() == Some(value.as_str()) {
        println!(
            "{}",
            "  This folder still uses it in .claude/settings.local.json; switch with: ccl key use <name>"
                .dark_grey()
        );
    }
    Ok(())
}
