//! Runtime-to-ADS symbol publication.

use std::sync::{Arc, Mutex, RwLock};

use glob::Pattern;
use smol_str::SmolStr;
use trust_ads_core::{
    AdsDataTypeDescriptor, ArrayDimension, IecDataType, SymbolFlag, SymbolSnapshot,
};
use trust_ads_server::{
    build_server_symbol_snapshot, ServerSymbolError, ServerSymbolSpec, SymbolSource,
    SymbolVersionCounter, DEFAULT_SERVER_SYMBOL_INDEX_GROUP,
};

use crate::debug::DebugSnapshot;
use crate::error::RuntimeError;
use crate::value::Value;

use super::contracts::AdsServerRuntimeConfig;

/// Builds an ADS symbol snapshot from the current runtime globals.
///
/// Empty `runtime.ads_server.expose` means "expose nothing".
///
/// # Errors
///
/// Returns a runtime configuration error when glob compilation, type mapping,
/// or deterministic ADS address assignment fails.
pub fn build_runtime_symbol_snapshot(
    config: &AdsServerRuntimeConfig,
    snapshot: &DebugSnapshot,
) -> Result<SymbolSnapshot, RuntimeError> {
    let expose = compile_patterns(&config.expose, "runtime.ads_server.expose")?;
    let writable = compile_patterns(&config.writable, "runtime.ads_server.writable")?;
    let mut specs = Vec::new();

    if expose.is_empty() {
        return build_server_symbol_snapshot(
            route_name(config),
            specs,
            DEFAULT_SERVER_SYMBOL_INDEX_GROUP,
        )
        .map_err(symbol_error);
    }

    for (name, value) in snapshot.storage.globals() {
        let canonical = server_symbol_name(config, name.as_str());
        if !matches_any(&expose, canonical.as_str(), name.as_str()) {
            continue;
        }
        let Some(descriptor) = descriptor_for_value(value, config.max_string_bytes) else {
            continue;
        };
        let mut spec = ServerSymbolSpec::readable(canonical.clone(), descriptor);
        if config.writes_enabled && matches_any(&writable, canonical.as_str(), name.as_str()) {
            spec = spec.with_flag(SymbolFlag::Write);
        }
        specs.push(spec);
        if specs.len() >= config.max_symbols {
            break;
        }
    }

    build_server_symbol_snapshot(route_name(config), specs, DEFAULT_SERVER_SYMBOL_INDEX_GROUP)
        .map_err(symbol_error)
}

/// Returns the ADS symbol name for a global variable.
#[must_use]
pub fn server_symbol_name(config: &AdsServerRuntimeConfig, global_name: &str) -> String {
    if config.symbol_namespace.is_empty() {
        format!("global.{global_name}")
    } else {
        format!("{}.global.{global_name}", config.symbol_namespace)
    }
}

/// Resolves a server ADS symbol name back to a runtime global name.
#[must_use]
pub fn global_name_for_server_symbol<'a>(
    config: &AdsServerRuntimeConfig,
    symbol_name: &'a str,
) -> Option<&'a str> {
    let prefix = if config.symbol_namespace.is_empty() {
        "global.".to_string()
    } else {
        format!("{}.global.", config.symbol_namespace)
    };
    symbol_name.strip_prefix(prefix.as_str())
}

/// Maps a runtime value to the v1 ADS descriptor set.
///
/// Returns `None` for values v1 does not expose over ADS server yet, such as
/// structs, function-block instances, references, and IEC date/time values.
#[must_use]
pub fn descriptor_for_value(
    value: &Value,
    max_string_bytes: usize,
) -> Option<AdsDataTypeDescriptor> {
    match value {
        Value::Bool(_) => Some(scalar("BOOL", IecDataType::Bool)),
        Value::SInt(_) => Some(scalar("SINT", IecDataType::Sint)),
        Value::Int(_) => Some(scalar("INT", IecDataType::Int)),
        Value::DInt(_) => Some(scalar("DINT", IecDataType::Dint)),
        Value::LInt(_) => Some(scalar("LINT", IecDataType::Lint)),
        Value::USInt(_) => Some(scalar("USINT", IecDataType::Usint)),
        Value::UInt(_) => Some(scalar("UINT", IecDataType::Uint)),
        Value::UDInt(_) => Some(scalar("UDINT", IecDataType::Udint)),
        Value::ULInt(_) => Some(scalar("ULINT", IecDataType::Ulint)),
        Value::Real(_) => Some(scalar("REAL", IecDataType::Real)),
        Value::LReal(_) => Some(scalar("LREAL", IecDataType::Lreal)),
        Value::Byte(_) => Some(scalar("BYTE", IecDataType::Byte)),
        Value::Word(_) => Some(scalar("WORD", IecDataType::Word)),
        Value::DWord(_) => Some(scalar("DWORD", IecDataType::Dword)),
        Value::LWord(_) => Some(scalar("LWORD", IecDataType::Lword)),
        Value::String(value) => {
            let len = value
                .len()
                .max(80)
                .min(max_string_bytes)
                .min(usize::from(u16::MAX));
            Some(AdsDataTypeDescriptor::string("STRING", len as u16))
        }
        Value::Array(array) => {
            let first = array.elements().first()?;
            let mut descriptor = descriptor_for_value(first, max_string_bytes)?;
            if array.elements().iter().any(|value| {
                descriptor_for_value(value, max_string_bytes) != Some(descriptor.clone())
            }) {
                return None;
            }
            descriptor.source_name = format!("ARRAY OF {}", descriptor.source_name);
            descriptor.dimensions = array
                .dimensions()
                .iter()
                .map(|(lower, upper)| ArrayDimension {
                    lower: *lower,
                    upper: *upper,
                })
                .collect();
            Some(descriptor)
        }
        _ => None,
    }
}

/// Runtime ADS symbol provider.
#[derive(Debug, Clone)]
pub struct AdsServerSymbolSource {
    snapshot: Arc<RwLock<Arc<SymbolSnapshot>>>,
    counter: Arc<Mutex<SymbolVersionCounter>>,
}

impl AdsServerSymbolSource {
    /// Builds a symbol source from a runtime snapshot.
    ///
    /// # Errors
    ///
    /// Returns a runtime configuration error when symbol publication fails.
    pub fn from_runtime_snapshot(
        config: &AdsServerRuntimeConfig,
        snapshot: &DebugSnapshot,
    ) -> Result<Self, RuntimeError> {
        let snapshot = build_runtime_symbol_snapshot(config, snapshot)?;
        let mut counter = SymbolVersionCounter::new();
        counter.observe_snapshot(&snapshot).map_err(symbol_error)?;
        Ok(Self {
            snapshot: Arc::new(RwLock::new(Arc::new(snapshot))),
            counter: Arc::new(Mutex::new(counter)),
        })
    }

    /// Refreshes the symbol table from a new runtime snapshot and returns the ADS symbol version.
    ///
    /// The version bumps only when the exposed symbol layout changes. Existing
    /// client requests keep using the snapshot clone they already acquired;
    /// subsequent requests see the refreshed table and `ADSIGRP_SYM_VERSION`.
    ///
    /// # Errors
    ///
    /// Returns a runtime configuration error when symbol publication fails.
    pub fn refresh_from_runtime_snapshot(
        &self,
        config: &AdsServerRuntimeConfig,
        snapshot: &DebugSnapshot,
    ) -> Result<u32, RuntimeError> {
        let next = build_runtime_symbol_snapshot(config, snapshot)?;
        let version = {
            let mut counter = self
                .counter
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            counter.observe_snapshot(&next).map_err(symbol_error)?
        };
        let mut guard = self
            .snapshot
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *guard = Arc::new(next);
        Ok(version)
    }
}

impl SymbolSource for AdsServerSymbolSource {
    fn snapshot(&self) -> Arc<SymbolSnapshot> {
        self.snapshot
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    fn version(&self) -> u32 {
        self.counter
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .version()
    }
}

fn route_name(config: &AdsServerRuntimeConfig) -> String {
    config
        .ams_net_id
        .as_ref()
        .map(|net_id| net_id.0.clone())
        .unwrap_or_else(|| "trust-runtime".to_string())
}

fn scalar(source_name: &str, iec_type: IecDataType) -> AdsDataTypeDescriptor {
    AdsDataTypeDescriptor::scalar(source_name, iec_type)
}

fn compile_patterns(patterns: &[SmolStr], field: &str) -> Result<Vec<Pattern>, RuntimeError> {
    patterns
        .iter()
        .map(|pattern| {
            Pattern::new(pattern.as_str()).map_err(|err| {
                RuntimeError::InvalidConfig(
                    format!("{field} invalid pattern '{}': {err}", pattern).into(),
                )
            })
        })
        .collect()
}

fn matches_any(patterns: &[Pattern], canonical: &str, bare: &str) -> bool {
    patterns
        .iter()
        .any(|pattern| pattern.matches(canonical) || pattern.matches(bare))
}

fn symbol_error(error: ServerSymbolError) -> RuntimeError {
    RuntimeError::InvalidConfig(
        format!("runtime.ads_server symbol publication failed: {error}").into(),
    )
}
