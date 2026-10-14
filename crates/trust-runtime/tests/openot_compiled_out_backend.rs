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

#[cfg(not(any(
    feature = "openot-database-sqlite",
    feature = "openot-database-postgresql",
    feature = "openot-database-timescaledb",
    feature = "openot-database-mysql",
    feature = "openot-database-sqlserver",
    feature = "openot-database-influxdb3"
)))]
#[test]
fn composition_without_database_drivers_rejects_every_selected_backend() {
    for backend in [
        OpenOtPersistenceBackend::Sqlite,
        OpenOtPersistenceBackend::PostgreSql,
        OpenOtPersistenceBackend::TimescaleDb,
        OpenOtPersistenceBackend::MySql,
        OpenOtPersistenceBackend::SqlServer,
        OpenOtPersistenceBackend::InfluxDb3,
    ] {
        let config = OpenOtPersistenceConfig {
            enabled: true,
            backend: Some(backend),
            ..OpenOtPersistenceConfig::default()
        };
        let error = trust_runtime::openot_persistence::OpenOtDocumentSink::open(
            &config,
            Path::new("missing-bundle"),
        )
        .expect_err("an omitted driver must reject before trying to connect");
        assert!(matches!(error, PersistenceError::BackendUnavailable(_)));
    }
}
