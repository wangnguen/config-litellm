//! `.claude/settings.local.json`, the file Claude Code reads in a folder.
//!
//! ccl owns a handful of `env` variables and `effortLevel`, and leaves every
//! other field as it found it, so permissions or hooks added by hand survive
//! each save. A folder's first save starts from `~/.ccl/template.json`.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};

use crate::keys::mask;
use crate::models::Slots;
use crate::{Error, Result, file, paths};

pub const ENV_BASE_URL: &str = "ANTHROPIC_BASE_URL";
/// LiteLLM virtual keys authenticate as bearer tokens.
pub const ENV_AUTH_TOKEN: &str = "ANTHROPIC_AUTH_TOKEN";
/// Sent as x-api-key instead. Removed whenever ccl writes a token: with both
/// in one folder, which key is used would be up to Claude Code.
pub const ENV_API_KEY: &str = "ANTHROPIC_API_KEY";
pub const ENV_MODEL: &str = "ANTHROPIC_MODEL";
pub const ENV_OPUS: &str = "ANTHROPIC_DEFAULT_OPUS_MODEL";
pub const ENV_SONNET: &str = "ANTHROPIC_DEFAULT_SONNET_MODEL";
pub const ENV_HAIKU: &str = "ANTHROPIC_DEFAULT_HAIKU_MODEL";
/// Deprecated in Claude Code, still read by older versions. Always the Haiku slot.
pub const ENV_SMALL_FAST: &str = "ANTHROPIC_SMALL_FAST_MODEL";
pub const EFFORT: &str = "effortLevel";

/// Each slot's name and variable, in the order people read them.
pub const SLOT_VARS: [(&str, &str); 4] = [
    ("main", ENV_MODEL),
    ("opus", ENV_OPUS),
    ("sonnet", ENV_SONNET),
    ("haiku", ENV_HAIKU),
];

/// Every variable ccl writes, for spotting where else they are set.
pub const OWNED_VARS: [&str; 8] = [
    ENV_BASE_URL,
    ENV_AUTH_TOKEN,
    ENV_API_KEY,
    ENV_MODEL,
    ENV_OPUS,
    ENV_SONNET,
    ENV_HAIKU,
    ENV_SMALL_FAST,
];

/// Lines in a `.gitignore` that already keep the settings file out of git.
const IGNORE_PATTERNS: [&str; 8] = [
    ".claude/settings.local.json",
    "/.claude/settings.local.json",
    "settings.local.json",
    "**/settings.local.json",
    ".claude",
    ".claude/",
    "/.claude",
    "/.claude/",
];
const IGNORE_LINE: &str = ".claude/settings.local.json";

/// What a folder starts from while `~/.ccl/template.json` does not exist:
/// gateway model discovery on, background traffic off, and the tools ccc's
/// own template allowed.
pub fn default_template() -> Value {
    json!({
        "env": {
            "CLAUDE_CODE_ENABLE_GATEWAY_MODEL_DISCOVERY": "1",
            "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC": "1"
        },
        "permissions": {
            "allow": ["Write", "Bash", "Read", "Edit", "Glob", "Grep", "Agent", "WebFetch", "WebSearch"]
        }
    })
}

/// `~/.ccl/template.json`, written from [`default_template`] the first time
/// so there is a file to edit.
pub fn ensure_template() -> Result<Value> {
    let path = paths::template_file()?;
    if let Some(template) = file::read_json::<Value>(&path)? {
        return Ok(template);
    }
    let template = default_template();
    file::write_json(&path, &template, false)?;
    Ok(template)
}

/// Any settings file, as plain JSON: `None` when it does not exist.
pub fn read_any(path: &Path) -> Result<Option<Value>> {
    file::read_json(path)
}

/// One folder's `settings.local.json`.
#[derive(Debug, Clone)]
pub struct Settings {
    path: PathBuf,
    json: Value,
    exists: bool,
}

impl Settings {
    pub fn load(dir: &Path) -> Result<Self> {
        let path = paths::local_settings(dir);
        match file::read_json::<Value>(&path)? {
            Some(json) if json.is_object() => Ok(Self {
                path,
                json,
                exists: true,
            }),
            Some(_) => Err(Error::Invalid(format!(
                "{} is not a JSON object; fix or delete it",
                path.display()
            ))),
            None => Ok(Self {
                path,
                json: Value::Object(Map::new()),
                exists: false,
            }),
        }
    }

    pub fn exists(&self) -> bool {
        self.exists
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn json(&self) -> &Value {
        &self.json
    }

    /// A non-empty `env` value.
    pub fn env(&self, name: &str) -> Option<&str> {
        self.json
            .get("env")?
            .get(name)?
            .as_str()
            .filter(|v| !v.trim().is_empty())
    }

    pub fn key(&self) -> Option<&str> {
        self.env(ENV_AUTH_TOKEN).or_else(|| self.env(ENV_API_KEY))
    }

    pub fn main_model(&self) -> Option<&str> {
        self.env(ENV_MODEL)
    }

    pub fn effort(&self) -> Option<&str> {
        self.json.get(EFFORT)?.as_str()
    }

    /// Point the folder at a gateway, key and models, plus an effort when one
    /// is given. A folder without a file yet starts from `template`; nothing
    /// outside ccl's own fields is touched.
    pub fn apply(
        &mut self,
        template: &Value,
        base_url: &str,
        key: &str,
        slots: &Slots,
        effort: Option<&str>,
    ) {
        if !self.exists && template.is_object() {
            self.json = template.clone();
        }
        let root = object(&mut self.json);
        let env = object(root.entry("env").or_insert_with(|| json!({})));
        // Claude Code appends `/v1/messages` itself; a trailing slash doubles it.
        env.insert(
            ENV_BASE_URL.into(),
            base_url.trim().trim_end_matches('/').into(),
        );
        env.insert(ENV_AUTH_TOKEN.into(), key.into());
        env.shift_remove(ENV_API_KEY);
        for ((_, var), (_, id)) in SLOT_VARS.iter().zip(slots.named()) {
            env.insert((*var).into(), id.into());
        }
        env.insert(ENV_SMALL_FAST.into(), slots.haiku.as_str().into());
        if let Some(effort) = effort {
            root.insert(EFFORT.into(), effort.into());
        }
    }

    pub fn save(&mut self) -> Result<()> {
        file::write_json(&self.path, &self.json, true)?;
        self.exists = true;
        Ok(())
    }

    /// A copy safe to print or share a screen with: keys cut to their ends.
    pub fn masked(&self) -> Value {
        let mut json = self.json.clone();
        if let Some(env) = json.get_mut("env").and_then(Value::as_object_mut) {
            for var in [ENV_AUTH_TOKEN, ENV_API_KEY] {
                if let Some(Value::String(key)) = env.get_mut(var) {
                    *key = mask(key);
                }
            }
        }
        json
    }
}

/// `value` as an object, replacing anything else; ccl's fields need a home.
fn object(value: &mut Value) -> &mut Map<String, Value> {
    if !value.is_object() {
        *value = Value::Object(Map::new());
    }
    match value {
        Value::Object(map) => map,
        _ => unreachable!("replaced by an object above"),
    }
}

pub fn is_git_repo(dir: &Path) -> bool {
    dir.join(".git").exists()
}

/// Whether the folder's `.gitignore` already keeps the settings file out.
pub fn is_gitignored(dir: &Path) -> bool {
    fs::read_to_string(dir.join(".gitignore")).is_ok_and(|text| {
        text.lines()
            .map(str::trim)
            .any(|line| IGNORE_PATTERNS.contains(&line))
    })
}

/// Add the settings file to `.gitignore` in a git repository that does not
/// ignore it yet. Only that one file: `.claude/settings.json`, skills and
/// commands stay committable. True when a line was added.
pub fn ensure_gitignored(dir: &Path) -> Result<bool> {
    if !is_git_repo(dir) || is_gitignored(dir) {
        return Ok(false);
    }
    let path = dir.join(".gitignore");
    let mut text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(Error::io(&path, e)),
    };
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str(IGNORE_LINE);
    text.push('\n');
    fs::write(&path, text).map_err(|e| Error::io(&path, e))?;
    Ok(true)
}
