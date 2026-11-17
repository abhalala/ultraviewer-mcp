//! UltraViewer protocol core extracted from MacViewer's LinuxViewer Rust port.

#![allow(
    clippy::chunks_exact_to_as_chunks,
    clippy::cloned_ref_to_slice_refs,
    clippy::manual_is_multiple_of,
    clippy::octal_escapes,
    clippy::too_many_arguments
)]

pub mod audio;
pub mod chat;
pub mod client;
pub mod clipboard;
pub mod command;
pub mod console;
pub mod control;
pub mod cursor;
pub mod direct;
pub mod files;
pub mod input;
pub mod keys;
pub mod mcp;
pub mod messages;
pub mod profile;
pub mod screen;
pub mod screenshot;
pub mod session;
pub mod tls;
pub mod wire;
