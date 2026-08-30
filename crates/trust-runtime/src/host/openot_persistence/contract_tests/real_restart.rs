use super::*;
#[cfg(feature = "openot-real-database-tests")]
#[derive(Clone, Copy)]
pub(super) enum RealRestartProduct {
    PostgreSql,
    TimescaleDb,
    MySql,
    MariaDb,
    SqlServer,
    InfluxDb3,
}

#[cfg(feature = "openot-real-database-tests")]
impl RealRestartProduct {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::PostgreSql => "postgresql",
            Self::TimescaleDb => "timescaledb",
            Self::MySql => "mysql",
            Self::MariaDb => "mariadb",
            Self::SqlServer => "sqlserver",
            Self::InfluxDb3 => "influxdb3",
        }
    }

    fn container_env(self) -> &'static str {
        match self {
            Self::PostgreSql => "TRUST_TEST_OPENOT_POSTGRES_CONTAINER",
            Self::TimescaleDb => "TRUST_TEST_OPENOT_TIMESCALE_CONTAINER",
            Self::MySql => "TRUST_TEST_OPENOT_MYSQL_CONTAINER",
            Self::MariaDb => "TRUST_TEST_OPENOT_MARIADB_CONTAINER",
            Self::SqlServer => "TRUST_TEST_OPENOT_SQLSERVER_CONTAINER",
            Self::InfluxDb3 => "TRUST_TEST_OPENOT_INFLUX_CONTAINER",
        }
    }

    pub(super) fn config(
        self,
        root: &std::path::Path,
        stamp: u64,
    ) -> crate::config::OpenOtPersistenceConfig {
        use crate::config::{
            OpenOtInfluxDb3PersistenceConfig, OpenOtMySqlPersistenceConfig,
            OpenOtPersistenceBackend, OpenOtPersistenceConfig, OpenOtPersistenceTlsMode,
            OpenOtPostgreSqlPersistenceConfig, OpenOtSqlServerPersistenceConfig,
            OpenOtTimescaleDbPersistenceConfig,
        };

        let mut config = OpenOtPersistenceConfig {
            enabled: true,
            ..OpenOtPersistenceConfig::default()
        };
        let schema = format!("openot_restart_{stamp}");
        match self {
            Self::PostgreSql => {
                config.backend = Some(OpenOtPersistenceBackend::PostgreSql);
                config.postgresql = Some(OpenOtPostgreSqlPersistenceConfig {
                    connection_url_env: "TRUST_TEST_OPENOT_POSTGRES_URL".into(),
                    schema: schema.into(),
                    tls: OpenOtPersistenceTlsMode::Require,
                    ca_cert_path: Some(
                        std::env::var("TRUST_TEST_OPENOT_POSTGRES_CA")
                            .expect("PostgreSQL CA")
                            .into(),
                    ),
                });
            }
            Self::TimescaleDb => {
                config.backend = Some(OpenOtPersistenceBackend::TimescaleDb);
                config.timescaledb = Some(OpenOtTimescaleDbPersistenceConfig {
                    connection_url_env: "TRUST_TEST_OPENOT_TIMESCALE_URL".into(),
                    schema: schema.into(),
                    tls: OpenOtPersistenceTlsMode::Require,
                    ca_cert_path: Some(
                        std::env::var("TRUST_TEST_OPENOT_TIMESCALE_CA")
                            .expect("TimescaleDB CA")
                            .into(),
                    ),
                });
            }
            Self::MySql | Self::MariaDb => {
                let (url_env, ca_env) = match self {
                    Self::MySql => ("TRUST_TEST_OPENOT_MYSQL_URL", "TRUST_TEST_OPENOT_MYSQL_CA"),
                    Self::MariaDb => (
                        "TRUST_TEST_OPENOT_MARIADB_URL",
                        "TRUST_TEST_OPENOT_MARIADB_CA",
                    ),
                    _ => unreachable!(),
                };
                config.backend = Some(OpenOtPersistenceBackend::MySql);
                config.mysql = Some(OpenOtMySqlPersistenceConfig {
                    connection_url_env: url_env.into(),
                    database: "openot".into(),
                    tls: OpenOtPersistenceTlsMode::Require,
                    ca_cert_path: Some(std::env::var(ca_env).expect("MySQL-family CA").into()),
                });
            }
            Self::SqlServer => {
                config.backend = Some(OpenOtPersistenceBackend::SqlServer);
                config.sqlserver = Some(OpenOtSqlServerPersistenceConfig {
                    connection_url_env: "TRUST_TEST_OPENOT_SQLSERVER_URL".into(),
                    schema: schema.into(),
                    tls: OpenOtPersistenceTlsMode::Require,
                    ca_cert_path: Some(
                        std::env::var("TRUST_TEST_OPENOT_SQLSERVER_CA")
                            .expect("SQL Server CA")
                            .into(),
                    ),
                });
            }
            Self::InfluxDb3 => {
                config.backend = Some(OpenOtPersistenceBackend::InfluxDb3);
                config.influxdb3 = Some(OpenOtInfluxDb3PersistenceConfig {
                    host_env: "TRUST_TEST_OPENOT_INFLUX_HOST".into(),
                    token_env: "TRUST_TEST_OPENOT_INFLUX_TOKEN".into(),
                    database: "openot".into(),
                    spool_path: root.join("influx-spool.sqlite3"),
                    max_bytes: 1_073_741_824,
                    ca_cert_path: Some(
                        std::env::var("TRUST_TEST_OPENOT_INFLUX_CA")
                            .expect("InfluxDB CA")
                            .into(),
                    ),
                });
            }
        }
        config
    }
}

#[cfg(feature = "openot-real-database-tests")]
pub(super) fn canonical_documents_for_run(run_id: u64) -> Vec<Document> {
    let mut documents = canonical_documents();
    for document in &mut documents {
        match document {
            Document::Event(document) => document.provenance.run_id = run_id,
            Document::Loss(document) => document.provenance.run_id = run_id,
            Document::Placeholder(document) => document.provenance.run_id = run_id,
        }
    }
    documents
}

#[cfg(feature = "openot-real-database-tests")]
fn assert_real_product_restart_recovery(product: RealRestartProduct) {
    struct RestartGuard(String);
    impl Drop for RestartGuard {
        fn drop(&mut self) {
            let _ = std::process::Command::new("docker")
                .args(["start", self.0.as_str()])
                .status();
        }
    }

    let container = std::env::var(product.container_env()).unwrap_or_else(|_| {
        panic!(
            "{} must identify the real container",
            product.container_env()
        )
    });
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos() as u64;
    let root = std::env::temp_dir().join(format!(
        "trust-openot-{}-restart-{}-{stamp}",
        product.label(),
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("restart test root");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))
            .expect("secure restart test root");
    }
    let config = product.config(&root, stamp);
    let mut sink = OpenOtDocumentSink::open_with_definitions(
        &config,
        &root,
        &[open_ot_definition::sample_definition()],
    )
    .unwrap_or_else(|error| panic!("open {} before restart: {error:?}", product.label()));
    let baseline = PersistenceBatch {
        documents: canonical_documents_for_run(stamp),
        checkpoint: PersistenceCheckpoint {
            buffer_id: 7,
            run_id: stamp,
            cursor_abs: 4096,
        },
    };
    sink.commit(&baseline)
        .unwrap_or_else(|error| panic!("commit {} baseline: {error:?}", product.label()));
    let stopped = std::process::Command::new("docker")
        .args(["stop", container.as_str()])
        .status()
        .expect("stop real database product");
    assert!(stopped.success(), "stop {}", product.label());
    let _restart_guard = RestartGuard(container.clone());
    let recovery = PersistenceBatch {
        documents: canonical_documents_for_run(stamp.saturating_add(1)),
        checkpoint: PersistenceCheckpoint {
            buffer_id: 7,
            run_id: stamp.saturating_add(1),
            cursor_abs: 8192,
        },
    };

    if matches!(product, RealRestartProduct::InfluxDb3) {
        let outcome = sink
            .commit(&recovery)
            .expect("InfluxDB outage must accept into its durable spool");
        assert_eq!(outcome.inserted, CANONICAL_DOCUMENT_COUNT);
        assert!(outcome.remote_pending >= CANONICAL_DOCUMENT_COUNT);
    } else {
        assert!(
            sink.commit(&recovery).is_err(),
            "{} outage must not be acknowledged as a remote commit",
            product.label()
        );
    }

    let started = std::process::Command::new("docker")
        .args(["start", container.as_str()])
        .status()
        .expect("restart real database product");
    assert!(started.success(), "restart {}", product.label());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(90);
    loop {
        let result = if matches!(product, RealRestartProduct::InfluxDb3) {
            sink.maintenance().and_then(|pending| {
                if pending == 0 {
                    Ok(())
                } else {
                    Err(super::PersistenceError::Commit(format!(
                        "InfluxDB restart still has {pending} pending documents"
                    )))
                }
            })
        } else {
            OpenOtDocumentSink::open_with_definitions(
                &config,
                &root,
                &[open_ot_definition::sample_definition()],
            )
            .and_then(|mut reopened| reopened.commit(&recovery).map(|_| ()))
        };
        if result.is_ok() {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "{} did not recover before deadline: {result:?}",
            product.label()
        );
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    let checkpoint = if matches!(product, RealRestartProduct::InfluxDb3) {
        sink.load_checkpoint(7, recovery.checkpoint.run_id)
    } else {
        OpenOtDocumentSink::open_with_definitions(
            &config,
            &root,
            &[open_ot_definition::sample_definition()],
        )
        .and_then(|mut verifier| verifier.load_checkpoint(7, recovery.checkpoint.run_id))
    }
    .expect("load recovery checkpoint");
    assert_eq!(checkpoint, Some(recovery.checkpoint));
    std::fs::remove_dir_all(root).ok();
}

#[cfg(feature = "openot-real-database-tests")]
#[test]
fn postgresql_real_server_restart_recovers_without_backend_fallback() {
    assert_real_product_restart_recovery(RealRestartProduct::PostgreSql);
}

#[cfg(feature = "openot-real-database-tests")]
#[test]
fn timescaledb_real_server_restart_recovers_without_plain_postgresql_fallback() {
    assert_real_product_restart_recovery(RealRestartProduct::TimescaleDb);
}

#[cfg(feature = "openot-real-database-tests")]
#[test]
fn mysql_real_server_restart_recovers_without_backend_fallback() {
    assert_real_product_restart_recovery(RealRestartProduct::MySql);
}

#[cfg(feature = "openot-real-database-tests")]
#[test]
fn mariadb_real_server_restart_recovers_on_the_shared_mysql_adapter() {
    assert_real_product_restart_recovery(RealRestartProduct::MariaDb);
}

#[cfg(feature = "openot-real-database-tests")]
#[test]
fn sqlserver_real_server_restart_recovers_without_backend_fallback() {
    assert_real_product_restart_recovery(RealRestartProduct::SqlServer);
}

#[cfg(feature = "openot-real-database-tests")]
#[test]
fn influxdb3_real_server_restart_drains_the_required_durable_spool() {
    assert_real_product_restart_recovery(RealRestartProduct::InfluxDb3);
}

#[cfg(feature = "openot-real-database-tests")]
#[test]
fn every_real_network_backend_migrates_v1_and_rejects_newer_schema() {
    fn downgrade(sink: &mut OpenOtDocumentSink) {
        match sink {
            OpenOtDocumentSink::PostgreSql(sink) => sink
                .downgrade_checkpoint_to_v1_for_test()
                .expect("downgrade PostgreSQL fixture"),
            OpenOtDocumentSink::TimescaleDb(sink) => sink
                .downgrade_checkpoint_to_v1_for_test()
                .expect("downgrade TimescaleDB fixture"),
            OpenOtDocumentSink::MySql(sink) => sink
                .downgrade_checkpoint_to_v1_for_test()
                .expect("downgrade MySQL-family fixture"),
            OpenOtDocumentSink::SqlServer(sink) => sink
                .downgrade_checkpoint_to_v1_for_test()
                .expect("downgrade SQL Server fixture"),
            OpenOtDocumentSink::InfluxDb3(sink) => sink
                .downgrade_checkpoint_to_v1_for_test()
                .expect("downgrade InfluxDB spool fixture"),
            OpenOtDocumentSink::Sqlite(_) => unreachable!("network matrix excludes SQLite"),
        }
    }

    fn set_version(sink: &mut OpenOtDocumentSink, version: u32) {
        match sink {
            OpenOtDocumentSink::PostgreSql(sink) => sink
                .set_schema_version_for_test(version)
                .expect("set PostgreSQL schema version"),
            OpenOtDocumentSink::TimescaleDb(sink) => sink
                .set_schema_version_for_test(version)
                .expect("set TimescaleDB schema version"),
            OpenOtDocumentSink::MySql(sink) => sink
                .set_schema_version_for_test(version)
                .expect("set MySQL-family schema version"),
            OpenOtDocumentSink::SqlServer(sink) => sink
                .set_schema_version_for_test(version)
                .expect("set SQL Server schema version"),
            OpenOtDocumentSink::InfluxDb3(sink) => sink
                .set_schema_version_for_test(version)
                .expect("set InfluxDB spool schema version"),
            OpenOtDocumentSink::Sqlite(_) => unreachable!("network matrix excludes SQLite"),
        }
    }

    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos() as u64;
    for (index, product) in [
        RealRestartProduct::PostgreSql,
        RealRestartProduct::TimescaleDb,
        RealRestartProduct::MySql,
        RealRestartProduct::MariaDb,
        RealRestartProduct::SqlServer,
        RealRestartProduct::InfluxDb3,
    ]
    .into_iter()
    .enumerate()
    {
        let root = std::env::temp_dir().join(format!(
            "trust-openot-{}-migration-{}-{stamp}",
            product.label(),
            std::process::id()
        ));
        std::fs::create_dir_all(&root).expect("migration root");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))
                .expect("secure migration root");
        }
        let config = product.config(&root, stamp.saturating_add(index as u64));
        let mut v2 = OpenOtDocumentSink::open_with_definitions(
            &config,
            &root,
            &[open_ot_definition::sample_definition()],
        )
        .unwrap_or_else(|error| panic!("open {} v2 fixture: {error:?}", product.label()));
        downgrade(&mut v2);
        drop(v2);

        let mut migrated = OpenOtDocumentSink::open_with_definitions(
            &config,
            &root,
            &[open_ot_definition::sample_definition()],
        )
        .unwrap_or_else(|error| panic!("migrate {} v1 fixture: {error:?}", product.label()));
        let run_id = stamp.saturating_add(10_000 + index as u64);
        let batch = PersistenceBatch {
            documents: canonical_documents_for_run(run_id),
            checkpoint: PersistenceCheckpoint {
                buffer_id: 7,
                run_id,
                cursor_abs: 24_576,
            },
        };
        migrated
            .commit(&batch)
            .unwrap_or_else(|error| panic!("commit migrated {}: {error:?}", product.label()));
        assert_eq!(
            migrated
                .load_checkpoint(7, run_id)
                .unwrap_or_else(|error| panic!("load migrated {}: {error:?}", product.label())),
            Some(batch.checkpoint)
        );

        set_version(&mut migrated, 4);
        let error = OpenOtDocumentSink::open_with_definitions(
            &config,
            &root,
            &[open_ot_definition::sample_definition()],
        )
        .expect_err("newer schema must fail closed");
        assert!(
            format!("{error:?}").contains("newer"),
            "{} newer-schema error was not actionable: {error:?}",
            product.label()
        );
        set_version(&mut migrated, 3);
        drop(migrated);
        std::fs::remove_dir_all(root).ok();
    }
}
