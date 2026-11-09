mod alerts;
mod app;
mod controller;
mod embedded_input;
mod host;
mod input;
mod input_verification;
#[allow(unsafe_code)]
mod macos_native;
#[allow(unsafe_code)]
mod native_input;
mod network;
mod normal_input;
mod theme;
mod verification;
mod view;

use gpui::{
    App, AppContext, Application, Bounds, TitlebarOptions, WindowBounds, WindowOptions, point, px,
    size,
};
use std::borrow::Cow;

const VERIFICATION_FAILED: &str = "Native embedding verification failed. See report.json.";

pub fn run() -> Result<(), &'static str> {
    let verification = verification::Options::parse()?;
    let verification_result = verification.as_ref().map(|options| options.result.clone());
    let exit_result = verification_result.clone();
    Application::new().run(move |cx: &mut App| {
        // AppKit's terminate: exits with status 0, so `run` never returns. GPUI polls quit
        // futures after every quit observer has run and its windows are gone; only then can
        // the verification status replace that exit status on every quit path.
        if let Some(result) = exit_result {
            cx.on_app_quit(move |_| {
                let result = result.clone();
                async move {
                    if result.get() != Some(true) {
                        eprintln!("OpenTrickster: {VERIFICATION_FAILED}");
                        std::process::exit(1);
                    }
                }
            })
            .detach();
        }
        if let Err(message) = macos_native::install_termination_gate() {
            eprintln!("OpenTrickster: {message}");
            macos_native::allow_termination();
            cx.quit();
            return;
        }
        for font in [
            include_bytes!("../../resources/fonts/IBMPlexSans-Regular.ttf").as_slice(),
            include_bytes!("../../resources/fonts/IBMPlexSans-Medium.ttf").as_slice(),
            include_bytes!("../../resources/fonts/IBMPlexSans-SemiBold.ttf").as_slice(),
            include_bytes!("../../resources/fonts/IBMPlexSansThai-Regular.ttf").as_slice(),
            include_bytes!("../../resources/fonts/IBMPlexSansThai-Medium.ttf").as_slice(),
            include_bytes!("../../resources/fonts/IBMPlexSansThai-SemiBold.ttf").as_slice(),
            include_bytes!("../../resources/fonts/IBMPlexMono-Regular.ttf").as_slice(),
        ] {
            cx.text_system()
                .add_fonts(vec![Cow::Borrowed(font)])
                .expect("bundled IBM Plex font");
        }
        input::bind(cx);
        app::bind(cx);
        let bounds = Bounds::centered(None, size(px(1400.), px(900.)), cx);
        let result = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(972.), px(641.))),
                titlebar: Some(TitlebarOptions {
                    title: Some("OpenTrickster".into()),
                    appears_transparent: true,
                    traffic_light_position: Some(point(px(16.), px(18.))),
                }),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| app::Desktop::new(window, cx, verification)),
        );
        if result.is_err() {
            eprintln!("OpenTrickster: The application window could not open.");
            macos_native::allow_termination();
            cx.quit();
        }
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        cx.activate(true);
    });
    if verification_result.is_some_and(|result| result.get() != Some(true)) {
        return Err(VERIFICATION_FAILED);
    }
    Ok(())
}
