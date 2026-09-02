#![cfg(all(unix, not(feature = "openot-database-sqlite")))]

use std::path::Path;

use trust_runtime::config::{
    OpenOtPersistenceBackend, OpenOtPersistenceConfig, OpenOtTelemetryConfig,
};
use trust_runtime::openot_persistence::{OpenOtPersistenceService, PersistenceError};

#[test]
fn service_rejects_a_selected_backend_omitted_from_the_binary_synchronously() {
    let config = OpenOtTelemetryConfig {
        enabled: true,
        persistence: OpenOtPersistenceConfig {
            enabled: true,
            backend: Some(OpenOtPersistenceBackend::Sqlite),
            ..OpenOtPersistenceConfig::default()
        },
        ..OpenOtTelemetryConfig::default()
    };

    let error = match OpenOtPersistenceService::start(&config, Path::new("missing-bundle")) {
        Err(error) => error,
        Ok(_) => panic!("compiled-out backend must reject startup before worker spawn"),
    };
    assert!(matches!(error, PersistenceError::BackendUnavailable(_)));
}
