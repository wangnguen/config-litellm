//! What the team gateway listed for a virtual key in October 2026, plus the
//! names that must never be offered. The list is a test corpus, not a source
//! of truth: ccl asks the gateway every time, and these tests pin down how
//! it reads whatever comes back.

use ccl_core::config::Config;
use ccl_core::models::{
    Family, base_id, fill_slots, has_long_context, keep_main, newest, usable, version,
};

const TEAM_MODELS: [&str; 8] = [
    "claude-opus-5-vn",
    "claude-sonnet-5-vn",
    "claude-haiku-4-5-vn",
    "claude-opus-4-8-vn",
    "claude-sonnet-4-6-vn",
    "claude-opus-5-5-vn",
    "claude-sonnet-5-5-vn",
    "claude-haiku-5-5-vn",
];

/// Claude Code's built-in names and other gateways' models: listed, but a
/// team key gets a 403 for every one of them.
const NOT_FOR_THE_KEY: [&str; 4] = [
    "claude-sonnet-5",
    "claude-haiku-4-5-20251001",
    "gpt-4o",
    "claude-opus-5-5-VN-preview",
];

fn listed() -> Vec<String> {
    TEAM_MODELS
        .iter()
        .chain(NOT_FOR_THE_KEY.iter())
        .map(|id| id.to_string())
        .collect()
}

#[test]
fn only_suffixed_models_are_offered_most_capable_and_newest_first() {
    let offered = usable(&listed(), &Config::default());
    assert_eq!(
        offered,
        [
            "claude-opus-5-5-vn",
            "claude-opus-5-vn",
            "claude-opus-4-8-vn",
            "claude-sonnet-5-5-vn",
            "claude-sonnet-5-vn",
            "claude-sonnet-4-6-vn",
            "claude-haiku-5-5-vn",
            "claude-haiku-4-5-vn",
        ]
    );
}

#[test]
fn an_empty_suffix_offers_everything_listed() {
    let config = Config {
        model_suffix: String::new(),
        ..Config::default()
    };
    assert_eq!(usable(&listed(), &config).len(), listed().len());
}

#[test]
fn hidden_models_disappear_whatever_their_case_or_marker() {
    let config = Config {
        hidden_models: vec!["CLAUDE-OPUS-5-5-VN[1m]".into()],
        ..Config::default()
    };
    let offered = usable(&listed(), &config);
    assert!(!offered.iter().any(|id| id == "claude-opus-5-5-vn"));
    assert_eq!(offered.len(), 7);
}

#[test]
fn versions_compare_as_numbers_not_strings() {
    assert_eq!(version("claude-opus-5-5-vn"), [5, 5]);
    assert_eq!(version("claude-opus-5-vn[1m]"), [5]);
    assert_eq!(version("claude-sonnet-4-10-vn"), [4, 10]);
    assert!(version("claude-sonnet-4-10-vn") > version("claude-sonnet-4-8-vn"));
    assert!(version("claude-opus-5-5-vn") > version("claude-opus-5-vn"));
}

#[test]
fn the_newest_of_each_family_wins() {
    let offered = usable(&listed(), &Config::default());
    assert_eq!(newest(&offered, Family::Opus), Some("claude-opus-5-5-vn"));
    assert_eq!(
        newest(&offered, Family::Sonnet),
        Some("claude-sonnet-5-5-vn")
    );
    assert_eq!(newest(&offered, Family::Haiku), Some("claude-haiku-5-5-vn"));
}

#[test]
fn the_long_context_marker_is_client_side() {
    assert_eq!(base_id("claude-sonnet-5-vn[1m]"), "claude-sonnet-5-vn");
    assert_eq!(base_id("claude-sonnet-5-vn[1M] "), "claude-sonnet-5-vn");
    assert_eq!(base_id("claude-sonnet-5-vn"), "claude-sonnet-5-vn");
    assert!(has_long_context("claude-opus-5-vn[1m]"));
    assert!(!has_long_context("claude-opus-5-vn"));
}

#[test]
fn one_pick_fills_every_slot() {
    let config = Config::default();
    let offered = usable(&listed(), &config);
    let slots = fill_slots("claude-opus-4-8-vn", &offered, &config, true);
    assert_eq!(slots.main, "claude-opus-4-8-vn[1m]");
    assert_eq!(slots.opus, "claude-opus-5-5-vn[1m]");
    assert_eq!(slots.sonnet, "claude-sonnet-5-5-vn[1m]");
    // No 1M variant for Haiku, whatever the switch says.
    assert_eq!(slots.haiku, "claude-haiku-5-5-vn");

    let slots = fill_slots("claude-sonnet-5-vn", &offered, &config, false);
    assert_eq!(slots.main, "claude-sonnet-5-vn");
    assert_eq!(slots.opus, "claude-opus-5-5-vn");
}

#[test]
fn a_pinned_slot_is_written_exactly_as_configured() {
    let mut config = Config::default();
    config.slots.haiku = "claude-haiku-4-5-vn".into();
    config.slots.opus = "claude-opus-5-vn[1m]".into();
    let offered = usable(&listed(), &config);
    let slots = fill_slots("claude-sonnet-5-5-vn", &offered, &config, false);
    assert_eq!(slots.haiku, "claude-haiku-4-5-vn");
    assert_eq!(slots.opus, "claude-opus-5-vn[1m]");
}

#[test]
fn a_family_the_key_lacks_falls_back_to_the_pick() {
    let config = Config::default();
    let offered = vec!["claude-sonnet-5-5-vn".to_string()];
    let slots = fill_slots("claude-sonnet-5-5-vn", &offered, &config, true);
    assert_eq!(slots.opus, "claude-sonnet-5-5-vn[1m]");
    assert_eq!(slots.haiku, "claude-sonnet-5-5-vn[1m]");
}

#[test]
fn switching_keys_keeps_the_main_model_while_it_is_allowed() {
    let offered = usable(&listed(), &Config::default());
    assert_eq!(
        keep_main(Some("claude-opus-4-8-vn[1m]"), &offered).as_deref(),
        Some("claude-opus-4-8-vn")
    );
    assert_eq!(
        keep_main(Some("claude-sonnet-5"), &offered).as_deref(),
        Some("claude-sonnet-5-5-vn")
    );
    assert_eq!(keep_main(None, &[]), None);
}
