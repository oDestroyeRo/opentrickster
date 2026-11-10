# Build and package on macOS

Install Rust with Cargo, Python 3.9 or newer, and Apple Command Line Tools
(`xcode-select --install`). The GPUI application uses runtime Metal shaders;
the build does not require the standalone Metal compiler.

From the repository root, build a local application:

```sh
python3 scripts/package_macos.py
open dist/OpenTrickster.app
```

To use the app, follow the [operator guide](operator-guide.md).

The script uses the desktop executable reported by Cargo, includes the application
and bundled-font licenses, checks for unbundled dynamic libraries, and verifies
an ad-hoc code signature. It records the source revision, dirty status, build
profile, architecture and binary deployment target inside
`Contents/Resources/build.json`. Fonts and page scripts are embedded in the
executable. The package contains no downloaded game assets or account settings.

Existing output paths are refused. Choose a new path for another build:

```sh
python3 scripts/package_macos.py --debug --output dist/OpenTrickster-Debug.app
```

Builds use the current host architecture. `--target aarch64-apple-darwin` or
`--target x86_64-apple-darwin` selects a Rust target installed with `rustup`.
The script rejects an executable that depends on libraries outside macOS system
locations; those dependencies must be bundled before distributing that build.
The ad-hoc signature supports local use. Developer ID signing and notarization
are separate distribution work.

Check the source before packaging a release:

```sh
cargo fmt --all --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

Use a fresh `CARGO_TARGET_DIR` for acceptance checks when switching between
worktrees or combining archived source. This avoids reusing proof from a
different source tree.

GitHub Actions runs format, tests and Clippy on Linux, along with the release
tooling and build-migration tests, then desktop tests, Clippy and release
packaging on the Apple silicon `macos-15` runner. It pins Rust 1.99.1 and the
action revisions. Each successful macOS job uploads
`OpenTrickster-macOS-arm64.zip`, containing the app with executable permissions
preserved. Download and unzip that artifact to obtain `OpenTrickster.app`.
CI uses no game account or private credentials and does not establish live
gameplay acceptance. Runner labels follow GitHub's
[hosted runner reference](https://docs.github.com/en/actions/reference/runners/github-hosted-runners).

## Releases

Each merge to `main` that holds a releasable change publishes a GitHub Release.
CI reads the Conventional Commit subjects and bodies on `main` since the newest
`v*` tag, in semver order, and picks the next version:

| Commit | Next version |
| --- | --- |
| `!` after the type or scope, or a `BREAKING CHANGE:` footer | major |
| `feat` | minor |
| `fix`, `perf`, `revert` | patch |
| `docs`, `ci`, `chore`, `test`, `refactor`, `build`, `style`, or a subject that is not conventional | no release |

The highest bump in the range wins. The first release is the workspace version
in `Cargo.toml` (`0.1.0`), whatever the commit types. `Cargo.toml` keeps its
version. The release version reaches the app only through packaging. Keep the
squash title of each pull request a Conventional Commit, because it becomes the
commit subject. The version script needs Python 3.11 or newer and runs in CI.

A release has the tag `vX.Y.Z` on the commit CI tested, the title
`OpenTrickster X.Y.Z` and generated notes. It has two assets:
`OpenTrickster-macOS-arm64.zip` and `SHA256SUMS`. `SHA256SUMS` holds one line,
the SHA-256 of the zip and its name. The release job runs only after the
contract tests and the macOS job pass. It uses the workflow token and no other
secret. If the tag already exists on the same commit, the job succeeds without
publishing again. If the tag exists on another commit, the job fails.

CI runs on `main` queue instead of cancelling each other, so a release never
stops halfway. Pull request runs still cancel. A merge made with `GITHUB_TOKEN`
starts no `push` run. A workflow that merges that way releases by running
`gh workflow run ci.yml --ref main`.

To package a release build locally, commit your changes and pass the version:

```sh
python3 scripts/package_macos.py --release-version 1.2.3
```

The flag needs a clean work tree and refuses `--debug`. It sets
`OPENTRICKSTER_RELEASE` for the Cargo build only. It writes the version to
`Contents/Resources/build.json`, `CFBundleShortVersionString` and
`CFBundleVersion`. A build made without the flag carries no release version.

Account-free verification runs the packaged executable directly so its exit
status can be checked:

```sh
dist/OpenTrickster.app/Contents/MacOS/opentrickster --verify-embedding
dist/OpenTrickster.app/Contents/MacOS/opentrickster --verify-application
```

These checks exercise the host and application. Live login, bounded combat and
completed digging in each mode remain separate acceptance requirements in
[MVP behavior and acceptance](mvp.md). Keep credentials and private gameplay
captures out of verification output, Git and issue comments. The
[MVP acceptance checklist](acceptance.md) lists every automated and live check,
its expected result and its current status.

The application bundle follows Apple's
[macOS bundle structure](https://developer.apple.com/library/archive/documentation/CoreFoundation/Conceptual/CFBundles/BundleTypes/BundleTypes.html).
