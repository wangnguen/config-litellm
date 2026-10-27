//! A settings file belongs to its folder first: whatever someone added by
//! hand must still be there after ccl saves.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use ccl_core::models::Slots;
use ccl_core::settings::{self, ENV_API_KEY, ENV_AUTH_TOKEN, Settings};
use serde_json::json;

/// A fresh folder under the system temp dir, unique per test.
fn scratch() -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "ccl-test-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn slots() -> Slots {
    Slots {
        main: "claude-sonnet-5-5-vn[1m]".into(),
        opus: "claude-opus-5-5-vn[1m]".into(),
        sonnet: "claude-sonnet-5-5-vn[1m]".into(),
        haiku: "claude-haiku-5-5-vn".into(),
    }
}

#[test]
fn a_new_folder_starts_from_the_template() {
    let dir = scratch();
    let mut s = Settings::load(&dir).unwrap();
    assert!(!s.exists());
    s.apply(
        &settings::default_template(),
        "https://gw/",
        "sk-1",
        &slots(),
        Some("high"),
    );
    s.save().unwrap();

    let s = Settings::load(&dir).unwrap();
    assert!(s.exists());
    assert_eq!(
        s.env("CLAUDE_CODE_ENABLE_GATEWAY_MODEL_DISCOVERY"),
        Some("1")
    );
    assert_eq!(
        s.env("ANTHROPIC_SMALL_FAST_MODEL"),
        Some("claude-haiku-5-5-vn")
    );
    assert_eq!(s.main_model(), Some("claude-sonnet-5-5-vn[1m]"));
    assert_eq!(s.effort(), Some("high"));
    assert_eq!(s.key(), Some("sk-1"));
}

#[test]
fn hand_written_fields_survive_a_save() {
    let dir = scratch();
    let file = dir.join(".claude").join("settings.local.json");
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    let original = json!({
        "hooks": {"Stop": [{"command": "say done"}]},
        "env": {"MY_VAR": "kept", "ANTHROPIC_API_KEY": "sk-old"},
        "permissions": {"deny": ["Bash(rm:*)"]}
    });
    fs::write(&file, original.to_string()).unwrap();

    let mut s = Settings::load(&dir).unwrap();
    s.apply(
        &settings::default_template(),
        "https://gw",
        "sk-new",
        &slots(),
        None,
    );
    s.save().unwrap();

    let saved: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
    assert_eq!(saved["hooks"], original["hooks"]);
    assert_eq!(saved["permissions"], original["permissions"]);
    assert_eq!(saved["env"]["MY_VAR"], "kept");
    assert_eq!(saved["env"][ENV_AUTH_TOKEN], "sk-new");
    assert!(
        saved["env"].get(ENV_API_KEY).is_none(),
        "both keys would leave the choice to Claude Code"
    );
    assert!(
        saved.get("effortLevel").is_none(),
        "no effort given, none written"
    );
}

#[test]
fn broken_json_is_reported_and_never_overwritten() {
    let dir = scratch();
    let file = dir.join(".claude").join("settings.local.json");
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(&file, "{ \"env\": ").unwrap();
    assert!(Settings::load(&dir).is_err());
    assert_eq!(fs::read_to_string(&file).unwrap(), "{ \"env\": ");
}

#[test]
fn masked_copy_hides_the_key() {
    let dir = scratch();
    let mut s = Settings::load(&dir).unwrap();
    s.apply(
        &json!({}),
        "https://gw",
        "sk-abcdefghijklmnop",
        &slots(),
        None,
    );
    assert_eq!(s.masked()["env"][ENV_AUTH_TOKEN], "sk-…mnop");
    assert_eq!(s.key(), Some("sk-abcdefghijklmnop"));
}

#[test]
fn gitignore_gets_the_one_file_once_and_only_in_a_repo() {
    let dir = scratch();
    assert!(
        !settings::ensure_gitignored(&dir).unwrap(),
        "not a git repo"
    );

    fs::create_dir(dir.join(".git")).unwrap();
    fs::write(dir.join(".gitignore"), "target").unwrap();
    assert!(settings::ensure_gitignored(&dir).unwrap());
    assert!(!settings::ensure_gitignored(&dir).unwrap());
    assert_eq!(
        fs::read_to_string(dir.join(".gitignore")).unwrap(),
        "target\n.claude/settings.local.json\n"
    );
}

#[test]
fn an_ignored_claude_folder_counts() {
    let dir = scratch();
    fs::create_dir(dir.join(".git")).unwrap();
    fs::write(dir.join(".gitignore"), "/.claude/\n").unwrap();
    assert!(!settings::ensure_gitignored(&dir).unwrap());
}
