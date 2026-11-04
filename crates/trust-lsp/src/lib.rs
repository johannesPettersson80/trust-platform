//! The truST language server for IEC 61131-3 Structured Text: the server
//! ([`StLanguageServer`]), its `check` and `command` command lines, the HMI agent tools over MCP,
//! and reusable protocol-boundary helpers. The `trust-lsp` binary is a thin entry point over it.

// Unsafe code is forbidden. Test builds deny it instead, so that the counting allocator that the
// `perf_alloc_metrics` tests install may allow it (`perf.rs`, its sites registered in
// xtask/config/full_map_policy.json).
#![cfg_attr(not(test), forbid(unsafe_code))]
#![cfg_attr(test, deny(unsafe_code))]

pub mod check;
pub mod command_line;
mod config;
pub mod document_text;
mod external_diagnostics;
mod handlers;
pub mod hmi_agent_mcp;
mod index_cache;
mod library_docs;
mod library_graph;
#[cfg(test)]
#[allow(unsafe_code)]
mod perf;
mod server;
mod state;
mod telemetry;
#[cfg(test)]
mod test_support;

pub use server::StLanguageServer;
