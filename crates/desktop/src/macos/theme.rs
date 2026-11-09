use gpui::{Hsla, WindowAppearance, rgb, rgba};

#[derive(Clone, Copy)]
pub struct Theme {
    pub base: Hsla,
    pub panel: Hsla,
    pub raised: Hsla,
    pub selected: Hsla,
    pub line: Hsla,
    pub control: Hsla,
    pub text: Hsla,
    pub secondary: Hsla,
    pub muted: Hsla,
    pub accent: Hsla,
    pub on_accent: Hsla,
    pub warning: Hsla,
    pub critical: Hsla,
    pub good: Hsla,
    pub serious: Hsla,
    pub good_subtle: Hsla,
    pub warning_subtle: Hsla,
    pub serious_subtle: Hsla,
    pub critical_subtle: Hsla,
    pub neutral_subtle: Hsla,
}
impl Theme {
    pub fn current(window: &gpui::Window) -> Self {
        Self::for_appearance(
            super::macos_native::appearance(window).unwrap_or_else(|| window.appearance()),
        )
    }

    pub fn for_appearance(appearance: WindowAppearance) -> Self {
        let dark = matches!(
            appearance,
            WindowAppearance::Dark | WindowAppearance::VibrantDark
        );
        let c = |d, l| rgb(if dark { d } else { l }).into();
        let alpha = |d, l| rgba(if dark { d } else { l }).into();
        Self {
            base: c(0x131518, 0xe9ecef),
            panel: c(0x1a1d21, 0xf6f7f9),
            raised: c(0x24282d, 0xffffff),
            selected: c(0x33383f, 0xe3e7eb),
            line: c(0x2a2e34, 0xdee2e7),
            control: c(0x6e7580, 0x858c96),
            text: c(0xe3e7eb, 0x1a1d21),
            secondary: c(0xa7adb5, 0x4b525c),
            muted: c(0x7f8690, 0x6a717b),
            accent: c(0x3cc8bf, 0x0b7c75),
            on_accent: c(0x062e2c, 0xffffff),
            warning: c(0xe9b341, 0xb07800),
            critical: c(0xef5a5f, 0xcc2f35),
            good: c(0x43bf5c, 0x1f9d3a),
            serious: c(0xee8a55, 0xc9561a),
            good_subtle: alpha(0x43bf5c24, 0x1f9d3a1f),
            warning_subtle: alpha(0xe9b34124, 0xb0780024),
            serious_subtle: alpha(0xee8a5524, 0xc9561a1f),
            critical_subtle: alpha(0xef5a5f26, 0xcc2f351c),
            neutral_subtle: alpha(0x7f869022, 0x6a717b1c),
        }
    }
}
