//! What every command needs before it starts: the folder, ccl's config, the
//! saved keys and the folder's Claude Code settings.

pub mod check;
pub mod config;
pub mod home;
pub mod key;
pub mod ls;

use std::env;
use std::path::PathBuf;

use anyhow::{Result, anyhow};
use ccl_core::config::Config;
use ccl_core::keys::KeyStore;
use ccl_core::models::Slots;
use ccl_core::settings::{self, Settings};
use ccl_core::{Error, gateway};

use crate::term::{self, Paint};

pub struct Ctx {
    pub dir: PathBuf,
    pub config: Config,
    pub keys: KeyStore,
    pub settings: Settings,
}

/// The key a command works with, and the name it was saved under.
pub struct ActiveKey {
    pub name: Option<String>,
    pub value: String,
    /// From this folder's settings rather than the default key.
    pub from_folder: bool,
}

impl ActiveKey {
    pub fn label(&self) -> String {
        self.name.clone().unwrap_or_else(|| "unsaved key".into())
    }
}

impl Ctx {
    pub fn load() -> Result<Self> {
        let dir = env::current_dir()?;
        let config = Config::load()?;
        let (keys, imported) = KeyStore::load()?;
        if imported > 0 {
            eprintln!(
                "{}",
                format!("Imported {imported} key(s) from ccc (~/.ccc/keys.json).").dark_grey()
            );
        }
        let settings = Settings::load(&dir)?;
        Ok(Self {
            dir,
            config,
            keys,
            settings,
        })
    }

    /// The key this folder uses, else the default one.
    pub fn key(&self) -> Option<ActiveKey> {
        if let Some(value) = self.settings.key() {
            return Some(ActiveKey {
                name: self.keys.name_of(value).map(str::to_string),
                value: value.to_string(),
                from_folder: true,
            });
        }
        self.keys.active().map(|(name, value)| ActiveKey {
            name: Some(name.to_string()),
            value: value.to_string(),
            from_folder: false,
        })
    }

    pub fn require_key(&self) -> Result<ActiveKey> {
        self.key()
            .ok_or_else(|| anyhow!("no key yet; save one with: ccl key add <name>"))
    }

    pub fn folder_name(&self) -> String {
        self.dir.file_name().map_or_else(
            || self.dir.display().to_string(),
            |n| n.to_string_lossy().into_owned(),
        )
    }

    /// Every id the gateway lists for `key`, with a note on screen meanwhile.
    pub fn list_models(&self, key: &str) -> Result<Vec<String>> {
        term::waiting("Loading models…", || {
            gateway::list_models(&self.config.base_url, key)
        })
        .map_err(|err| self.explain(err))
    }

    /// A gateway error, plus what to do about it.
    pub fn explain(&self, err: Error) -> anyhow::Error {
        let hint = match &err {
            Error::Rejected(_) => {
                "use another key: ccl key use <name>, or save a new one: ccl key add <name>".into()
            }
            Error::Unreachable(_) | Error::Unexpected(_) => format!(
                "gateway is {}; change base_url with: ccl config edit",
                self.config.base_url
            ),
            _ => return err.into(),
        };
        anyhow!("{err}\n  {hint}")
    }

    /// Write the folder's settings and say what they now hold.
    pub fn save(&mut self, key: &str, slots: &Slots, effort: Option<&str>) -> Result<()> {
        let template = settings::ensure_template()?;
        self.settings
            .apply(&template, &self.config.base_url, key, slots, effort);
        self.settings.save()?;

        println!("{} Saved .claude/settings.local.json", "✓".green());
        for (slot, id) in slots.named() {
            println!("    {slot:<7} {id}");
        }
        if let Some(effort) = effort {
            println!("    {:<7} {effort}", "effort");
        }
        if settings::ensure_gitignored(&self.dir)? {
            println!("  Added .claude/settings.local.json to .gitignore.");
        }
        println!(
            "{}",
            "  Open a new Claude Code session to use it.".dark_grey()
        );
        Ok(())
    }
}
