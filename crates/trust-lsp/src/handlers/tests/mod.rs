use super::features::{references, workspace_symbol};
use super::namespace_move_workspace_edit;
use super::*;
use crate::config::{
    BuildConfig, DiagnosticSettings, IndexingConfig, LibraryDependency, LibrarySpec, ProjectConfig,
    RuntimeConfig, StdlibSettings, TargetProfile, TelemetryConfig, WorkspaceSettings,
};
use crate::state::ServerState;
use crate::test_support::test_client;
use expect_test::expect;
use insta::assert_snapshot;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

fn position_at(source: &str, needle: &str) -> tower_lsp::lsp_types::Position {
    let offset = source
        .find(needle)
        .unwrap_or_else(|| panic!("missing needle '{needle}'"));
    super::lsp_utils::offset_to_position(source, offset as u32)
}

fn utf16_position_at(source: &str, needle: &str) -> tower_lsp::lsp_types::Position {
    let offset = source
        .find(needle)
        .unwrap_or_else(|| panic!("missing needle '{needle}'"));
    let prefix = &source[..offset];
    let line = prefix.chars().filter(|ch| *ch == '\n').count() as u32;
    let line_start = prefix.rfind('\n').map_or(0, |index| index + 1);
    let character = source[line_start..offset].encode_utf16().count() as u32;
    tower_lsp::lsp_types::Position { line, character }
}

fn utf16_len(text: &str) -> u32 {
    text.encode_utf16().count() as u32
}

fn inlay_label_contains(label: &tower_lsp::lsp_types::InlayHintLabel, needle: &str) -> bool {
    match label {
        tower_lsp::lsp_types::InlayHintLabel::String(value) => value.contains(needle),
        tower_lsp::lsp_types::InlayHintLabel::LabelParts(parts) => {
            parts.iter().any(|part| part.value.contains(needle))
        }
    }
}

fn temp_dir(prefix: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let dir = std::env::temp_dir().join(format!("{prefix}-{stamp}"));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn document_snapshot(state: &ServerState, uri: &tower_lsp::lsp_types::Url) -> Value {
    match state.get_document(uri) {
        Some(doc) => json!({
            "version": doc.version,
            "isOpen": doc.is_open,
            "content": doc.content,
        }),
        None => Value::Null,
    }
}

mod code_actions_and_commands;
mod completion_hover;
mod core;
mod formatting_and_navigation;
mod lsp_trace_cases;
mod rename_trace_cases;
mod trace_support;

mod mod_part_01;
mod mod_part_02;
