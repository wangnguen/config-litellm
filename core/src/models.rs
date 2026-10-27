//! Reading a gateway's model list: which ids a key may pick, the family and
//! version of each, and what fills each of Claude Code's slots.
//!
//! Ids look like `claude-<family>-<major>[-<minor>]<suffix>`, for example
//! `claude-opus-5-5-vn`. Versions compare as numbers, part by part, so 5-5
//! beats 5 beats 4-8. As strings, "4-8" would beat "4-10".

use std::cmp::Reverse;

use crate::config::Config;

/// Client-side marker for the 1M-context variant. Claude Code strips it and
/// sends a beta header instead, so the gateway never lists it.
pub const LONG_CONTEXT: &str = "[1m]";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Family {
    Opus,
    Sonnet,
    Haiku,
}

impl Family {
    /// Display order: the most capable first.
    pub const ALL: [Family; 3] = [Family::Opus, Family::Sonnet, Family::Haiku];

    pub fn name(self) -> &'static str {
        match self {
            Family::Opus => "opus",
            Family::Sonnet => "sonnet",
            Family::Haiku => "haiku",
        }
    }

    /// The family named by one `-`-separated part of an id.
    fn of_part(part: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|f| part == f.name())
    }

    pub fn of(id: &str) -> Option<Self> {
        base_id(id)
            .to_ascii_lowercase()
            .split('-')
            .find_map(Self::of_part)
    }
}

/// The id the gateway knows: `claude-sonnet-5-vn[1m]` → `claude-sonnet-5-vn`.
pub fn base_id(id: &str) -> &str {
    let id = id.trim();
    let marker = id.len().checked_sub(LONG_CONTEXT.len()).and_then(|at| {
        id.get(at..)
            .filter(|end| end.eq_ignore_ascii_case(LONG_CONTEXT))
            .map(|_| at)
    });
    match marker {
        Some(at) => id[..at].trim_end(),
        None => id,
    }
}

pub fn has_long_context(id: &str) -> bool {
    base_id(id).len() != id.trim().len()
}

pub fn with_long_context(id: &str, on: bool) -> String {
    if on {
        format!("{}{LONG_CONTEXT}", base_id(id))
    } else {
        base_id(id).to_string()
    }
}

/// The numbers after the family name: `claude-opus-5-5-vn` → `[5, 5]`.
pub fn version(id: &str) -> Vec<u64> {
    let id = base_id(id).to_ascii_lowercase();
    let parts: Vec<&str> = id.split('-').collect();
    let start = parts
        .iter()
        .position(|part| Family::of_part(part).is_some())
        .map_or(0, |at| at + 1);
    parts[start..]
        .iter()
        .skip_while(|part| part.parse::<u64>().is_err())
        .map_while(|part| part.parse().ok())
        .collect()
}

pub fn matches_suffix(id: &str, suffix: &str) -> bool {
    let suffix = suffix.trim().to_ascii_lowercase();
    suffix.is_empty() || base_id(id).to_ascii_lowercase().ends_with(&suffix)
}

pub fn supports_long_context(id: &str, config: &Config) -> bool {
    Family::of(id).is_some_and(|family| {
        config
            .long_context_families
            .iter()
            .any(|name| name.trim().eq_ignore_ascii_case(family.name()))
    })
}

/// The listed ids a key may pick, in display order: Opus, Sonnet, Haiku,
/// then anything else, newest first within each.
pub fn usable(listed: &[String], config: &Config) -> Vec<String> {
    let hidden = |id: &str| {
        config
            .hidden_models
            .iter()
            .any(|h| base_id(h).eq_ignore_ascii_case(id))
    };
    let mut ids: Vec<String> = listed
        .iter()
        .map(|id| base_id(id).to_string())
        .filter(|id| matches_suffix(id, &config.model_suffix) && !hidden(id))
        .collect();
    ids.sort_by_cached_key(|id| {
        (
            Family::of(id).map_or(Family::ALL.len(), |f| f as usize),
            Reverse(version(id)),
            id.clone(),
        )
    });
    ids.dedup();
    ids
}

/// Whether the key may use `id`, `[1m]` or not.
pub fn is_usable(id: &str, usable: &[String]) -> bool {
    let id = base_id(id);
    usable.iter().any(|u| u.eq_ignore_ascii_case(id))
}

/// The newest usable id of a family.
pub fn newest(usable: &[String], family: Family) -> Option<&str> {
    usable
        .iter()
        .filter(|id| Family::of(id) == Some(family))
        .max_by_key(|id| version(id))
        .map(String::as_str)
}

/// The main model a folder keeps when its key changes: the current one while
/// the key may still use it, else the newest Sonnet, else the first usable id.
pub fn keep_main(current: Option<&str>, usable: &[String]) -> Option<String> {
    current
        .filter(|id| is_usable(id, usable))
        .map(|id| base_id(id).to_string())
        .or_else(|| newest(usable, Family::Sonnet).map(str::to_string))
        .or_else(|| usable.first().cloned())
}

/// What a folder's four model variables are set to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slots {
    pub main: String,
    pub opus: String,
    pub sonnet: String,
    pub haiku: String,
}

impl Slots {
    /// Each slot with its name, in the order people read them.
    pub fn named(&self) -> [(&'static str, &str); 4] {
        [
            ("main", &self.main),
            ("opus", &self.opus),
            ("sonnet", &self.sonnet),
            ("haiku", &self.haiku),
        ]
    }
}

/// Fill every slot from one pick. The pick is the main model; each family's
/// slot follows its pin in config.json, else the newest model of that family.
/// A family the key has nothing of falls back to the pick, so its `/model`
/// alias still lands on a model the key may use rather than on a 403.
pub fn fill_slots(main: &str, usable: &[String], config: &Config, long_context: bool) -> Slots {
    let marked =
        |id: &str| with_long_context(id, long_context && supports_long_context(id, config));
    let slot = |family: Family| match config.pin(family) {
        Some(pinned) => pinned.to_string(),
        None => marked(newest(usable, family).unwrap_or(main)),
    };
    Slots {
        main: marked(main),
        opus: slot(Family::Opus),
        sonnet: slot(Family::Sonnet),
        haiku: slot(Family::Haiku),
    }
}
