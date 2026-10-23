use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::{Deserialize, Serialize};
use trust_ads_core::{
    AdsDataTypeDescriptor, IecDataType, PointAccess, SymbolDescriptor, SymbolSnapshot, UpdateMode,
};

use crate::ads::diagnostics::{LocalIdentity, TargetIdentity};
use crate::ads::{generate_ads_interface, parse_ads_toml, AdsInterfaceError};

/// Symbol browse/import request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymbolImportRequest {
    /// Connection name in `ads.toml`.
    pub connection_name: String,
    /// Explicit symbol names selected by the user.
    #[serde(default)]
    pub symbols: Vec<String>,
    /// Optional symbol include patterns selected by the user.
    #[serde(default)]
    pub include_patterns: Vec<String>,
    /// Optional prefix for generated local variable names.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name_prefix: Option<String>,
}

/// Candidate exposed to authoring surfaces during symbol import.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymbolImportCandidate {
    /// Remote ADS symbol descriptor.
    pub descriptor: SymbolDescriptor,
    /// Suggested local generated variable name.
    pub suggested_var: String,
    /// Namespace/program/GVL bucket used by authoring surfaces.
    pub namespace: String,
    /// Final symbol path segment for compact labels.
    pub leaf_name: String,
    /// Default access direction; onboarding defaults to read-only.
    pub access: PointAccess,
    /// Default update mode; onboarding defaults to cyclic poll.
    pub mode: UpdateMode,
    /// Whether the candidate is currently selected for import.
    #[serde(default)]
    pub selected: bool,
}

/// Grouped symbol-import response shared by CLI, setup web, and VS Code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymbolImportResponse {
    /// Connection being imported.
    pub connection_name: String,
    /// Canonical snapshot for offline/CI use.
    pub snapshot: SymbolSnapshot,
    /// Group summaries for UI tree/search surfaces.
    #[serde(default)]
    pub groups: Vec<SymbolImportGroup>,
    /// Candidate symbols.
    #[serde(default)]
    pub candidates: Vec<SymbolImportCandidate>,
}

/// Group summary for symbol browser UX.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymbolImportGroup {
    pub namespace: String,
    pub symbol_count: usize,
    pub selected_count: usize,
}

/// Request to merge selected ADS symbols into project authoring artifacts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymbolImportApplyRequest {
    /// Import response containing selected symbols for one connection.
    pub response: SymbolImportResponse,
    /// Target PLC identity selected by the user.
    pub target: TargetIdentity,
    /// Runtime-host identity proven by the identity/doctor step.
    pub local: LocalIdentity,
    /// Cached snapshots for existing connections.
    #[serde(default)]
    pub existing_snapshots: Vec<SymbolSnapshot>,
    /// Explicit acknowledgement that selected write-capable bindings may command the PLC.
    #[serde(default)]
    pub write_acknowledged: bool,
}

/// Deterministic project artifacts produced by applying a symbol import.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymbolImportArtifacts {
    /// Canonical merged `ads.toml` contents.
    pub ads_toml: String,
    /// Updated cached snapshots, including the imported connection snapshot.
    pub snapshots: Vec<SymbolSnapshot>,
    /// Deterministic single-file generated ST source.
    pub generated_st: String,
    /// Number of selected points in the imported connection.
    pub selected_count: usize,
}

/// Failure while applying symbol import selections.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymbolImportApplyError {
    InvalidExistingToml(String),
    NoSelectedSymbols { connection_name: String },
    WriteBindingRequiresAcknowledgement { symbol: String },
    Generate(AdsInterfaceError),
    InvalidMergedToml(String),
    SerializeToml(String),
}

impl fmt::Display for SymbolImportApplyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidExistingToml(detail) => {
                write!(f, "existing ads.toml could not be parsed: {detail}")
            }
            Self::NoSelectedSymbols { connection_name } => {
                write!(
                    f,
                    "connection '{connection_name}' has no selected ADS symbols"
                )
            }
            Self::WriteBindingRequiresAcknowledgement { symbol } => write!(
                f,
                "ADS write binding for symbol '{symbol}' requires explicit write acknowledgement"
            ),
            Self::Generate(error) => write!(f, "{error}"),
            Self::InvalidMergedToml(detail) => write!(f, "merged ads.toml is invalid: {detail}"),
            Self::SerializeToml(detail) => write!(f, "failed to serialize ads.toml: {detail}"),
        }
    }
}

impl std::error::Error for SymbolImportApplyError {}

/// Build deterministic import candidates from ADS symbol metadata.
#[must_use]
pub fn build_symbol_import_response(
    request: &SymbolImportRequest,
    symbols: Vec<SymbolDescriptor>,
) -> SymbolImportResponse {
    let snapshot = SymbolSnapshot::new(request.connection_name.clone(), symbols);
    let mut used_names = BTreeSet::new();
    let candidates = snapshot
        .symbols
        .iter()
        .map(|descriptor| {
            let (namespace, leaf_name) = split_symbol_name(descriptor.name.as_str());
            let suggested_var = unique_var_name(
                request.name_prefix.as_deref(),
                descriptor.name.as_str(),
                &mut used_names,
            );
            SymbolImportCandidate {
                descriptor: descriptor.clone(),
                suggested_var,
                namespace,
                leaf_name,
                access: PointAccess::Read,
                mode: UpdateMode::Poll,
                selected: symbol_selected(
                    descriptor.name.as_str(),
                    &request.symbols,
                    &request.include_patterns,
                ),
            }
        })
        .collect::<Vec<_>>();
    let groups = group_candidates(&candidates);
    SymbolImportResponse {
        connection_name: request.connection_name.clone(),
        snapshot,
        groups,
        candidates,
    }
}

/// Merge one imported ADS connection into canonical project artifacts.
///
/// The imported connection is replaced by name, other connections are preserved
/// semantically, `local_net_id` is pinned to the runtime-host identity, and the
/// single generated ST file is regenerated from all cached snapshots.
pub fn apply_symbol_import(
    existing_ads_toml: Option<&str>,
    request: SymbolImportApplyRequest,
) -> Result<SymbolImportArtifacts, SymbolImportApplyError> {
    let mut document = existing_ads_toml
        .filter(|text| !text.trim().is_empty())
        .map(parse_ads_document)
        .transpose()?
        .unwrap_or_default();
    let connection_name = request.response.connection_name.clone();
    let mut used_names = used_point_names_except(document.connections.as_slice(), &connection_name);
    let connection = imported_connection(&request, &mut used_names)?;
    document
        .connections
        .retain(|connection| connection.name != connection_name);
    document.connections.push(connection);
    document
        .connections
        .sort_by(|left, right| left.name.cmp(&right.name));

    let ads_toml = serialize_ads_document(&document)?;
    let config = parse_ads_toml(ads_toml.as_str())
        .map_err(|error| SymbolImportApplyError::InvalidMergedToml(error.to_string()))?;
    let mut snapshots = merged_snapshots(
        request.existing_snapshots,
        request.response.snapshot.clone(),
        connection_name.as_str(),
    );
    for snapshot in &mut snapshots {
        snapshot.canonicalize();
    }
    let generated =
        generate_ads_interface(&config, &snapshots).map_err(SymbolImportApplyError::Generate)?;
    let selected_count = document
        .connections
        .iter()
        .find(|connection| connection.name == connection_name)
        .map(|connection| connection.points.len())
        .unwrap_or(0);
    Ok(SymbolImportArtifacts {
        ads_toml,
        snapshots,
        generated_st: generated.source,
        selected_count,
    })
}

fn split_symbol_name(symbol: &str) -> (String, String) {
    let mut parts = symbol.rsplitn(2, '.');
    let leaf = parts.next().unwrap_or(symbol).to_string();
    let namespace = parts.next().unwrap_or("").to_string();
    (namespace, leaf)
}

fn symbol_selected(symbol: &str, symbols: &[String], patterns: &[String]) -> bool {
    if !symbols.is_empty() {
        return symbols.iter().any(|candidate| candidate == symbol);
    }
    if patterns.is_empty() {
        return true;
    }
    let symbol = symbol.to_ascii_lowercase();
    patterns
        .iter()
        .any(|pattern| wildcard_match(&symbol, pattern.to_ascii_lowercase().as_str()))
}

fn wildcard_match(text: &str, pattern: &str) -> bool {
    if pattern.is_empty() {
        return false;
    }
    if pattern == "*" {
        return true;
    }
    if !pattern.contains('*') {
        return text.contains(pattern);
    }

    let text = text.chars().collect::<Vec<_>>();
    let pattern = pattern.chars().collect::<Vec<_>>();
    let (mut text_index, mut pattern_index) = (0usize, 0usize);
    let mut star_index = None;
    let mut star_match_index = 0usize;
    while text_index < text.len() {
        if pattern_index < pattern.len() && pattern[pattern_index] == text[text_index] {
            text_index += 1;
            pattern_index += 1;
        } else if pattern_index < pattern.len() && pattern[pattern_index] == '*' {
            star_index = Some(pattern_index);
            pattern_index += 1;
            star_match_index = text_index;
        } else if let Some(star) = star_index {
            star_match_index += 1;
            text_index = star_match_index;
            pattern_index = star + 1;
        } else {
            return false;
        }
    }
    while pattern_index < pattern.len() && pattern[pattern_index] == '*' {
        pattern_index += 1;
    }
    pattern_index == pattern.len()
}

fn parse_ads_document(text: &str) -> Result<AdsTomlDocument, SymbolImportApplyError> {
    toml::from_str(text)
        .map_err(|error| SymbolImportApplyError::InvalidExistingToml(error.to_string()))
}

fn serialize_ads_document(document: &AdsTomlDocument) -> Result<String, SymbolImportApplyError> {
    let mut text = toml::to_string_pretty(document)
        .map_err(|error| SymbolImportApplyError::SerializeToml(error.to_string()))?;
    if !text.ends_with('\n') {
        text.push('\n');
    }
    Ok(text)
}

fn used_point_names_except(
    connections: &[ConnectionToml],
    imported_connection_name: &str,
) -> BTreeSet<String> {
    let mut used = BTreeSet::new();
    for connection in connections
        .iter()
        .filter(|connection| connection.name != imported_connection_name)
    {
        for point in &connection.points {
            used.insert(point.var.clone());
            used.insert(quality_name(point.var.as_str()));
        }
    }
    used
}

fn imported_connection(
    request: &SymbolImportApplyRequest,
    used_names: &mut BTreeSet<String>,
) -> Result<ConnectionToml, SymbolImportApplyError> {
    let selected = request
        .response
        .candidates
        .iter()
        .filter(|candidate| candidate.selected)
        .collect::<Vec<_>>();
    if selected.is_empty() {
        return Err(SymbolImportApplyError::NoSelectedSymbols {
            connection_name: request.response.connection_name.clone(),
        });
    }

    let mut points = Vec::with_capacity(selected.len());
    for candidate in selected {
        if writes_to_plc(candidate.access) && !request.write_acknowledged {
            return Err(
                SymbolImportApplyError::WriteBindingRequiresAcknowledgement {
                    symbol: candidate.descriptor.name.clone(),
                },
            );
        }
        let var = unique_imported_name(candidate.suggested_var.as_str(), used_names);
        points.push(PointToml {
            var,
            symbol: Some(candidate.descriptor.name.clone()),
            index_group: None,
            index_offset: None,
            size: None,
            type_name: iec_type_name(&candidate.descriptor.data_type).to_string(),
            string_len: candidate.descriptor.data_type.string_len,
            dimensions: dimensions_toml(&candidate.descriptor.data_type),
            access: Some(point_access_name(candidate.access).to_string()),
            mode: Some(update_mode_name(candidate.mode).to_string()),
            notification_mode: None,
            allow_retain_read: None,
        });
    }

    Ok(ConnectionToml {
        name: request.response.connection_name.clone(),
        target_net_id: request.target.ams_net_id.clone(),
        host: request.target.ip.clone(),
        ams_port: Some(request.target.ams_port),
        local_net_id: Some(request.local.ams_net_id.clone()),
        transport: Some("plain".to_string()),
        insecure_transport: Some(true),
        auto_add_route: None,
        points,
    })
}

fn unique_imported_name(base: &str, used_names: &mut BTreeSet<String>) -> String {
    let base = sanitize_var_name(base);
    let quality = quality_name(base.as_str());
    if !used_names.contains(base.as_str()) && !used_names.contains(quality.as_str()) {
        used_names.insert(base.clone());
        used_names.insert(quality);
        return base;
    }
    for suffix in 2usize.. {
        let candidate = format!("{base}_{suffix}");
        let quality = quality_name(candidate.as_str());
        if !used_names.contains(candidate.as_str()) && !used_names.contains(quality.as_str()) {
            used_names.insert(candidate.clone());
            used_names.insert(quality);
            return candidate;
        }
    }
    unreachable!("usize suffix space exhausted")
}

fn quality_name(point_name: &str) -> String {
    format!("{point_name}_quality")
}

fn merged_snapshots(
    existing: Vec<SymbolSnapshot>,
    imported: SymbolSnapshot,
    imported_connection_name: &str,
) -> Vec<SymbolSnapshot> {
    let mut snapshots = existing
        .into_iter()
        .filter(|snapshot| snapshot.route_name != imported_connection_name)
        .collect::<Vec<_>>();
    snapshots.push(imported);
    snapshots.sort_by(|left, right| left.route_name.cmp(&right.route_name));
    snapshots
}

fn dimensions_toml(descriptor: &AdsDataTypeDescriptor) -> Option<Vec<DimensionToml>> {
    if descriptor.dimensions.is_empty() {
        return None;
    }
    Some(
        descriptor
            .dimensions
            .iter()
            .map(|dimension| DimensionToml {
                lower: dimension.lower,
                upper: dimension.upper,
            })
            .collect(),
    )
}

fn iec_type_name(descriptor: &AdsDataTypeDescriptor) -> &'static str {
    match descriptor.iec_type {
        IecDataType::Bool => "BOOL",
        IecDataType::Sint => "SINT",
        IecDataType::Int => "INT",
        IecDataType::Dint => "DINT",
        IecDataType::Lint => "LINT",
        IecDataType::Usint => "USINT",
        IecDataType::Uint => "UINT",
        IecDataType::Udint => "UDINT",
        IecDataType::Ulint => "ULINT",
        IecDataType::Real => "REAL",
        IecDataType::Lreal => "LREAL",
        IecDataType::Byte => "BYTE",
        IecDataType::Word => "WORD",
        IecDataType::Dword => "DWORD",
        IecDataType::Lword => "LWORD",
        IecDataType::String => "STRING",
    }
}

fn point_access_name(access: PointAccess) -> &'static str {
    match access {
        PointAccess::Read => "read",
        PointAccess::Write => "write",
        PointAccess::ReadWrite => "read_write",
    }
}

fn writes_to_plc(access: PointAccess) -> bool {
    matches!(access, PointAccess::Write | PointAccess::ReadWrite)
}

fn update_mode_name(mode: UpdateMode) -> &'static str {
    match mode {
        UpdateMode::Poll => "poll",
        UpdateMode::Notify => "notify",
    }
}

fn unique_var_name(
    prefix: Option<&str>,
    symbol: &str,
    used_names: &mut BTreeSet<String>,
) -> String {
    let base = sanitize_var_name(format!("{}{}", prefix.unwrap_or(""), symbol).as_str());
    if used_names.insert(base.clone()) {
        return base;
    }
    for suffix in 2usize.. {
        let candidate = format!("{base}_{suffix}");
        if used_names.insert(candidate.clone()) {
            return candidate;
        }
    }
    unreachable!("usize suffix space exhausted")
}

fn sanitize_var_name(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut previous_was_underscore = false;
    for ch in text.chars() {
        let normalized = if ch.is_ascii_alphanumeric() {
            ch.to_ascii_lowercase()
        } else {
            '_'
        };
        if normalized == '_' {
            if previous_was_underscore {
                continue;
            }
            previous_was_underscore = true;
        } else {
            previous_was_underscore = false;
        }
        output.push(normalized);
    }
    let output = output.trim_matches('_');
    let mut output = if output.is_empty() {
        "ads_symbol".to_string()
    } else {
        output.to_string()
    };
    if output.chars().next().is_some_and(|ch| ch.is_ascii_digit())
        || is_reserved_identifier(output.as_str())
    {
        output.insert_str(0, "ads_");
    }
    output
}

fn is_reserved_identifier(identifier: &str) -> bool {
    if identifier.eq_ignore_ascii_case("ADS_QUALITY") {
        return true;
    }
    let tokens = trust_syntax::lex(identifier);
    tokens.len() != 1 || tokens[0].kind != trust_syntax::TokenKind::Ident
}

fn group_candidates(candidates: &[SymbolImportCandidate]) -> Vec<SymbolImportGroup> {
    let mut groups: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    for candidate in candidates {
        let entry = groups.entry(candidate.namespace.clone()).or_default();
        entry.0 += 1;
        if candidate.selected {
            entry.1 += 1;
        }
    }
    groups
        .into_iter()
        .map(
            |(namespace, (symbol_count, selected_count))| SymbolImportGroup {
                namespace,
                symbol_count,
                selected_count,
            },
        )
        .collect()
}

#[cfg(test)]
#[path = "import/apply_tests.rs"]
mod apply_tests;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
struct AdsTomlDocument {
    #[serde(default)]
    connections: Vec<ConnectionToml>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ConnectionToml {
    name: String,
    target_net_id: String,
    host: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    ams_port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    local_net_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    transport: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    insecure_transport: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    auto_add_route: Option<bool>,
    #[serde(default)]
    points: Vec<PointToml>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct PointToml {
    var: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    symbol: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    index_group: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    index_offset: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    size: Option<u32>,
    #[serde(rename = "type")]
    type_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    string_len: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dimensions: Option<Vec<DimensionToml>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    access: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    notification_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    allow_retain_read: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
struct DimensionToml {
    lower: i64,
    upper: i64,
}

#[cfg(test)]
mod tests {
    use trust_ads_core::{AdsDataTypeDescriptor, IecDataType, SymbolFlag};

    use super::*;

    #[test]
    fn apply_import_preserves_non_plc_port_and_generates_single_file() {
        let response = build_symbol_import_response(
            &SymbolImportRequest {
                connection_name: "line1".to_string(),
                symbols: Vec::new(),
                include_patterns: Vec::new(),
                name_prefix: Some("line1_".to_string()),
            },
            vec![real_symbol("MAIN.Temperature")],
        );

        let mut import_target = target("5.23.91.12.1.1", "192.168.10.5");
        import_target.ams_port = 301;
        let artifacts = apply_symbol_import(
            None,
            SymbolImportApplyRequest {
                response,
                target: import_target,
                local: local("192.168.10.20.1.1"),
                existing_snapshots: Vec::new(),
                write_acknowledged: false,
            },
        )
        .expect("apply import");

        assert!(artifacts
            .ads_toml
            .contains("local_net_id = \"192.168.10.20.1.1\""));
        assert!(artifacts.ads_toml.contains("ams_port = 301"));
        assert!(artifacts.generated_st.contains("TYPE\n    ADS_QUALITY"));
        assert!(artifacts
            .generated_st
            .contains("line1_main_temperature : REAL;"));
        assert_eq!(artifacts.selected_count, 1);
    }

    #[test]
    fn apply_import_merges_second_connection_and_is_idempotent() {
        let line1 = apply_symbol_import(
            None,
            SymbolImportApplyRequest {
                response: import_response("line1", "line_", "MAIN.Temperature"),
                target: target("5.23.91.12.1.1", "192.168.10.5"),
                local: local("192.168.10.20.1.1"),
                existing_snapshots: Vec::new(),
                write_acknowledged: false,
            },
        )
        .expect("line1 import");
        let merged = apply_symbol_import(
            Some(line1.ads_toml.as_str()),
            SymbolImportApplyRequest {
                response: import_response("line2", "line_", "MAIN.Temperature"),
                target: target("5.23.91.12.1.2", "192.168.10.6"),
                local: local("192.168.10.20.1.1"),
                existing_snapshots: line1.snapshots,
                write_acknowledged: false,
            },
        )
        .expect("line2 merge");
        let config = parse_ads_toml(merged.ads_toml.as_str()).expect("merged config");

        assert_eq!(config.connections.len(), 2);
        assert_eq!(config.connections[0].route.name, "line1");
        assert_eq!(config.connections[1].route.name, "line2");
        assert_eq!(
            config.connections[0].points[0].point_name,
            "line_main_temperature"
        );
        assert_eq!(
            config.connections[1].points[0].point_name,
            "line_main_temperature_2"
        );
        assert!(merged
            .generated_st
            .contains("line_main_temperature : REAL;"));
        assert!(merged
            .generated_st
            .contains("line_main_temperature_2 : REAL;"));

        let repeated = apply_symbol_import(
            Some(merged.ads_toml.as_str()),
            SymbolImportApplyRequest {
                response: import_response("line2", "line_", "MAIN.Temperature"),
                target: target("5.23.91.12.1.2", "192.168.10.6"),
                local: local("192.168.10.20.1.1"),
                existing_snapshots: merged.snapshots.clone(),
                write_acknowledged: false,
            },
        )
        .expect("idempotent line2 merge");
        assert_eq!(repeated.ads_toml, merged.ads_toml);
        assert_eq!(repeated.generated_st, merged.generated_st);
    }

    #[test]
    fn apply_import_rejects_empty_selection() {
        let response = build_symbol_import_response(
            &SymbolImportRequest {
                connection_name: "line1".to_string(),
                symbols: Vec::new(),
                include_patterns: vec!["NoMatch".to_string()],
                name_prefix: None,
            },
            vec![real_symbol("MAIN.Temperature")],
        );

        let error = apply_symbol_import(
            None,
            SymbolImportApplyRequest {
                response,
                target: target("5.23.91.12.1.1", "192.168.10.5"),
                local: local("192.168.10.20.1.1"),
                existing_snapshots: Vec::new(),
                write_acknowledged: false,
            },
        )
        .expect_err("empty selection rejected");

        assert!(matches!(
            error,
            SymbolImportApplyError::NoSelectedSymbols { .. }
        ));
    }

    #[test]
    fn build_import_response_honors_explicit_symbol_list() {
        let response = build_symbol_import_response(
            &SymbolImportRequest {
                connection_name: "line1".to_string(),
                symbols: vec!["GVL.Setpoint".to_string()],
                include_patterns: vec!["MAIN.*".to_string()],
                name_prefix: Some("line1_".to_string()),
            },
            vec![
                real_symbol("MAIN.Temperature"),
                real_symbol("GVL.Setpoint"),
                real_symbol("GVL.SetpointShadow"),
            ],
        );

        let selected = response
            .candidates
            .iter()
            .filter(|candidate| candidate.selected)
            .map(|candidate| candidate.descriptor.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(selected, vec!["GVL.Setpoint"]);
    }

    #[test]
    fn build_import_response_empty_pattern_selects_nothing() {
        let response = build_symbol_import_response(
            &SymbolImportRequest {
                connection_name: "line1".to_string(),
                symbols: Vec::new(),
                include_patterns: vec![String::new()],
                name_prefix: None,
            },
            vec![real_symbol("MAIN.Temperature")],
        );

        assert!(!response.candidates[0].selected);
    }

    #[test]
    fn build_import_response_glob_backtracks_to_anchored_suffix() {
        let response = build_symbol_import_response(
            &SymbolImportRequest {
                connection_name: "line1".to_string(),
                symbols: Vec::new(),
                include_patterns: vec!["MAIN.*Temp".to_string()],
                name_prefix: None,
            },
            vec![real_symbol("MAIN.Temp.Temp")],
        );

        assert!(response.candidates[0].selected);
    }

    #[test]
    fn build_import_response_prefixes_all_reserved_st_keywords() {
        for keyword in ["AT", "RETAIN", "NON_RETAIN", "INITIAL_STEP"] {
            let response = build_symbol_import_response(
                &SymbolImportRequest {
                    connection_name: "line1".to_string(),
                    symbols: Vec::new(),
                    include_patterns: Vec::new(),
                    name_prefix: None,
                },
                vec![real_symbol(keyword)],
            );

            assert_eq!(
                response.candidates[0].suggested_var,
                format!("ads_{}", keyword.to_ascii_lowercase()),
                "{keyword} must not be emitted as a variable identifier"
            );
        }
    }

    #[test]
    fn apply_import_rejects_write_binding_without_acknowledgement() {
        let mut response = import_response("line1", "line1_", "MAIN.Setpoint");
        response.candidates[0].access = PointAccess::Write;
        response.candidates[0].descriptor = response.candidates[0]
            .descriptor
            .clone()
            .with_flag(SymbolFlag::Write);
        response.snapshot.symbols[0] = response.snapshot.symbols[0]
            .clone()
            .with_flag(SymbolFlag::Write);

        let error = apply_symbol_import(
            None,
            SymbolImportApplyRequest {
                response: response.clone(),
                target: target("5.23.91.12.1.1", "192.168.10.5"),
                local: local("192.168.10.20.1.1"),
                existing_snapshots: Vec::new(),
                write_acknowledged: false,
            },
        )
        .expect_err("write import requires acknowledgement");
        assert!(matches!(
            error,
            SymbolImportApplyError::WriteBindingRequiresAcknowledgement { .. }
        ));

        let artifacts = apply_symbol_import(
            None,
            SymbolImportApplyRequest {
                response,
                target: target("5.23.91.12.1.1", "192.168.10.5"),
                local: local("192.168.10.20.1.1"),
                existing_snapshots: Vec::new(),
                write_acknowledged: true,
            },
        )
        .expect("acknowledged write import");
        assert!(artifacts.ads_toml.contains("access = \"write\""));
    }

    fn import_response(connection: &str, prefix: &str, symbol: &str) -> SymbolImportResponse {
        build_symbol_import_response(
            &SymbolImportRequest {
                connection_name: connection.to_string(),
                symbols: Vec::new(),
                include_patterns: Vec::new(),
                name_prefix: Some(prefix.to_string()),
            },
            vec![real_symbol(symbol)],
        )
    }

    fn real_symbol(name: &str) -> SymbolDescriptor {
        SymbolDescriptor::new(
            name,
            AdsDataTypeDescriptor::scalar("REAL", IecDataType::Real),
            0x4020,
            0,
            4,
        )
        .with_flag(SymbolFlag::Read)
    }

    fn target(net_id: &str, ip: &str) -> TargetIdentity {
        TargetIdentity {
            name: Some("CX".to_string()),
            ip: ip.to_string(),
            ams_net_id: net_id.to_string(),
            ams_port: 851,
            tc_version: Some("3.1.4024".to_string()),
        }
    }

    fn local(net_id: &str) -> LocalIdentity {
        LocalIdentity {
            host_name: Some("line-controller".to_string()),
            chosen_ip: "192.168.10.20".to_string(),
            ams_net_id: net_id.to_string(),
            nic: Some("eth0".to_string()),
            candidates: Vec::new(),
            classification: crate::ads::diagnostics::LocalNetworkClassification::Lan,
        }
    }
}
