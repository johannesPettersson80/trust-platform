use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use smol_str::SmolStr;
use trust_ads_core::{
    AdsDataTypeDescriptor, IecDataType, PointQuality, PointStatus, QualityState, SymbolDescriptor,
};
use trust_runtime_core::native::image::type_kind;
use trust_runtime_core::native::memory::TypeShape;

use crate::error::RuntimeError;
use crate::harness::CompileError;
use crate::runtime::{BoundGlobal, GlobalPort};
use crate::value::{EnumValue, Value};
use crate::CompiledRuntime;

use super::contracts::{AdsConnectionConfig, AdsPointConfig};
use super::descriptors::{point_reads, point_writes, validate_point_against_symbols};
use super::transport::{AdsTransport, AdsTransportError};

mod cache;
#[cfg(test)]
mod compiled_tests;
mod cycle_port;
#[cfg(test)]
mod management_tests;
mod worker;

use cache::AdsSharedCache;
pub use worker::{AdsConnectionWorker, AdsWorkerThread};

const DEFAULT_RECONNECT_BACKOFF_MS: u64 = 2_000;
const DEFAULT_SYMBOL_VERSION_CHECK_INTERVAL_MS: u64 = 1_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdsConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    Faulted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum AdsBridgeErrorKind {
    Validation,
    Transport,
    Execution(RuntimeError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdsBridgeError {
    message: String,
    kind: AdsBridgeErrorKind,
}

impl AdsBridgeError {
    fn execution(error: RuntimeError) -> Self {
        Self {
            message: error.to_string(),
            kind: AdsBridgeErrorKind::Execution(error),
        }
    }

    pub(crate) fn execution_cause(&self) -> Option<&RuntimeError> {
        match &self.kind {
            AdsBridgeErrorKind::Execution(error) => Some(error),
            _ => None,
        }
    }

    fn validation(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            kind: AdsBridgeErrorKind::Validation,
        }
    }

    fn transport(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            kind: AdsBridgeErrorKind::Transport,
        }
    }

    pub fn message(&self) -> &str {
        self.message.as_str()
    }

    fn is_transport(&self) -> bool {
        self.kind == AdsBridgeErrorKind::Transport
    }
}

impl fmt::Display for AdsBridgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message.as_str())
    }
}

impl std::error::Error for AdsBridgeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.execution_cause()
            .map(|error| error as &dyn std::error::Error)
    }
}

impl From<AdsTransportError> for AdsBridgeError {
    fn from(value: AdsTransportError) -> Self {
        Self::transport(value.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdsBinding {
    pub point: AdsPointConfig,
    /// The global the point reads into or writes from.
    pub target: BoundGlobal,
    /// The global `<point>_quality` that takes the point's quality, when the program declares one.
    pub quality_target: Option<BoundGlobal>,
    pub retain: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AdsLiveValue {
    pub connection_name: String,
    pub ams_port: u16,
    pub point_name: String,
    pub input: bool,
    pub writable: bool,
    pub value: Value,
    pub forced: bool,
}

/// The bindings of a connection's points (specification 11's ADS client, specification 44 §13.3):
/// each point binds the global of its name, found through the program's symbol table; the program
/// resolves the name again at each access.
///
/// # Errors
///
/// A point bound twice, a global the program does not declare, a `<point>_quality` global of
/// another type than `ADS_QUALITY`, a configured type that does not match the declared one, and a
/// read of a retained global without `allow_retain_read`.
pub fn resolve_compiled_bindings(
    program: &CompiledRuntime,
    connection: &AdsConnectionConfig,
) -> Result<Vec<AdsBinding>, CompileError> {
    let mut seen = BTreeSet::new();
    let mut bindings = Vec::with_capacity(connection.points.len());
    for point in &connection.points {
        if !seen.insert(point.point_name.clone()) {
            return Err(CompileError::new(format!(
                "ADS point '{}' is bound more than once",
                point.point_name
            )));
        }
        let global = program.global(point.point_name.as_str()).ok_or_else(|| {
            CompileError::new(format!(
                "failed to resolve declared global '{}' for ADS binding",
                point.point_name
            ))
        })?;
        let quality_name = format!("{}_quality", point.point_name);
        let quality_target = match program.global(quality_name.as_str()) {
            Some(quality) => {
                let is_quality = matches!(
                    &quality.shape,
                    TypeShape::Enumeration { name, .. } if name.eq_ignore_ascii_case("ADS_QUALITY")
                );
                if !is_quality {
                    return Err(CompileError::new(format!(
                        "ADS quality binding '{quality_name}' must have type ADS_QUALITY, got {}",
                        quality.type_name
                    )));
                }
                Some(BoundGlobal(quality.name))
            }
            None => None,
        };
        if !ads_descriptor_matches_shape(&point.data_type, &global.shape) {
            return Err(CompileError::new(format!(
                "ADS point '{}' type mismatch: config {:?} does not match declared type {}",
                point.point_name, point.data_type, global.type_name
            )));
        }
        if point_reads(point.access) && global.retained && !point.allow_retain_read {
            return Err(CompileError::new(format!(
                "ADS read binding '{}' targets a RETAIN/PERSISTENT global; set allow_retain_read=true to acknowledge cold-start stale quality",
                point.point_name
            )));
        }
        bindings.push(AdsBinding {
            point: point.clone(),
            target: BoundGlobal(global.name),
            quality_target,
            retain: global.retained,
        });
    }
    Ok(bindings)
}

#[derive(Clone)]
pub struct AdsConnectionBridge {
    bindings: Vec<AdsBinding>,
    shared: AdsSharedCache,
    last_queued_values: BTreeMap<String, Value>,
}

impl AdsConnectionBridge {
    pub fn new(bindings: Vec<AdsBinding>) -> Result<Self, AdsBridgeError> {
        validate_bindings(&bindings)?;
        Ok(Self {
            shared: AdsSharedCache::new(&bindings),
            bindings,
            last_queued_values: BTreeMap::new(),
        })
    }

    pub fn with_transport<T: AdsTransport>(
        transport: T,
        bindings: Vec<AdsBinding>,
    ) -> Result<(Self, AdsConnectionWorker<T>), AdsBridgeError> {
        let bridge = Self::new(bindings)?;
        let worker = AdsConnectionWorker::new(
            transport,
            bridge.bindings.clone(),
            bridge.shared.clone(),
            DEFAULT_RECONNECT_BACKOFF_MS,
            DEFAULT_SYMBOL_VERSION_CHECK_INTERVAL_MS,
        );
        Ok((bridge, worker))
    }

    fn apply_inputs_with(
        &mut self,
        now_ms: u64,
        port: &mut (impl GlobalPort + ?Sized),
    ) -> Result<(), AdsBridgeError> {
        let snapshot = self.shared.snapshot();
        for binding in self
            .bindings
            .iter()
            .filter(|binding| point_reads(binding.point.access))
        {
            let Some(quality) = snapshot.qualities.get(binding.point.point_name.as_str()) else {
                self.shared.set_quality(
                    binding.point.point_name.as_str(),
                    PointQuality::error(
                        now_ms,
                        format!(
                            "ADS cache has no quality for '{}'",
                            binding.point.point_name
                        ),
                    ),
                );
                continue;
            };
            self.write_quality_with(binding, quality, now_ms, port)?;
            if quality.state != QualityState::Good {
                continue;
            }
            let Some(value) = snapshot
                .values
                .get(binding.point.point_name.as_str())
                .cloned()
            else {
                self.shared.set_quality(
                    binding.point.point_name.as_str(),
                    PointQuality::error(
                        now_ms,
                        format!(
                            "ADS cache has good quality without value for '{}'",
                            binding.point.point_name
                        ),
                    ),
                );
                continue;
            };
            let baseline = if point_writes(binding.point.access) {
                Some(value.clone())
            } else {
                None
            };
            if let Err(error) = port.write(&binding.target, value) {
                self.shared.set_quality(
                    binding.point.point_name.as_str(),
                    PointQuality::error(
                        now_ms,
                        format!(
                            "failed to write ADS input '{}': {} ({})",
                            binding.point.point_name,
                            error,
                            error.stable_code()
                        ),
                    ),
                );
                cycle_port::propagate_execution_error(error)?;
                continue;
            }
            if let Some(value) = baseline {
                self.last_queued_values
                    .insert(binding.point.point_name.clone(), value);
            }
        }
        Ok(())
    }

    fn write_quality_with(
        &self,
        binding: &AdsBinding,
        quality: &PointQuality,
        now_ms: u64,
        port: &mut (impl GlobalPort + ?Sized),
    ) -> Result<(), AdsBridgeError> {
        let Some(target) = binding.quality_target.as_ref() else {
            return Ok(());
        };
        if let Err(error) = port.write(target, quality_value(quality.state)) {
            self.shared.set_quality(
                binding.point.point_name.as_str(),
                PointQuality::error(
                    now_ms,
                    format!(
                        "failed to write ADS quality '{}_quality': {} ({})",
                        binding.point.point_name,
                        error,
                        error.stable_code()
                    ),
                ),
            );
            cycle_port::propagate_execution_error(error)?;
        }
        Ok(())
    }

    pub fn mark_reconnecting(&self, now_ms: u64, detail: impl Into<String>) {
        self.shared
            .mark_reconnecting(now_ms, DEFAULT_RECONNECT_BACKOFF_MS, detail.into());
    }

    pub fn state(&self) -> AdsConnectionState {
        self.shared.state()
    }

    pub fn statuses(&self) -> Vec<PointStatus> {
        self.shared.statuses()
    }

    pub fn status(&self, point_name: &str) -> Option<PointStatus> {
        self.shared.status(point_name)
    }

    pub fn pending_write(&self, point_name: &str) -> Option<Value> {
        self.shared
            .snapshot()
            .pending_writes
            .get(point_name)
            .cloned()
    }

    pub fn live_value_writable(&self, point_name: &str) -> Option<bool> {
        self.bindings
            .iter()
            .find(|binding| binding.point.point_name == point_name)
            .map(|binding| point_writes(binding.point.access))
    }

    pub fn queue_live_write(&mut self, point_name: &str, value: Value) -> bool {
        if !self.bindings.iter().any(|binding| {
            binding.point.point_name == point_name && point_writes(binding.point.access)
        }) {
            return false;
        }
        self.commit_qualified_live_write(point_name, value);
        true
    }

    /// Owner-only commit after selecting this bridge's writable binding. The
    /// exclusive bridge borrow keeps that qualification valid across PLC writes.
    pub(crate) fn commit_qualified_live_write(&mut self, point_name: &str, value: Value) {
        self.shared
            .queue_write(point_name.to_string(), value.clone());
        self.last_queued_values
            .insert(point_name.to_string(), value);
    }
}

fn contains_non_finite_float(value: &Value) -> bool {
    match value {
        Value::Real(value) => !value.is_finite(),
        Value::LReal(value) => !value.is_finite(),
        Value::Array(array) => array.elements().iter().any(contains_non_finite_float),
        _ => false,
    }
}

fn validate_bindings(bindings: &[AdsBinding]) -> Result<(), AdsBridgeError> {
    let mut point_names = BTreeSet::new();
    for binding in bindings {
        if !point_names.insert(binding.point.point_name.clone()) {
            return Err(AdsBridgeError::validation(format!(
                "ADS point '{}' is bound more than once",
                binding.point.point_name
            )));
        }
        if point_reads(binding.point.access) && binding.retain && !binding.point.allow_retain_read {
            return Err(AdsBridgeError::validation(format!(
                "ADS read binding '{}' targets a retained global without allow_retain_read=true",
                binding.point.point_name
            )));
        }
    }
    Ok(())
}

fn quality_value(state: QualityState) -> Value {
    let (variant, ordinal) = match state {
        QualityState::Stale => ("Stale", 0),
        QualityState::Good => ("Good", 1),
        QualityState::Error => ("Error", 2),
    };
    Value::Enum(Box::new(EnumValue::from_canonical_parts(
        SmolStr::new("ADS_QUALITY"),
        SmolStr::new(variant),
        ordinal,
    )))
}

fn validate_remote_symbols(
    bindings: &[AdsBinding],
    symbols: &[SymbolDescriptor],
) -> Result<(), AdsBridgeError> {
    for binding in bindings {
        validate_point_against_symbols(&binding.point, symbols)
            .map_err(|err| AdsBridgeError::validation(err.to_string()))?;
    }
    Ok(())
}

/// Whether the ADS descriptor fits a compiled program's global of `shape`, as
/// [`ads_descriptor_matches_type_id`] decides for an interpreted program's. The type table holds
/// an enumeration's size, not its base type, so an enumeration takes the integer types of its size.
fn ads_descriptor_matches_shape(descriptor: &AdsDataTypeDescriptor, shape: &TypeShape) -> bool {
    match shape {
        TypeShape::Elementary(kind) | TypeShape::Subrange(kind) => {
            descriptor.dimensions.is_empty() && scalar_matches_kind(descriptor.iec_type, *kind)
        }
        TypeShape::Enumeration { size, .. } => {
            descriptor.dimensions.is_empty()
                && matches!(
                    (descriptor.iec_type, size),
                    (IecDataType::Sint | IecDataType::Usint, 1)
                        | (IecDataType::Int | IecDataType::Uint, 2)
                        | (IecDataType::Dint | IecDataType::Udint, 4)
                        | (IecDataType::Lint | IecDataType::Ulint, 8)
                )
        }
        TypeShape::Text {
            wide: false,
            capacity,
        } => {
            descriptor.dimensions.is_empty()
                && descriptor.iec_type == IecDataType::String
                && descriptor
                    .string_len
                    .is_some_and(|len| u32::from(len) == *capacity)
        }
        TypeShape::Array {
            element,
            dimensions,
        } => {
            let ads_dimensions = descriptor
                .dimensions
                .iter()
                .map(|dimension| (dimension.lower, dimension.upper))
                .collect::<Vec<_>>();
            let scalar = AdsDataTypeDescriptor {
                source_name: descriptor.source_name.clone(),
                iec_type: descriptor.iec_type,
                dimensions: Vec::new(),
                string_len: descriptor.string_len,
            };
            ads_dimensions == *dimensions && ads_descriptor_matches_shape(&scalar, element)
        }
        TypeShape::Text { wide: true, .. } | TypeShape::Other => false,
    }
}

/// Whether the ADS IEC type is the elementary type `kind` of the type table.
fn scalar_matches_kind(iec_type: IecDataType, kind: u32) -> bool {
    matches!(
        (iec_type, kind),
        (IecDataType::Bool, type_kind::BOOL)
            | (IecDataType::Sint, type_kind::SINT)
            | (IecDataType::Int, type_kind::INT)
            | (IecDataType::Dint, type_kind::DINT)
            | (IecDataType::Lint, type_kind::LINT)
            | (IecDataType::Usint, type_kind::USINT)
            | (IecDataType::Uint, type_kind::UINT)
            | (IecDataType::Udint, type_kind::UDINT)
            | (IecDataType::Ulint, type_kind::ULINT)
            | (IecDataType::Real, type_kind::REAL)
            | (IecDataType::Lreal, type_kind::LREAL)
            | (IecDataType::Byte, type_kind::BYTE)
            | (IecDataType::Word, type_kind::WORD)
            | (IecDataType::Dword, type_kind::DWORD)
            | (IecDataType::Lword, type_kind::LWORD)
    )
}
