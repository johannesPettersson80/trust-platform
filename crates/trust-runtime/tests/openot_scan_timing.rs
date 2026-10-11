#![cfg(unix)]

// The shared source corpus also contains fixtures used by the functional suites.
#[allow(dead_code)]
mod openot_support;

use openot_support::build_st_runtime;
use std::path::PathBuf;
use trust_runtime::config::{
    OpenOtPersistenceBackend, OpenOtPersistenceConfig, OpenOtSqlitePersistenceConfig,
    OpenOtTelemetryConfig, OpenOtTelemetryFenceMode, OpenOtTelemetrySource,
};
use trust_runtime::harness::SourceFile;
use trust_runtime::openot_authoring;
use trust_runtime::openot_persistence::{OpenOtPersistenceService, OpenOtPersistenceState};
use trust_runtime::Runtime;

#[test]
fn openot_persistence_service_does_not_materially_regress_plc_scan_timing() {
    const PROGRAM: &str = r#"
PROGRAM Main
VAR
    Trigger : BOOL {attribute 'oot' := 'message', 'template' := 'scan timing event'};
END_VAR
Trigger := NOT Trigger;
END_PROGRAM
"#;

    fn runtime_with_ring(path: PathBuf) -> Runtime {
        let mut runtime = build_st_runtime(PROGRAM);
        runtime
            .configure_openot_telemetry(
                &OpenOtTelemetryConfig {
                    enabled: true,
                    path,
                    capacity: 1_048_576,
                    fence_mode: OpenOtTelemetryFenceMode::Fenced,
                    allow_unfenced_for_proof: false,
                    source: OpenOtTelemetrySource::StFb,
                    producer_instance: Some("Main.OotProducer".into()),
                    producer_instances: vec!["Main.OotProducer".into()],
                    persistence: Default::default(),
                },
                None,
            )
            .expect("configure scan timing telemetry");
        runtime
    }

    // Atomic directory creation prevents another test/process from sharing state.
    let root = (0u64..)
        .find_map(|attempt| {
            let path = std::env::temp_dir().join(format!(
                "trust-runtime-openot-persistence-scan-timing-{}-{attempt}.shm",
                std::process::id()
            ));
            match std::fs::create_dir(&path) {
                Ok(()) => Some(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => None,
                Err(error) => panic!("create scan timing root: {error}"),
            }
        })
        .expect("unique scan timing root");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))
            .expect("secure scan timing root");
    }
    let disabled_ring = root.join("disabled.shm");
    let enabled_ring = root.join("enabled.shm");
    let mut disabled_runtime = runtime_with_ring(disabled_ring);
    let mut enabled_runtime = runtime_with_ring(enabled_ring.clone());
    let definition = openot_authoring::definition_json_from_sources(&[SourceFile::with_path(
        "Main.st", PROGRAM,
    )])
    .expect("scan timing definition");
    std::fs::write(
        root.join("openot-definition.json"),
        serde_json::to_vec_pretty(&definition).expect("serialize scan timing definition"),
    )
    .expect("write scan timing definition");
    let config = OpenOtTelemetryConfig {
        enabled: true,
        path: enabled_ring,
        persistence: OpenOtPersistenceConfig {
            enabled: true,
            backend: Some(OpenOtPersistenceBackend::Sqlite),
            batch_size: 256,
            flush_interval_ms: 10,
            sqlite: Some(OpenOtSqlitePersistenceConfig {
                path: root.join("history/trust-logging.sqlite3"),
            }),
            ..OpenOtPersistenceConfig::default()
        },
        ..OpenOtTelemetryConfig::default()
    };
    let mut service = OpenOtPersistenceService::start(&config, &root)
        .expect("start scan timing persistence")
        .expect("enabled scan timing persistence");
    for _ in 0..4 {
        disabled_runtime
            .execute_cycle()
            .expect("warm disabled scan");
        enabled_runtime.execute_cycle().expect("warm enabled scan");
    }

    let mut disabled_samples = [0u128; 3];
    let mut enabled_samples = [0u128; 3];
    for window in 0_usize..3 {
        for pair in 0_usize..10 {
            // Alternating order keeps warm-up/load drift out of the comparison.
            let samples = if (window * 10 + pair).is_multiple_of(2) {
                [
                    (&mut disabled_runtime, &mut disabled_samples[window]),
                    (&mut enabled_runtime, &mut enabled_samples[window]),
                ]
            } else {
                [
                    (&mut enabled_runtime, &mut enabled_samples[window]),
                    (&mut disabled_runtime, &mut disabled_samples[window]),
                ]
            };
            for (runtime, sample) in samples {
                let started = std::time::Instant::now();
                runtime.execute_cycle().expect("scan timing cycle");
                *sample += started.elapsed().as_nanos();
            }
        }
        disabled_samples[window] /= 10;
        enabled_samples[window] /= 10;
    }
    eprintln!("OpenOT paired windows (wall ns/scan): disabled={disabled_samples:?} enabled={enabled_samples:?}");
    disabled_samples.sort_unstable();
    enabled_samples.sort_unstable();
    let disabled_ns = disabled_samples[1];
    let enabled_ns = enabled_samples[1];
    eprintln!(
        "OpenOT scan timing: disabled={disabled_ns} ns/scan enabled={enabled_ns} ns/scan ratio={:.3}",
        enabled_ns as f64 / disabled_ns.max(1) as f64
    );
    assert!(
        enabled_ns <= disabled_ns.saturating_mul(3) / 2 + 1_000_000,
        "persistence worker materially regressed scan timing: disabled={disabled_ns}ns enabled={enabled_ns}ns"
    );
    service.shutdown();
    let status = service.status();
    assert_eq!(status.state, OpenOtPersistenceState::Stopped);
    assert!(
        status.last_error.is_none(),
        "worker fault: {:?}",
        status.last_error
    );
    assert!(
        status.documents_committed > 0,
        "real worker committed no documents"
    );
    assert_eq!(
        status.cursor_abs, status.head_abs,
        "worker did not drain carriage"
    );
    std::fs::remove_dir_all(root).ok();
}
