//! Named keys, kept in `~/.ccl/keys.json`.
//!
//! The shape is ccc's own `{"active": name, "keys": {name: key}}`, so the
//! first run can take ccc's keys over unchanged.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{Error, Result, file, paths};

/// Keys this short are shown as stars only: their ends would be most of them.
const MIN_MASKABLE: usize = 12;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyStore {
    /// The key a folder without one of its own gets.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active: Option<String>,
    #[serde(default)]
    pub keys: BTreeMap<String, String>,
}

impl KeyStore {
    /// `~/.ccl/keys.json`. The first time, before that file exists, start
    /// from ccc's keys; the count says how many came over.
    pub fn load() -> Result<(Self, usize)> {
        let path = paths::keys_file()?;
        if let Some(mut store) = file::read_json::<Self>(&path)? {
            store.repair();
            return Ok((store, 0));
        }
        // A ccc file that is missing or broken is not ccl's to fix: start empty.
        let mut store: Self = paths::ccc_keys_file()
            .ok()
            .and_then(|ccc| file::read_json(&ccc).ok().flatten())
            .unwrap_or_default();
        store.repair();
        let imported = store.keys.len();
        if imported > 0 {
            store.save()?;
        }
        Ok((store, imported))
    }

    pub fn save(&self) -> Result<()> {
        file::write_json(&paths::keys_file()?, self, true)
    }

    /// Save `key` as `name`; the first key saved becomes the default.
    /// True when it replaced a key of the same name.
    pub fn add(&mut self, name: &str, key: &str) -> bool {
        let replaced = self
            .keys
            .insert(name.to_string(), key.to_string())
            .is_some();
        self.repair();
        replaced
    }

    pub fn remove(&mut self, name: &str) -> Result<()> {
        self.keys.remove(name).ok_or_else(|| unknown(name))?;
        self.repair();
        Ok(())
    }

    pub fn set_active(&mut self, name: &str) -> Result<()> {
        if !self.keys.contains_key(name) {
            return Err(unknown(name));
        }
        self.active = Some(name.to_string());
        Ok(())
    }

    pub fn get(&self, name: &str) -> Option<&str> {
        self.keys.get(name).map(String::as_str)
    }

    /// The name a key was saved under, if it was.
    pub fn name_of(&self, key: &str) -> Option<&str> {
        self.keys
            .iter()
            .find(|(_, saved)| saved.as_str() == key)
            .map(|(name, _)| name.as_str())
    }

    /// The default key, as `(name, key)`.
    pub fn active(&self) -> Option<(&str, &str)> {
        let name = self.active.as_deref()?;
        self.get(name).map(|key| (name, key))
    }

    /// An `active` that names no saved key points at the first one instead.
    fn repair(&mut self) {
        if self
            .active
            .as_ref()
            .is_none_or(|name| !self.keys.contains_key(name))
        {
            self.active = self.keys.keys().next().cloned();
        }
    }
}

fn unknown(name: &str) -> Error {
    Error::Invalid(format!("no saved key named '{name}' (see: ccl key ls)"))
}

/// Refuse what would only fail later: a name that cannot be typed back as
/// one argument, or a key mangled by a bad paste.
pub fn validate(name: &str, key: &str) -> Result<()> {
    if name.is_empty() || name.chars().any(char::is_whitespace) {
        return Err(Error::Invalid(
            "a key name cannot be empty or contain spaces".into(),
        ));
    }
    if key.is_empty() {
        return Err(Error::Invalid("the key is empty".into()));
    }
    if key.chars().any(char::is_whitespace) {
        return Err(Error::Invalid(
            "the key contains spaces; paste it again".into(),
        ));
    }
    Ok(())
}

/// Enough of a key to tell two apart, not enough to use one: `sk-…f3c9`.
pub fn mask(key: &str) -> String {
    let chars: Vec<char> = key.chars().collect();
    if chars.len() < MIN_MASKABLE {
        return "*".repeat(chars.len().max(3));
    }
    let head: String = chars[..3].iter().collect();
    let tail: String = chars[chars.len() - 4..].iter().collect();
    format!("{head}…{tail}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store(names: &[&str]) -> KeyStore {
        let mut store = KeyStore::default();
        for name in names {
            store.add(name, &format!("sk-{name}-0123456789"));
        }
        store
    }

    #[test]
    fn first_key_becomes_the_default() {
        let store = store(&["work"]);
        assert_eq!(store.active.as_deref(), Some("work"));
    }

    #[test]
    fn removing_the_default_hands_it_to_another_key() {
        let mut store = store(&["a", "b"]);
        store.set_active("b").unwrap();
        store.remove("b").unwrap();
        assert_eq!(store.active.as_deref(), Some("a"));
        store.remove("a").unwrap();
        assert_eq!(store.active, None);
    }

    #[test]
    fn unknown_names_are_refused() {
        let mut store = store(&["a"]);
        assert!(store.set_active("zzz").is_err());
        assert!(store.remove("zzz").is_err());
    }

    #[test]
    fn reads_ccc_key_files_as_they_are() {
        let ccc = r#"{"active": "chung33", "keys": {"chung33": "sk-1", "lite": "sk-2"}}"#;
        let store: KeyStore = serde_json::from_str(ccc).unwrap();
        assert_eq!(store.active(), Some(("chung33", "sk-1")));
        assert_eq!(store.name_of("sk-2"), Some("lite"));
    }

    #[test]
    fn mask_keeps_only_the_ends() {
        assert_eq!(mask("sk-abcdefghijklmnop"), "sk-…mnop");
        assert_eq!(mask("short"), "*****");
        assert_eq!(mask(""), "***");
    }

    #[test]
    fn validate_catches_bad_pastes() {
        assert!(validate("work", "sk-1").is_ok());
        assert!(validate("my key", "sk-1").is_err());
        assert!(validate("work", "sk-1 sk-2").is_err());
        assert!(validate("work", "").is_err());
    }
}
