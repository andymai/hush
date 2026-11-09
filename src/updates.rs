//! The opt-in check for a newer Hush release.
//!
//! With `updates.check` on, the daemon asks the GitHub releases API for the
//! latest release at most once a day and shows a notification once for each
//! newer version. The request carries no data other than the `hush/<version>`
//! user agent. A package-manager install never checks, because the package
//! manager handles its updates.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;
use tracing::{debug, info, warn};

pub const LATEST_RELEASE_API: &str = "https://api.github.com/repos/andymai/hush/releases/latest";
pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

const INTERVAL: chrono::Duration = chrono::Duration::hours(24);
const STARTUP_DELAY: Duration = Duration::from_secs(30);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
// tokio's timers stop while the machine sleeps, so a single 24 h sleep can
// last days on a laptop. Waking each hour to compare wall-clock time keeps
// the interval close to a day.
const MAX_SLEEP: Duration = Duration::from_secs(3600);

/// How this copy of Hush was installed, which decides whether it checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallKind {
    AppImage,
    PackageManager,
    /// The install script's tarball, a source build, or anything else that
    /// no package manager updates.
    SelfManaged,
}

impl InstallKind {
    pub fn detect(exe: &Path, appimage: Option<&OsStr>) -> Self {
        if appimage.is_some_and(|value| !value.is_empty()) {
            return InstallKind::AppImage;
        }
        let managed = (exe.starts_with("/usr") && !exe.starts_with("/usr/local"))
            || exe.starts_with("/nix/store");
        if managed {
            InstallKind::PackageManager
        } else {
            InstallKind::SelfManaged
        }
    }

    /// This process's install kind, found once.
    pub fn current() -> Self {
        static KIND: OnceLock<InstallKind> = OnceLock::new();
        *KIND.get_or_init(|| {
            let exe = std::env::current_exe().unwrap_or_default();
            Self::detect(&exe, std::env::var_os("APPIMAGE").as_deref())
        })
    }

    pub fn checks(self) -> bool {
        self != InstallKind::PackageManager
    }

    pub fn describe(self) -> &'static str {
        match self {
            InstallKind::AppImage => "AppImage",
            InstallKind::PackageManager => "package manager",
            InstallKind::SelfManaged => "tarball or source build",
        }
    }
}

/// `X.Y.Z`, with or without a leading `v`. A pre-release suffix is not a
/// release version, so it gives `None`.
pub fn parse_version(text: &str) -> Option<(u64, u64, u64)> {
    let text = text.trim();
    let text = text.strip_prefix('v').unwrap_or(text);
    let mut parts = text.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
}

pub fn is_newer(candidate: &str, current: &str) -> bool {
    match (parse_version(candidate), parse_version(current)) {
        (Some(candidate), Some(current)) => candidate > current,
        _ => false,
    }
}

/// How long to wait before the next check. A missing or future timestamp
/// (the clock moved back) means now.
pub fn next_check_in(last_checked: Option<DateTime<Utc>>, now: DateTime<Utc>) -> Duration {
    let Some(last) = last_checked else {
        return Duration::ZERO;
    };
    let elapsed = now - last;
    if elapsed < chrono::Duration::zero() || elapsed >= INTERVAL {
        return Duration::ZERO;
    }
    (INTERVAL - elapsed).to_std().unwrap_or(Duration::ZERO)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    /// Without the leading `v`.
    pub version: String,
    pub url: String,
}

pub fn parse_release(body: &str) -> anyhow::Result<Release> {
    #[derive(Deserialize)]
    struct Latest {
        tag_name: String,
        html_url: String,
    }
    let latest: Latest = serde_json::from_str(body)?;
    let version = latest.tag_name.trim().trim_start_matches('v').to_string();
    if parse_version(&version).is_none() {
        anyhow::bail!("unexpected release tag '{}'", latest.tag_name);
    }
    Ok(Release {
        version,
        url: latest.html_url,
    })
}

/// What the last check found, kept in `$XDG_STATE_HOME/hush/update-check.json`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UpdateState {
    pub last_checked: Option<DateTime<Utc>>,
    pub latest_version: Option<String>,
    pub latest_url: Option<String>,
    pub notified_version: Option<String>,
}

impl UpdateState {
    pub fn path() -> PathBuf {
        crate::config::paths::state_dir().join("update-check.json")
    }

    /// The saved state, or an empty one when the file is missing or unreadable.
    pub fn load_from(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn save_to(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_string_pretty(self)?)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }

    /// The newer release and its page, when the last check found one.
    pub fn newer_than(&self, current: &str) -> Option<(&str, &str)> {
        let version = self.latest_version.as_deref()?;
        let url = self.latest_url.as_deref()?;
        is_newer(version, current).then_some((version, url))
    }

    /// Record a check at `now`. A failed request still counts, so a machine
    /// without network access does not ask again at each daemon start.
    pub fn record(&mut self, now: DateTime<Utc>, release: Option<Release>) {
        self.last_checked = Some(now);
        if let Some(release) = release {
            self.latest_version = Some(release.version);
            self.latest_url = Some(release.url);
        }
    }

    /// The version to announce: a newer release that got no notification yet.
    pub fn to_announce(&self, current: &str) -> Option<(String, String)> {
        let (version, url) = self.newer_than(current)?;
        if self.notified_version.as_deref() == Some(version) {
            return None;
        }
        Some((version.to_string(), url.to_string()))
    }
}

async fn fetch_latest() -> anyhow::Result<Release> {
    let client = reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .user_agent(format!("hush/{}", CURRENT_VERSION))
        .build()?;
    let body = client
        .get(LATEST_RELEASE_API)
        .header(reqwest::header::ACCEPT, "application/vnd.github+json")
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    parse_release(&body)
}

async fn check_once(path: &Path) {
    let mut state = UpdateState::load_from(path);
    let release = match fetch_latest().await {
        Ok(release) => {
            debug!("Latest Hush release: {}", release.version);
            Some(release)
        },
        Err(e) => {
            warn!("Update check failed: {}", e);
            None
        },
    };
    state.record(Utc::now(), release);
    if let Some((version, url)) = state.to_announce(CURRENT_VERSION) {
        info!("Hush {} is available: {}", version, url);
        crate::notify::notify(
            &format!("Hush {} is available", version),
            &format!(
                "You use {}. Get the new release at {}",
                CURRENT_VERSION, url
            ),
        );
        state.notified_version = Some(version);
    }
    if let Err(e) = state.save_to(path) {
        warn!("Could not save the update check result: {}", e);
    }
}

/// The daemon's update loop. It returns at once for a package-manager install.
pub async fn run(kind: InstallKind) {
    if !kind.checks() {
        info!("Update check is on, but the package manager handles updates");
        return;
    }
    let path = UpdateState::path();
    tokio::time::sleep(STARTUP_DELAY).await;
    loop {
        let state = UpdateState::load_from(&path);
        let wait = next_check_in(state.last_checked, Utc::now());
        if wait.is_zero() {
            check_once(&path).await;
        } else {
            tokio::time::sleep(wait.min(MAX_SLEEP)).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_by_number() {
        assert_eq!(parse_version("v0.7.3"), Some((0, 7, 3)));
        assert_eq!(parse_version("1.10.0"), Some((1, 10, 0)));
        assert_eq!(parse_version("1.0.0-rc.1"), None);
        assert_eq!(parse_version("1.0"), None);
        assert_eq!(parse_version("1.0.0.1"), None);
        assert!(is_newer("v0.10.0", "0.9.9"));
        assert!(is_newer("1.0.0", "0.7.3"));
        assert!(!is_newer("0.7.3", "0.7.3"));
        assert!(!is_newer("0.7.2", "0.7.3"));
        assert!(!is_newer("garbage", "0.7.3"));
    }

    #[test]
    fn the_install_kind_follows_the_binary_location() {
        let detect = |exe: &str, appimage: Option<&str>| {
            InstallKind::detect(Path::new(exe), appimage.map(OsStr::new))
        };
        assert_eq!(
            detect(
                "/tmp/.mount_hush/usr/bin/hush",
                Some("/home/a/Hush.AppImage")
            ),
            InstallKind::AppImage
        );
        assert_eq!(detect("/usr/bin/hush", None), InstallKind::PackageManager);
        assert_eq!(
            detect("/nix/store/abc-hush-0.7.3/bin/hush", None),
            InstallKind::PackageManager
        );
        assert_eq!(
            detect("/usr/local/bin/hush", None),
            InstallKind::SelfManaged
        );
        assert_eq!(
            detect("/home/a/.local/bin/hush", None),
            InstallKind::SelfManaged
        );
        assert_eq!(
            detect("/usr/bin/hush", Some("")),
            InstallKind::PackageManager
        );
        assert!(!InstallKind::PackageManager.checks());
        assert!(InstallKind::AppImage.checks());
        assert!(InstallKind::SelfManaged.checks());
    }

    #[test]
    fn a_check_waits_a_day_after_the_last_one() {
        let now = Utc::now();
        assert_eq!(next_check_in(None, now), Duration::ZERO);
        assert_eq!(
            next_check_in(Some(now - chrono::Duration::hours(25)), now),
            Duration::ZERO
        );
        assert_eq!(
            next_check_in(Some(now - chrono::Duration::hours(24)), now),
            Duration::ZERO
        );
        assert_eq!(
            next_check_in(Some(now - chrono::Duration::hours(23)), now),
            Duration::from_secs(3600)
        );
        assert_eq!(
            next_check_in(Some(now + chrono::Duration::hours(2)), now),
            Duration::ZERO,
            "a clock that moved back does not postpone the check"
        );
    }

    #[test]
    fn the_state_file_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/update-check.json");
        assert_eq!(UpdateState::load_from(&path), UpdateState::default());

        let mut state = UpdateState::default();
        state.record(
            Utc::now(),
            Some(Release {
                version: "9.0.0".to_string(),
                url: "https://example.invalid/v9.0.0".to_string(),
            }),
        );
        state.save_to(&path).unwrap();
        assert_eq!(UpdateState::load_from(&path), state);

        std::fs::write(&path, "not json").unwrap();
        assert_eq!(UpdateState::load_from(&path), UpdateState::default());
    }

    #[test]
    fn a_new_version_is_announced_once() {
        let mut state = UpdateState::default();
        state.record(Utc::now(), None);
        assert!(state.last_checked.is_some(), "a failed check still counts");
        assert_eq!(state.to_announce("0.7.3"), None);

        state.record(
            Utc::now(),
            Some(Release {
                version: "0.8.0".to_string(),
                url: "https://github.com/andymai/hush/releases/tag/v0.8.0".to_string(),
            }),
        );
        let (version, url) = state.to_announce("0.7.3").unwrap();
        assert_eq!(version, "0.8.0");
        assert!(url.ends_with("v0.8.0"));
        state.notified_version = Some(version);
        assert_eq!(state.to_announce("0.7.3"), None);
        assert_eq!(state.to_announce("0.8.0"), None, "nothing newer than 0.8.0");
        assert!(state.newer_than("0.8.0").is_none());
    }

    #[test]
    fn the_release_response_parses() {
        let body = r#"{"url":"https://api.github.com/repos/andymai/hush/releases/1",
            "html_url":"https://github.com/andymai/hush/releases/tag/v0.7.3",
            "tag_name":"v0.7.3","name":"v0.7.3","draft":false,"prerelease":false}"#;
        let release = parse_release(body).unwrap();
        assert_eq!(release.version, "0.7.3");
        assert_eq!(
            release.url,
            "https://github.com/andymai/hush/releases/tag/v0.7.3"
        );
        assert!(parse_release(r#"{"tag_name":"nightly","html_url":"x"}"#).is_err());
        assert!(parse_release(r#"{"message":"Not Found"}"#).is_err());
    }
}
