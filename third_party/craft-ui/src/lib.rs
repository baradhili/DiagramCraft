// Strict for shipped code; tests may unwrap/expect/panic freely
// (DiagramCraft #61: the unconditional deny made
// `cargo clippy --workspace --all-targets` abort on 111 test-module
// lints — path deps are auto workspace members, so the workspace
// gate compiles this crate's cfg(test) code too).
#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unimplemented,
        clippy::todo,
        clippy::unreachable
    )
)]

//! Small, reusable egui components for the Craft frontends.
//!
//! Applications own their assets, themes, layouts, commands and document semantics. Components
//! accept focused style values and return egui responses; no application engine is required.

pub mod buttons;
pub mod docking;
pub mod icons;
pub mod layout;
pub mod printing;
pub mod tabs;
pub mod theme;
