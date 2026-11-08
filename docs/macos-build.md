# Build and package on macOS

Install Rust with Cargo, Python 3.9 or newer, and Apple Command Line Tools
(`xcode-select --install`). The GPUI application uses runtime Metal shaders;
the build does not require the standalone Metal compiler.

From the repository root, build a local application:

```sh
python3 scripts/package_macos.py
open dist/OpenTrickster.app
```

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

Account-free verification runs the packaged executable directly so its exit
status can be checked:

```sh
dist/OpenTrickster.app/Contents/MacOS/opentrickster --verify-embedding
dist/OpenTrickster.app/Contents/MacOS/opentrickster --verify-application
```

These checks exercise the host and application. Live login, bounded combat and
completed digging in each mode remain separate acceptance requirements in
[MVP behavior and acceptance](mvp.md). Keep credentials and private gameplay
captures out of verification output, Git and issue comments.

The application bundle follows Apple's
[macOS bundle structure](https://developer.apple.com/library/archive/documentation/CoreFoundation/Conceptual/CFBundles/BundleTypes/BundleTypes.html).
