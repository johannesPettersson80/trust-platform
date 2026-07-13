use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use smol_str::SmolStr;
use trust_ads_core::{
    AdsDataTypeDescriptor, IecDataType, PointQuality, PointStatus, QualityState, SymbolDescriptor,
};
use trust_hir::{Type, TypeId};

use crate::harness::CompileError;
use crate::memory::VariableStorage;
use crate::value::{EnumValue, Value, ValueRef};
use crate::{RetainPolicy, Runtime};

use super::contracts::{AdsConnectionConfig, AdsPointConfig};
use super::descriptors::{point_reads, point_writes, validate_point_against_symbols};
use super::transport::{AdsTransport, AdsTransportError};

mod cache;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AdsBridgeErrorKind {
    Validation,
    Transport,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdsBridgeError {
    message: String,
    kind: AdsBridgeErrorKind,
}

impl AdsBridgeError {
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

impl std::error::Error for AdsBridgeError {}

impl From<AdsTransportError> for AdsBridgeError {
    fn from(value: AdsTransportError) -> Self {
        Self::transport(value.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdsBinding {
    pub point: AdsPointConfig,
    pub reference: ValueRef,
    pub quality_reference: Option<ValueRef>,
    pub type_id: TypeId,
    pub retain: bool,
}

pub fn resolve_declared_bindings(
    runtime: &Runtime,
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
        let meta = runtime
            .globals()
            .get(point.point_name.as_str())
            .ok_or_else(|| {
                CompileError::new(format!(
                    "failed to resolve declared global '{}' for ADS binding",
                    point.point_name
                ))
            })?;
        let reference = runtime
            .storage()
            .ref_for_global(point.point_name.as_str())
            .ok_or_else(|| {
                CompileError::new(format!(
                    "failed to resolve ValueRef for ADS binding '{}'",
                    point.point_name
                ))
            })?;
        let quality_reference = resolve_quality_reference(runtime, point)?;
        validate_declared_type(point, meta.type_id, runtime.registry())?;
        let retain = matches!(meta.retain, RetainPolicy::Retain | RetainPolicy::Persistent);
        if point_reads(point.access) && retain && !point.allow_retain_read {
            return Err(CompileError::new(format!(
                "ADS read binding '{}' targets a RETAIN/PERSISTENT global; set allow_retain_read=true to acknowledge cold-start stale quality",
                point.point_name
            )));
        }
        bindings.push(AdsBinding {
            point: point.clone(),
            reference,
            quality_reference,
            type_id: meta.type_id,
            retain,
        });
    }
    Ok(bindings)
}

#[derive(Clone)]
pub struct AdsConnectionBridge {
    bindings: Vec<AdsBinding>,
    shared: AdsSharedCache,
    live_values: super::live_values::AdsAppliedLiveValues,
    last_queued_values: BTreeMap<String, Value>,
}

impl AdsConnectionBridge {
    pub fn new(bindings: Vec<AdsBinding>) -> Result<Self, AdsBridgeError> {
        validate_bindings(&bindings)?;
        Ok(Self {
            shared: AdsSharedCache::new(&bindings),
            live_values: super::live_values::AdsAppliedLiveValues::new(&bindings),
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

    pub fn apply_inputs(
        &mut self,
        storage: &mut VariableStorage,
        now_ms: u64,
    ) -> Result<(), AdsBridgeError> {
        let snapshot = self.shared.snapshot();
        for binding in self
            .bindings
            .iter()
            .filter(|binding| point_reads(binding.point.access))
        {
            let Some(quality) = snapshot.qualities.get(binding.point.point_name.as_str()) else {
                let quality = PointQuality::error(
                    now_ms,
                    format!(
                        "ADS cache has no quality for '{}'",
                        binding.point.point_name
                    ),
                );
                self.shared
                    .set_quality(binding.point.point_name.as_str(), quality.clone());
                self.live_values
                    .commit_quality(binding.point.point_name.as_str(), quality);
                continue;
            };
            let applied_quality = self.write_quality(storage, binding, quality, now_ms);
            if quality.state != QualityState::Good {
                self.live_values
                    .commit_quality(binding.point.point_name.as_str(), applied_quality);
                continue;
            }
            let Some(value) = snapshot
                .values
                .get(binding.point.point_name.as_str())
                .cloned()
            else {
                let quality = PointQuality::error(
                    now_ms,
                    format!(
                        "ADS cache has good quality without value for '{}'",
                        binding.point.point_name
                    ),
                );
                self.shared
                    .set_quality(binding.point.point_name.as_str(), quality.clone());
                self.live_values
                    .commit_quality(binding.point.point_name.as_str(), quality);
                continue;
            };
            let baseline = if point_writes(binding.point.access) {
                Some(value.clone())
            } else {
                None
            };
            if !storage.write_by_ref(binding.reference.clone(), value) {
                let quality = PointQuality::error(
                    now_ms,
                    format!("failed to write ADS input '{}'", binding.point.point_name),
                );
                self.shared
                    .set_quality(binding.point.point_name.as_str(), quality.clone());
                self.live_values
                    .commit_quality(binding.point.point_name.as_str(), quality);
                continue;
            }
            let applied_value = storage
                .read_by_ref(binding.reference.clone())
                .cloned()
                .unwrap_or(Value::Null);
            self.live_values.commit_value(
                binding.point.point_name.as_str(),
                applied_value,
                applied_quality,
            );
            if let Some(value) = baseline {
                self.last_queued_values
                    .insert(binding.point.point_name.clone(), value);
            }
        }
        Ok(())
    }

    pub fn capture_outputs(
        &mut self,
        storage: &mut VariableStorage,
        now_ms: u64,
    ) -> Result<(), AdsBridgeError> {
        let snapshot = self.shared.snapshot();
        for binding in self
            .bindings
            .iter()
            .filter(|binding| point_writes(binding.point.access))
        {
            let cache_quality = snapshot
                .qualities
                .get(binding.point.point_name.as_str())
                .cloned()
                .unwrap_or_else(|| {
                    PointQuality::error(
                        now_ms,
                        format!(
                            "ADS cache has no quality for '{}'",
                            binding.point.point_name
                        ),
                    )
                });
            let written_quality = self.write_quality(storage, binding, &cache_quality, now_ms);
            let Some(value) = storage.read_by_ref(binding.reference.clone()).cloned() else {
                let quality = PointQuality::error(
                    now_ms,
                    format!("failed to read ADS output '{}'", binding.point.point_name),
                );
                self.shared
                    .set_quality(binding.point.point_name.as_str(), quality.clone());
                self.live_values
                    .commit_quality(binding.point.point_name.as_str(), quality);
                continue;
            };
            let applied_quality = if point_reads(binding.point.access) {
                self.live_values
                    .quality(binding.point.point_name.as_str())
                    .unwrap_or(written_quality)
            } else {
                written_quality
            };
            self.live_values.commit_value(
                binding.point.point_name.as_str(),
                value.clone(),
                applied_quality,
            );
            if self
                .last_queued_values
                .get(binding.point.point_name.as_str())
                .is_some_and(|previous| previous == &value)
            {
                continue;
            }
            self.shared
                .queue_write(binding.point.point_name.clone(), value.clone());
            self.last_queued_values
                .insert(binding.point.point_name.clone(), value);
        }
        Ok(())
    }

    fn write_quality(
        &self,
        storage: &mut VariableStorage,
        binding: &AdsBinding,
        quality: &PointQuality,
        now_ms: u64,
    ) -> PointQuality {
        let Some(reference) = binding.quality_reference.clone() else {
            return quality.clone();
        };
        if !storage.write_by_ref(reference, quality_value(quality.state)) {
            let error = PointQuality::error(
                now_ms,
                format!(
                    "failed to write ADS quality '{}_quality'",
                    binding.point.point_name
                ),
            );
            self.shared
                .set_quality(binding.point.point_name.as_str(), error.clone());
            return error;
        }
        quality.clone()
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

    pub(crate) fn initialize_live_values(&mut self, storage: &VariableStorage) {
        self.live_values.initialize(&self.bindings, storage);
    }

    pub(crate) fn live_value_entries(&self, connection: &str) -> Vec<super::AdsLiveValueEntry> {
        self.live_values.entries(connection, &self.bindings)
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

fn resolve_quality_reference(
    runtime: &Runtime,
    point: &AdsPointConfig,
) -> Result<Option<ValueRef>, CompileError> {
    let quality_name = format!("{}_quality", point.point_name);
    let Some(meta) = runtime.globals().get(quality_name.as_str()) else {
        return Ok(None);
    };
    validate_quality_type(quality_name.as_str(), meta.type_id, runtime.registry())?;
    let reference = runtime
        .storage()
        .ref_for_global(quality_name.as_str())
        .ok_or_else(|| {
            CompileError::new(format!(
                "failed to resolve ValueRef for ADS quality binding '{quality_name}'"
            ))
        })?;
    Ok(Some(reference))
}

fn validate_quality_type(
    quality_name: &str,
    type_id: TypeId,
    registry: &trust_hir::types::TypeRegistry,
) -> Result<(), CompileError> {
    if is_ads_quality_type(type_id, registry) {
        return Ok(());
    }
    Err(CompileError::new(format!(
        "ADS quality binding '{quality_name}' must have type ADS_QUALITY, got {}",
        type_name(type_id, registry)
    )))
}

fn is_ads_quality_type(type_id: TypeId, registry: &trust_hir::types::TypeRegistry) -> bool {
    let Some(ty) = registry.get(type_id) else {
        return false;
    };
    match ty {
        Type::Alias { target, .. } => is_ads_quality_type(*target, registry),
        Type::Enum { name, .. } => name.eq_ignore_ascii_case("ADS_QUALITY"),
        _ => false,
    }
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

fn validate_declared_type(
    point: &AdsPointConfig,
    type_id: TypeId,
    registry: &trust_hir::types::TypeRegistry,
) -> Result<(), CompileError> {
    if ads_descriptor_matches_type_id(&point.data_type, type_id, registry) {
        Ok(())
    } else {
        Err(CompileError::new(format!(
            "ADS point '{}' type mismatch: config {:?} does not match declared type {}",
            point.point_name,
            point.data_type,
            type_name(type_id, registry)
        )))
    }
}

fn ads_descriptor_matches_type_id(
    descriptor: &AdsDataTypeDescriptor,
    type_id: TypeId,
    registry: &trust_hir::types::TypeRegistry,
) -> bool {
    let Some(ty) = registry.get(type_id) else {
        return descriptor.dimensions.is_empty()
            && scalar_matches_type_id(descriptor.iec_type, type_id);
    };
    match ty {
        Type::Alias { target, .. } => ads_descriptor_matches_type_id(descriptor, *target, registry),
        Type::Subrange { base, .. } => {
            descriptor.dimensions.is_empty() && scalar_matches_type_id(descriptor.iec_type, *base)
        }
        Type::Enum { base, .. } => {
            descriptor.dimensions.is_empty() && scalar_matches_type_id(descriptor.iec_type, *base)
        }
        Type::Array {
            element,
            dimensions,
        } => {
            let ads_dimensions = descriptor
                .dimensions
                .iter()
                .map(|dimension| (dimension.lower, dimension.upper))
                .collect::<Vec<_>>();
            ads_dimensions == *dimensions
                && scalar_descriptor_matches_type_id(descriptor, *element, registry)
        }
        _ => descriptor.dimensions.is_empty() && scalar_type_matches(descriptor, ty, type_id),
    }
}

fn scalar_descriptor_matches_type_id(
    descriptor: &AdsDataTypeDescriptor,
    type_id: TypeId,
    registry: &trust_hir::types::TypeRegistry,
) -> bool {
    let scalar = AdsDataTypeDescriptor {
        source_name: descriptor.source_name.clone(),
        iec_type: descriptor.iec_type,
        dimensions: Vec::new(),
        string_len: descriptor.string_len,
    };
    ads_descriptor_matches_type_id(&scalar, type_id, registry)
}

fn scalar_type_matches(descriptor: &AdsDataTypeDescriptor, ty: &Type, type_id: TypeId) -> bool {
    match (descriptor.iec_type, ty) {
        (IecDataType::Bool, Type::Bool)
        | (IecDataType::Sint, Type::SInt)
        | (IecDataType::Int, Type::Int)
        | (IecDataType::Dint, Type::DInt)
        | (IecDataType::Lint, Type::LInt)
        | (IecDataType::Usint, Type::USInt)
        | (IecDataType::Uint, Type::UInt)
        | (IecDataType::Udint, Type::UDInt)
        | (IecDataType::Ulint, Type::ULInt)
        | (IecDataType::Real, Type::Real)
        | (IecDataType::Lreal, Type::LReal)
        | (IecDataType::Byte, Type::Byte)
        | (IecDataType::Word, Type::Word)
        | (IecDataType::Dword, Type::DWord)
        | (IecDataType::Lword, Type::LWord) => true,
        (IecDataType::String, Type::String { max_len }) => descriptor
            .string_len
            .is_some_and(|len| max_len.is_none_or(|max| u32::from(len) == max)),
        _ => scalar_matches_type_id(descriptor.iec_type, type_id),
    }
}

fn scalar_matches_type_id(iec_type: IecDataType, type_id: TypeId) -> bool {
    matches!(
        (iec_type, type_id),
        (IecDataType::Bool, TypeId::BOOL)
            | (IecDataType::Sint, TypeId::SINT)
            | (IecDataType::Int, TypeId::INT)
            | (IecDataType::Dint, TypeId::DINT)
            | (IecDataType::Lint, TypeId::LINT)
            | (IecDataType::Usint, TypeId::USINT)
            | (IecDataType::Uint, TypeId::UINT)
            | (IecDataType::Udint, TypeId::UDINT)
            | (IecDataType::Ulint, TypeId::ULINT)
            | (IecDataType::Real, TypeId::REAL)
            | (IecDataType::Lreal, TypeId::LREAL)
            | (IecDataType::Byte, TypeId::BYTE)
            | (IecDataType::Word, TypeId::WORD)
            | (IecDataType::Dword, TypeId::DWORD)
            | (IecDataType::Lword, TypeId::LWORD)
            | (IecDataType::String, TypeId::STRING)
    )
}

fn type_name(type_id: TypeId, registry: &trust_hir::types::TypeRegistry) -> SmolStr {
    registry
        .type_name(type_id)
        .unwrap_or_else(|| SmolStr::new(format!("TypeId({})", type_id.0)))
}
