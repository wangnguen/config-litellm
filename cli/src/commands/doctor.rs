//! `ccl doctor`: everything that could make Claude Code use something other
//! than what this folder's settings say. Files only, no network.
//!
//! Priority, highest first: managed policy, command-line flags,
//! `.claude/settings.local.json`, `.claude/settings.json`, then
//! `~/.claude/settings.json`. Only the first can beat what ccl writes.

use std::env;
use std::path::Path;
use std::process::Command;

use anyhow::Result;
use ccl_core::models::base_id;
use ccl_core::paths;
use ccl_core::settings::{self, ENV_BASE_URL, ENV_MODEL, OWNED_VARS, SLOT_VARS};
use serde_json::Value;

use super::Ctx;
use crate::term::{self, Mark};

pub fn run() -> Result<()> {
    let ctx = Ctx::load()?;

    match claude_version() {
        Some(version) => term::row(Mark::Ok, "claude", version),
        None => term::row(Mark::Warn, "claude", "Claude Code not found on PATH"),
    }

    check_folder(&ctx);

    let managed = paths::managed_settings();
    if let Some(json) = read(&managed) {
        let set = owned_fields(&json);
        if set.is_empty() {
            term::row(
                Mark::Info,
                "policy",
                format!("{} sets none of ccl's fields", managed.display()),
            );
        } else {
            term::row(
                Mark::Warn,
                "policy",
                format!(
                    "{} sets {}; it beats this folder",
                    managed.display(),
                    set.join(", ")
                ),
            );
        }
    }

    let project = paths::project_settings(&ctx.dir);
    if let Some(json) = read(&project) {
        let set = owned_fields(&json);
        if !set.is_empty() {
            term::row(
                Mark::Info,
                "project",
                format!(
                    ".claude/settings.json also sets {} (settings.local.json wins)",
                    set.join(", ")
                ),
            );
        }
    }

    if let Ok(user) = paths::user_settings()
        && let Some(json) = read(&user)
    {
        check_user(&ctx, &json);
    }

    for var in OWNED_VARS {
        if env::var_os(var).is_some_and(|v| !v.is_empty()) {
            term::row(
                Mark::Info,
                "shell",
                format!("{var} is set in your shell; the settings file's value is the one used"),
            );
        }
    }
    Ok(())
}

fn check_folder(ctx: &Ctx) {
    if !ctx.settings.exists() {
        term::row(
            Mark::Fail,
            "folder",
            "no .claude/settings.local.json; run ccl",
        );
        return;
    }
    match ctx.settings.key() {
        Some(_) => term::row(
            Mark::Ok,
            "key",
            ctx.key().map_or_else(String::new, |k| k.label()),
        ),
        None => term::row(
            Mark::Fail,
            "key",
            "none in settings.local.json; run ccl key use <name>",
        ),
    }
    match ctx.settings.env(ENV_BASE_URL) {
        Some(url) if url.trim_end_matches('/') == ctx.config.base_url.trim_end_matches('/') => {
            term::row(Mark::Ok, "gateway", url);
        }
        Some(url) => term::row(
            Mark::Warn,
            "gateway",
            format!(
                "{url}, but config.json says {}; run ccl to update",
                ctx.config.base_url
            ),
        ),
        None => term::row(Mark::Fail, "gateway", "ANTHROPIC_BASE_URL not set; run ccl"),
    }
    let missing: Vec<&str> = SLOT_VARS
        .iter()
        .filter(|(_, var)| ctx.settings.env(var).is_none())
        .map(|(slot, _)| *slot)
        .collect();
    if missing.is_empty() {
        term::row(Mark::Ok, "slots", "all four set");
    } else {
        term::row(
            Mark::Fail,
            "slots",
            format!(
                "{} not set; their /model aliases will 403. Run ccl",
                missing.join(", ")
            ),
        );
    }
    if settings::is_git_repo(&ctx.dir) && !settings::is_gitignored(&ctx.dir) {
        term::row(
            Mark::Warn,
            "git",
            "settings.local.json is not in .gitignore; the key could be committed",
        );
    }
}

fn check_user(ctx: &Ctx, json: &Value) {
    if let Some(model) = json["model"].as_str()
        && ctx.settings.env(ENV_MODEL).is_some()
    {
        term::row(
            Mark::Info,
            "user",
            format!(
                "~/.claude/settings.json model \"{model}\" is ignored here: ANTHROPIC_MODEL is set"
            ),
        );
    }
    // Claude Code keeps a per-model effort next to the shared one.
    if let Some(main) = ctx.settings.main_model()
        && let Some(effort) = json["modelSettings"][base_id(main)]["effortLevel"].as_str()
    {
        term::row(
            Mark::Warn,
            "user",
            format!(
                "~/.claude/settings.json sets effort \"{effort}\" for {}",
                base_id(main)
            ),
        );
    }
}

/// ccl's fields that a settings file sets.
fn owned_fields(json: &Value) -> Vec<&'static str> {
    let mut set: Vec<&'static str> = OWNED_VARS
        .into_iter()
        .filter(|var| json["env"][var].is_string())
        .collect();
    if json[settings::EFFORT].is_string() {
        set.push(settings::EFFORT);
    }
    set
}

fn read(path: &Path) -> Option<Value> {
    settings::read_any(path).ok().flatten()
}

fn claude_version() -> Option<String> {
    // npm installs a claude.cmd shim on Windows, which Command does not find
    // under the bare name.
    let names: &[&str] = if cfg!(windows) {
        &["claude", "claude.cmd"]
    } else {
        &["claude"]
    };
    names.iter().find_map(|name| {
        let output = Command::new(name).arg("--version").output().ok()?;
        output
            .status
            .success()
            .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
    })
}
