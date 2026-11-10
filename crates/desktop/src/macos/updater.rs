use crate::update::{
    self, Action, Failure, Input, Machine, Occupancy, OffReason, Plan, Reject, Release, Staged,
    State, Version,
};
use sha2::{Digest as _, Sha256};
use std::{
    fs,
    io::{Read, Write},
    os::unix::{fs::MetadataExt, process::CommandExt},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const STAGING: &str = ".OpenTrickster-update-";
const BACKUP: &str = ".OpenTrickster-backup-";
const INSTANCE: &str = ".OpenTrickster-instance-";
const LOCK: &str = ".lock";
const BUILD_JSON: &str = "Contents/Resources/build.json";
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
const CLAIM_TRIES: usize = 5;
// A scan reads the whole folder, so a waiting update repeats it at most this often.
const LOOK_EVERY: Duration = Duration::from_secs(1);
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

/// Where this copy runs, and the marker that proves it is alive.
struct Site {
    plan: Plan,
    instance: Instance,
}

pub struct Updater {
    machine: Machine,
    site: Option<Site>,
    other_copy: bool,
    looked: Option<Instant>,
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
        // A folder that takes no marker cannot tell live copies apart, so it cannot update.
        let planned = update::plan(&environment, bundled_version, writable).and_then(|plan| {
            Instance::claim(&plan.parent, &plan.bundle, std::process::id())
                .map(|instance| Site { plan, instance })
                .ok_or(OffReason::ReadOnly)
        });
        let state = State::initial(&planned);
        let site = planned.ok();
        if let Some(site) = &site {
            let parent = site.plan.parent.clone();
            thread::spawn(move || clean_leftovers(&parent));
        }
        let (outbox, inbox) = mpsc::channel();
        Self {
            machine: Machine::new(state),
            site,
            other_copy: false,
            looked: None,
            inbox,
            outbox,
        }
    }

    pub fn state(&self) -> &State {
        self.machine.state()
    }

    pub fn notice(&self, world: Occupancy) -> Option<update::Notice> {
        update::notice(self.state(), self.seen(world))
    }

    pub fn poll(&mut self, now: Instant, world: Occupancy, build_changed: bool) -> Vec<Handoff> {
        let mut handoffs = Vec::new();
        while let Ok(input) = self.inbox.try_recv() {
            handoffs.extend(self.feed(input, now, world));
        }
        if matches!(self.state(), State::Ready { .. }) {
            self.look(now, false);
        }
        if build_changed {
            handoffs.extend(self.feed(Input::BuildChanged, now, world));
        }
        handoffs.extend(self.feed(Input::Tick, now, world));
        handoffs
    }

    pub fn restart(&mut self, now: Instant, world: Occupancy) -> Option<Handoff> {
        self.look(now, true);
        self.feed(Input::Restart, now, world)
    }

    pub fn install(&mut self, staged: &Staged) -> Result<(), Failure> {
        let site = self.site.as_ref().ok_or(Failure::Install)?;
        install(
            &site.plan,
            &site.instance,
            staged,
            std::process::id(),
            Path::new(OPEN),
        )
    }

    pub fn install_failed(&mut self, failure: Failure, now: Instant) {
        self.feed(Input::InstallFailed(failure), now, Occupancy::default());
    }

    fn seen(&self, world: Occupancy) -> Occupancy {
        Occupancy {
            other_copy: self.other_copy,
            ..world
        }
    }

    /// Refreshes what the updater knows of other live copies.
    fn look(&mut self, now: Instant, fresh: bool) {
        let Some(site) = &self.site else {
            return;
        };
        let due = self
            .looked
            .is_none_or(|at| now.saturating_duration_since(at) >= LOOK_EVERY);
        if fresh || due {
            self.other_copy = site.instance.others();
            self.looked = Some(now);
        }
    }

    /// A staged update that left the disk can never install, so no decision may build on it.
    fn staged_vanished(&self) -> bool {
        matches!(self.state(), State::Ready { staged, .. } if !staged.bundle.is_dir())
    }

    fn feed(&mut self, input: Input, now: Instant, world: Occupancy) -> Option<Handoff> {
        let input = if matches!(input, Input::Tick | Input::Restart) && self.staged_vanished() {
            Input::Vanished
        } else {
            input
        };
        match self.machine.step(input, now, self.seen(world))? {
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
        let Some(current) = self.site.as_ref().map(|site| site.plan.current) else {
            return;
        };
        self.work(Input::Checked(Err(Failure::Malformed)), move || {
            Input::Checked(check(current))
        });
    }

    fn spawn_download(&self, release: Release) {
        let Some(parent) = self.site.as_ref().map(|site| site.plan.parent.clone()) else {
            return;
        };
        self.work(Input::Downloaded(Err(Failure::Download)), move || {
            Input::Downloaded(download(&release, &parent))
        });
    }

    fn work(&self, fallback: Input, job: impl FnOnce() -> Input + Send + 'static) {
        let reply = Reply {
            outbox: self.outbox.clone(),
            fallback: Some(fallback),
        };
        thread::spawn(move || reply.send(job()));
    }
}

/// A worker's one answer. A worker that panics first sends `fallback` as it unwinds, so the
/// state never waits on a thread that is gone.
struct Reply {
    outbox: mpsc::Sender<Input>,
    fallback: Option<Input>,
}

impl Reply {
    fn send(mut self, input: Input) {
        self.fallback = None;
        let _ = self.outbox.send(input);
    }
}

impl Drop for Reply {
    fn drop(&mut self) {
        if let Some(input) = self.fallback.take() {
            let _ = self.outbox.send(input);
        }
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
    let build =
        read_small(&bundle.join(BUILD_JSON), BUILD_JSON_MAX).ok_or(rejected(Reject::BuildInfo))?;
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

/// The version that the running bundle's own `build.json` names.
fn bundled_version(bundle: &Path) -> Option<Version> {
    let json = read_small(&bundle.join(BUILD_JSON), BUILD_JSON_MAX)?;
    update::built_version(&json)
}

/// A live copy holds an exclusive lock on a file beside its bundle. The kernel drops the lock
/// when the process ends, however it ends, so a lock that nobody holds marks a dead copy. The
/// file name carries the process id and the bundle's name.
struct Instance {
    _lock: fs::File,
    parent: PathBuf,
    bundle: String,
    pid: u32,
}

impl Instance {
    fn claim(parent: &Path, bundle: &Path, pid: u32) -> Option<Self> {
        let bundle = bundle.file_name()?.to_str()?.to_owned();
        let path = parent.join(format!("{INSTANCE}{pid}-{bundle}{LOCK}"));
        for _ in 0..CLAIM_TRIES {
            let file = fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .ok()?;
            // A cleanup may hold the lock for an instant, or unlink the file between the open
            // and the lock. Either way a copy that is not in the folder must try again.
            if file.try_lock().is_ok() && same_file(&file, &path) {
                return Some(Self {
                    _lock: file,
                    parent: parent.to_owned(),
                    bundle,
                    pid,
                });
            }
            thread::sleep(Duration::from_millis(20));
        }
        None
    }

    /// Whether another live copy of this bundle runs from the same folder. A folder that cannot
    /// be read counts as a yes, because an install must not guess.
    fn others(&self) -> bool {
        let Ok(markers) = markers(&self.parent) else {
            return true;
        };
        markers.iter().any(|(path, pid, bundle)| {
            *pid != self.pid && *bundle == self.bundle && take_free(path).is_none()
        })
    }
}

fn same_file(file: &fs::File, path: &Path) -> bool {
    matches!(
        (file.metadata(), fs::metadata(path)),
        (Ok(held), Ok(named)) if held.dev() == named.dev() && held.ino() == named.ino()
    )
}

/// The instance markers in a folder, each as its path, process id and bundle name.
fn markers(parent: &Path) -> std::io::Result<Vec<(PathBuf, u32, String)>> {
    Ok(fs::read_dir(parent)?
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .filter_map(|entry| {
            let name = entry.file_name();
            let (pid, bundle) = name
                .to_str()?
                .strip_prefix(INSTANCE)?
                .strip_suffix(LOCK)?
                .split_once('-')?;
            Some((entry.path(), pid.parse().ok()?, bundle.to_owned()))
        })
        .collect())
}

/// The lock on a marker that no process holds. A marker that a process holds, or that cannot be
/// opened, yields none, so the caller errs toward a live copy.
fn take_free(path: &Path) -> Option<fs::File> {
    let file = fs::File::open(path).ok()?;
    file.try_lock().ok()?;
    Some(file)
}

/// Removes what a dead process left behind. A name belongs to the process whose id it carries,
/// and a process is alive while it holds its marker. That covers this process too.
fn clean_leftovers(parent: &Path) {
    let Ok(markers) = markers(parent) else {
        return;
    };
    let mut alive = Vec::new();
    for (path, pid, _) in markers {
        match take_free(&path) {
            // Unlink while holding the lock, so a copy that opens the file next cannot lock a
            // file that is about to vanish.
            Some(lock) => {
                let _ = fs::remove_file(&path);
                drop(lock);
            }
            None => alive.push(pid),
        }
    }
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
        let owner = rest
            .split_once('-')
            .and_then(|(pid, _)| pid.parse::<u32>().ok());
        let orphan = !owner.is_some_and(|pid| alive.contains(&pid));
        if orphan && entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            let _ = fs::remove_dir_all(entry.path());
        }
    }
}

/// Moves the staged app into place and starts the relauncher. The caller quits next. The staging
/// directory goes on every path, because nothing keeps it for another try.
fn install(
    plan: &Plan,
    instance: &Instance,
    staged: &Staged,
    pid: u32,
    opener: &Path,
) -> Result<(), Failure> {
    let result = replace(plan, instance, staged, pid, opener);
    let _ = fs::remove_dir_all(&staged.directory);
    result
}

fn replace(
    plan: &Plan,
    instance: &Instance,
    staged: &Staged,
    pid: u32,
    opener: &Path,
) -> Result<(), Failure> {
    // A copy may have opened since the last look. Swapping now would pull it out from under it.
    if instance.others() {
        return Err(Failure::Install);
    }
    let backup = plan.parent.join(unique(BACKUP));
    swap(&plan.bundle, &staged.bundle, &backup).map_err(|_| Failure::Install)?;
    if relaunch(opener, pid, &plan.bundle).is_err() {
        undo(&plan.bundle, &staged.bundle, &backup);
        return Err(Failure::Install);
    }
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
        let instance = claim(&apps, &bundle, 100);

        install(&plan, &instance, &staged, old.id(), &opener).unwrap();

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
        // The shell creates the file before it writes into it, so wait for the text.
        let opened = || fs::read_to_string(&marker).is_ok_and(|text| !text.is_empty());
        let deadline = Instant::now() + Duration::from_secs(10);
        while !opened() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(
            fs::read_to_string(&marker).expect("the relauncher opened the bundle"),
            bundle.to_string_lossy()
        );
    }

    /// A lock drops when its last descriptor closes. A child that another test is spawning can
    /// hold a copy of the descriptor until its exec, so a release may lag by a few milliseconds.
    fn eventually(mut condition: impl FnMut() -> bool) -> bool {
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            if condition() {
                return true;
            }
            thread::sleep(Duration::from_millis(10));
        }
        condition()
    }

    fn claim(apps: &Path, bundle: &Path, pid: u32) -> Instance {
        Instance::claim(apps, bundle, pid).expect("the marker is claimed")
    }

    fn dirs(apps: &Path, names: &[String]) {
        for name in names {
            fs::create_dir_all(apps.join(name).join("inner")).unwrap();
        }
    }

    #[test]
    fn leftovers_of_dead_copies_are_removed_and_everything_else_stays() {
        let temp = Temp::new();
        let apps = temp.path();
        let dead = u32::MAX;
        let gone = [
            format!("{STAGING}{dead}-1"),
            format!("{BACKUP}{dead}-2"),
            format!("{STAGING}7-3"),
            format!("{STAGING}no-pid"),
        ];
        let kept = [
            ".OpenTrickster-other".to_owned(),
            "OpenTrickster.app".to_owned(),
            ".DS_Store-dir".to_owned(),
        ];
        dirs(apps, &gone);
        dirs(apps, &kept);
        fs::write(apps.join(format!("{STAGING}{dead}-file")), "x").unwrap();
        let link = apps.join(format!("{BACKUP}{dead}-link"));
        std::os::unix::fs::symlink(apps.join("OpenTrickster.app"), &link).unwrap();
        let marker = apps.join(format!("{INSTANCE}{dead}-OpenTrickster.app{LOCK}"));
        fs::write(&marker, "").unwrap();

        clean_leftovers(apps);

        for name in &gone {
            assert!(!apps.join(name).exists(), "{name}");
        }
        assert!(!marker.exists(), "a marker that nobody holds is removed");
        for name in &kept {
            assert!(apps.join(name).exists(), "{name}");
        }
        assert!(apps.join(format!("{STAGING}{dead}-file")).exists());
        assert!(link.symlink_metadata().is_ok());
        assert!(apps.join("OpenTrickster.app/inner").exists());
    }

    #[test]
    fn a_live_copys_staging_and_backup_survive_another_copys_cleanup() {
        let temp = Temp::new();
        let apps = temp.path();
        let bundle = apps.join("OpenTrickster.app");
        let live = claim(apps, &bundle, 4_000_000_001);
        let own = claim(apps, &bundle, std::process::id());
        let theirs = [
            format!("{STAGING}4000000001-1"),
            format!("{BACKUP}4000000001-2"),
        ];
        let mine = [
            format!("{STAGING}{}-3", std::process::id()),
            format!("{BACKUP}{}-4", std::process::id()),
        ];
        let dead = format!("{STAGING}4000000002-5");
        dirs(apps, &theirs);
        dirs(apps, &mine);
        dirs(apps, std::slice::from_ref(&dead));

        clean_leftovers(apps);

        for name in theirs.iter().chain(&mine) {
            assert!(apps.join(name).exists(), "{name}");
        }
        assert!(!apps.join(&dead).exists());
        drop(live);
        assert!(
            eventually(|| {
                clean_leftovers(apps);
                !apps.join(&theirs[0]).exists()
            }),
            "its process has ended"
        );
        assert!(apps.join(&mine[0]).exists());
        drop(own);
    }

    #[test]
    fn a_name_with_this_pid_goes_when_this_process_holds_no_marker() {
        let temp = Temp::new();
        let apps = temp.path();
        let name = format!("{STAGING}{}-1", std::process::id());
        dirs(apps, std::slice::from_ref(&name));
        clean_leftovers(apps);
        assert!(!apps.join(&name).exists());
    }

    #[test]
    fn a_copy_sees_another_live_copy_of_its_bundle_and_no_other_marker() {
        let temp = Temp::new();
        let apps = temp.path();
        let bundle = apps.join("OpenTrickster.app");
        let first = claim(apps, &bundle, 100);
        assert!(!first.others());
        let second = claim(apps, &bundle, 200);
        assert!(first.others());
        assert!(second.others());
        let elsewhere = claim(apps, &apps.join("Other.app"), 300);
        drop(second);
        assert!(
            eventually(|| !first.others()),
            "a copy that ended leaves only a free marker"
        );
        assert!(!elsewhere.others());
        let dead = apps.join(format!("{INSTANCE}400-OpenTrickster.app{LOCK}"));
        fs::write(&dead, "").unwrap();
        assert!(!first.others(), "a marker nobody holds is no copy");
    }

    #[test]
    fn a_folder_that_cannot_be_read_counts_as_another_copy() {
        let temp = Temp::new();
        let apps = temp.path().join("Applications");
        fs::create_dir(&apps).unwrap();
        let first = claim(&apps, &apps.join("OpenTrickster.app"), 100);
        assert!(!first.others());
        fs::remove_dir_all(&apps).unwrap();
        assert!(first.others());
    }

    #[test]
    fn a_lock_on_a_file_that_was_unlinked_is_not_a_claim() {
        let temp = Temp::new();
        let path = temp.path().join("marker");
        let open = || {
            fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .unwrap()
        };
        let file = open();
        assert!(same_file(&file, &path));
        // A cleanup that found the file free unlinked it before the claim locked it.
        fs::remove_file(&path).unwrap();
        assert!(!same_file(&file, &path));
        let replacement = open();
        assert!(same_file(&replacement, &path));
        assert!(!same_file(&file, &path));
    }

    #[test]
    fn an_install_beside_a_live_copy_is_refused_and_keeps_the_old_app() {
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
        let plan = Plan {
            current: v(0, 1, 0),
            bundle: bundle.clone(),
            parent: apps.clone(),
        };
        let instance = claim(&apps, &bundle, 100);
        let _other = claim(&apps, &bundle, 200);

        assert_eq!(
            install(&plan, &instance, &staged, 100, Path::new("/usr/bin/true")),
            Err(Failure::Install)
        );

        assert_eq!(
            fs::read_to_string(bundle.join("Contents/Resources/label")).unwrap(),
            "0.1.0"
        );
        assert!(!directory.exists());
    }

    #[test]
    fn a_failed_install_removes_its_staging_and_keeps_the_old_app() {
        let temp = Temp::new();
        let apps = temp.path().join("Applications");
        fs::create_dir(&apps).unwrap();
        let bundle = good_bundle(&apps, "0.1.0");
        let directory = apps.join(".OpenTrickster-update-1-1");
        fs::create_dir_all(directory.join("unpacked")).unwrap();
        let staged = Staged {
            bundle: directory.join("unpacked/OpenTrickster.app"),
            directory: directory.clone(),
        };
        let plan = Plan {
            current: v(0, 1, 0),
            bundle: bundle.clone(),
            parent: apps.clone(),
        };
        let instance = claim(&apps, &bundle, 100);

        assert_eq!(
            install(&plan, &instance, &staged, 100, Path::new("/usr/bin/true")),
            Err(Failure::Install)
        );

        assert!(!directory.exists(), "the staging is not left behind");
        assert_eq!(
            fs::read_to_string(bundle.join("Contents/Resources/label")).unwrap(),
            "0.1.0"
        );
        let backups = fs::read_dir(&apps)
            .unwrap()
            .flatten()
            .filter(|entry| entry.file_name().to_string_lossy().starts_with(BACKUP))
            .count();
        assert_eq!(backups, 0);
    }

    #[test]
    fn the_running_bundle_names_its_own_version() {
        let temp = Temp::new();
        let bundle = good_bundle(temp.path(), "0.1.0");
        assert_eq!(bundled_version(&bundle), Some(v(0, 1, 0)));
        fs::write(bundle.join(BUILD_JSON), "not json").unwrap();
        assert_eq!(bundled_version(&bundle), None);
        fs::remove_file(bundle.join(BUILD_JSON)).unwrap();
        assert_eq!(bundled_version(&bundle), None);
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
        updater_at(state, None)
    }

    fn updater_at(state: State, site: Option<Site>) -> Updater {
        let (outbox, inbox) = mpsc::channel();
        Updater {
            machine: Machine::new(state),
            site,
            other_copy: false,
            looked: None,
            inbox,
            outbox,
        }
    }

    /// A staged update that exists on disk, as the machine expects of a ready one.
    fn staged_in(apps: &Path) -> Staged {
        let directory = apps.join(".OpenTrickster-update-1-1");
        fs::create_dir_all(directory.join("unpacked")).unwrap();
        Staged {
            bundle: good_bundle(&directory.join("unpacked"), "0.2.0"),
            directory,
        }
    }

    fn ready(staged: &Staged) -> State {
        State::Ready {
            version: v(0, 2, 0),
            staged: staged.clone(),
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
        let temp = Temp::new();
        let staged = staged_in(temp.path());
        let now = Instant::now();
        let mut busy = updater(ready(&staged));
        let world = Occupancy {
            open: true,
            ..Occupancy::default()
        };
        assert!(busy.poll(now, world, false).is_empty());
        assert_eq!(busy.state(), &ready(&staged));
        assert!(matches!(busy.restart(now, world), Some(Handoff::Close)));
        let mut idle = updater(ready(&staged));
        let handoffs = idle.poll(now, Occupancy::default(), false);
        assert!(matches!(handoffs.as_slice(), [Handoff::Install(s)] if *s == staged));
        assert_eq!(
            idle.state(),
            &State::Installing {
                version: v(0, 2, 0)
            }
        );
        assert!(idle.poll(now, Occupancy::default(), false).is_empty());
    }

    #[test]
    fn a_restart_for_an_update_that_left_the_disk_closes_nothing() {
        let temp = Temp::new();
        let staged = staged_in(temp.path());
        fs::remove_dir_all(&staged.directory).unwrap();
        let now = Instant::now();
        let world = Occupancy {
            open: true,
            ..Occupancy::default()
        };
        let mut updater = updater(ready(&staged));
        assert!(updater.restart(now, world).is_none());
        assert_eq!(updater.state(), &State::Idle { last_check: None });
    }

    #[test]
    fn a_waiting_update_that_left_the_disk_is_noticed_without_a_click() {
        let temp = Temp::new();
        let staged = staged_in(temp.path());
        fs::remove_dir_all(&staged.directory).unwrap();
        let world = Occupancy {
            open: true,
            ..Occupancy::default()
        };
        let mut updater = updater(ready(&staged));
        assert!(updater.poll(Instant::now(), world, false).is_empty());
        assert_eq!(updater.state(), &State::Idle { last_check: None });
        assert_eq!(updater.notice(world), None);
    }

    #[test]
    fn a_copy_beside_another_neither_closes_the_game_nor_installs() {
        let temp = Temp::new();
        let apps = temp.path();
        let bundle = good_bundle(apps, "0.1.0");
        let staged = staged_in(apps);
        let plan = Plan {
            current: v(0, 1, 0),
            bundle: bundle.clone(),
            parent: apps.to_owned(),
        };
        let instance = claim(apps, &bundle, 100);
        let peer = claim(apps, &bundle, 200);
        let mut updater = updater_at(ready(&staged), Some(Site { plan, instance }));
        let now = Instant::now();
        let quiet = Occupancy::default();
        let open = Occupancy {
            open: true,
            ..quiet
        };

        assert!(updater.poll(now, quiet, false).is_empty());
        assert!(updater.restart(now, open).is_none());
        assert_eq!(updater.state(), &ready(&staged));
        for world in [quiet, open] {
            let shown = updater.notice(world).expect("the row says why");
            assert_eq!(
                shown.text,
                "Another OpenTrickster window is open. Quit it to update."
            );
            assert_eq!(shown.control, None);
        }

        drop(peer);
        let mut later = now;
        let mut handoffs = Vec::new();
        assert!(eventually(|| {
            later += LOOK_EVERY;
            handoffs = updater.poll(later, quiet, false);
            !handoffs.is_empty()
        }));
        assert!(matches!(handoffs.as_slice(), [Handoff::Install(s)] if *s == staged));
    }

    #[test]
    fn a_worker_that_panics_still_answers() {
        let updater = updater(State::Checking);
        updater.work(Input::Checked(Err(Failure::Malformed)), || {
            panic!("the worker failed")
        });
        let answer = updater
            .inbox
            .recv_timeout(Duration::from_secs(10))
            .expect("the guard answers as the worker unwinds");
        assert_eq!(answer, Input::Checked(Err(Failure::Malformed)));
    }

    #[test]
    fn a_worker_that_finishes_answers_once() {
        let updater = updater(State::Checking);
        updater.work(Input::Checked(Err(Failure::Malformed)), || {
            Input::Checked(Ok(None))
        });
        assert_eq!(
            updater.inbox.recv_timeout(Duration::from_secs(10)),
            Ok(Input::Checked(Ok(None)))
        );
        assert!(
            updater
                .inbox
                .recv_timeout(Duration::from_millis(200))
                .is_err()
        );
    }

    #[test]
    fn a_state_that_waits_on_a_dead_worker_fails_instead() {
        let mut updater = updater(State::Downloading {
            version: v(0, 2, 0),
        });
        updater.work(Input::Downloaded(Err(Failure::Download)), || {
            panic!("the worker failed")
        });
        let deadline = Instant::now() + Duration::from_secs(10);
        let now = Instant::now();
        while matches!(updater.state(), State::Downloading { .. }) && Instant::now() < deadline {
            updater.poll(now, Occupancy::default(), false);
            thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(
            updater.state(),
            &State::Failed {
                reason: Failure::Download,
                at: now
            }
        );
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
