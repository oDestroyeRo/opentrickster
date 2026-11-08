# opentrickster

[DESIGN.md](DESIGN.md) is the design reference for the macOS app. It lists the design decisions confirmed in issue #9 and specifies the layout, session states, keyboard shortcuts, visual tokens, and components.

OpenTrickster is a Rust/GPUI macOS application for one existing Trickster Online
character. It embeds the game in client-assisted mode and uses a native state
dashboard in clientless mode. Combat and digging share one automation engine.

See [MVP behavior and acceptance](docs/mvp.md),
[architecture](docs/architecture.md), and [macOS build instructions](docs/macos-build.md).
Track implementation and runtime acceptance in
[MVP delivery](https://github.com/oDestroyeRo/opentrickster/issues/19).
