#![forbid(unsafe_code)]
//! Local stdio Model Context Protocol adapter for ContextDrop.
//!
//! The MCP layer owns stdio framing and optional local file reads. The
//! compression engine remains pure and is called in-process.

mod server;
mod tools;

pub use server::serve;
