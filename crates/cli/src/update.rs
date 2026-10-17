//! Is there a newer diagram? At most once a day the editor asks GitHub for
//! the latest release (in the background, through `curl`); the answer is
//! cached, and a failure (offline, rate limit) is quiet.
//! `DIAGRAM_NO_UPDATE_CHECK=1` turns it off; so does `CI`.
use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

const REPO: &str = "dobrinov/diagram";
const DAY: u64 = 24 * 60 * 60;

/// A release newer than this binary.
#[derive(Debug, Clone)]
pub struct Update {
    pub latest: String,
    /// How to get it: `brew upgrade diagram`, or where to download it.
    pub how: String,
    /// The release, with its notes.
    pub url: String,
}

/// The answer of the check [`start`] began: `None` while it runs, then
/// `Some(None)` when this is the latest (or the check is off or failed).
static FOUND: OnceLock<Option<Update>> = OnceLock::new();

/// Look for a newer release in the background; say so on stderr when there is one.
pub fn start() {
    std::thread::spawn(|| {
        let found = check();
        if let Some(u) = &found {
            eprintln!(
                "diagram: {} is out (you have {}): {}",
                u.latest,
                env!("CARGO_PKG_VERSION"),
                u.how
            );
        }
        let _ = FOUND.set(found);
    });
}

/// What [`start`] found, once it is done.
pub fn found() -> Option<&'static Option<Update>> {
    FOUND.get()
}

fn check() -> Option<Update> {
    if off() {
        return None;
    }
    let latest = latest()?;
    newer(&latest, env!("CARGO_PKG_VERSION")).then(|| Update {
        url: format!("https://github.com/{REPO}/releases/tag/v{latest}"),
        how: how(),
        latest,
    })
}

fn off() -> bool {
    let set = |k: &str| std::env::var(k).is_ok_and(|v| !v.is_empty() && v != "0");
    set("DIAGRAM_NO_UPDATE_CHECK") || set("CI")
}

/// The latest release's version: from the cache when it is less than a day old.
fn latest() -> Option<String> {
    let file = cache_file();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if let Some(cached) = file
        .as_ref()
        .and_then(|f| std::fs::read_to_string(f).ok())
        .and_then(|s| fresh(&s, now))
    {
        return Some(cached);
    }
    let v = fetch()?;
    if let Some(f) = &file {
        if let Some(dir) = f.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(f, format!("{now} {v}\n"));
    }
    Some(v)
}

/// The version in a cache file (`SECONDS VERSION`) written less than a day before `now`.
fn fresh(cache: &str, now: u64) -> Option<String> {
    let (at, v) = cache.trim().split_once(' ')?;
    let at: u64 = at.parse().ok()?;
    (now.saturating_sub(at) < DAY && !v.is_empty()).then(|| v.to_string())
}

fn fetch() -> Option<String> {
    let out = Command::new("curl")
        .args([
            "-fsSL",
            "--max-time",
            "5",
            "-H",
            "Accept: application/vnd.github+json",
        ])
        .arg(format!(
            "https://api.github.com/repos/{REPO}/releases/latest"
        ))
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
    Some(v["tag_name"].as_str()?.trim_start_matches('v').to_string())
}

fn cache_file() -> Option<PathBuf> {
    let env = |k: &str| {
        std::env::var_os(k)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    let dir = env("XDG_CACHE_HOME").or_else(|| {
        if cfg!(windows) {
            env("LOCALAPPDATA")
        } else if cfg!(target_os = "macos") {
            env("HOME").map(|h| h.join("Library/Caches"))
        } else {
            env("HOME").map(|h| h.join(".cache"))
        }
    })?;
    Some(dir.join("diagram").join("latest-version"))
}

/// Whether version `a` comes after `b` (`x.y.z`; a missing part is 0).
fn newer(a: &str, b: &str) -> bool {
    let parts = |v: &str| -> Vec<u64> {
        v.split(['.', '-'])
            .take(3)
            .map(|p| p.parse().unwrap_or(0))
            .collect()
    };
    parts(a) > parts(b)
}

/// How this copy was installed decides how to upgrade it.
fn how() -> String {
    let brew = std::env::current_exe()
        .ok()
        .and_then(|p| p.canonicalize().ok())
        .is_some_and(|p| p.to_string_lossy().contains("/Cellar/"));
    if brew {
        "brew upgrade diagram".into()
    } else {
        format!("https://github.com/{REPO}/releases/latest")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_by_number() {
        assert!(newer("0.34.0", "0.33.0"));
        assert!(newer("0.10.0", "0.9.9"));
        assert!(newer("1.0.0", "0.99.0"));
        assert!(!newer("0.33.0", "0.33.0"));
        assert!(!newer("0.32.1", "0.33.0"));
    }

    #[test]
    fn the_cache_lasts_a_day() {
        assert_eq!(
            fresh("1000 0.34.0\n", 1000 + DAY - 1).as_deref(),
            Some("0.34.0")
        );
        assert_eq!(fresh("1000 0.34.0\n", 1000 + DAY), None);
        assert_eq!(fresh("garbage", 1000), None);
        assert_eq!(fresh("", 1000), None);
    }
}
