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
pub struct AdsTransportError {
    message: String,
}

impl AdsTransportError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn message(&self) -> &str {
        self.message.as_str()
    }
}

impl fmt::Display for AdsTransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message.as_str())
    }
}

impl std::error::Error for AdsTransportError {}

/// A failed index-group read: the target answered with an ADS error, or no usable reply came back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdsIndexReadError {
    /// The target answered with this ADS error code and the code's name.
    Ads { code: u32, description: String },
    /// The connection failed, closed, or gave no usable reply.
    Transport(String),
}

impl fmt::Display for AdsIndexReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ads { code, description } => write!(f, "ADS error {code:#x}: {description}"),
            Self::Transport(message) => f.write_str(message),
        }
    }
}

/// A read-only index-group read over a connected ADS transport. It has no write, so a caller
/// that holds only this seam cannot change the target (specification 11, O20).
pub trait AdsIndexRead {
    /// Reads from `index_offset` of `index_group` into `buffer` and returns the bytes the target
    /// sent.
    fn read_index(
        &mut self,
        index_group: u32,
        index_offset: u32,
        buffer: &mut [u8],
    ) -> Result<usize, AdsIndexReadError>;
}

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
