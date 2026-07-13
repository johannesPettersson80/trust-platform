use std::fmt;

use trust_ads_core::{AdsDataTypeDescriptor, PointQuality, SymbolDescriptor, UpdateMode};
use trust_runtime_core::value::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdsPointAddress {
    Symbol(String),
    Index {
        index_group: u32,
        index_offset: u32,
        size: u32,
    },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AdsDeviceState {
    Run,
    Stop,
    Fault,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdsHandleRequest {
    pub point_name: String,
    pub address: AdsPointAddress,
    pub data_type: AdsDataTypeDescriptor,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdsResolvedHandle {
    pub point_name: String,
    pub address: AdsPointAddress,
    pub data_type: AdsDataTypeDescriptor,
    pub handle: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AdsReadResult {
    pub point_name: String,
    pub value: Option<Value>,
    pub quality: PointQuality,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AdsNotificationSample {
    pub point_name: String,
    pub subscription_id: u32,
    pub value: Option<Value>,
    pub quality: PointQuality,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AdsWriteRequest {
    pub handle: AdsResolvedHandle,
    pub value: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdsSubscribeRequest {
    pub handle: AdsResolvedHandle,
    pub mode: UpdateMode,
    pub notification_mode: AdsNotificationMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdsSubscription {
    pub point_name: String,
    pub subscription_id: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdsNotificationMode {
    OnChange,
    Cyclic,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdsTransportErrorInfo {
    pub code: u32,
    pub name: String,
}

impl AdsTransportErrorInfo {
    #[must_use]
    pub fn new(code: u32, name: impl Into<String>) -> Self {
        Self {
            code,
            name: name.into(),
        }
    }
}

/// Stable transport-level failure semantics that callers can classify without
/// parsing platform-specific error messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdsTransportFailureKind {
    TimedOut,
    ConnectionRefused,
    HostUnreachable,
    NetworkUnreachable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdsTransportError {
    message: String,
    ads_error: Option<AdsTransportErrorInfo>,
    failure_kind: Option<AdsTransportFailureKind>,
}

impl AdsTransportError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            ads_error: None,
            failure_kind: None,
        }
    }

    #[must_use]
    pub fn with_ads_error(mut self, code: u32, name: impl Into<String>) -> Self {
        self.ads_error = Some(AdsTransportErrorInfo::new(code, name));
        self
    }

    #[must_use]
    pub fn with_failure_kind(mut self, failure_kind: AdsTransportFailureKind) -> Self {
        self.failure_kind = Some(failure_kind);
        self
    }

    pub fn message(&self) -> &str {
        self.message.as_str()
    }

    #[must_use]
    pub fn ads_error(&self) -> Option<&AdsTransportErrorInfo> {
        self.ads_error.as_ref()
    }

    #[must_use]
    pub fn failure_kind(&self) -> Option<AdsTransportFailureKind> {
        self.failure_kind
    }
}

impl fmt::Display for AdsTransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message.as_str())
    }
}

impl std::error::Error for AdsTransportError {}

pub trait AdsTransport {
    fn connect(&mut self) -> Result<(), AdsTransportError>;
    fn disconnect(&mut self) -> Result<(), AdsTransportError>;
    fn read_state(&mut self) -> Result<AdsDeviceState, AdsTransportError>;
    fn upload_symbol_table(&mut self) -> Result<Vec<SymbolDescriptor>, AdsTransportError>;
    fn resolve_handles(
        &mut self,
        requests: &[AdsHandleRequest],
    ) -> Result<Vec<AdsResolvedHandle>, AdsTransportError>;
    fn sumup_read(
        &mut self,
        handles: &[AdsResolvedHandle],
    ) -> Result<Vec<AdsReadResult>, AdsTransportError>;
    fn sumup_write(
        &mut self,
        writes: &[AdsWriteRequest],
    ) -> Result<Vec<PointQuality>, AdsTransportError>;
    fn subscribe(
        &mut self,
        request: AdsSubscribeRequest,
    ) -> Result<AdsSubscription, AdsTransportError>;
    fn drain_notifications(&mut self) -> Result<Vec<AdsNotificationSample>, AdsTransportError>;
    fn symbol_version(&mut self) -> Result<u32, AdsTransportError>;
}
