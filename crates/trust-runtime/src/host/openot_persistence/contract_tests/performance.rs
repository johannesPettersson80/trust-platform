use super::*;
#[cfg(feature = "openot-real-database-tests")]
fn benchmark_real_sink(
    name: &str,
    sink: &mut OpenOtDocumentSink,
    first_run_id: u64,
) -> (f64, f64, std::time::Duration) {
    const CATCH_UP_BATCHES: u64 = 32;
    const SUSTAINED_BATCHES: u64 = 32;
    const SUSTAINED_INTERVAL: std::time::Duration = std::time::Duration::from_millis(124);
    let storage_before = sink
        .storage_bytes()
        .unwrap_or_else(|error| panic!("{name} storage before benchmark: {error:?}"));
    let cpu_before = process_cpu_ticks();
    let rss_before_kib = process_rss_kib();
    let mut canonical_payload_bytes = 0u64;
    let mut maintenance_elapsed = std::time::Duration::ZERO;
    let mut latencies = Vec::new();
    let catch_up_started = std::time::Instant::now();
    for offset in 0..CATCH_UP_BATCHES {
        let run_id = first_run_id.saturating_add(offset);
        let batch = PersistenceBatch {
            documents: canonical_documents_for_run(run_id),
            checkpoint: PersistenceCheckpoint {
                buffer_id: 7,
                run_id,
                cursor_abs: run_id.saturating_mul(4096),
            },
        };
        canonical_payload_bytes = canonical_payload_bytes.saturating_add(
            batch
                .documents
                .iter()
                .map(|document| {
                    open_ot_document::to_json(document)
                        .expect("benchmark canonical JSON")
                        .len() as u64
                })
                .sum::<u64>(),
        );
        let started = std::time::Instant::now();
        sink.commit(&batch)
            .unwrap_or_else(|error| panic!("{name} catch-up commit: {error:?}"));
        latencies.push(started.elapsed());
    }
    let maintenance_started = std::time::Instant::now();
    while sink
        .maintenance()
        .unwrap_or_else(|error| panic!("{name} catch-up maintenance: {error:?}"))
        > 0
    {}
    maintenance_elapsed += maintenance_started.elapsed();
    let catch_up_elapsed = catch_up_started.elapsed();
    let catch_up_rate = (CATCH_UP_BATCHES as f64 * CANONICAL_DOCUMENT_COUNT as f64)
        / catch_up_elapsed.as_secs_f64();
    latencies.sort_unstable();
    let p95_index = (latencies.len() * 95).div_ceil(100).saturating_sub(1);
    let p95 = latencies[p95_index];

    let sustained_started = std::time::Instant::now();
    for offset in 0..SUSTAINED_BATCHES {
        let run_id = first_run_id
            .saturating_add(CATCH_UP_BATCHES)
            .saturating_add(offset);
        let batch = PersistenceBatch {
            documents: canonical_documents_for_run(run_id),
            checkpoint: PersistenceCheckpoint {
                buffer_id: 7,
                run_id,
                cursor_abs: run_id.saturating_mul(4096),
            },
        };
        canonical_payload_bytes = canonical_payload_bytes.saturating_add(
            batch
                .documents
                .iter()
                .map(|document| {
                    open_ot_document::to_json(document)
                        .expect("benchmark canonical JSON")
                        .len() as u64
                })
                .sum::<u64>(),
        );
        sink.commit(&batch)
            .unwrap_or_else(|error| panic!("{name} sustained commit: {error:?}"));
        let target = sustained_started
            + SUSTAINED_INTERVAL.saturating_mul(u32::try_from(offset + 1).expect("batch count"));
        if let Some(remaining) = target.checked_duration_since(std::time::Instant::now()) {
            std::thread::sleep(remaining);
        }
    }
    let maintenance_started = std::time::Instant::now();
    while sink
        .maintenance()
        .unwrap_or_else(|error| panic!("{name} sustained maintenance: {error:?}"))
        > 0
    {}
    maintenance_elapsed += maintenance_started.elapsed();
    let sustained_elapsed = sustained_started.elapsed();
    let sustained_rate = (SUSTAINED_BATCHES as f64 * CANONICAL_DOCUMENT_COUNT as f64)
        / sustained_elapsed.as_secs_f64();
    let storage_after = sink
        .storage_bytes()
        .unwrap_or_else(|error| panic!("{name} storage after benchmark: {error:?}"));
    let storage_growth = storage_after.saturating_sub(storage_before);
    let write_amplification = storage_growth as f64 / canonical_payload_bytes.max(1) as f64;
    let cpu_ticks = process_cpu_ticks().saturating_sub(cpu_before);
    let rss_after_kib = process_rss_kib();
    eprintln!(
        "OpenOT benchmark {name}: sustained={sustained_rate:.1} docs/s catch_up={catch_up_rate:.1} docs/s p95_commit_ms={:.1} canonical_bytes={canonical_payload_bytes} storage_before={storage_before} storage_after={storage_after} storage_growth={storage_growth} write_amplification={write_amplification:.3} cpu_ticks={cpu_ticks} rss_before_kib={rss_before_kib} rss_after_kib={rss_after_kib} maintenance_ms={:.1}",
        p95.as_secs_f64() * 1000.0,
        maintenance_elapsed.as_secs_f64() * 1000.0,
    );
    (sustained_rate, catch_up_rate, p95)
}

#[cfg(all(feature = "openot-real-database-tests", target_os = "linux"))]
fn process_cpu_ticks() -> u64 {
    let stat = std::fs::read_to_string("/proc/self/stat").expect("read process CPU stat");
    let fields = stat
        .rsplit_once(") ")
        .expect("process stat command delimiter")
        .1
        .split_whitespace()
        .collect::<Vec<_>>();
    fields[11]
        .parse::<u64>()
        .expect("process user CPU ticks")
        .saturating_add(fields[12].parse::<u64>().expect("process system CPU ticks"))
}

#[cfg(all(feature = "openot-real-database-tests", not(target_os = "linux")))]
fn process_cpu_ticks() -> u64 {
    0
}

#[cfg(all(feature = "openot-real-database-tests", target_os = "linux"))]
fn process_rss_kib() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .expect("read process status")
        .lines()
        .find_map(|line| {
            line.strip_prefix("VmRSS:")
                .and_then(|value| value.split_whitespace().next())
                .and_then(|value| value.parse().ok())
        })
        .expect("process resident memory")
}

#[cfg(all(feature = "openot-real-database-tests", not(target_os = "linux")))]
fn process_rss_kib() -> u64 {
    0
}

#[cfg(feature = "openot-real-database-tests")]
#[cfg_attr(
    debug_assertions,
    ignore = "OpenOT throughput floors are release-profile qualification"
)]
#[test]
fn every_real_backend_meets_openot_ingest_and_catch_up_qualification_floors() {
    use crate::config::{
        OpenOtPersistenceBackend, OpenOtPersistenceConfig, OpenOtSqlitePersistenceConfig,
    };

    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos() as u64;
    let root = std::env::temp_dir().join(format!(
        "trust-openot-real-benchmark-{}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("benchmark root");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))
            .expect("secure benchmark root");
    }
    let sqlite_config = OpenOtPersistenceConfig {
        enabled: true,
        backend: Some(OpenOtPersistenceBackend::Sqlite),
        sqlite: Some(OpenOtSqlitePersistenceConfig {
            path: root.join("sqlite/openot.sqlite3"),
        }),
        ..OpenOtPersistenceConfig::default()
    };
    let mut cases = Vec::new();
    cases.push(("SQLite", sqlite_config));
    for product in [
        RealRestartProduct::PostgreSql,
        RealRestartProduct::TimescaleDb,
        RealRestartProduct::MySql,
        RealRestartProduct::MariaDb,
        RealRestartProduct::SqlServer,
        RealRestartProduct::InfluxDb3,
    ] {
        let product_root = root.join(product.label());
        std::fs::create_dir_all(&product_root).expect("product benchmark root");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&product_root, std::fs::Permissions::from_mode(0o700))
                .expect("secure product benchmark root");
        }
        cases.push((product.label(), product.config(&product_root, stamp)));
    }

    for (index, (name, config)) in cases.into_iter().enumerate() {
        let mut sink = OpenOtDocumentSink::open_with_definitions(
            &config,
            &root,
            &[open_ot_definition::sample_definition()],
        )
        .unwrap_or_else(|error| panic!("open {name} benchmark sink: {error:?}"));
        let (sustained, catch_up, p95) = benchmark_real_sink(
            name,
            &mut sink,
            stamp.saturating_add((index as u64).saturating_mul(10_000)),
        );
        assert!(sustained >= 100.0, "{name} sustained {sustained:.1} docs/s");
        assert!(catch_up >= 250.0, "{name} catch-up {catch_up:.1} docs/s");
        assert!(
            p95 <= std::time::Duration::from_millis(500),
            "{name} p95 commit {p95:?}"
        );
    }
    std::fs::remove_dir_all(root).ok();
}
