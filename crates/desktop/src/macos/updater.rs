use crate::update::{
    self, Action, Failure, Input, Occupancy, Plan, Reject, Release, Staged, State, Version,
};
use sha2::{Digest as _, Sha256};
use std::{
    fs,
    io::{Read, Write},
    os::unix::process::CommandExt,
    path::Path,
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const STAGING: &str = ".OpenTrickster-update-";
const BACKUP: &str = ".OpenTrickster-backup-";
const OPEN: &str = "/usr/bin/open";
const DITTO: &str = "/usr/bin/ditto";
const CODESIGN: &str = "/usr/bin/codesign";
const PLUTIL: &str = "/usr/bin/plutil";
const SW_VERS: &str = "/usr/bin/sw_vers";
const JSON_TIMEOUT: Duration = Duration::from_secs(30);
const ARCHIVE_TIMEOUT: Duration = Duration::from_secs(15 * 60);
const STALL_TIMEOUT: Duration = Duration::from_secs(20);
const TOOL_TIMEOUT: Duration = Duration::from_secs(60);
const UNPACK_TIMEOUT: Duration = Duration::from_secs(120);
const BUILD_JSON_MAX: u64 = 64 * 1024;
const MAX_REDIRECTS: usize = 5;
// Waits at most a minute for the old process to exit, then opens the bundle anyway.
const RELAUNCH: &str = r#"n=0
while kill -0 "$1" 2>/dev/null && [ "$n" -lt 600 ]; do
  sleep 0.1
  n=$((n + 1))
done
exec "$2" "$3""#;

/// What the app must do for the updater, because only the app can close its game and session.
pub enum Handoff {
    Close,
    Install(Staged),
}

pub struct Updater {
    state: State,
    plan: Option<Plan>,
    inbox: mpsc::Receiver<Input>,
    outbox: mpsc::Sender<Input>,
}

impl Updater {
    pub fn new(verification: bool) -> Self {
        let executable = std::env::current_exe().and_then(fs::canonicalize).ok();
        let environment = update::Environment {
            release: option_env!("OPENTRICKSTER_RELEASE"),
            verification,
            arm64: cfg!(target_arch = "aarch64"),
            executable: executable.as_deref(),
        };
        let plan = update::plan(&environment, writable);
        let state = State::initial(&plan);
        let plan = plan.ok();
        if let Some(plan) = &plan {
            let parent = plan.parent.clone();
            thread::spawn(move || clean_leftovers(&parent));
        }
        let (outbox, inbox) = mpsc::channel();
        Self {
            state,
            plan,
            inbox,
            outbox,
        }
    }

    pub fn state(&self) -> &State {
        &self.state
    }

    pub fn poll(&mut self, now: Instant, world: Occupancy, build_changed: bool) -> Vec<Handoff> {
        let mut handoffs = Vec::new();
        while let Ok(input) = self.inbox.try_recv() {
            handoffs.extend(self.feed(input, now, world));
        }
        if build_changed {
            handoffs.extend(self.feed(Input::BuildChanged, now, world));
        }
        handoffs.extend(self.feed(Input::Tick, now, world));
        handoffs
    }

    pub fn restart(&mut self, now: Instant, world: Occupancy) -> Option<Handoff> {
        self.feed(Input::Restart, now, world)
    }

    pub fn install(&mut self, staged: &Staged) -> Result<(), Failure> {
        let plan = self.plan.as_ref().ok_or(Failure::Install)?;
        install(plan, staged, std::process::id(), Path::new(OPEN))
    }

    pub fn install_failed(&mut self, failure: Failure, now: Instant) {
        self.feed(Input::InstallFailed(failure), now, Occupancy::default());
    }

    fn feed(&mut self, input: Input, now: Instant, world: Occupancy) -> Option<Handoff> {
        let (state, action) = self.state.clone().step(input, now, world);
        self.state = state;
        match action? {
            Action::Check => {
                self.spawn_check();
                None
            }
            Action::Download(release) => {
                self.spawn_download(release);
                None
            }
            Action::Close => Some(Handoff::Close),
            Action::Install(staged) => Some(Handoff::Install(staged)),
        }
    }

    fn spawn_check(&self) {
        let Some(current) = self.plan.as_ref().map(|plan| plan.current) else {
            return;
        };
        let outbox = self.outbox.clone();
        thread::spawn(move || {
            let _ = outbox.send(Input::Checked(check(current)));
        });
    }

    fn spawn_download(&self, release: Release) {
        let Some(parent) = self.plan.as_ref().map(|plan| plan.parent.clone()) else {
            return;
        };
        let outbox = self.outbox.clone();
        thread::spawn(move || {
            let _ = outbox.send(Input::Downloaded(download(&release, &parent)));
        });
    }
}

fn block_on<T>(
    future: impl std::future::Future<Output = Result<T, Failure>>,
) -> Result<T, Failure> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| Failure::Network)?
        .block_on(future)
}

fn client() -> Result<reqwest::Client, Failure> {
    reqwest::Client::builder()
        .user_agent(update::USER_AGENT)
        .https_only(true)
        .connect_timeout(Duration::from_secs(10))
        .read_timeout(STALL_TIMEOUT)
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            let url = attempt.url();
            if attempt.previous().len() < MAX_REDIRECTS
                && update::trusted_hop(url.scheme(), url.host_str())
            {
                attempt.follow()
            } else {
                attempt.stop()
            }
        }))
        .build()
        .map_err(|_| Failure::Network)
}

async fn get(
    client: &reqwest::Client,
    url: &str,
    timeout: Duration,
) -> Result<reqwest::Response, Failure> {
    let response = client
        .get(url)
        .timeout(timeout)
        .send()
        .await
        .map_err(|_| Failure::Network)?;
    if response.status().is_success() {
        Ok(response)
    } else {
        Err(Failure::Network)
    }
}

async fn read_capped(mut response: reqwest::Response, limit: u64) -> Result<Vec<u8>, Failure> {
    if response
        .content_length()
        .is_some_and(|length| length > limit)
    {
        return Err(Failure::Malformed);
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| Failure::Network)? {
        if body.len() as u64 + chunk.len() as u64 > limit {
            return Err(Failure::Malformed);
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

fn check(current: Version) -> Result<Option<Release>, Failure> {
    block_on(async {
        let response = client()?
            .get(update::RELEASES_API)
            .header(reqwest::header::ACCEPT, "application/vnd.github+json")
            .timeout(JSON_TIMEOUT)
            .send()
            .await
            .map_err(|_| Failure::Network)?;
        let status = response.status();
        // A repository without a release answers 404. That is no update, not a failure.
        if status == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !status.is_success() {
            return Err(Failure::Network);
        }
        let body = read_capped(response, update::MAX_JSON).await?;
        update::offer(&body, current).map_err(|_| Failure::Malformed)
    })
}

async fn fetch(release: &Release, archive: &Path) -> Result<(), Failure> {
    let client = client()?;
    let sums = get(&client, &release.checksums_url(), JSON_TIMEOUT).await?;
    let sums = read_capped(sums, update::MAX_JSON).await?;
    let expected = std::str::from_utf8(&sums)
        .ok()
        .and_then(|sums| update::expected_digest(sums, update::ARCHIVE_NAME))
        .ok_or(Failure::Rejected(Reject::Checksum))?;
    let mut response = get(&client, &release.archive_url(), ARCHIVE_TIMEOUT).await?;
    if response
        .content_length()
        .is_some_and(|length| length > update::MAX_ARCHIVE)
    {
        return Err(Failure::Malformed);
    }
    let mut file = fs::File::create_new(archive).map_err(|_| Failure::Disk)?;
    let mut hasher = Sha256::new();
    let mut size = 0u64;
    while let Some(chunk) = response.chunk().await.map_err(|_| Failure::Network)? {
        size += chunk.len() as u64;
        if size > update::MAX_ARCHIVE {
            return Err(Failure::Malformed);
        }
        hasher.update(&chunk);
        file.write_all(&chunk).map_err(|_| Failure::Disk)?;
    }
    let actual: update::Digest = hasher.finalize().into();
    update::digest_matches(&actual, &expected).map_err(Failure::Rejected)
}

/// Stages beside the running bundle, on the same volume, so the install is a rename.
fn download(release: &Release, parent: &Path) -> Result<Staged, Failure> {
    let directory = parent.join(unique(STAGING));
    fs::create_dir(&directory).map_err(|_| Failure::Disk)?;
    let archive = directory.join(update::ARCHIVE_NAME);
    let staged = block_on(fetch(release, &archive))
        .and_then(|()| stage(&archive, &directory, release.version, &Codesign));
    if staged.is_err() {
        let _ = fs::remove_dir_all(&directory);
    }
    staged
}

trait Signing {
    fn verify(&self, bundle: &Path) -> bool;
}

struct Codesign;

impl Signing for Codesign {
    fn verify(&self, bundle: &Path) -> bool {
        run(
            Command::new(CODESIGN)
                .args(["--verify", "--strict"])
                .arg(bundle),
            TOOL_TIMEOUT,
        )
        .is_some()
    }
}

fn stage(
    archive: &Path,
    directory: &Path,
    version: Version,
    signing: &dyn Signing,
) -> Result<Staged, Failure> {
    let unpacked = directory.join("unpacked");
    fs::create_dir(&unpacked).map_err(|_| Failure::Disk)?;
    run(
        Command::new(DITTO)
            .args(["-x", "-k"])
            .arg(archive)
            .arg(&unpacked),
        UNPACK_TIMEOUT,
    )
    .ok_or(Failure::Rejected(Reject::Layout))?;
    let _ = fs::remove_file(archive);
    let bundle = unpacked_bundle(&unpacked)?;
    inspect(&bundle, version, signing)?;
    Ok(Staged {
        directory: directory.to_owned(),
        bundle,
    })
}

fn unpacked_bundle(directory: &Path) -> Result<std::path::PathBuf, Failure> {
    let layout = Failure::Rejected(Reject::Layout);
    let mut bundle = None;
    for entry in fs::read_dir(directory).map_err(|_| Failure::Disk)? {
        let entry = entry.map_err(|_| Failure::Disk)?;
        let is_bundle = entry.file_name() == "OpenTrickster.app"
            && entry.file_type().is_ok_and(|kind| kind.is_dir());
        if !is_bundle {
            return Err(layout);
        }
        bundle = Some(entry.path());
    }
    bundle.ok_or(layout)
}

fn inspect(bundle: &Path, version: Version, signing: &dyn Signing) -> Result<(), Failure> {
    let rejected = Failure::Rejected;
    let plist = bundle.join("Contents/Info.plist");
    let identifier =
        plist_string(&plist, "CFBundleIdentifier").ok_or(rejected(Reject::Identifier))?;
    update::accept_identifier(&identifier).map_err(rejected)?;
    let build = read_small(
        &bundle.join("Contents/Resources/build.json"),
        BUILD_JSON_MAX,
    )
    .ok_or(rejected(Reject::BuildInfo))?;
    update::accept_build(&build, version).map_err(rejected)?;
    if let Some(minimum) = plist_string(&plist, "LSMinimumSystemVersion") {
        let allowed =
            system_version().is_some_and(|running| update::system_allows(&running, &minimum));
        if !allowed {
            return Err(rejected(Reject::System));
        }
    }
    if !signing.verify(bundle) {
        return Err(rejected(Reject::Signature));
    }
    Ok(())
}

fn read_small(path: &Path, limit: u64) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();
    fs::File::open(path)
        .ok()?
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    (bytes.len() as u64 <= limit).then_some(bytes)
}

fn plist_string(plist: &Path, key: &str) -> Option<String> {
    let value = run(
        Command::new(PLUTIL)
            .args(["-extract", key, "raw", "-o", "-"])
            .arg(plist),
        TOOL_TIMEOUT,
    )?;
    Some(value.trim().to_owned())
}

fn system_version() -> Option<String> {
    let version = run(Command::new(SW_VERS).arg("-productVersion"), TOOL_TIMEOUT)?;
    Some(version.trim().to_owned())
}

/// Runs a system tool and returns its output, or none when it fails or outlives `timeout`.
fn run(command: &mut Command, timeout: Duration) -> Option<String> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut stdout = child.stdout.take()?;
    let reader = thread::spawn(move || {
        let mut text = String::new();
        let _ = stdout.by_ref().take(64 * 1024).read_to_string(&mut text);
        text
    });
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    };
    let text = reader.join().ok()?;
    status.success().then_some(text)
}

fn unique(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    format!("{prefix}{}-{nanos}", std::process::id())
}

/// The swap renames beside the bundle, so only a real write there proves it can work.
fn writable(parent: &Path) -> bool {
    let probe = parent.join(unique(STAGING));
    fs::create_dir(&probe).is_ok() && fs::remove_dir(&probe).is_ok()
}

/// Removes what an earlier run left behind. Names that carry this process's id belong to a
/// download that may be running now.
fn clean_leftovers(parent: &Path) {
    let own = format!("{}-", std::process::id());
    let Ok(entries) = fs::read_dir(parent) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(rest) = name.to_str().and_then(|name| {
            name.strip_prefix(STAGING)
                .or_else(|| name.strip_prefix(BACKUP))
        }) else {
            continue;
        };
        if !rest.starts_with(&own) && entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            let _ = fs::remove_dir_all(entry.path());
        }
    }
}

/// Moves the staged app into place and starts the relauncher. The caller quits next.
fn install(plan: &Plan, staged: &Staged, pid: u32, opener: &Path) -> Result<(), Failure> {
    let backup = plan.parent.join(unique(BACKUP));
    swap(&plan.bundle, &staged.bundle, &backup).map_err(|_| Failure::Install)?;
    if relaunch(opener, pid, &plan.bundle).is_err() {
        undo(&plan.bundle, &staged.bundle, &backup);
        return Err(Failure::Install);
    }
    let _ = fs::remove_dir_all(&staged.directory);
    Ok(())
}

fn swap(bundle: &Path, staged: &Path, backup: &Path) -> std::io::Result<()> {
    fs::rename(bundle, backup)?;
    if let Err(error) = fs::rename(staged, bundle) {
        let _ = fs::rename(backup, bundle);
        return Err(error);
    }
    Ok(())
}

fn undo(bundle: &Path, staged: &Path, backup: &Path) {
    let _ = fs::rename(bundle, staged);
    let _ = fs::rename(backup, bundle);
}

fn relaunch(opener: &Path, pid: u32, bundle: &Path) -> std::io::Result<()> {
    Command::new("/bin/sh")
        .args(["-c", RELAUNCH, "sh"])
        .arg(pid.to_string())
        .arg(opener)
        .arg(bundle)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .map(drop)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        os::unix::fs::PermissionsExt,
        path::PathBuf,
        sync::atomic::{AtomicU32, Ordering},
    };

    struct Temp(PathBuf);

    impl Temp {
        fn new() -> Self {
            static NEXT: AtomicU32 = AtomicU32::new(0);
            let path = std::env::temp_dir().join(format!(
                "opentrickster-updater-test-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::set_permissions(&self.0, fs::Permissions::from_mode(0o755));
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    struct Signed(bool);

    impl Signing for Signed {
        fn verify(&self, _: &Path) -> bool {
            self.0
        }
    }

    const IDENTIFIER: &str = "org.opentrickster.desktop";

    fn build_json(version: &str, dirty: bool, profile: &str) -> String {
        format!(
            r#"{{"version":"{version}","source_revision":"abc","source_dirty":{dirty},"profile":"{profile}","architectures":["arm64"],"minimum_macos":"14.0"}}"#
        )
    }

    fn fake_bundle(
        directory: &Path,
        name: &str,
        identifier: &str,
        build: &str,
        minimum: Option<&str>,
        label: &str,
    ) -> PathBuf {
        let bundle = directory.join(name);
        fs::create_dir_all(bundle.join("Contents/MacOS")).unwrap();
        fs::create_dir_all(bundle.join("Contents/Resources")).unwrap();
        fs::write(bundle.join("Contents/MacOS/opentrickster"), "#!/bin/sh\n").unwrap();
        fs::write(bundle.join("Contents/Resources/label"), label).unwrap();
        fs::write(bundle.join("Contents/Resources/build.json"), build).unwrap();
        let minimum = minimum
            .map(|minimum| format!("<key>LSMinimumSystemVersion</key><string>{minimum}</string>"))
            .unwrap_or_default();
        fs::write(
            bundle.join("Contents/Info.plist"),
            format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict><key>CFBundleIdentifier</key><string>{identifier}</string>{minimum}</dict></plist>
"#
            ),
        )
        .unwrap();
        bundle
    }

    fn good_bundle(directory: &Path, version: &str) -> PathBuf {
        fake_bundle(
            directory,
            "OpenTrickster.app",
            IDENTIFIER,
            &build_json(version, false, "release"),
            Some("11.0"),
            version,
        )
    }

    fn zip(directory: &Path, bundle: &Path) -> PathBuf {
        let archive = directory.join("release.zip");
        run(
            Command::new(DITTO)
                .args(["-c", "-k", "--sequesterRsrc", "--keepParent"])
                .arg(bundle)
                .arg(&archive),
            TOOL_TIMEOUT,
        )
        .expect("ditto creates the archive");
        archive
    }

    fn zip_contents(directory: &Path, source: &Path) -> PathBuf {
        let archive = directory.join("contents.zip");
        run(
            Command::new(DITTO)
                .args(["-c", "-k", "--sequesterRsrc"])
                .arg(source)
                .arg(&archive),
            TOOL_TIMEOUT,
        )
        .expect("ditto creates the archive");
        archive
    }

    fn v(major: u64, minor: u64, patch: u64) -> Version {
        Version::new(major, minor, patch)
    }

    fn staged_from(
        temp: &Temp,
        bundle: &Path,
        signing: &Signed,
        version: Version,
    ) -> Result<Staged, Failure> {
        let archive = zip(temp.path(), bundle);
        let directory = temp.path().join("staging");
        fs::create_dir(&directory).unwrap();
        stage(&archive, &directory, version, signing)
    }

    #[test]
    fn a_verified_archive_is_unpacked_and_removed() {
        let temp = Temp::new();
        let source = temp.path().join("source");
        fs::create_dir(&source).unwrap();
        let bundle = good_bundle(&source, "0.2.0");
        let directory = temp.path().join("staging");
        fs::create_dir(&directory).unwrap();
        let archive = archive_in(&directory);
        fs::copy(zip(temp.path(), &bundle), &archive).unwrap();
        let staged = stage(&archive, &directory, v(0, 2, 0), &Signed(true)).unwrap();
        assert_eq!(staged.directory, directory);
        assert_eq!(staged.bundle, directory.join("unpacked/OpenTrickster.app"));
        assert_eq!(
            fs::read_to_string(staged.bundle.join("Contents/Resources/label")).unwrap(),
            "0.2.0"
        );
        assert!(!archive.exists());
    }

    #[test]
    fn an_archive_that_changed_no_longer_matches_its_digest() {
        let temp = Temp::new();
        let source = temp.path().join("source");
        fs::create_dir(&source).unwrap();
        let bundle = good_bundle(&source, "0.2.0");
        let bytes = fs::read(zip(temp.path(), &bundle)).unwrap();
        let sums = format!(
            "{}  {}\n",
            hex(Sha256::digest(&bytes).as_slice()),
            update::ARCHIVE_NAME
        );
        let expected = update::expected_digest(&sums, update::ARCHIVE_NAME).unwrap();
        let actual: update::Digest = Sha256::digest(&bytes).into();
        assert_eq!(update::digest_matches(&actual, &expected), Ok(()));
        let mut changed = bytes;
        changed[0] ^= 1;
        let actual: update::Digest = Sha256::digest(&changed).into();
        assert_eq!(
            update::digest_matches(&actual, &expected),
            Err(Reject::Checksum)
        );
    }

    fn archive_in(directory: &Path) -> PathBuf {
        directory.join(update::ARCHIVE_NAME)
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    #[test]
    fn a_failed_signature_check_rejects_the_archive() {
        let temp = Temp::new();
        let source = temp.path().join("source");
        fs::create_dir(&source).unwrap();
        let bundle = good_bundle(&source, "0.2.0");
        assert_eq!(
            staged_from(&temp, &bundle, &Signed(false), v(0, 2, 0)),
            Err(Failure::Rejected(Reject::Signature))
        );
    }

    #[test]
    fn a_bundle_with_another_identifier_is_rejected() {
        let temp = Temp::new();
        let source = temp.path().join("source");
        fs::create_dir(&source).unwrap();
        let bundle = fake_bundle(
            &source,
            "OpenTrickster.app",
            "com.example.other",
            &build_json("0.2.0", false, "release"),
            None,
            "x",
        );
        assert_eq!(
            staged_from(&temp, &bundle, &Signed(true), v(0, 2, 0)),
            Err(Failure::Rejected(Reject::Identifier))
        );
    }

    #[test]
    fn a_build_file_that_disagrees_with_the_release_is_rejected() {
        for (build, reject) in [
            (build_json("0.1.9", false, "release"), Reject::Version),
            (build_json("0.2.0", true, "release"), Reject::Dirty),
            (build_json("0.2.0", false, "debug"), Reject::Profile),
            ("not json".to_owned(), Reject::BuildInfo),
        ] {
            let temp = Temp::new();
            let source = temp.path().join("source");
            fs::create_dir(&source).unwrap();
            let bundle = fake_bundle(&source, "OpenTrickster.app", IDENTIFIER, &build, None, "x");
            assert_eq!(
                staged_from(&temp, &bundle, &Signed(true), v(0, 2, 0)),
                Err(Failure::Rejected(reject)),
                "{build}"
            );
        }
    }

    #[test]
    fn a_missing_build_file_is_rejected() {
        let temp = Temp::new();
        let source = temp.path().join("source");
        fs::create_dir(&source).unwrap();
        let bundle = good_bundle(&source, "0.2.0");
        fs::remove_file(bundle.join("Contents/Resources/build.json")).unwrap();
        assert_eq!(
            staged_from(&temp, &bundle, &Signed(true), v(0, 2, 0)),
            Err(Failure::Rejected(Reject::BuildInfo))
        );
    }

    #[test]
    fn a_bundle_that_needs_a_newer_macos_is_rejected() {
        let temp = Temp::new();
        let source = temp.path().join("source");
        fs::create_dir(&source).unwrap();
        let bundle = fake_bundle(
            &source,
            "OpenTrickster.app",
            IDENTIFIER,
            &build_json("0.2.0", false, "release"),
            Some("999.0"),
            "x",
        );
        assert_eq!(
            staged_from(&temp, &bundle, &Signed(true), v(0, 2, 0)),
            Err(Failure::Rejected(Reject::System))
        );
    }

    #[test]
    fn an_archive_with_anything_besides_the_app_is_rejected() {
        let temp = Temp::new();
        let source = temp.path().join("source");
        fs::create_dir(&source).unwrap();
        good_bundle(&source, "0.2.0");
        fs::write(source.join("extra.txt"), "x").unwrap();
        let archive = zip_contents(temp.path(), &source);
        let directory = temp.path().join("staging");
        fs::create_dir(&directory).unwrap();
        assert_eq!(
            stage(&archive, &directory, v(0, 2, 0), &Signed(true)),
            Err(Failure::Rejected(Reject::Layout))
        );
    }

    #[test]
    fn a_link_standing_in_for_the_app_is_rejected() {
        let temp = Temp::new();
        let real = temp.path().join("real");
        fs::create_dir(&real).unwrap();
        let bundle = good_bundle(&real, "0.2.0");
        let source = temp.path().join("source");
        fs::create_dir(&source).unwrap();
        std::os::unix::fs::symlink(&bundle, source.join("OpenTrickster.app")).unwrap();
        let archive = zip_contents(temp.path(), &source);
        let directory = temp.path().join("staging");
        fs::create_dir(&directory).unwrap();
        assert_eq!(
            stage(&archive, &directory, v(0, 2, 0), &Signed(true)),
            Err(Failure::Rejected(Reject::Layout))
        );
    }

    #[test]
    fn an_archive_that_is_not_a_zip_is_rejected() {
        let temp = Temp::new();
        let archive = temp.path().join("bad.zip");
        fs::write(&archive, "not a zip file").unwrap();
        let directory = temp.path().join("staging");
        fs::create_dir(&directory).unwrap();
        assert_eq!(
            stage(&archive, &directory, v(0, 2, 0), &Signed(true)),
            Err(Failure::Rejected(Reject::Layout))
        );
    }

    #[test]
    fn the_staged_app_replaces_the_bundle_and_the_old_one_moves_aside() {
        let temp = Temp::new();
        let apps = temp.path().join("Applications");
        fs::create_dir(&apps).unwrap();
        let bundle = good_bundle(&apps, "0.1.0");
        let directory = apps.join(".OpenTrickster-update-1-1");
        fs::create_dir_all(directory.join("unpacked")).unwrap();
        let staged = good_bundle(&directory.join("unpacked"), "0.2.0");
        let backup = apps.join(".OpenTrickster-backup-1-1");
        swap(&bundle, &staged, &backup).unwrap();
        let label =
            |bundle: &Path| fs::read_to_string(bundle.join("Contents/Resources/label")).unwrap();
        assert_eq!(label(&bundle), "0.2.0");
        assert_eq!(label(&backup), "0.1.0");
        assert!(!staged.exists());
        undo(&bundle, &staged, &backup);
        assert_eq!(label(&bundle), "0.1.0");
        assert_eq!(label(&staged), "0.2.0");
        assert!(!backup.exists());
    }

    #[test]
    fn a_swap_that_cannot_finish_leaves_the_old_bundle_in_place() {
        let temp = Temp::new();
        let apps = temp.path().join("Applications");
        fs::create_dir(&apps).unwrap();
        let bundle = good_bundle(&apps, "0.1.0");
        let missing = apps.join("nothing-staged-here");
        let backup = apps.join(".OpenTrickster-backup-1-1");
        assert!(swap(&bundle, &missing, &backup).is_err());
        assert_eq!(
            fs::read_to_string(bundle.join("Contents/Resources/label")).unwrap(),
            "0.1.0"
        );
        assert!(!backup.exists());
    }

    #[test]
    fn an_install_swaps_now_and_reopens_the_bundle_once_the_old_process_exits() {
        let temp = Temp::new();
        let apps = temp.path().join("Applications");
        fs::create_dir(&apps).unwrap();
        let bundle = good_bundle(&apps, "0.1.0");
        let directory = apps.join(".OpenTrickster-update-1-1");
        fs::create_dir_all(directory.join("unpacked")).unwrap();
        let staged = Staged {
            bundle: good_bundle(&directory.join("unpacked"), "0.2.0"),
            directory: directory.clone(),
        };
        let marker = temp.path().join("opened");
        let opener = temp.path().join("opener.sh");
        fs::write(
            &opener,
            format!("#!/bin/sh\nprintf '%s' \"$1\" > '{}'\n", marker.display()),
        )
        .unwrap();
        fs::set_permissions(&opener, fs::Permissions::from_mode(0o755)).unwrap();
        let mut old = Command::new("/bin/cat")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .spawn()
            .unwrap();
        let plan = Plan {
            current: v(0, 1, 0),
            bundle: bundle.clone(),
            parent: apps.clone(),
        };

        install(&plan, &staged, old.id(), &opener).unwrap();

        assert_eq!(
            fs::read_to_string(bundle.join("Contents/Resources/label")).unwrap(),
            "0.2.0"
        );
        assert!(!directory.exists());
        let leftovers: Vec<_> = fs::read_dir(&apps)
            .unwrap()
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        assert!(leftovers.iter().any(|name| name.starts_with(BACKUP)));
        thread::sleep(Duration::from_millis(400));
        assert!(
            !marker.exists(),
            "the bundle opened while the old process ran"
        );
        drop(old.stdin.take());
        old.wait().unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while !marker.exists() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(
            fs::read_to_string(&marker).expect("the relauncher opened the bundle"),
            bundle.to_string_lossy()
        );
    }

    #[test]
    fn leftovers_from_other_runs_are_removed_and_everything_else_stays() {
        let temp = Temp::new();
        let apps = temp.path();
        let other = u32::MAX;
        let own = std::process::id();
        let mine = [
            format!("{STAGING}{other}-1"),
            format!("{BACKUP}{other}-2"),
            format!("{STAGING}7-3"),
        ];
        let kept = [
            format!("{STAGING}{own}-4"),
            format!("{BACKUP}{own}-5"),
            ".OpenTrickster-other".to_owned(),
            "OpenTrickster.app".to_owned(),
            ".DS_Store-dir".to_owned(),
        ];
        for name in mine.iter().chain(&kept) {
            fs::create_dir_all(apps.join(name).join("inner")).unwrap();
        }
        fs::write(apps.join(format!("{STAGING}{other}-file")), "x").unwrap();
        let link = apps.join(format!("{BACKUP}{other}-link"));
        std::os::unix::fs::symlink(apps.join("OpenTrickster.app"), &link).unwrap();

        clean_leftovers(apps);

        for name in &mine {
            assert!(!apps.join(name).exists(), "{name}");
        }
        for name in &kept {
            assert!(apps.join(name).exists(), "{name}");
        }
        assert!(apps.join(format!("{STAGING}{other}-file")).exists());
        assert!(link.symlink_metadata().is_ok());
        assert!(apps.join("OpenTrickster.app/inner").exists());
    }

    #[test]
    fn a_folder_is_writable_only_when_a_directory_can_be_made_in_it() {
        let temp = Temp::new();
        assert!(writable(temp.path()));
        assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 0);
        let locked = temp.path().join("locked");
        fs::create_dir(&locked).unwrap();
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o555)).unwrap();
        let root = fs::create_dir(locked.join("probe")).is_ok();
        if !root {
            assert!(!writable(&locked));
        }
        assert!(!writable(&temp.path().join("missing")));
    }

    fn updater(state: State) -> Updater {
        let (outbox, inbox) = mpsc::channel();
        Updater {
            state,
            plan: None,
            inbox,
            outbox,
        }
    }

    fn staged() -> Staged {
        Staged {
            directory: "/a/.OpenTrickster-update-1-1".into(),
            bundle: "/a/.OpenTrickster-update-1-1/unpacked/OpenTrickster.app".into(),
        }
    }

    #[test]
    fn a_finished_check_reaches_the_machine_through_the_channel() {
        let mut updater = updater(State::Checking);
        updater.outbox.send(Input::Checked(Ok(None))).unwrap();
        let now = Instant::now();
        assert!(updater.poll(now, Occupancy::default(), false).is_empty());
        assert_eq!(
            updater.state(),
            &State::Idle {
                last_check: Some(now)
            }
        );
    }

    #[test]
    fn a_ready_update_is_handed_over_for_install_only_when_nothing_is_open() {
        let ready = || State::Ready {
            version: v(0, 2, 0),
            staged: staged(),
        };
        let now = Instant::now();
        let mut busy = updater(ready());
        let world = Occupancy {
            open: true,
            ..Occupancy::default()
        };
        assert!(busy.poll(now, world, false).is_empty());
        assert_eq!(busy.state(), &ready());
        assert!(matches!(busy.restart(now, world), Some(Handoff::Close)));
        let mut idle = updater(ready());
        let handoffs = idle.poll(now, Occupancy::default(), false);
        assert!(matches!(handoffs.as_slice(), [Handoff::Install(s)] if *s == staged()));
        assert_eq!(
            idle.state(),
            &State::Installing {
                version: v(0, 2, 0)
            }
        );
        assert!(idle.poll(now, Occupancy::default(), false).is_empty());
    }

    #[test]
    fn a_failed_install_is_recorded() {
        let mut updater = updater(State::Installing {
            version: v(0, 2, 0),
        });
        let now = Instant::now();
        updater.install_failed(Failure::Install, now);
        assert_eq!(
            updater.state(),
            &State::Failed {
                reason: Failure::Install,
                at: now
            }
        );
    }

    #[test]
    fn the_http_client_builds() {
        assert!(client().is_ok());
    }
}
