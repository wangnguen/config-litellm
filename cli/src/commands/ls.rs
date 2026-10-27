//! `ccl ls`: the models the key may use, and which slot of this folder uses
//! each. Ids the gateway lists but the key may not use are counted, not listed.

use anyhow::Result;
use ccl_core::models::{self, base_id};
use ccl_core::settings::SLOT_VARS;

use super::Ctx;
use crate::term::{self, Mark, Paint};

pub fn run() -> Result<()> {
    let ctx = Ctx::load()?;
    let key = ctx.require_key()?;
    let listed = ctx.list_models(&key.value)?;
    let usable = models::usable(&listed, &ctx.config);

    println!(
        "{} models for key {} at {}",
        usable.len(),
        key.label().bold(),
        term::host(&ctx.config.base_url)
    );
    let width = usable.iter().map(|id| id.len()).max().unwrap_or(0);
    for id in &usable {
        let slots: Vec<&str> = SLOT_VARS
            .iter()
            .filter(|(_, var)| {
                ctx.settings
                    .env(var)
                    .is_some_and(|set| base_id(set).eq_ignore_ascii_case(id))
            })
            .map(|(slot, _)| *slot)
            .collect();
        println!("  {id:<width$}  {}", slots.join(", ").cyan());
    }

    let skipped = listed.len().saturating_sub(usable.len());
    if skipped > 0 {
        println!(
            "{}",
            format!(
                "  {skipped} more listed but not offered (not ending in '{}', or hidden in config.json)",
                ctx.config.model_suffix
            )
            .dark_grey()
        );
    }
    for (slot, var) in SLOT_VARS {
        if let Some(id) = ctx.settings.env(var)
            && !models::is_usable(id, &usable)
        {
            term::row(
                Mark::Fail,
                slot,
                format!("uses {id}, which this key cannot use; run ccl to pick again"),
            );
        }
    }
    Ok(())
}
