//! `ccl check`: does the key work, and does each slot's model answer? Being
//! listed is not enough, so every model checked gets a real one-token call.

use std::thread;
use std::time::Duration;

use anyhow::{Result, bail};
use ccl_core::gateway;
use ccl_core::models::{self, base_id};
use ccl_core::settings::SLOT_VARS;

use super::Ctx;
use crate::term::{self, Mark, Paint};

pub fn run(all: bool) -> Result<()> {
    let ctx = Ctx::load()?;
    let key = ctx.require_key()?;
    let listed = match ctx.list_models(&key.value) {
        Ok(listed) => listed,
        Err(err) => {
            term::row(Mark::Fail, "key", key.label());
            return Err(err);
        }
    };
    let usable = models::usable(&listed, &ctx.config);
    term::row(
        Mark::Ok,
        "key",
        format!(
            "{}: {} listed, {} usable",
            key.label(),
            listed.len(),
            usable.len()
        ),
    );
    if !ctx.settings.exists() {
        term::row(
            Mark::Warn,
            "folder",
            "no .claude/settings.local.json yet; run ccl",
        );
    }

    let targets: Vec<(String, String)> = if all {
        usable
            .iter()
            .map(|id| (String::new(), id.clone()))
            .collect()
    } else {
        SLOT_VARS
            .iter()
            .filter_map(|(slot, var)| {
                ctx.settings
                    .env(var)
                    .map(|id| (slot.to_string(), id.to_string()))
            })
            .collect()
    };
    let mut failures = 0;
    let (to_call, unlisted): (Vec<_>, Vec<_>) = targets
        .into_iter()
        .partition(|(_, id)| models::is_usable(id, &usable));
    for (slot, id) in &unlisted {
        failures += 1;
        term::row(Mark::Fail, slot, format!("{id}  this key cannot use it"));
    }

    let results = term::waiting(&format!("Calling {} model(s)…", to_call.len()), || {
        probe_all(&ctx.config.base_url, &key.value, &to_call)
    });
    let width = to_call.iter().map(|(_, id)| id.len()).max().unwrap_or(0);
    for ((slot, id), result) in to_call.iter().zip(results) {
        // `--all` lists models rather than slots; the slots using one go last.
        let (label, used_by) = if all {
            (String::new(), slot_of(&ctx, id))
        } else {
            (slot.clone(), String::new())
        };
        let (mark, outcome) = match result {
            Ok(took) => (Mark::Ok, millis(took)),
            Err(err) => {
                failures += 1;
                (Mark::Fail, err.to_string())
            }
        };
        let used_by = if used_by.is_empty() {
            used_by
        } else {
            format!("  {}", used_by.cyan())
        };
        term::row(mark, &label, format!("{id:<width$}  {outcome}{used_by}"));
    }

    match ctx.settings.effort() {
        Some(effort) if ctx.config.effort_index(effort).is_some() => {
            term::row(Mark::Ok, "effort", effort);
        }
        Some(effort) => term::row(
            Mark::Warn,
            "effort",
            format!("{effort} (not in config.json's effort_levels)"),
        ),
        None if ctx.settings.exists() => term::row(
            Mark::Info,
            "effort",
            "not set; Claude Code's default applies",
        ),
        None => {}
    }

    if failures > 0 {
        bail!("{failures} check(s) failed");
    }
    Ok(())
}

/// One thread per model: each call waits on the network, not the CPU.
fn probe_all(
    base_url: &str,
    key: &str,
    targets: &[(String, String)],
) -> Vec<ccl_core::Result<Duration>> {
    thread::scope(|scope| {
        let calls: Vec<_> = targets
            .iter()
            .map(|(_, id)| scope.spawn(move || gateway::probe(base_url, key, id)))
            .collect();
        calls
            .into_iter()
            .map(|call| {
                call.join().unwrap_or_else(|_| {
                    Err(ccl_core::Error::Unexpected("the check crashed".into()))
                })
            })
            .collect()
    })
}

/// The slots of this folder that use `id`, for `--all`.
fn slot_of(ctx: &Ctx, id: &str) -> String {
    SLOT_VARS
        .iter()
        .filter(|(_, var)| {
            ctx.settings
                .env(var)
                .is_some_and(|set| base_id(set).eq_ignore_ascii_case(id))
        })
        .map(|(slot, _)| *slot)
        .collect::<Vec<_>>()
        .join(",")
}

fn millis(took: Duration) -> String {
    format!("{:>5} ms", took.as_millis())
}
