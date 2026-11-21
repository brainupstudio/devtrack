//! Is a newer DevTrack out? One read of the latest release of the public site repository,
//! where the packages are published. Nothing about the person or their data is sent.
//!
//! On macOS the app also updates itself: it downloads the new disk image, checks it against the
//! published SHA-256 checksums, copies the new DevTrack.app next to its data, and leaves a small
//! script waiting for the app to end. When it ends (Quit, ⌘Q, logout, or "Restart and update")
//! the script swaps the app in place, and reopens it if asked. A file the app downloads itself
//! is not marked as downloaded from the internet, so macOS opens the new version without the
//! warning shown for unsigned apps.

use std::path::{Path, PathBuf};
use std::time::Duration;

/// This build's version.
pub const CURRENT: &str = env!("CARGO_PKG_VERSION");

const REPO: &str = "brainupstudio/devtrack.site";

/// The version this copy compares against: its own, or the one in DEVTRACK_PRETEND_VERSION, so
/// the whole update can be tried from a build that is not older than the latest release.
pub fn current() -> String {
    std::env::var("DEVTRACK_PRETEND_VERSION").ok().filter(|v| parse(v).is_some()).unwrap_or_else(|| CURRENT.to_string())
}

/// Set when the version is pretended: the check then runs at every start.
pub fn pretending() -> bool {
    std::env::var("DEVTRACK_PRETEND_VERSION").map(|v| parse(&v).is_some()).unwrap_or(false)
}

/// The DevTrack.app this process runs from, when it runs from one (not a development build).
pub fn installed_app() -> Option<PathBuf> {
    if !cfg!(target_os = "macos") {
        return None;
    }
    let exe = std::env::current_exe().ok()?.canonicalize().ok()?;
    // .../DevTrack.app/Contents/MacOS/devtrack
    let app = exe.parent()?.parent()?.parent()?.to_path_buf();
    if app.extension().map(|e| e == "app").unwrap_or(false) && app.join("Contents/Info.plist").is_file() {
        Some(app)
    } else {
        None
    }
}

/// The app can replace itself: it runs from a DevTrack.app in a folder it can write to.
pub fn can_update_itself() -> Option<PathBuf> {
    let app = installed_app()?;
    let dir = app.parent()?;
    let probe = dir.join(format!(".devtrack-write-test-{}", std::process::id()));
    match std::fs::File::create(&probe) {
        Ok(_) => {
            let _ = std::fs::remove_file(&probe);
            Some(app)
        }
        Err(_) => None,
    }
}

/// Where a downloaded version waits to be installed.
fn update_dir() -> PathBuf {
    dirs::cache_dir().unwrap_or_else(std::env::temp_dir).join("DevTrack").join("update")
}

/// The prepared copy of `version`, when it is there and complete.
pub fn staged(version: &str) -> Option<PathBuf> {
    let app = update_dir().join(version).join("DevTrack.app");
    app.join("Contents/Info.plist").is_file().then_some(app)
}

/// The file that asks the waiting script to reopen the app after the swap.
fn restart_flag() -> PathBuf {
    update_dir().join("restart")
}

/// Ask the waiting script to reopen DevTrack once it has installed the new version.
pub fn request_restart() -> Result<(), String> {
    std::fs::create_dir_all(update_dir()).and_then(|_| std::fs::write(restart_flag(), b"1")).map_err(|e| e.to_string())
}

fn sha256_hex(bytes: &[u8]) -> String {
    ring::digest::digest(&ring::digest::SHA256, bytes).as_ref().iter().map(|b| format!("{:02x}", b)).collect()
}

fn fetch(url: &str) -> Result<Vec<u8>, String> {
    let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(300)).build();
    let resp = agent.get(url).set("User-Agent", &format!("DevTrack/{}", CURRENT)).call().map_err(|e| match e {
        ureq::Error::Status(..) => "The update server did not answer as expected.".to_string(),
        ureq::Error::Transport(_) => "No connection to the update server.".to_string(),
    })?;
    let mut bytes = Vec::new();
    std::io::Read::read_to_end(&mut resp.into_reader(), &mut bytes).map_err(|_| "The download did not finish.".to_string())?;
    Ok(bytes)
}

/// Download `version` for macOS, check it, and copy its DevTrack.app into the update folder.
/// Returns the prepared app.
pub fn download(version: &str) -> Result<PathBuf, String> {
    if let Some(app) = staged(version) {
        return Ok(app);
    }
    let file = format!("DevTrack-{}-macOS.dmg", version);
    let base = format!("https://github.com/{}/releases/download/v{}", REPO, version);
    let dmg = fetch(&format!("{}/{}", base, file))?;
    let sums = String::from_utf8(fetch(&format!("{}/SHA256SUMS.txt", base))?).map_err(|_| "The update server sent an unreadable answer.".to_string())?;
    let expected = sums
        .lines()
        .filter_map(|l| {
            let mut it = l.split_whitespace();
            Some((it.next()?, it.next()?.trim_start_matches('*')))
        })
        .find(|(_, name)| *name == file)
        .map(|(h, _)| h.to_lowercase())
        .ok_or_else(|| "The new version has no published checksum, so it was not installed.".to_string())?;
    if sha256_hex(&dmg) != expected {
        return Err("The download does not match its checksum, so it was not installed.".into());
    }
    let dir = update_dir();
    // only the version being prepared is kept
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for e in entries.flatten() {
            if e.path().is_dir() {
                let _ = std::fs::remove_dir_all(e.path());
            }
        }
    }
    let work = dir.join(format!("{}.part", version));
    std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;
    let dmg_path = work.join(&file);
    std::fs::write(&dmg_path, &dmg).map_err(|e| e.to_string())?;
    let mount = work.join("volume");
    std::fs::create_dir_all(&mount).map_err(|e| e.to_string())?;
    let attached = std::process::Command::new("hdiutil")
        .args(["attach", "-nobrowse", "-readonly", "-quiet", "-mountpoint"])
        .arg(&mount)
        .arg(&dmg_path)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !attached {
        let _ = std::fs::remove_dir_all(&work);
        return Err("The new version could not be opened.".into());
    }
    let target = dir.join(version);
    let _ = std::fs::create_dir_all(&target);
    let copied = std::process::Command::new("ditto").arg(mount.join("DevTrack.app")).arg(target.join("DevTrack.app")).status().map(|s| s.success()).unwrap_or(false);
    let _ = std::process::Command::new("hdiutil").args(["detach", "-quiet"]).arg(&mount).status();
    let _ = std::fs::remove_dir_all(&work);
    if !copied {
        let _ = std::fs::remove_dir_all(&target);
        return Err("The new version could not be prepared.".into());
    }
    staged(version).ok_or_else(|| "The new version could not be prepared.".to_string())
}

/// Leave a script waiting for this process to end; then it replaces `app` with `new_app` and,
/// when a restart was asked for, opens it again. If the copy fails the old app is put back.
pub fn install_when_closed(app: &Path, new_app: &Path) -> Result<(), String> {
    let dir = update_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    // a fresh start: no restart unless asked for again
    let _ = std::fs::remove_file(restart_flag());
    let q = |p: &Path| format!("'{}'", p.display().to_string().replace('\'', "'\\''"));
    let script = format!(
        r#"#!/bin/sh
# DevTrack update: waits for DevTrack to end, then replaces it with the prepared version.
PID={pid}
APP={app}
NEW={new}
FLAG={flag}
while kill -0 "$PID" 2>/dev/null; do sleep 0.5; done
[ -d "$NEW" ] || exit 0
OLD="$APP.previous"
rm -rf "$OLD"
mv "$APP" "$OLD" || exit 1
if ditto "$NEW" "$APP"; then
    rm -rf "$OLD" "$(dirname "$NEW")"
else
    rm -rf "$APP"
    mv "$OLD" "$APP"
fi
xattr -dr com.apple.quarantine "$APP" 2>/dev/null
if [ -f "$FLAG" ]; then
    rm -f "$FLAG"
    open "$APP"
fi
"#,
        pid = std::process::id(),
        app = q(app),
        new = q(new_app),
        flag = q(&restart_flag()),
    );
    let path = dir.join("install.sh");
    std::fs::write(&path, script).map_err(|e| e.to_string())?;
    std::process::Command::new("/bin/sh")
        .arg(&path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Where the packages are published. GitHub answers this address with a redirect to the latest
/// release, ".../releases/tag/v0.2.0": the version is read from there. Not the GitHub API, which
/// allows 60 requests an hour per address without an account, shared by a whole office network.
const LATEST_URL: &str = "https://github.com/brainupstudio/devtrack.site/releases/latest";
pub const DOWNLOAD_PAGE: &str = "https://brainupstudio.github.io/devtrack.site/#download";

/// Once a day is enough: releases are rare.
pub const CHECK_EVERY_MS: i64 = 24 * 60 * 60 * 1000;

/// The latest published version ("0.2.0", without the "v").
pub fn latest() -> Result<String, String> {
    let unreadable = || "The update server sent an unreadable answer.".to_string();
    // the redirect itself is the answer: it is not followed, so no page is downloaded
    let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(15)).redirects(0).build();
    let resp = agent
        .head(LATEST_URL)
        .set("User-Agent", &format!("DevTrack/{}", CURRENT))
        .call()
        .map_err(|e| match e {
            ureq::Error::Status(..) => "The update server did not answer as expected.".to_string(),
            ureq::Error::Transport(_) => "No connection to the update server.".to_string(),
        })?;
    let location = resp.header("location").ok_or_else(unreadable)?;
    version_from_url(location).ok_or_else(unreadable)
}

/// "https://github.com/o/r/releases/tag/v0.2.0" → "0.2.0"; anything else (for instance the
/// releases list, when there is no release yet) → None.
fn version_from_url(url: &str) -> Option<String> {
    let path = url.split(['?', '#']).next()?.trim_end_matches('/');
    let (head, tag) = path.rsplit_once('/')?;
    if !head.ends_with("/releases/tag") {
        return None;
    }
    let version = tag.trim().trim_start_matches(['v', 'V']).to_string();
    parse(&version)?;
    Some(version)
}

/// "1.2.3" (or "1.2") as numbers; anything after a "-" or "+" is ignored.
fn parse(v: &str) -> Option<(u64, u64, u64)> {
    let core = v.trim().trim_start_matches(['v', 'V']).split(['-', '+']).next()?;
    let mut parts = core.split('.').map(|p| p.parse::<u64>().ok());
    let major = parts.next()??;
    let minor = parts.next().unwrap_or(Some(0))?;
    let patch = parts.next().unwrap_or(Some(0))?;
    Some((major, minor, patch))
}

/// `latest` is a later version than `current`.
pub fn is_newer(latest: &str, current: &str) -> bool {
    match (parse(latest), parse(current)) {
        (Some(a), Some(b)) => a > b,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_versions_as_numbers() {
        assert!(is_newer("0.2.0", "0.1.0"));
        assert!(is_newer("v0.10.0", "0.9.3"));
        assert!(is_newer("1.0", "0.9.9"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("0.1.0", "0.2.0"));
        assert!(!is_newer("0.2.0-beta", "0.2.0"));
        assert!(!is_newer("garbage", "0.1.0"));
    }

    #[test]
    fn reads_the_version_from_the_release_address() {
        assert_eq!(version_from_url("https://github.com/o/r/releases/tag/v0.1.2").as_deref(), Some("0.1.2"));
        assert_eq!(version_from_url("https://github.com/o/r/releases/tag/0.10.0/").as_deref(), Some("0.10.0"));
        assert_eq!(version_from_url("https://github.com/o/r/releases"), None);
        assert_eq!(version_from_url("https://github.com/o/r/releases/tag/latest-build"), None);
    }

    /// Asks GitHub for real: `cargo test -- --ignored latest_release`
    #[test]
    #[ignore]
    fn latest_release_from_github() {
        let v = latest().expect("an answer from GitHub");
        assert!(parse(&v).is_some(), "{v}");
        println!("latest: {v}");
    }
}

#[cfg(test)]
mod download_tests {
    /// Downloads and prepares a real release: `cargo test -- --ignored prepares_a_release`
    #[test]
    #[ignore]
    fn prepares_a_release() {
        let v = std::env::var("DEVTRACK_TEST_VERSION").unwrap_or_else(|_| "0.1.2".into());
        match super::download(&v) {
            Ok(p) => println!("prepared: {}", p.display()),
            Err(e) => panic!("download failed: {e}"),
        }
    }
}

