use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use trust_ads_core::{
    AdsDataTypeDescriptor, IecDataType, PointAccess, PointQuality, SymbolDescriptor, SymbolFlag,
    UpdateMode,
};
use trust_hir::TypeId;

use crate::memory::{MemoryLocation, VariableStorage};
use crate::value::{Value, ValueRef};

use super::super::super::AdsConnectionBridge;
use super::super::*;
use crate::ads::contracts::AdsPointConfig;
use crate::ads::transport::{
    AdsDeviceState, AdsNotificationMode, AdsNotificationSample, AdsPointAddress, AdsReadResult,
    AdsTransportError,
};

enum ScriptStep<T> {
    Ok(T),
    Err(&'static str),
}

impl<T> ScriptStep<T> {
    fn finish(self) -> Result<T, AdsTransportError> {
        match self {
            Self::Ok(value) => Ok(value),
            Self::Err(message) => Err(AdsTransportError::new(message)),
        }
    }
}

#[derive(Clone, Default)]
pub(super) struct TransportProbe {
    inner: Arc<Mutex<TransportProbeState>>,
}

#[derive(Default)]
struct TransportProbeState {
    connects: usize,
    disconnects: usize,
    symbol_version_calls: usize,
}

impl TransportProbe {
    pub(super) fn connects(&self) -> usize {
        self.inner.lock().expect("transport probe lock").connects
    }

    pub(super) fn disconnects(&self) -> usize {
        self.inner.lock().expect("transport probe lock").disconnects
    }

    pub(super) fn symbol_version_calls(&self) -> usize {
        self.inner
            .lock()
            .expect("transport probe lock")
            .symbol_version_calls
    }
}

pub(super) struct ScriptedAdsTransport {
    connected: bool,
    symbols: Vec<SymbolDescriptor>,
    handle_batches: VecDeque<ScriptStep<Vec<AdsResolvedHandle>>>,
    read_batches: VecDeque<ScriptStep<Vec<AdsReadResult>>>,
    write_batches: VecDeque<ScriptStep<Vec<PointQuality>>>,
    subscription_results: VecDeque<ScriptStep<AdsSubscription>>,
    notification_batches: VecDeque<ScriptStep<Vec<AdsNotificationSample>>>,
    symbol_versions: VecDeque<ScriptStep<u32>>,
    last_symbol_version: u32,
    next_handle: u32,
    next_subscription: u32,
    write_hook: Option<Box<dyn FnMut() + Send>>,
    probe: TransportProbe,
}

impl ScriptedAdsTransport {
    pub(super) fn new(symbols: Vec<SymbolDescriptor>) -> Self {
        Self {
            connected: false,
            symbols,
            handle_batches: VecDeque::new(),
            read_batches: VecDeque::new(),
            write_batches: VecDeque::new(),
            subscription_results: VecDeque::new(),
            notification_batches: VecDeque::new(),
            symbol_versions: VecDeque::new(),
            last_symbol_version: 1,
            next_handle: 1,
            next_subscription: 1,
            write_hook: None,
            probe: TransportProbe::default(),
        }
    }

    pub(super) fn for_bindings(bindings: &[AdsBinding]) -> Self {
        let symbols = bindings
            .iter()
            .enumerate()
            .filter_map(|(index, binding)| symbol_for_binding(binding, index))
            .collect();
        Self::new(symbols)
    }

    pub(super) fn probe(&self) -> TransportProbe {
        self.probe.clone()
    }

    pub(super) fn script_handles(&mut self, handles: Vec<AdsResolvedHandle>) {
        self.handle_batches.push_back(ScriptStep::Ok(handles));
    }

    pub(super) fn script_reads(&mut self, reads: Vec<AdsReadResult>) {
        self.read_batches.push_back(ScriptStep::Ok(reads));
    }

    pub(super) fn script_read_error(&mut self, message: &'static str) {
        self.read_batches.push_back(ScriptStep::Err(message));
    }

    pub(super) fn script_writes(&mut self, qualities: Vec<PointQuality>) {
        self.write_batches.push_back(ScriptStep::Ok(qualities));
    }

    pub(super) fn script_write_error(&mut self, message: &'static str) {
        self.write_batches.push_back(ScriptStep::Err(message));
    }

    pub(super) fn script_subscription(&mut self, subscription: AdsSubscription) {
        self.subscription_results
            .push_back(ScriptStep::Ok(subscription));
    }

    pub(super) fn script_notifications(&mut self, samples: Vec<AdsNotificationSample>) {
        self.notification_batches.push_back(ScriptStep::Ok(samples));
    }

    pub(super) fn script_symbol_versions(&mut self, versions: impl IntoIterator<Item = u32>) {
        self.symbol_versions
            .extend(versions.into_iter().map(ScriptStep::Ok));
    }

    pub(super) fn on_write(&mut self, hook: impl FnMut() + Send + 'static) {
        self.write_hook = Some(Box::new(hook));
    }

    fn require_connected(&self) -> Result<(), AdsTransportError> {
        if self.connected {
            Ok(())
        } else {
            Err(AdsTransportError::new(
                "scripted ADS transport is disconnected",
            ))
        }
    }
}

impl AdsTransport for ScriptedAdsTransport {
    fn connect(&mut self) -> Result<(), AdsTransportError> {
        self.connected = true;
        self.probe
            .inner
            .lock()
            .expect("transport probe lock")
            .connects += 1;
        Ok(())
    }

    fn disconnect(&mut self) -> Result<(), AdsTransportError> {
        self.connected = false;
        self.probe
            .inner
            .lock()
            .expect("transport probe lock")
            .disconnects += 1;
        Ok(())
    }

    fn read_state(&mut self) -> Result<AdsDeviceState, AdsTransportError> {
        self.require_connected()?;
        Ok(AdsDeviceState::Run)
    }

    fn upload_symbol_table(&mut self) -> Result<Vec<SymbolDescriptor>, AdsTransportError> {
        self.require_connected()?;
        Ok(self.symbols.clone())
    }

    fn resolve_handles(
        &mut self,
        requests: &[AdsHandleRequest],
    ) -> Result<Vec<AdsResolvedHandle>, AdsTransportError> {
        self.require_connected()?;
        if let Some(step) = self.handle_batches.pop_front() {
            return step.finish();
        }
        let handles = requests
            .iter()
            .map(|request| {
                let handle = self.next_handle;
                self.next_handle = self.next_handle.wrapping_add(1);
                AdsResolvedHandle {
                    point_name: request.point_name.clone(),
                    address: request.address.clone(),
                    data_type: request.data_type.clone(),
                    handle,
                }
            })
            .collect();
        Ok(handles)
    }

    fn sumup_read(
        &mut self,
        handles: &[AdsResolvedHandle],
    ) -> Result<Vec<AdsReadResult>, AdsTransportError> {
        self.require_connected()?;
        if let Some(step) = self.read_batches.pop_front() {
            return step.finish();
        }
        Ok(handles
            .iter()
            .map(|handle| AdsReadResult {
                point_name: handle.point_name.clone(),
                value: None,
                quality: PointQuality::error(0, "scripted read has no value"),
            })
            .collect())
    }

    fn sumup_write(
        &mut self,
        writes: &[AdsWriteRequest],
    ) -> Result<Vec<PointQuality>, AdsTransportError> {
        self.require_connected()?;
        if let Some(hook) = self.write_hook.as_mut() {
            hook();
        }
        if let Some(step) = self.write_batches.pop_front() {
            return step.finish();
        }
        Ok(writes.iter().map(|_| PointQuality::good(0)).collect())
    }

    fn subscribe(
        &mut self,
        request: AdsSubscribeRequest,
    ) -> Result<AdsSubscription, AdsTransportError> {
        self.require_connected()?;
        if let Some(step) = self.subscription_results.pop_front() {
            return step.finish();
        }
        let subscription = AdsSubscription {
            point_name: request.handle.point_name,
            subscription_id: self.next_subscription,
        };
        self.next_subscription = self.next_subscription.wrapping_add(1);
        Ok(subscription)
    }

    fn drain_notifications(&mut self) -> Result<Vec<AdsNotificationSample>, AdsTransportError> {
        self.require_connected()?;
        self.notification_batches
            .pop_front()
            .map_or_else(|| Ok(Vec::new()), ScriptStep::finish)
    }

    fn symbol_version(&mut self) -> Result<u32, AdsTransportError> {
        self.require_connected()?;
        self.probe
            .inner
            .lock()
            .expect("transport probe lock")
            .symbol_version_calls += 1;
        let value = self
            .symbol_versions
            .pop_front()
            .map_or_else(|| Ok(self.last_symbol_version), ScriptStep::finish)?;
        self.last_symbol_version = value;
        Ok(value)
    }
}

pub(super) fn real_descriptor() -> AdsDataTypeDescriptor {
    AdsDataTypeDescriptor::scalar("REAL", IecDataType::Real)
}

pub(super) fn point(
    point_name: &str,
    symbol_name: &str,
    access: PointAccess,
    mode: UpdateMode,
) -> AdsPointConfig {
    AdsPointConfig {
        point_name: point_name.to_string(),
        address: AdsPointAddress::Symbol(symbol_name.to_string()),
        data_type: real_descriptor(),
        access,
        mode,
        notification_mode: AdsNotificationMode::OnChange,
        allow_retain_read: false,
    }
}

pub(super) fn binding(point: AdsPointConfig) -> AdsBinding {
    AdsBinding {
        point,
        reference: ValueRef {
            location: MemoryLocation::Global,
            offset: 0,
            path: Vec::new(),
        },
        quality_reference: None,
        type_id: TypeId::REAL,
        retain: false,
    }
}

fn symbol_for_binding(binding: &AdsBinding, index: usize) -> Option<SymbolDescriptor> {
    let AdsPointAddress::Symbol(symbol_name) = &binding.point.address else {
        return None;
    };
    let byte_size = u32::try_from(binding.point.data_type.byte_len().ok()?).ok()?;
    let mut symbol = SymbolDescriptor::new(
        symbol_name,
        binding.point.data_type.clone(),
        0x4020,
        u32::try_from(index)
            .unwrap_or(u32::MAX)
            .saturating_mul(byte_size),
        byte_size,
    );
    if point_reads(binding.point.access) {
        symbol = symbol.with_flag(SymbolFlag::Read);
    }
    if point_writes(binding.point.access) {
        symbol = symbol.with_flag(SymbolFlag::Write);
    }
    Some(symbol)
}

pub(super) fn resolved(binding: &AdsBinding, handle: u32) -> AdsResolvedHandle {
    AdsResolvedHandle {
        point_name: binding.point.point_name.clone(),
        address: binding.point.address.clone(),
        data_type: binding.point.data_type.clone(),
        handle,
    }
}

pub(super) fn worker_fixture(
    bindings: Vec<AdsBinding>,
    transport: ScriptedAdsTransport,
) -> (AdsSharedCache, AdsConnectionWorker<ScriptedAdsTransport>) {
    let shared = AdsSharedCache::new(&bindings);
    let worker = AdsConnectionWorker::new(transport, bindings, shared.clone(), 25, 100);
    (shared, worker)
}

pub(super) fn output_fixture(
    initial: Value,
) -> (
    VariableStorage,
    AdsConnectionBridge,
    AdsConnectionWorker<ScriptedAdsTransport>,
) {
    typed_output_fixture(initial, real_descriptor(), TypeId::REAL)
}

pub(super) fn typed_output_fixture(
    initial: Value,
    descriptor: AdsDataTypeDescriptor,
    type_id: TypeId,
) -> (
    VariableStorage,
    AdsConnectionBridge,
    AdsConnectionWorker<ScriptedAdsTransport>,
) {
    let mut storage = VariableStorage::new();
    storage.set_global("out", initial);
    let reference = storage.ref_for_global("out").expect("output reference");
    let mut output = binding(point(
        "out",
        "MAIN.Out",
        PointAccess::Write,
        UpdateMode::Poll,
    ));
    output.reference = reference;
    output.point.data_type = descriptor;
    output.type_id = type_id;
    let transport = ScriptedAdsTransport::for_bindings(std::slice::from_ref(&output));
    let (bridge, worker) =
        AdsConnectionBridge::with_transport(transport, vec![output]).expect("output bridge");
    (storage, bridge, worker)
}

pub(super) fn read_result(point_name: &str, value: Value, quality: PointQuality) -> AdsReadResult {
    AdsReadResult {
        point_name: point_name.to_string(),
        value: Some(value),
        quality,
    }
}

pub(super) fn notification(
    point_name: &str,
    subscription_id: u32,
    value: Value,
) -> AdsNotificationSample {
    AdsNotificationSample {
        point_name: point_name.to_string(),
        subscription_id,
        value: Some(value),
        quality: PointQuality::good(1),
    }
}
