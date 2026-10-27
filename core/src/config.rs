//! `~/.ccl/config.json`: how to read the gateway, never what is on it.
//!
//! No model id is written into the code. The list comes from the gateway on
//! every run; this file only says which of those ids count (`model_suffix`),
//! which to hide, which slot to pin and which effort levels to offer. A model
//! added or retired upstream therefore needs, at most, an edit here.

use serde::{Deserialize, Serialize};

use crate::models::Family;
use crate::{Result, file, paths};

/// LiteLLM proxy the team's virtual keys are issued against.
pub const TEAM_BASE_URL: &str = "https://litellm-proxy-ep-cncyfugmcnadc6g4.a02.azurefd.net";
/// The team's keys may only use ids ending in this.
pub const TEAM_SUFFIX: &str = "-vn";
/// A slot set to this follows the gateway instead of naming an id.
pub const AUTO: &str = "auto";

// `default` on the container fills any field the file leaves out from
// `Config::default()`, so an old or hand-trimmed file keeps working.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub base_url: String,
    /// Only listed ids ending in this are offered; empty offers them all.
    pub model_suffix: String,
    /// Listed ids never offered, e.g. a model the team should not use yet.
    pub hidden_models: Vec<String>,
    /// What fills the opus, sonnet and haiku slots.
    pub slots: SlotPins,
    /// Whether a new pick carries `[1m]`, on the families below only.
    pub long_context: bool,
    /// Families that have a 1M-context variant.
    pub long_context_families: Vec<String>,
    /// What the picker's effort slider offers, lowest first.
    pub effort_levels: Vec<EffortLevel>,
    /// Effort for a folder that has none yet.
    pub default_effort: String,
}

/// `auto` (the newest listed model of the family) or an id written as-is,
/// `[1m]` included.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SlotPins {
    pub opus: String,
    pub sonnet: String,
    pub haiku: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EffortLevel {
    /// The value Claude Code reads from `effortLevel`.
    pub id: String,
    /// The line under the slider.
    pub description: String,
}

impl Default for SlotPins {
    fn default() -> Self {
        Self {
            opus: AUTO.into(),
            sonnet: AUTO.into(),
            haiku: AUTO.into(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        let level = |id: &str, description: &str| EffortLevel {
            id: id.into(),
            description: description.into(),
        };
        Self {
            base_url: TEAM_BASE_URL.into(),
            model_suffix: TEAM_SUFFIX.into(),
            hidden_models: Vec::new(),
            slots: SlotPins::default(),
            long_context: true,
            long_context_families: vec!["sonnet".into(), "opus".into()],
            effort_levels: vec![
                level("low", "Quick answers with light reasoning — fastest"),
                level("medium", "Balanced reasoning for everyday work"),
                level("high", "Deep reasoning for complex problems"),
                level("xhigh", "Extra-deep reasoning — slower"),
                level(
                    "max",
                    "Deepest reasoning for complex problems — slower but strongest",
                ),
            ],
            default_effort: "high".into(),
        }
    }
}

impl Config {
    /// `~/.ccl/config.json`, written with the defaults the first time so
    /// there is a file to edit.
    pub fn load() -> Result<Self> {
        let path = paths::config_file()?;
        if let Some(config) = file::read_json(&path)? {
            return Ok(config);
        }
        let config = Self::default();
        file::write_json(&path, &config, false)?;
        Ok(config)
    }

    /// The id a family's slot is pinned to; `None` follows the gateway.
    pub fn pin(&self, family: Family) -> Option<&str> {
        let pin = match family {
            Family::Opus => &self.slots.opus,
            Family::Sonnet => &self.slots.sonnet,
            Family::Haiku => &self.slots.haiku,
        }
        .trim();
        (!pin.is_empty() && !pin.eq_ignore_ascii_case(AUTO)).then_some(pin)
    }

    pub fn effort_index(&self, id: &str) -> Option<usize> {
        self.effort_levels
            .iter()
            .position(|level| level.id.eq_ignore_ascii_case(id.trim()))
    }

    /// Where the slider starts for a folder: its own effort when the slider
    /// offers it, else the default, else the lowest level.
    pub fn start_effort(&self, folder: Option<&str>) -> usize {
        folder
            .and_then(|id| self.effort_index(id))
            .or_else(|| self.effort_index(&self.default_effort))
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_trimmed_file_takes_the_rest_from_the_defaults() {
        let config: Config = serde_json::from_str(r#"{"model_suffix": ""}"#).unwrap();
        assert_eq!(config.model_suffix, "");
        assert_eq!(config.base_url, TEAM_BASE_URL);
        assert_eq!(config.effort_levels.len(), 5);
    }

    #[test]
    fn auto_and_blank_pins_follow_the_gateway() {
        let mut config = Config::default();
        assert_eq!(config.pin(Family::Opus), None);
        config.slots.opus = "  ".into();
        assert_eq!(config.pin(Family::Opus), None);
        config.slots.opus = "claude-opus-4-8-vn".into();
        assert_eq!(config.pin(Family::Opus), Some("claude-opus-4-8-vn"));
    }

    #[test]
    fn unknown_folder_effort_starts_at_the_default() {
        let config = Config::default();
        assert_eq!(config.start_effort(Some("xhigh")), 3);
        assert_eq!(config.start_effort(Some("ultra")), 2);
        assert_eq!(config.start_effort(None), 2);
    }
}
