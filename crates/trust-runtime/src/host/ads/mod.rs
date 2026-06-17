//! Beckhoff ADS client integration contracts.

#![allow(missing_docs)]

use crate::error::RuntimeError;

pub mod client;
pub mod contracts;
mod descriptors;
pub mod diagnostics;
pub mod generate;
pub mod mock;
pub mod onboarding;
pub mod server;
pub mod transport;
pub mod validate;

#[cfg(feature = "ads-wire")]
pub mod backend_ads_rs;

#[cfg(test)]
mod tests;

#[cfg(feature = "ads-wire")]
pub use backend_ads_rs::{AdsRsTimeouts, AdsRsTransport};
pub use client::{
    resolve_declared_bindings, AdsBinding, AdsBridgeError, AdsConnectionBridge, AdsConnectionState,
    AdsConnectionWorker, AdsWorkerThread,
};
pub use contracts::{
    parse_ads_toml, validate_ads_config_local_identity, AdsClientConfig, AdsConnectionConfig,
    AdsPointConfig,
};
pub use generate::{generate_ads_interface, AdsGeneratedInterface, AdsInterfaceError};
pub use mock::MockAdsTransport;
pub use transport::{
    AdsDeviceState, AdsHandleRequest, AdsNotificationMode, AdsNotificationSample, AdsPointAddress,
    AdsReadResult, AdsResolvedHandle, AdsSubscribeRequest, AdsSubscription, AdsTransport,
    AdsTransportError, AdsWriteRequest,
};
pub use validate::{
    validate_ads_interface_offline, AdsOfflineValidationReport, AdsValidationError,
};
