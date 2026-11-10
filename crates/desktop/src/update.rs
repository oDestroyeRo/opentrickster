use serde::Deserialize;
use std::{
    fmt,
    path::{Path, PathBuf},
    str::FromStr,
    time::{Duration, Instant},
};

pub const RELEASES_API: &str =
    "https://api.github.com/repos/oDestroyeRo/opentrickster/releases/latest";
pub const RELEASES_PAGE: &str = "https://github.com/oDestroyeRo/opentrickster/releases/latest";
const DOWNLOADS: &str = "https://github.com/oDestroyeRo/opentrickster/releases/download";
pub const USER_AGENT: &str = "OpenTrickster-Updater";
pub const ARCHIVE_NAME: &str = "OpenTrickster-macOS-arm64.zip";
pub const CHECKSUMS_NAME: &str = "SHA256SUMS";
pub const BUNDLE_IDENTIFIER: &str = "org.opentrickster.desktop";
pub const MAX_JSON: u64 = 1 << 20;
pub const MAX_ARCHIVE: u64 = 300 << 20;
pub const CHECK_EVERY: Duration = Duration::from_secs(6 * 60 * 60);
pub const RETRY_AFTER: Duration = Duration::from_secs(15 * 60);
pub const RECHECK_AFTER: Duration = Duration::from_secs(60);
pub const GAME_UPDATED_UPDATING: &str = "The game updated. OpenTrickster is updating itself.";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version {
    major: u64,
    minor: u64,
    patch: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BadVersion;

impl Version {
    pub const fn new(major: u64, minor: u64, patch: u64) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    pub fn from_tag(tag: &str) -> Option<Self> {
        tag.strip_prefix('v')?.parse().ok()
    }
}

impl FromStr for Version {
    type Err = BadVersion;

    fn from_str(text: &str) -> Result<Self, BadVersion> {
        let mut parts = text.split('.');
        let (Some(major), Some(minor), Some(patch), None) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            return Err(BadVersion);
        };
        Ok(Self::new(number(major)?, number(minor)?, number(patch)?))
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

// `u64::from_str` accepts a leading plus sign, and semver forbids leading zeros.
fn number(part: &str) -> Result<u64, BadVersion> {
    let digits = !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit());
    if !digits || (part.len() > 1 && part.starts_with('0')) {
        return Err(BadVersion);
    }
    part.parse().map_err(|_| BadVersion)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Release {
    pub version: Version,
}

impl Release {
    pub fn archive_url(&self) -> String {
        format!("{DOWNLOADS}/v{}/{ARCHIVE_NAME}", self.version)
    }

    pub fn checksums_url(&self) -> String {
        format!("{DOWNLOADS}/v{}/{CHECKSUMS_NAME}", self.version)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Malformed;

#[derive(Deserialize)]
struct ReleaseJson {
    tag_name: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<AssetJson>,
}

#[derive(Deserialize)]
struct AssetJson {
    name: String,
    size: u64,
    #[serde(default)]
    state: Option<String>,
}

impl AssetJson {
    fn usable(&self, name: &str, cap: u64) -> bool {
        self.name == name
            && self
                .state
                .as_deref()
                .is_none_or(|state| state == "uploaded")
            && (1..=cap).contains(&self.size)
    }
}

/// The release the app should install, or none. Only a malformed answer is an error.
pub fn offer(json: &[u8], current: Version) -> Result<Option<Release>, Malformed> {
    let release: ReleaseJson = serde_json::from_slice(json).map_err(|_| Malformed)?;
    let Some(version) = Version::from_tag(&release.tag_name) else {
        return Ok(None);
    };
    if release.draft || release.prerelease || version <= current {
        return Ok(None);
    }
    let has = |name, cap| release.assets.iter().any(|asset| asset.usable(name, cap));
    if has(ARCHIVE_NAME, MAX_ARCHIVE) && has(CHECKSUMS_NAME, MAX_JSON) {
        Ok(Some(Release { version }))
    } else {
        Ok(None)
    }
}

/// GitHub serves release assets from its own hosts. Every hop of a redirect must stay on them.
pub fn trusted_hop(scheme: &str, host: Option<&str>) -> bool {
    scheme == "https"
        && host.is_some_and(|host| {
            host == "github.com"
                || host == "api.github.com"
                || host.ends_with(".githubusercontent.com")
        })
}

pub type Digest = [u8; 32];

/// The digest that `SHA256SUMS` lists for `name`. A malformed line, a missing entry, or two
/// different entries for the name all yield none.
pub fn expected_digest(sums: &str, name: &str) -> Option<Digest> {
    let mut found = None;
    for line in sums.lines().filter(|line| !line.trim().is_empty()) {
        let (hash, rest) = line.split_once(' ')?;
        let entry = rest
            .strip_prefix(' ')
            .or_else(|| rest.strip_prefix('*'))?
            .trim_end();
        let digest = decode_digest(hash)?;
        if entry == name {
            if found.is_some_and(|other| other != digest) {
                return None;
            }
            found = Some(digest);
        }
    }
    found
}

fn decode_digest(hex: &str) -> Option<Digest> {
    if hex.len() != 64 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let mut digest = [0; 32];
    for (byte, pair) in digest.iter_mut().zip(hex.as_bytes().chunks(2)) {
        *byte = u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok()?;
    }
    Some(digest)
}

pub fn digest_matches(actual: &Digest, expected: &Digest) -> Result<(), Reject> {
    if actual == expected {
        Ok(())
    } else {
        Err(Reject::Checksum)
    }
}

#[derive(Deserialize)]
struct BuildJson {
    version: String,
    source_dirty: bool,
    profile: String,
}

pub fn accept_build(json: &[u8], release: Version) -> Result<(), Reject> {
    let build: BuildJson = serde_json::from_slice(json).map_err(|_| Reject::BuildInfo)?;
    if build.version.parse::<Version>() != Ok(release) {
        Err(Reject::Version)
    } else if build.source_dirty {
        Err(Reject::Dirty)
    } else if build.profile != "release" {
        Err(Reject::Profile)
    } else {
        Ok(())
    }
}

pub fn accept_identifier(found: &str) -> Result<(), Reject> {
    if found.trim() == BUNDLE_IDENTIFIER {
        Ok(())
    } else {
        Err(Reject::Identifier)
    }
}

/// Whether the running macOS meets a bundle's `LSMinimumSystemVersion`. A value that does not
/// parse fails, because a wrong yes would replace a working app with one that cannot open.
pub fn system_allows(running: &str, minimum: &str) -> bool {
    fn parts(text: &str) -> Option<Vec<u64>> {
        text.trim()
            .split('.')
            .map(|part| {
                if part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()) {
                    return None;
                }
                part.parse().ok()
            })
            .collect()
    }
    let (Some(mut running), Some(mut minimum)) = (parts(running), parts(minimum)) else {
        return false;
    };
    let width = running.len().max(minimum.len());
    running.resize(width, 0);
    minimum.resize(width, 0);
    running >= minimum
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reject {
    Checksum,
    Layout,
    Signature,
    Identifier,
    BuildInfo,
    Version,
    Dirty,
    Profile,
    System,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    Network,
    Malformed,
    Rejected(Reject),
    Disk,
    Install,
}

impl Failure {
    pub fn backoff(self) -> Duration {
        if self == Self::Network {
            RETRY_AFTER
        } else {
            CHECK_EVERY
        }
    }

    /// What the user reads. A network failure is quiet, because the next check retries it.
    fn notice(self) -> Option<&'static str> {
        match self {
            Self::Network | Self::Malformed => None,
            Self::Rejected(Reject::Checksum) => {
                Some("The update failed its checksum and was discarded.")
            }
            Self::Rejected(Reject::Signature) => {
                Some("The update failed its signature check and was discarded.")
            }
            Self::Rejected(Reject::System) => Some("The update needs a newer version of macOS."),
            Self::Rejected(_) => {
                Some("The update is not a valid OpenTrickster release and was discarded.")
            }
            Self::Disk => Some("The update could not be saved to disk."),
            Self::Install => Some("The update could not be installed."),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OffReason {
    DevBuild,
    Verification,
    Architecture,
    NotBundled,
    Translocated,
    ReadOnly,
}

impl OffReason {
    fn notice(self) -> Option<&'static str> {
        match self {
            Self::Translocated => {
                Some("Updates are off. Move OpenTrickster to Applications to turn them on.")
            }
            Self::ReadOnly => Some(
                "Updates are off because OpenTrickster cannot write to its folder. Move it to Applications.",
            ),
            _ => None,
        }
    }
}

pub struct Environment<'a> {
    pub release: Option<&'a str>,
    pub verification: bool,
    pub arm64: bool,
    pub executable: Option<&'a Path>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    pub current: Version,
    pub bundle: PathBuf,
    pub parent: PathBuf,
}

pub fn enclosing_bundle(executable: &Path) -> Option<&Path> {
    let macos = executable.parent()?;
    let contents = macos.parent()?;
    let bundle = contents.parent()?;
    (macos.file_name()? == "MacOS"
        && contents.file_name()? == "Contents"
        && bundle.extension()? == "app")
        .then_some(bundle)
}

/// Whether this build may update itself, and where its bundle lives. `writable` probes the
/// folder that holds the bundle, because the swap renames beside it.
pub fn plan(
    environment: &Environment,
    writable: impl FnOnce(&Path) -> bool,
) -> Result<Plan, OffReason> {
    let current = environment
        .release
        .and_then(|release| release.parse::<Version>().ok())
        .ok_or(OffReason::DevBuild)?;
    if environment.verification {
        return Err(OffReason::Verification);
    }
    if !environment.arm64 {
        return Err(OffReason::Architecture);
    }
    let bundle = environment
        .executable
        .and_then(enclosing_bundle)
        .ok_or(OffReason::NotBundled)?;
    if bundle.to_string_lossy().contains("/AppTranslocation/") {
        return Err(OffReason::Translocated);
    }
    let parent = bundle.parent().ok_or(OffReason::NotBundled)?;
    if !writable(parent) {
        return Err(OffReason::ReadOnly);
    }
    Ok(Plan {
        current,
        bundle: bundle.to_owned(),
        parent: parent.to_owned(),
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Staged {
    pub directory: PathBuf,
    pub bundle: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum State {
    Off(OffReason),
    Idle {
        last_check: Option<Instant>,
    },
    Checking,
    Downloading {
        version: Version,
    },
    Ready {
        version: Version,
        staged: Staged,
    },
    /// The user asked to restart. The game and session are closing first.
    Closing {
        version: Version,
        staged: Staged,
    },
    Installing {
        version: Version,
    },
    Failed {
        reason: Failure,
        at: Instant,
    },
}

/// What the app holds that an install would end.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Occupancy {
    /// A game page or a session exists, is opening, or is closing.
    pub open: bool,
    /// A run is held after a lost connection. Closing the session does not clear it, because
    /// the engine keeps a live run paused when its session closes.
    pub held: bool,
    pub quitting: bool,
}

impl Occupancy {
    fn busy(self) -> bool {
        self.open || self.held
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Input {
    Tick,
    BuildChanged,
    Checked(Result<Option<Release>, Failure>),
    Downloaded(Result<Staged, Failure>),
    Restart,
    InstallFailed(Failure),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    Check,
    Download(Release),
    Close,
    Install(Staged),
}

fn due(since: Option<Instant>, wait: Duration, now: Instant) -> bool {
    since.is_none_or(|at| now.saturating_duration_since(at) >= wait)
}

impl State {
    pub fn initial(plan: &Result<Plan, OffReason>) -> Self {
        match plan {
            Ok(_) => Self::Idle { last_check: None },
            Err(reason) => Self::Off(*reason),
        }
    }

    pub fn next_check(&self) -> Option<Instant> {
        match self {
            Self::Idle {
                last_check: Some(at),
            } => Some(*at + CHECK_EVERY),
            Self::Failed { reason, at } => Some(*at + reason.backoff()),
            _ => None,
        }
    }

    /// An update is on its way, so the game halt that follows a build change has a remedy.
    pub fn is_updating(&self) -> bool {
        matches!(
            self,
            Self::Downloading { .. }
                | Self::Ready { .. }
                | Self::Closing { .. }
                | Self::Installing { .. }
        )
    }

    /// An install happens when a staged update meets nothing open, or when the user restarts.
    /// A user who is quitting gets no install they did not ask for.
    pub fn step(self, input: Input, now: Instant, world: Occupancy) -> (Self, Option<Action>) {
        let check = || (Self::Checking, Some(Action::Check));
        match (self, input) {
            (Self::Idle { last_check }, Input::Tick) if due(last_check, CHECK_EVERY, now) => {
                check()
            }
            (Self::Idle { last_check }, Input::BuildChanged)
                if due(last_check, RECHECK_AFTER, now) =>
            {
                check()
            }
            (Self::Failed { reason, at }, Input::Tick) if due(Some(at), reason.backoff(), now) => {
                check()
            }
            (Self::Failed { at, .. }, Input::BuildChanged) if due(Some(at), RECHECK_AFTER, now) => {
                check()
            }
            (Self::Checking, Input::Checked(result)) => match result {
                Ok(None) => (
                    Self::Idle {
                        last_check: Some(now),
                    },
                    None,
                ),
                Ok(Some(release)) => (
                    Self::Downloading {
                        version: release.version,
                    },
                    Some(Action::Download(release)),
                ),
                Err(reason) => (Self::Failed { reason, at: now }, None),
            },
            (Self::Downloading { version }, Input::Downloaded(result)) => match result {
                Ok(staged) => (Self::Ready { version, staged }, None),
                Err(reason) => (Self::Failed { reason, at: now }, None),
            },
            (Self::Ready { version, staged }, Input::Tick) if !world.busy() && !world.quitting => {
                (Self::Installing { version }, Some(Action::Install(staged)))
            }
            (Self::Ready { version, staged }, Input::Restart) if world.open => {
                (Self::Closing { version, staged }, Some(Action::Close))
            }
            (Self::Ready { version, staged }, Input::Restart) => {
                (Self::Installing { version }, Some(Action::Install(staged)))
            }
            (Self::Closing { version, staged }, Input::Tick) if !world.open => {
                (Self::Installing { version }, Some(Action::Install(staged)))
            }
            (Self::Installing { .. }, Input::InstallFailed(reason)) => {
                (Self::Failed { reason, at: now }, None)
            }
            (state, _) => (state, None),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Control {
    RestartToUpdate,
    ReleasePage,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notice {
    pub text: String,
    pub caption: Option<&'static str>,
    pub control: Option<Control>,
}

impl Notice {
    fn text(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            caption: None,
            control: None,
        }
    }

    fn with_release_page(text: &str) -> Self {
        Self {
            control: Some(Control::ReleasePage),
            ..Self::text(text)
        }
    }
}

/// What the update row shows. Most states show nothing.
pub fn notice(state: &State, world: Occupancy) -> Option<Notice> {
    match state {
        State::Off(reason) => reason.notice().map(Notice::with_release_page),
        State::Idle { .. } | State::Checking => None,
        State::Downloading { version } => Some(Notice::text(format!(
            "Downloading OpenTrickster {version}."
        ))),
        State::Ready { version, .. } if world.busy() => Some(Notice {
            text: format!("Update {version} is ready."),
            caption: Some("Restarting closes the game and ends a run."),
            control: Some(Control::RestartToUpdate),
        }),
        State::Ready { .. } => None,
        State::Closing { version, .. } => Some(Notice::text(format!(
            "Closing the game, then installing OpenTrickster {version}."
        ))),
        State::Installing { version } => {
            Some(Notice::text(format!("Installing OpenTrickster {version}.")))
        }
        State::Failed { reason, .. } => reason.notice().map(Notice::with_release_page),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn v(major: u64, minor: u64, patch: u64) -> Version {
        Version::new(major, minor, patch)
    }

    fn staged() -> Staged {
        Staged {
            directory: "/Applications/.OpenTrickster-update-1-2".into(),
            bundle: "/Applications/.OpenTrickster-update-1-2/unpacked/OpenTrickster.app".into(),
        }
    }

    fn release_json(
        tag: &str,
        draft: bool,
        prerelease: bool,
        assets: &[(&str, u64, &str)],
    ) -> String {
        let assets: Vec<String> = assets
            .iter()
            .map(|(name, size, state)| {
                format!(
                    r#"{{"url":"https://api.github.com/repos/oDestroyeRo/opentrickster/releases/assets/1","id":1,"name":"{name}","label":null,"state":"{state}","content_type":"application/zip","size":{size},"download_count":0,"browser_download_url":"https://github.com/oDestroyeRo/opentrickster/releases/download/{tag}/{name}"}}"#
                )
            })
            .collect();
        format!(
            r#"{{"url":"https://api.github.com/repos/oDestroyeRo/opentrickster/releases/1","id":1,"tag_name":"{tag}","target_commitish":"main","name":"OpenTrickster","draft":{draft},"prerelease":{prerelease},"created_at":"2026-11-10T00:00:00Z","assets":[{}],"body":"notes"}}"#,
            assets.join(",")
        )
    }

    fn full_assets() -> [(&'static str, u64, &'static str); 2] {
        [
            (ARCHIVE_NAME, 25_000_000, "uploaded"),
            (CHECKSUMS_NAME, 98, "uploaded"),
        ]
    }

    #[test]
    fn versions_order_by_number_not_by_text() {
        assert!(v(0, 10, 0) > v(0, 9, 9));
        assert!(v(1, 0, 0) > v(0, 99, 99));
        assert!(v(0, 1, 10) > v(0, 1, 9));
        assert!(v(2, 0, 0) > v(1, 99, 99));
        assert_eq!(v(1, 2, 3), "1.2.3".parse::<Version>().unwrap());
        assert_eq!(v(12, 0, 7).to_string(), "12.0.7");
    }

    #[test]
    fn versions_parse_only_major_minor_patch() {
        assert_eq!("0.0.0".parse::<Version>(), Ok(v(0, 0, 0)));
        assert_eq!("10.20.30".parse::<Version>(), Ok(v(10, 20, 30)));
        for bad in [
            "",
            "1",
            "1.2",
            "1.2.3.4",
            "01.2.3",
            "1.02.3",
            "1.2.03",
            "+1.2.3",
            "-1.2.3",
            "1.2.3-rc1",
            "1.2.3+build",
            "v1.2.3",
            " 1.2.3",
            "1.2.3 ",
            "1.2.x",
            "1..3",
            "99999999999999999999.0.0",
        ] {
            assert_eq!(bad.parse::<Version>(), Err(BadVersion), "{bad:?}");
        }
    }

    #[test]
    fn tags_carry_a_v_prefix() {
        assert_eq!(Version::from_tag("v1.2.3"), Some(v(1, 2, 3)));
        for bad in ["1.2.3", "V1.2.3", "vv1.2.3", "v1.2", "nightly", "v"] {
            assert_eq!(Version::from_tag(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn a_newer_release_with_both_assets_is_offered() {
        let json = release_json("v0.2.0", false, false, &full_assets());
        assert_eq!(
            offer(json.as_bytes(), v(0, 1, 0)),
            Ok(Some(Release {
                version: v(0, 2, 0)
            }))
        );
    }

    #[test]
    fn a_release_that_is_not_newer_is_not_offered() {
        let json = release_json("v0.2.0", false, false, &full_assets());
        assert_eq!(offer(json.as_bytes(), v(0, 2, 0)), Ok(None));
        assert_eq!(offer(json.as_bytes(), v(0, 2, 1)), Ok(None));
        assert_eq!(offer(json.as_bytes(), v(1, 0, 0)), Ok(None));
    }

    #[test]
    fn a_release_without_its_assets_is_not_offered() {
        let current = v(0, 1, 0);
        let offered = |assets: &[(&str, u64, &str)]| {
            offer(
                release_json("v0.2.0", false, false, assets).as_bytes(),
                current,
            )
        };
        assert_eq!(offered(&[]), Ok(None));
        assert_eq!(offered(&[(ARCHIVE_NAME, 25_000_000, "uploaded")]), Ok(None));
        assert_eq!(offered(&[(CHECKSUMS_NAME, 98, "uploaded")]), Ok(None));
        assert_eq!(
            offered(&[
                ("OpenTrickster-macOS-x86_64.zip", 25_000_000, "uploaded"),
                (CHECKSUMS_NAME, 98, "uploaded"),
            ]),
            Ok(None)
        );
        assert_eq!(
            offered(&[
                (ARCHIVE_NAME, 25_000_000, "open"),
                (CHECKSUMS_NAME, 98, "uploaded"),
            ]),
            Ok(None)
        );
        assert_eq!(
            offered(&[
                (ARCHIVE_NAME, MAX_ARCHIVE + 1, "uploaded"),
                (CHECKSUMS_NAME, 98, "uploaded"),
            ]),
            Ok(None)
        );
        assert_eq!(
            offered(&[
                (ARCHIVE_NAME, 0, "uploaded"),
                (CHECKSUMS_NAME, 98, "uploaded"),
            ]),
            Ok(None)
        );
    }

    #[test]
    fn drafts_prereleases_and_unusual_tags_are_not_offered() {
        let current = v(0, 1, 0);
        for (tag, draft, prerelease) in [
            ("v0.2.0", true, false),
            ("v0.2.0", false, true),
            ("nightly", false, false),
            ("v0.2", false, false),
            ("v0.2.0-rc1", false, false),
        ] {
            let json = release_json(tag, draft, prerelease, &full_assets());
            assert_eq!(offer(json.as_bytes(), current), Ok(None), "{tag}");
        }
    }

    #[test]
    fn an_unreadable_answer_is_malformed() {
        for bad in [
            "",
            "not json",
            "[]",
            "{}",
            r#"{"message":"Not Found","status":"404"}"#,
            r#"{"tag_name":7}"#,
        ] {
            assert_eq!(offer(bad.as_bytes(), v(0, 1, 0)), Err(Malformed), "{bad:?}");
        }
    }

    #[test]
    fn downloads_come_from_the_release_tag() {
        let release = Release {
            version: v(1, 4, 2),
        };
        assert_eq!(
            release.archive_url(),
            "https://github.com/oDestroyeRo/opentrickster/releases/download/v1.4.2/OpenTrickster-macOS-arm64.zip"
        );
        assert_eq!(
            release.checksums_url(),
            "https://github.com/oDestroyeRo/opentrickster/releases/download/v1.4.2/SHA256SUMS"
        );
    }

    #[test]
    fn redirects_stay_on_github_over_https() {
        for host in [
            "github.com",
            "api.github.com",
            "objects.githubusercontent.com",
            "release-assets.githubusercontent.com",
        ] {
            assert!(trusted_hop("https", Some(host)), "{host}");
            assert!(!trusted_hop("http", Some(host)), "http {host}");
        }
        for host in [
            "example.com",
            "githubusercontent.com",
            "evilgithubusercontent.com",
            "github.com.example.com",
            "notgithub.com",
            "gist.github.com",
        ] {
            assert!(!trusted_hop("https", Some(host)), "{host}");
        }
        assert!(!trusted_hop("https", None));
        assert!(!trusted_hop("file", Some("github.com")));
    }

    const ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
    const OTHER: &str = "ca978112ca1bbdcafac231b39a23dc4da786eff8147c4e72b9807785afee48bb";

    fn digest(hex: &str) -> Digest {
        decode_digest(hex).unwrap()
    }

    #[test]
    fn a_checksum_line_names_one_archive() {
        let sums = format!("{ABC}  {ARCHIVE_NAME}\n");
        assert_eq!(expected_digest(&sums, ARCHIVE_NAME), Some(digest(ABC)));
        let binary = format!("{ABC} *{ARCHIVE_NAME}\n");
        assert_eq!(expected_digest(&binary, ARCHIVE_NAME), Some(digest(ABC)));
        let windows = format!("{ABC}  {ARCHIVE_NAME}\r\n");
        assert_eq!(expected_digest(&windows, ARCHIVE_NAME), Some(digest(ABC)));
        let upper = format!("{}  {ARCHIVE_NAME}\n", ABC.to_uppercase());
        assert_eq!(expected_digest(&upper, ARCHIVE_NAME), Some(digest(ABC)));
    }

    #[test]
    fn a_checksum_file_picks_the_archive_among_other_entries() {
        let sums = format!("{OTHER}  README.txt\n\n{ABC}  {ARCHIVE_NAME}\n");
        assert_eq!(expected_digest(&sums, ARCHIVE_NAME), Some(digest(ABC)));
        assert_eq!(expected_digest(&sums, "README.txt"), Some(digest(OTHER)));
    }

    #[test]
    fn an_unusable_checksum_file_yields_nothing() {
        for sums in [
            String::new(),
            format!("{ABC}  other.zip\n"),
            format!("{ABC} {ARCHIVE_NAME}\n"),
            format!("{ABC}{ARCHIVE_NAME}\n"),
            format!("{}  {ARCHIVE_NAME}\n", &ABC[..63]),
            format!("{}0  {ARCHIVE_NAME}\n", ABC),
            format!("{}g  {ARCHIVE_NAME}\n", &ABC[..63]),
            format!("{ABC}  {ARCHIVE_NAME}\nnot a checksum line\n"),
            format!("{ABC}  {ARCHIVE_NAME}\n{OTHER}  {ARCHIVE_NAME}\n"),
        ] {
            assert_eq!(expected_digest(&sums, ARCHIVE_NAME), None, "{sums:?}");
        }
        let twice = format!("{ABC}  {ARCHIVE_NAME}\n{ABC}  {ARCHIVE_NAME}\n");
        assert_eq!(expected_digest(&twice, ARCHIVE_NAME), Some(digest(ABC)));
    }

    #[test]
    fn a_digest_mismatch_rejects_the_download() {
        assert_eq!(digest_matches(&digest(ABC), &digest(ABC)), Ok(()));
        assert_eq!(
            digest_matches(&digest(ABC), &digest(OTHER)),
            Err(Reject::Checksum)
        );
        let mut flipped = digest(ABC);
        flipped[31] ^= 1;
        assert_eq!(
            digest_matches(&flipped, &digest(ABC)),
            Err(Reject::Checksum)
        );
    }

    fn build(version: &str, dirty: &str, profile: &str) -> String {
        format!(
            r#"{{"version":"{version}","source_revision":"abc","source_dirty":{dirty},"profile":"{profile}","architectures":["arm64"],"minimum_macos":"14.0"}}"#
        )
    }

    #[test]
    fn a_matching_clean_release_build_is_accepted() {
        let json = build("0.2.0", "false", "release");
        assert_eq!(accept_build(json.as_bytes(), v(0, 2, 0)), Ok(()));
    }

    #[test]
    fn a_build_that_disagrees_with_the_release_is_rejected() {
        let release = v(0, 2, 0);
        let case = |json: String| accept_build(json.as_bytes(), release);
        assert_eq!(
            case(build("0.1.9", "false", "release")),
            Err(Reject::Version)
        );
        assert_eq!(
            case(build("0.2.1", "false", "release")),
            Err(Reject::Version)
        );
        assert_eq!(
            case(build("v0.2.0", "false", "release")),
            Err(Reject::Version)
        );
        assert_eq!(case(build("0.2.0", "true", "release")), Err(Reject::Dirty));
        assert_eq!(case(build("0.2.0", "false", "debug")), Err(Reject::Profile));
        assert_eq!(case(build("0.2.0", "false", "")), Err(Reject::Profile));
    }

    #[test]
    fn a_build_file_that_cannot_prove_itself_is_rejected() {
        let release = v(0, 2, 0);
        for json in [
            "",
            "not json",
            "[]",
            "{}",
            r#"{"version":"0.2.0","profile":"release"}"#,
            r#"{"version":"0.2.0","source_dirty":null,"profile":"release"}"#,
            r#"{"version":"0.2.0","source_dirty":"false","profile":"release"}"#,
            r#"{"source_dirty":false,"profile":"release"}"#,
            r#"{"version":"0.2.0","source_dirty":false}"#,
            r#"{"version":2,"source_dirty":false,"profile":"release"}"#,
        ] {
            assert_eq!(
                accept_build(json.as_bytes(), release),
                Err(Reject::BuildInfo),
                "{json:?}"
            );
        }
    }

    #[test]
    fn only_the_opentrickster_identifier_is_accepted() {
        assert_eq!(accept_identifier("org.opentrickster.desktop"), Ok(()));
        assert_eq!(accept_identifier("org.opentrickster.desktop\n"), Ok(()));
        for bad in [
            "",
            "org.opentrickster.desktop.helper",
            "com.example.app",
            "ORG.OPENTRICKSTER.DESKTOP",
        ] {
            assert_eq!(accept_identifier(bad), Err(Reject::Identifier), "{bad:?}");
        }
    }

    #[test]
    fn a_bundle_needs_a_macos_no_newer_than_the_running_one() {
        assert!(system_allows("15.1.1", "14.0"));
        assert!(system_allows("14.0", "14.0"));
        assert!(system_allows("14", "14.0.0"));
        assert!(system_allows("26.0", "14.5"));
        assert!(!system_allows("13.6.9", "14.0"));
        assert!(!system_allows("14.0", "14.0.1"));
        assert!(!system_allows("14.4", "15"));
        assert!(!system_allows("15.0", "garbage"));
        assert!(!system_allows("", "14.0"));
        assert!(!system_allows("15.0", ""));
        assert!(!system_allows("15.0", "14..0"));
    }

    #[test]
    fn the_bundle_is_the_app_that_holds_the_executable() {
        let bundle = |path: &str| enclosing_bundle(Path::new(path)).map(Path::to_owned);
        assert_eq!(
            bundle("/Applications/OpenTrickster.app/Contents/MacOS/opentrickster"),
            Some("/Applications/OpenTrickster.app".into())
        );
        assert_eq!(
            bundle("/Users/a b/Apps/Open Trickster 2.app/Contents/MacOS/opentrickster"),
            Some("/Users/a b/Apps/Open Trickster 2.app".into())
        );
        for bad in [
            "/work/target/release/opentrickster",
            "/Applications/OpenTrickster.app/Contents/Resources/opentrickster",
            "/Applications/OpenTrickster/Contents/MacOS/opentrickster",
            "/Applications/OpenTrickster.app/MacOS/opentrickster",
            "/opentrickster",
            "opentrickster",
        ] {
            assert_eq!(bundle(bad), None, "{bad}");
        }
    }

    const EXECUTABLE: &str = "/Applications/OpenTrickster.app/Contents/MacOS/opentrickster";

    fn environment<'a>(release: Option<&'a str>, executable: &'a str) -> Environment<'a> {
        Environment {
            release,
            verification: false,
            arm64: true,
            executable: Some(Path::new(executable)),
        }
    }

    #[test]
    fn a_release_build_in_a_writable_app_may_update() {
        let plan = plan(&environment(Some("0.1.0"), EXECUTABLE), |parent| {
            parent == Path::new("/Applications")
        });
        assert_eq!(
            plan,
            Ok(Plan {
                current: v(0, 1, 0),
                bundle: "/Applications/OpenTrickster.app".into(),
                parent: "/Applications".into(),
            })
        );
        assert_eq!(State::initial(&plan), State::Idle { last_check: None });
    }

    #[test]
    fn a_build_without_a_valid_release_never_updates() {
        for release in [None, Some(""), Some("dev"), Some("0.1"), Some("v0.1.0")] {
            assert_eq!(
                plan(&environment(release, EXECUTABLE), |_| true),
                Err(OffReason::DevBuild),
                "{release:?}"
            );
        }
    }

    #[test]
    fn each_unsafe_place_turns_updates_off() {
        let off = |environment: Environment, writable: bool| {
            plan(&environment, |_| writable).unwrap_err()
        };
        let mut verification = environment(Some("0.1.0"), EXECUTABLE);
        verification.verification = true;
        assert_eq!(off(verification, true), OffReason::Verification);
        let mut intel = environment(Some("0.1.0"), EXECUTABLE);
        intel.arm64 = false;
        assert_eq!(off(intel, true), OffReason::Architecture);
        assert_eq!(
            off(
                environment(Some("0.1.0"), "/work/target/release/opentrickster"),
                true
            ),
            OffReason::NotBundled
        );
        let mut unknown = environment(Some("0.1.0"), EXECUTABLE);
        unknown.executable = None;
        assert_eq!(off(unknown, true), OffReason::NotBundled);
        assert_eq!(
            off(
                environment(
                    Some("0.1.0"),
                    "/private/var/folders/x/AppTranslocation/8A/d/OpenTrickster.app/Contents/MacOS/opentrickster"
                ),
                true
            ),
            OffReason::Translocated
        );
        assert_eq!(
            off(environment(Some("0.1.0"), EXECUTABLE), false),
            OffReason::ReadOnly
        );
    }

    #[test]
    fn a_folder_is_probed_only_when_nothing_else_rules_updates_out() {
        let mut probed = false;
        let _ = plan(&environment(None, EXECUTABLE), |_| {
            probed = true;
            true
        });
        let _ = plan(
            &environment(
                Some("0.1.0"),
                "/x/AppTranslocation/1/d/OpenTrickster.app/Contents/MacOS/opentrickster",
            ),
            |_| {
                probed = true;
                true
            },
        );
        assert!(!probed);
    }

    #[test]
    fn updates_that_are_off_stay_off() {
        let now = Instant::now();
        let later = now + CHECK_EVERY * 2;
        for reason in [
            OffReason::DevBuild,
            OffReason::Verification,
            OffReason::Architecture,
            OffReason::NotBundled,
            OffReason::Translocated,
            OffReason::ReadOnly,
        ] {
            for input in [
                Input::Tick,
                Input::BuildChanged,
                Input::Restart,
                Input::Checked(Ok(Some(Release {
                    version: v(9, 0, 0),
                }))),
                Input::Downloaded(Ok(staged())),
                Input::InstallFailed(Failure::Install),
            ] {
                for world in [
                    Occupancy::default(),
                    Occupancy {
                        open: true,
                        held: true,
                        quitting: true,
                    },
                ] {
                    assert_eq!(
                        State::Off(reason).step(input.clone(), later, world),
                        (State::Off(reason), None)
                    );
                }
            }
        }
    }

    #[test]
    fn a_new_launch_checks_at_once() {
        let now = Instant::now();
        let (state, action) =
            State::Idle { last_check: None }.step(Input::Tick, now, Occupancy::default());
        assert_eq!((state, action), (State::Checking, Some(Action::Check)));
    }

    #[test]
    fn the_next_check_comes_six_hours_after_the_last() {
        let start = Instant::now();
        let idle = State::Idle {
            last_check: Some(start),
        };
        assert_eq!(idle.next_check(), Some(start + CHECK_EVERY));
        let almost = start + CHECK_EVERY - Duration::from_secs(1);
        assert_eq!(
            idle.clone().step(Input::Tick, almost, Occupancy::default()),
            (idle.clone(), None)
        );
        assert_eq!(
            idle.step(Input::Tick, start + CHECK_EVERY, Occupancy::default()),
            (State::Checking, Some(Action::Check))
        );
    }

    #[test]
    fn a_check_runs_even_while_something_is_open() {
        let now = Instant::now();
        let world = Occupancy {
            open: true,
            held: true,
            quitting: false,
        };
        assert_eq!(
            State::Idle { last_check: None }.step(Input::Tick, now, world),
            (State::Checking, Some(Action::Check))
        );
    }

    #[test]
    fn a_build_change_checks_unless_a_check_just_ran() {
        let start = Instant::now();
        let world = Occupancy::default();
        let idle = State::Idle {
            last_check: Some(start),
        };
        let soon = start + RECHECK_AFTER - Duration::from_secs(1);
        assert_eq!(
            idle.clone().step(Input::BuildChanged, soon, world),
            (idle.clone(), None)
        );
        assert_eq!(
            idle.step(Input::BuildChanged, start + RECHECK_AFTER, world),
            (State::Checking, Some(Action::Check))
        );
        assert_eq!(
            State::Idle { last_check: None }.step(Input::BuildChanged, start, world),
            (State::Checking, Some(Action::Check))
        );
        let failed = State::Failed {
            reason: Failure::Network,
            at: start,
        };
        assert_eq!(
            failed.clone().step(Input::BuildChanged, soon, world),
            (failed.clone(), None)
        );
        assert_eq!(
            failed.step(Input::BuildChanged, start + RECHECK_AFTER, world),
            (State::Checking, Some(Action::Check))
        );
    }

    #[test]
    fn a_build_change_during_an_update_changes_nothing() {
        let now = Instant::now() + CHECK_EVERY * 2;
        let world = Occupancy::default();
        for state in [
            State::Checking,
            State::Downloading {
                version: v(0, 2, 0),
            },
            State::Ready {
                version: v(0, 2, 0),
                staged: staged(),
            },
            State::Closing {
                version: v(0, 2, 0),
                staged: staged(),
            },
            State::Installing {
                version: v(0, 2, 0),
            },
        ] {
            assert_eq!(
                state.clone().step(Input::BuildChanged, now, world),
                (state, None)
            );
        }
    }

    #[test]
    fn a_check_without_news_goes_back_to_waiting() {
        let now = Instant::now();
        assert_eq!(
            State::Checking.step(Input::Checked(Ok(None)), now, Occupancy::default()),
            (
                State::Idle {
                    last_check: Some(now)
                },
                None
            )
        );
    }

    #[test]
    fn a_found_release_starts_its_download() {
        let release = Release {
            version: v(0, 2, 0),
        };
        assert_eq!(
            State::Checking.step(
                Input::Checked(Ok(Some(release))),
                Instant::now(),
                Occupancy::default()
            ),
            (
                State::Downloading {
                    version: v(0, 2, 0)
                },
                Some(Action::Download(release))
            )
        );
    }

    #[test]
    fn a_failure_waits_before_the_next_try() {
        let start = Instant::now();
        let world = Occupancy::default();
        let (failed, action) =
            State::Checking.step(Input::Checked(Err(Failure::Network)), start, world);
        assert_eq!(
            (&failed, action),
            (
                &State::Failed {
                    reason: Failure::Network,
                    at: start
                },
                None
            )
        );
        assert_eq!(failed.next_check(), Some(start + RETRY_AFTER));
        let still = start + RETRY_AFTER - Duration::from_secs(1);
        assert_eq!(
            failed.clone().step(Input::Tick, still, world),
            (failed.clone(), None)
        );
        assert_eq!(
            failed.step(Input::Tick, start + RETRY_AFTER, world),
            (State::Checking, Some(Action::Check))
        );
        let rejected = State::Failed {
            reason: Failure::Rejected(Reject::Checksum),
            at: start,
        };
        assert_eq!(rejected.next_check(), Some(start + CHECK_EVERY));
        assert_eq!(
            rejected
                .clone()
                .step(Input::Tick, start + RETRY_AFTER, world),
            (rejected, None)
        );
    }

    #[test]
    fn a_download_ends_ready_or_failed() {
        let now = Instant::now();
        let downloading = State::Downloading {
            version: v(0, 2, 0),
        };
        assert_eq!(
            downloading
                .clone()
                .step(Input::Downloaded(Ok(staged())), now, Occupancy::default()),
            (
                State::Ready {
                    version: v(0, 2, 0),
                    staged: staged()
                },
                None
            )
        );
        assert_eq!(
            downloading.step(
                Input::Downloaded(Err(Failure::Rejected(Reject::Signature))),
                now,
                Occupancy::default()
            ),
            (
                State::Failed {
                    reason: Failure::Rejected(Reject::Signature),
                    at: now
                },
                None
            )
        );
    }

    #[test]
    fn a_late_answer_for_another_state_is_ignored() {
        let now = Instant::now();
        let world = Occupancy::default();
        let idle = State::Idle {
            last_check: Some(now),
        };
        for input in [
            Input::Checked(Ok(Some(Release {
                version: v(9, 0, 0),
            }))),
            Input::Downloaded(Ok(staged())),
            Input::InstallFailed(Failure::Install),
            Input::Restart,
        ] {
            assert_eq!(idle.clone().step(input, now, world), (idle.clone(), None));
        }
        let downloading = State::Downloading {
            version: v(0, 2, 0),
        };
        assert_eq!(
            downloading
                .clone()
                .step(Input::Checked(Ok(None)), now, world),
            (downloading, None)
        );
    }

    fn ready() -> State {
        State::Ready {
            version: v(0, 2, 0),
            staged: staged(),
        }
    }

    fn closing() -> State {
        State::Closing {
            version: v(0, 2, 0),
            staged: staged(),
        }
    }

    fn installing() -> (State, Option<Action>) {
        (
            State::Installing {
                version: v(0, 2, 0),
            },
            Some(Action::Install(staged())),
        )
    }

    #[test]
    fn a_ready_update_installs_when_nothing_is_open() {
        let now = Instant::now();
        assert_eq!(
            ready().step(Input::Tick, now, Occupancy::default()),
            installing()
        );
    }

    #[test]
    fn a_ready_update_waits_while_a_page_a_session_or_a_run_is_open() {
        let now = Instant::now();
        for (open, held) in [(true, false), (false, true), (true, true)] {
            let world = Occupancy {
                open,
                held,
                quitting: false,
            };
            assert_eq!(
                ready().step(Input::Tick, now, world),
                (ready(), None),
                "{world:?}"
            );
        }
    }

    #[test]
    fn a_user_who_is_quitting_gets_no_install() {
        let now = Instant::now();
        for (open, held) in [(false, false), (true, false), (false, true), (true, true)] {
            let world = Occupancy {
                open,
                held,
                quitting: true,
            };
            assert_eq!(
                ready().step(Input::Tick, now, world),
                (ready(), None),
                "{world:?}"
            );
        }
    }

    #[test]
    fn restarting_with_a_page_or_a_session_open_closes_it_first() {
        let now = Instant::now();
        let open = Occupancy {
            open: true,
            held: false,
            quitting: false,
        };
        assert_eq!(
            ready().step(Input::Restart, now, open),
            (closing(), Some(Action::Close))
        );
        assert_eq!(closing().step(Input::Tick, now, open), (closing(), None));
        let quit_by_the_restart = Occupancy {
            open: true,
            held: false,
            quitting: true,
        };
        assert_eq!(
            closing().step(Input::Tick, now, quit_by_the_restart),
            (closing(), None)
        );
    }

    #[test]
    fn a_restart_installs_once_the_game_and_session_have_closed() {
        let now = Instant::now();
        // The engine keeps a live run held after its session closes, so a held run must not
        // keep the install waiting: the quit that follows the close would end the app first.
        for held in [false, true] {
            for quitting in [false, true] {
                let closed = Occupancy {
                    open: false,
                    held,
                    quitting,
                };
                assert_eq!(
                    closing().step(Input::Tick, now, closed),
                    installing(),
                    "{closed:?}"
                );
            }
        }
    }

    #[test]
    fn restarting_with_nothing_open_installs_at_once() {
        let now = Instant::now();
        for held in [false, true] {
            for quitting in [false, true] {
                let world = Occupancy {
                    open: false,
                    held,
                    quitting,
                };
                assert_eq!(
                    ready().step(Input::Restart, now, world),
                    installing(),
                    "{world:?}"
                );
            }
        }
    }

    #[test]
    fn an_install_in_progress_does_not_start_again() {
        let now = Instant::now();
        let state = State::Installing {
            version: v(0, 2, 0),
        };
        for world in [
            Occupancy::default(),
            Occupancy {
                quitting: true,
                ..Occupancy::default()
            },
        ] {
            for input in [Input::Tick, Input::Restart, Input::BuildChanged] {
                assert_eq!(state.clone().step(input, now, world), (state.clone(), None));
            }
        }
    }

    #[test]
    fn a_failed_install_reports_and_waits() {
        let now = Instant::now();
        assert_eq!(
            State::Installing {
                version: v(0, 2, 0)
            }
            .step(
                Input::InstallFailed(Failure::Install),
                now,
                Occupancy::default()
            ),
            (
                State::Failed {
                    reason: Failure::Install,
                    at: now
                },
                None
            )
        );
    }

    #[test]
    fn the_install_decision_covers_every_state_and_occupancy() {
        let now = Instant::now() + CHECK_EVERY * 2;
        let long_ago = Instant::now();
        let states = [
            State::Off(OffReason::DevBuild),
            State::Idle { last_check: None },
            State::Idle {
                last_check: Some(now),
            },
            State::Checking,
            State::Downloading {
                version: v(0, 2, 0),
            },
            ready(),
            closing(),
            State::Installing {
                version: v(0, 2, 0),
            },
            State::Failed {
                reason: Failure::Install,
                at: now,
            },
            State::Failed {
                reason: Failure::Install,
                at: long_ago,
            },
        ];
        for state in states {
            for open in [false, true] {
                for held in [false, true] {
                    for quitting in [false, true] {
                        let world = Occupancy {
                            open,
                            held,
                            quitting,
                        };
                        let (_, action) = state.clone().step(Input::Tick, now, world);
                        let installs = matches!(action, Some(Action::Install(_)));
                        let expected = match &state {
                            State::Ready { .. } => !open && !held && !quitting,
                            State::Closing { .. } => !open,
                            _ => false,
                        };
                        assert_eq!(installs, expected, "{state:?} {world:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn only_an_update_in_flight_counts_as_updating() {
        assert!(!State::Off(OffReason::DevBuild).is_updating());
        assert!(!State::Idle { last_check: None }.is_updating());
        assert!(!State::Checking.is_updating());
        assert!(
            State::Downloading {
                version: v(0, 2, 0)
            }
            .is_updating()
        );
        assert!(ready().is_updating());
        assert!(closing().is_updating());
        assert!(
            State::Installing {
                version: v(0, 2, 0)
            }
            .is_updating()
        );
        assert!(
            !State::Failed {
                reason: Failure::Network,
                at: Instant::now()
            }
            .is_updating()
        );
    }

    fn notice_of(state: &State, open: bool) -> Option<Notice> {
        notice(
            state,
            Occupancy {
                open,
                ..Occupancy::default()
            },
        )
    }

    #[test]
    fn the_row_stays_quiet_when_nothing_needs_the_user() {
        let now = Instant::now();
        for state in [
            State::Off(OffReason::DevBuild),
            State::Off(OffReason::Verification),
            State::Off(OffReason::Architecture),
            State::Off(OffReason::NotBundled),
            State::Idle { last_check: None },
            State::Checking,
            State::Failed {
                reason: Failure::Network,
                at: now,
            },
            State::Failed {
                reason: Failure::Malformed,
                at: now,
            },
        ] {
            assert_eq!(notice_of(&state, true), None, "{state:?}");
            assert_eq!(notice_of(&state, false), None, "{state:?}");
        }
        assert_eq!(notice_of(&ready(), false), None);
    }

    #[test]
    fn the_row_shows_progress_without_a_control() {
        assert_eq!(
            notice_of(
                &State::Downloading {
                    version: v(0, 2, 0)
                },
                true
            ),
            Some(Notice {
                text: "Downloading OpenTrickster 0.2.0.".into(),
                caption: None,
                control: None,
            })
        );
        assert_eq!(
            notice_of(&closing(), true).map(|n| (n.text, n.control)),
            Some((
                "Closing the game, then installing OpenTrickster 0.2.0.".into(),
                None
            ))
        );
        assert_eq!(
            notice_of(
                &State::Installing {
                    version: v(0, 2, 0)
                },
                false
            )
            .map(|n| (n.text, n.control)),
            Some(("Installing OpenTrickster 0.2.0.".into(), None))
        );
    }

    #[test]
    fn the_row_offers_a_restart_only_while_something_is_open() {
        let offered = Some(Notice {
            text: "Update 0.2.0 is ready.".into(),
            caption: Some("Restarting closes the game and ends a run."),
            control: Some(Control::RestartToUpdate),
        });
        assert_eq!(notice_of(&ready(), true), offered);
        let held = Occupancy {
            held: true,
            ..Occupancy::default()
        };
        assert_eq!(notice(&ready(), held), offered);
        assert_eq!(notice(&ready(), Occupancy::default()), None);
    }

    #[test]
    fn the_row_sends_the_user_to_the_release_page_when_the_app_cannot_update() {
        let moved = notice_of(&State::Off(OffReason::Translocated), false).unwrap();
        assert_eq!(
            moved.text,
            "Updates are off. Move OpenTrickster to Applications to turn them on."
        );
        assert_eq!(moved.control, Some(Control::ReleasePage));
        let locked = notice_of(&State::Off(OffReason::ReadOnly), true).unwrap();
        assert!(locked.text.contains("cannot write to its folder"));
        assert!(locked.text.contains("Move it to Applications"));
        assert_eq!(locked.control, Some(Control::ReleasePage));
    }

    #[test]
    fn the_row_reports_a_failure_the_user_could_act_on() {
        let now = Instant::now();
        for (reason, text) in [
            (
                Failure::Rejected(Reject::Checksum),
                "The update failed its checksum and was discarded.",
            ),
            (
                Failure::Rejected(Reject::Signature),
                "The update failed its signature check and was discarded.",
            ),
            (
                Failure::Rejected(Reject::System),
                "The update needs a newer version of macOS.",
            ),
            (
                Failure::Rejected(Reject::Identifier),
                "The update is not a valid OpenTrickster release and was discarded.",
            ),
            (Failure::Disk, "The update could not be saved to disk."),
            (Failure::Install, "The update could not be installed."),
        ] {
            let shown = notice_of(&State::Failed { reason, at: now }, true).unwrap();
            assert_eq!(shown.text, text);
            assert_eq!(shown.control, Some(Control::ReleasePage));
        }
    }
}
