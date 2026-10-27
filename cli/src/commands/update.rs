//! `ccl update`: replace this binary with the latest GitHub release.
//!
//! Releases ship bare binaries, one per target, so an update is a download
//! and a rename: no archive to unpack, no installer to run.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};

use crate::term::{self, Paint};
use crate::{REPO, VERSION};

const CHECK_TIMEOUT: Duration = Duration::from_secs(20);
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(300);
/// Far above any ccl build; only there so a wrong URL cannot fill the disk.
const MAX_BINARY_BYTES: u64 = 64 * 1024 * 1024;

/// The release asset built for this machine, named as release.yml names it.
fn asset_name() -> Option<&'static str> {
    if cfg!(all(windows, target_arch = "x86_64")) {
        Some("ccl-x86_64-pc-windows-msvc.exe")
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        Some("ccl-x86_64-unknown-linux-gnu")
    } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        Some("ccl-aarch64-apple-darwin")
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        Some("ccl-x86_64-apple-darwin")
    } else {
        None
    }
}

pub fn run() -> Result<()> {
    let tag = term::waiting("Checking for a new release…", latest_tag)?;
    let latest = tag.trim_start_matches('v');
    if !is_newer(latest, VERSION) {
        println!("ccl {VERSION} is the latest.");
        return Ok(());
    }
    let asset = asset_name().ok_or_else(|| {
        anyhow!("no release build for this platform; build from source: cargo install --git https://github.com/{REPO} ccl")
    })?;
    println!("Updating ccl {VERSION} → {latest}");

    let url = format!("https://github.com/{REPO}/releases/download/{tag}/{asset}");
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(DOWNLOAD_TIMEOUT))
        .http_status_as_error(false)
        .build()
        .into();
    let mut resp = term::waiting("Downloading…", || {
        agent.get(&url).header("User-Agent", "ccl").call()
    })
    .context("download failed")?;
    match resp.status().as_u16() {
        200 => {}
        404 => bail!("release {tag} has no {asset}"),
        status => bail!("download failed: HTTP {status}"),
    }
    let bytes = resp
        .body_mut()
        .with_config()
        .limit(MAX_BINARY_BYTES)
        .read_to_vec()
        .context("download failed")?;
    replace_running_binary(&bytes)?;
    println!("{} Updated to {latest}.", "✓".green());
    Ok(())
}

/// The newest release's tag, e.g. `v0.2.0`.
///
/// `github.com/<repo>/releases/latest` redirects to `.../releases/tag/<tag>`,
/// so the tag is read off the redirect. api.github.com would allow only 60
/// unauthenticated requests an hour per IP, which an office shares.
fn latest_tag() -> Result<String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .max_redirects(0)
        .http_status_as_error(false)
        .timeout_global(Some(CHECK_TIMEOUT))
        .build()
        .into();
    let resp = agent
        .get(&format!("https://github.com/{REPO}/releases/latest"))
        .header("User-Agent", "ccl")
        .call()
        .context("cannot reach GitHub")?;
    let status = resp.status().as_u16();
    let location = resp
        .headers()
        .get("location")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    if !(300..400).contains(&status) {
        bail!("GitHub answered HTTP {status}");
    }
    match location.rsplit_once("/releases/tag/") {
        Some((_, tag)) if !tag.is_empty() => Ok(tag.to_string()),
        // With no release at all, "latest" points at the release list.
        _ => bail!("no release has been published yet"),
    }
}

/// True when `latest` is a higher version than `current`, part by part.
fn is_newer(latest: &str, current: &str) -> bool {
    let parts = |v: &str| -> Vec<u64> {
        v.split('.')
            .map(|p| p.trim().parse().unwrap_or(0))
            .collect()
    };
    parts(latest) > parts(current)
}

fn replace_running_binary(bytes: &[u8]) -> Result<()> {
    let exe = env::current_exe()?;
    let new = exe.with_extension("new");
    fs::write(&new, bytes).with_context(|| format!("cannot write {}", new.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&new, fs::Permissions::from_mode(0o755))?;
    }
    // Windows will not overwrite a running .exe, but it will rename one.
    if cfg!(windows) {
        let old = old_binary(&exe);
        let _ = fs::remove_file(&old);
        fs::rename(&exe, &old).with_context(|| format!("cannot move {} aside", exe.display()))?;
        if let Err(err) = fs::rename(&new, &exe) {
            let _ = fs::rename(&old, &exe);
            return Err(err).context("cannot put the new ccl.exe in place");
        }
        return Ok(());
    }
    fs::rename(&new, &exe).with_context(|| format!("cannot replace {}", exe.display()))
}

fn old_binary(exe: &Path) -> PathBuf {
    exe.with_extension("old.exe")
}

/// The previous ccl.exe, left behind by an update on Windows because it was
/// still running. Gone by the next start.
pub fn clean_leftovers() {
    if cfg!(windows)
        && let Ok(exe) = env::current_exe()
    {
        let _ = fs::remove_file(old_binary(&exe));
    }
}

#[cfg(test)]
mod tests {
    use super::is_newer;

    #[test]
    fn versions_compare_part_by_part() {
        assert!(is_newer("0.1.10", "0.1.9"));
        assert!(is_newer("1.0.0", "0.9.9"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("0.1.0", "0.2.0"));
    }
}
