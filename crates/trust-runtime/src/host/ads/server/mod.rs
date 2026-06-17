//! Beckhoff ADS server runtime integration contracts.

pub mod contracts;

#[cfg(feature = "ads-server")]
pub mod audit;
#[cfg(feature = "ads-server")]
pub mod doctor;
#[cfg(feature = "ads-server")]
pub mod lifecycle;
#[cfg(feature = "ads-server")]
pub mod policy;
#[cfg(feature = "ads-server")]
pub mod publisher;
#[cfg(feature = "ads-server")]
pub mod symbols;
#[cfg(feature = "ads-server")]
pub mod write_port;

pub use contracts::{
    AdsServerClientConfig, AdsServerRuntimeConfig, AdsServerSourcePin, DEFAULT_ADS_SERVER_ADS_PORT,
    DEFAULT_ADS_SERVER_IDLE_TIMEOUT_MS, DEFAULT_ADS_SERVER_MAX_CLIENTS,
    DEFAULT_ADS_SERVER_MAX_FRAME_BYTES, DEFAULT_ADS_SERVER_MAX_STRING_BYTES,
    DEFAULT_ADS_SERVER_MAX_SUBSCRIPTIONS_PER_CLIENT, DEFAULT_ADS_SERVER_MAX_SUMUP_ITEMS,
    DEFAULT_ADS_SERVER_MAX_SYMBOLS, DEFAULT_ADS_SERVER_MAX_TOTAL_SUBSCRIPTIONS,
    DEFAULT_ADS_SERVER_MAX_WRITE_BYTES, DEFAULT_ADS_SERVER_MIN_NOTIFICATION_CYCLE_MS,
    DEFAULT_ADS_SERVER_READ_TIMEOUT_MS,
};

#[cfg(feature = "ads-server")]
pub use audit::AdsServerRuntimeAuditSink;
#[cfg(feature = "ads-server")]
pub use doctor::{
    build_ads_server_status_report, run_ads_server_doctor, AdsServerDoctorInput,
    AdsServerExternalClientEvidence,
};
#[cfg(feature = "ads-server")]
pub use lifecycle::{start_ads_server_runtime, AdsServerRuntime};
#[cfg(feature = "ads-server")]
pub use policy::{AdsServerClientPolicy, AdsServerRefusedClient};
#[cfg(feature = "ads-server")]
pub use publisher::AdsServerValuePublisher;
#[cfg(feature = "ads-server")]
pub use symbols::{
    build_runtime_symbol_snapshot, descriptor_for_value, global_name_for_server_symbol,
    server_symbol_name, AdsServerSymbolSource,
};
#[cfg(feature = "ads-server")]
pub use write_port::AdsServerRuntimeWritePort;

#[cfg(all(test, feature = "ads-server"))]
mod tests;
