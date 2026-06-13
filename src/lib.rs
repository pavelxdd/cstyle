//! Library API for formatting C, C++, and Objective-C source text.
//!
//! Use [`api::Formatter`] for reusable in-memory formatting, [`api::format`]
//! for one-shot string formatting, or [`api::format_bytes`] when input encoding
//! and line endings must be preserved. Use [`config::FormatOptions`] to select
//! formatting behavior.

pub mod api;
#[doc(hidden)]
pub mod cli;
pub mod config;
mod formatter;
mod io;
mod source;
