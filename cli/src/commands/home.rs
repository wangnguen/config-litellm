//! `ccl` on its own: where the folder stands, then the Switch Model screen.
//! The first run in a folder (or on a machine) sets everything up from here.

use anyhow::{Result, bail};
use ccl_core::keys;
use ccl_core::models;
use ccl_core::settings::SLOT_VARS;

use super::{ActiveKey, Ctx};
use crate::picker::{self, Input};
use crate::term::{self, Mark, Paint};

/// Name given to the first key when none is typed.
const FIRST_KEY_NAME: &str = "default";

pub fn run() -> Result<()> {
    let mut ctx = Ctx::load()?;
    let (key, new_key) = match ctx.key() {
        Some(key) => (key, None),
        None => ask_first_key()?,
    };
    let listed = ctx.list_models(&key.value)?;
    // Only a key the gateway accepted is worth keeping.
    if let Some(name) = new_key {
        ctx.keys.add(&name, &key.value);
        ctx.keys.save()?;
        println!("{} Saved key '{name}'.", "✓".green());
    }

    let usable = models::usable(&listed, &ctx.config);
    if usable.is_empty() {
        bail!(
            "the gateway lists {} model(s) for this key, but none ends in '{}'\n  change model_suffix with: ccl config edit",
            listed.len(),
            ctx.config.model_suffix
        );
    }
    if !term::interactive() {
        print_status(&ctx, &key, &usable);
        return Ok(());
    }

    let current = ctx.settings.main_model().map(models::base_id);
    let long_context = ctx
        .settings
        .main_model()
        .map_or(ctx.config.long_context, models::has_long_context);
    let header = format!(
        "key: {}  |  gateway: {}  |  folder: {}",
        key.label(),
        term::host(&ctx.config.base_url),
        ctx.folder_name()
    );
    let config = &ctx.config;
    let pick = picker::run(Input {
        header,
        models: &usable,
        current,
        efforts: &config.effort_levels,
        effort: config.start_effort(ctx.settings.effort()),
        long_context,
        supports_long_context: &|id| models::supports_long_context(id, config),
    })?;

    let Some(pick) = pick else {
        println!("{}", "Nothing changed.".dark_grey());
        return Ok(());
    };
    let slots = models::fill_slots(&pick.model, &usable, &ctx.config, pick.long_context);
    ctx.save(&key.value, &slots, pick.effort.as_deref())
}

/// No key anywhere yet: ask for one. Saved only once the gateway accepts it.
fn ask_first_key() -> Result<(ActiveKey, Option<String>)> {
    if !term::interactive() {
        bail!("no key yet; save one with: ccl key add <name>");
    }
    println!("No saved key yet. Paste your LiteLLM virtual key; it will not be shown.");
    let Some(value) = term::secret("Key: ")? else {
        bail!("cancelled");
    };
    let name = term::prompt(&format!("Name for it [{FIRST_KEY_NAME}]: "), FIRST_KEY_NAME)?;
    keys::validate(&name, &value)?;
    Ok((
        ActiveKey {
            name: Some(name.clone()),
            value,
            from_folder: false,
        },
        Some(name),
    ))
}

/// The picker's information without the picker, for piped output.
fn print_status(ctx: &Ctx, key: &ActiveKey, usable: &[String]) {
    term::row(Mark::Info, "key", key.label());
    term::row(Mark::Info, "gateway", &ctx.config.base_url);
    for (slot, var) in SLOT_VARS {
        match ctx.settings.env(var) {
            Some(id) if models::is_usable(id, usable) => term::row(Mark::Ok, slot, id),
            Some(id) => term::row(Mark::Fail, slot, format!("{id} (this key cannot use it)")),
            None => term::row(Mark::Warn, slot, "not set"),
        }
    }
    if let Some(effort) = ctx.settings.effort() {
        term::row(Mark::Info, "effort", effort);
    }
    println!("\nModels this key can use:");
    for id in usable {
        println!("  {id}");
    }
}
