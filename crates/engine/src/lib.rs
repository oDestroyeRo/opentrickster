//! Deterministic automation policy. Adapters execute effects and report original-ingress facts.
mod model;
mod reducer;
mod world;

pub use model::*;
pub use reducer::Engine;
