#![deny(unsafe_code)]

#[cfg(target_os = "macos")]
fn main() {
    if let Err(error) = opentrickster_desktop::macos::run() {
        eprintln!("OpenTrickster: {error}");
        std::process::exit(1);
    }
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("OpenTrickster requires macOS.");
    std::process::exit(1);
}
