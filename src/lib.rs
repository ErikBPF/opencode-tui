//! opencode-tui library surface.
//!
//! `main.rs` is the application; this crate root exists so the contract binding
//! in `tests/` can drive the same transport and state seams the binary uses.

pub mod app;
pub mod component;
pub mod config;
pub mod context;
pub mod keymap;
pub mod routes;
pub mod runtime;
pub mod theme;
pub mod ui;
pub mod util;
