use std::path::PathBuf;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde_json::json;
use smol_str::SmolStr;
use trust_ads_core::{
    AdsDataTypeDescriptor, IecDataType, PointAccess, SymbolDescriptor, SymbolFlag, UpdateMode,
};
use trust_hir::TypeId;
use trust_runtime::ads::{
    AdsBinding, AdsConnectionBridge, AdsConnectionWorker, AdsNotificationMode, AdsPointAddress,
    AdsPointConfig, MockAdsTransport,
};
use trust_runtime::harness::TestHarness;
use trust_runtime::opcua::{
    OpcUaClientAuthConfig, OpcUaClientBinding, OpcUaClientBridge, OpcUaClientBridgeError,
    OpcUaClientConnectionConfig, OpcUaClientEventSink, OpcUaClientPointAccess,
    OpcUaClientPointConfig, OpcUaClientSessionInfo, OpcUaClientTransport, OpcUaClientWorker,
    OpcUaDataType, OpcUaMessageSecurityMode, OpcUaSecurityPolicy, OpcUaSecurityProfile,
};
use trust_runtime::retain::FileRetainStore;
use trust_runtime::value::{Duration, Value};
use trust_runtime::Runtime;

const WARMUP_CYCLES: usize = 100;
const DEBUG_SAMPLES: usize = 1_000;
const RETAIN_SAMPLES: usize = 200;
const PUBLISH_POINTS: usize = 1_000;
const PUBLISH_SAMPLES: usize = 200;
const CYCLE_STEP_MS: i64 = 10;

#[derive(Debug, Clone)]
struct Summary {
    samples: usize,
    min_us: f64,
    p50_us: f64,
    p95_us: f64,
    p99_us: f64,
    max_us: f64,
}

#[test]
#[ignore = "Phase 10 benchmark; run explicitly with --ignored --nocapture"]
fn phase10_debug_snapshot_overhead_baseline() {
    let disabled = measure_cycles(
        build_debug_snapshot_source(64),
        DebugMode::Disabled,
        DEBUG_SAMPLES,
    );
    let enabled = measure_cycles(
        build_debug_snapshot_source(64),
        DebugMode::Enabled,
        DEBUG_SAMPLES,
    );

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "benchmark": "phase10_debug_snapshot_overhead",
            "samples": DEBUG_SAMPLES,
            "warmup_cycles": WARMUP_CYCLES,
            "fixture": {
                "program_variables": 64,
                "cycle_step_ms": CYCLE_STEP_MS,
            },
            "debug_disabled": summary_json(&disabled),
            "debug_enabled": summary_json(&enabled),
            "delta": delta_json(&disabled, &enabled),
        }))
        .expect("serialize debug snapshot benchmark")
    );
}

#[test]
#[ignore = "Phase 10 benchmark; run explicitly with --ignored --nocapture"]
fn phase10_retain_fsync_impact_baseline() {
    let disabled = measure_retain_cycles(RetainMode::Disabled, RETAIN_SAMPLES);
    let periodic = measure_retain_cycles(RetainMode::Periodic, RETAIN_SAMPLES);
    let forced = measure_retain_cycles(RetainMode::ForcedEveryCycle, RETAIN_SAMPLES);

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "benchmark": "phase10_retain_fsync_impact",
            "samples": RETAIN_SAMPLES,
            "warmup_cycles": WARMUP_CYCLES,
            "fixture": {
                "retained_globals": 32,
                "cycle_step_ms": CYCLE_STEP_MS,
                "periodic_save_interval_ms": 100,
                "forced_save_interval_ms": 0,
            },
            "retain_disabled": summary_json(&disabled),
            "retain_periodic": summary_json(&periodic),
            "retain_forced_fsync_every_cycle": summary_json(&forced),
            "periodic_delta": delta_json(&disabled, &periodic),
            "forced_delta": delta_json(&disabled, &forced),
        }))
        .expect("serialize retain benchmark")
    );
}

#[test]
#[ignore = "Phase 10 benchmark; run explicitly with --ignored --nocapture"]
fn phase10_ads_opcua_publish_clone_partial_baseline() {
    let ads = build_ads_bridge(PUBLISH_POINTS);
    let opcua = build_opcua_bridge(PUBLISH_POINTS);
    let point_names = build_point_names(PUBLISH_POINTS);

    let ads_statuses = summarize(&measure(PUBLISH_SAMPLES, || {
        let statuses = ads.statuses();
        std::hint::black_box(statuses);
    }));
    let ads_pending_writes = summarize(&measure(PUBLISH_SAMPLES, || {
        for point_name in &point_names {
            std::hint::black_box(ads.pending_write(point_name));
        }
    }));
    let ads_status_json = summarize(&measure(PUBLISH_SAMPLES, || {
        let statuses = ads.statuses();
        let json = serde_json::to_string(&statuses).expect("serialize ADS statuses");
        std::hint::black_box(json);
    }));

    let opcua_snapshot = summarize(&measure(PUBLISH_SAMPLES, || {
        let snapshot = opcua.snapshot();
        std::hint::black_box(snapshot);
    }));
    let opcua_snapshot_json = summarize(&measure(PUBLISH_SAMPLES, || {
        let snapshot = opcua.snapshot();
        let projected = snapshot
            .point_statuses
            .iter()
            .map(|status| {
                json!({
                    "var": status.var.as_str(),
                    "node_id": status.node_id.as_str(),
                    "data_type": format!("{:?}", status.data_type),
                    "access": format!("{:?}", status.access),
                    "state": format!("{:?}", status.state),
                    "last_seen_ms": status.last_seen_ms,
                    "has_value": status.value.is_some(),
                    "detail": status.detail.as_str(),
                })
            })
            .collect::<Vec<_>>();
        let json = serde_json::to_string(&projected).expect("serialize OPC UA point statuses");
        std::hint::black_box(json);
    }));
    let (mut ads_runtime, mut ads_bridge, mut ads_worker) =
        build_ads_worker_fixture(PUBLISH_POINTS);
    let ads_capture_outputs = summarize(&measure(PUBLISH_SAMPLES, || {
        write_publish_values(&mut ads_runtime, PUBLISH_POINTS);
        ads_bridge
            .capture_outputs(ads_runtime.storage_mut(), 20)
            .expect("capture ADS outputs");
    }));
    let ads_worker_handoff = summarize(&measure(PUBLISH_SAMPLES, || {
        write_publish_values(&mut ads_runtime, PUBLISH_POINTS);
        ads_bridge
            .capture_outputs(ads_runtime.storage_mut(), 21)
            .expect("queue ADS outputs");
        ads_worker.tick(22).expect("publish ADS outputs");
    }));

    let (mut opcua_runtime, mut opcua_bridge, mut opcua_worker) =
        build_opcua_worker_fixture(PUBLISH_POINTS);
    let opcua_capture_outputs = summarize(&measure(PUBLISH_SAMPLES, || {
        write_publish_values(&mut opcua_runtime, PUBLISH_POINTS);
        opcua_bridge
            .capture_outputs(opcua_runtime.storage_mut(), 20)
            .expect("capture OPC UA outputs");
    }));
    let opcua_worker_handoff = summarize(&measure(PUBLISH_SAMPLES, || {
        write_publish_values(&mut opcua_runtime, PUBLISH_POINTS);
        opcua_bridge
            .capture_outputs(opcua_runtime.storage_mut(), 21)
            .expect("queue OPC UA outputs");
        opcua_worker.tick(22).expect("publish OPC UA outputs");
    }));

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "benchmark": "phase10_ads_opcua_publish_clone_partial",
            "samples": PUBLISH_SAMPLES,
            "points": PUBLISH_POINTS,
            "surfaces": {
                "ads_statuses_clone": summary_json(&ads_statuses),
                "ads_pending_write_lookup_clone_all_points": summary_json(&ads_pending_writes),
                "ads_status_json_serialization": summary_json(&ads_status_json),
                "ads_capture_outputs_all_points": summary_json(&ads_capture_outputs),
                "ads_worker_mock_transport_handoff_all_points": summary_json(&ads_worker_handoff),
                "opcua_snapshot_clone": summary_json(&opcua_snapshot),
                "opcua_point_status_json_serialization": summary_json(&opcua_snapshot_json),
                "opcua_capture_outputs_all_points": summary_json(&opcua_capture_outputs),
                "opcua_worker_mock_transport_handoff_all_points": summary_json(&opcua_worker_handoff),
            },
            "gap": "mock transport handoff is measured; live network/device latency is intentionally excluded",
        }))
        .expect("serialize ADS/OPC UA publish clone benchmark")
    );
}

#[derive(Debug, Clone, Copy)]
enum DebugMode {
    Disabled,
    Enabled,
}

#[derive(Debug, Clone, Copy)]
enum RetainMode {
    Disabled,
    Periodic,
    ForcedEveryCycle,
}

fn measure_cycles(source: String, debug: DebugMode, samples: usize) -> Summary {
    let mut harness = TestHarness::from_source(&source).expect("compile benchmark fixture");
    if matches!(debug, DebugMode::Enabled) {
        let _ = harness.runtime_mut().enable_debug();
    }
    warmup(&mut harness);
    summarize(&measure_runtime_cycles(&mut harness, samples))
}

fn measure_retain_cycles(mode: RetainMode, samples: usize) -> Summary {
    let mut harness = TestHarness::from_source(&build_retain_source(32))
        .expect("compile retain benchmark fixture");
    match mode {
        RetainMode::Disabled => {}
        RetainMode::Periodic => {
            let path = unique_temp_path("periodic-retain");
            harness.runtime_mut().set_retain_store(
                Some(Box::new(FileRetainStore::new(path))),
                Some(Duration::from_millis(100)),
            );
        }
        RetainMode::ForcedEveryCycle => {
            let path = unique_temp_path("forced-retain");
            harness.runtime_mut().set_retain_store(
                Some(Box::new(FileRetainStore::new(path))),
                Some(Duration::ZERO),
            );
        }
    }
    warmup(&mut harness);
    summarize(&measure_runtime_cycles(&mut harness, samples))
}

fn warmup(harness: &mut TestHarness) {
    for _ in 0..WARMUP_CYCLES {
        harness
            .runtime_mut()
            .execute_cycle()
            .expect("warmup cycle should succeed");
        harness
            .runtime_mut()
            .advance_time(Duration::from_millis(CYCLE_STEP_MS));
    }
}

fn measure_runtime_cycles(harness: &mut TestHarness, samples: usize) -> Vec<u64> {
    let mut samples_ns = Vec::with_capacity(samples);
    for _ in 0..samples {
        let started = Instant::now();
        harness
            .runtime_mut()
            .execute_cycle()
            .expect("measured cycle should succeed");
        samples_ns.push(duration_ns(started));
        harness
            .runtime_mut()
            .advance_time(Duration::from_millis(CYCLE_STEP_MS));
    }
    samples_ns
}

fn measure(samples: usize, mut f: impl FnMut()) -> Vec<u64> {
    let mut samples_ns = Vec::with_capacity(samples);
    for _ in 0..samples {
        let started = Instant::now();
        f();
        samples_ns.push(duration_ns(started));
    }
    samples_ns
}

fn build_debug_snapshot_source(var_count: usize) -> String {
    let mut source = String::from("PROGRAM Main\nVAR\n");
    for index in 0..var_count {
        source.push_str(&format!("    v{index} : DINT := {index};\n"));
    }
    source.push_str("END_VAR\n");
    for index in 0..var_count {
        source.push_str(&format!("    v{index} := v{index} + 1;\n"));
    }
    source.push_str("END_PROGRAM\n");
    source
}

fn build_retain_source(retained_count: usize) -> String {
    let mut source = String::from("VAR_GLOBAL RETAIN\n");
    for index in 0..retained_count {
        source.push_str(&format!("    r{index} : DINT := {index};\n"));
    }
    source.push_str("END_VAR\n\nPROGRAM Main\n");
    for index in 0..retained_count {
        source.push_str(&format!("    r{index} := r{index} + 1;\n"));
    }
    source.push_str("END_PROGRAM\n");
    source
}

fn build_publish_source(point_count: usize) -> String {
    let mut source = String::from("VAR_GLOBAL\n");
    for index in 0..point_count {
        source.push_str(&format!("    p{index} : REAL := REAL#{index}.0;\n"));
    }
    source.push_str("END_VAR\n\nPROGRAM Main\nEND_PROGRAM\n");
    source
}

fn build_ads_bridge(point_count: usize) -> AdsConnectionBridge {
    let mut harness =
        TestHarness::from_source(&build_publish_source(point_count)).expect("compile ADS fixture");
    let bindings = build_ads_bindings(harness.runtime(), point_count);
    let mut bridge = AdsConnectionBridge::new(bindings).expect("ADS bridge");
    bridge
        .capture_outputs(harness.runtime_mut().storage_mut(), 0)
        .expect("capture ADS outputs");
    bridge
}

fn build_ads_worker_fixture(
    point_count: usize,
) -> (
    Runtime,
    AdsConnectionBridge,
    AdsConnectionWorker<MockAdsTransport>,
) {
    let harness =
        TestHarness::from_source(&build_publish_source(point_count)).expect("compile ADS fixture");
    let bindings = build_ads_bindings(harness.runtime(), point_count);
    let symbols = (0..point_count)
        .map(|index| real_symbol(format!("MAIN.p{index}").as_str()))
        .collect::<Vec<_>>();
    let (bridge, mut worker) =
        AdsConnectionBridge::with_transport(MockAdsTransport::new(symbols), bindings)
            .expect("ADS bridge with worker");
    worker.tick(0).expect("connect ADS worker");
    (harness.into_runtime(), bridge, worker)
}

fn build_ads_bindings(runtime: &Runtime, point_count: usize) -> Vec<AdsBinding> {
    (0..point_count)
        .map(|index| {
            let point_name = format!("p{index}");
            let reference = runtime
                .storage()
                .ref_for_global(point_name.as_str())
                .expect("global ref");
            AdsBinding {
                point: AdsPointConfig {
                    point_name: point_name.clone(),
                    address: AdsPointAddress::Symbol(format!("MAIN.{point_name}")),
                    data_type: AdsDataTypeDescriptor::scalar("REAL", IecDataType::Real),
                    access: PointAccess::Write,
                    mode: UpdateMode::Poll,
                    notification_mode: AdsNotificationMode::OnChange,
                    allow_retain_read: true,
                },
                reference,
                quality_reference: None,
                type_id: TypeId::REAL,
                retain: false,
            }
        })
        .collect()
}

fn build_opcua_bridge(point_count: usize) -> OpcUaClientBridge {
    let harness = TestHarness::from_source(&build_publish_source(point_count))
        .expect("compile OPC UA fixture");
    let bindings = (0..point_count)
        .map(|index| {
            let point_name = SmolStr::new(format!("p{index}"));
            let reference = harness
                .runtime()
                .storage()
                .ref_for_global(point_name.as_str())
                .expect("global ref");
            OpcUaClientBinding {
                point: OpcUaClientPointConfig {
                    var: point_name,
                    node_id: format!("ns=2;i={}", index + 1),
                    data_type: OpcUaDataType::Float,
                    access: OpcUaClientPointAccess::ReadWrite,
                },
                reference,
            }
        })
        .collect::<Vec<_>>();
    let mut bridge = OpcUaClientBridge::new(bindings).expect("OPC UA bridge");
    let mut runtime = harness.into_runtime();
    bridge
        .capture_outputs(runtime.storage_mut(), 0)
        .expect("capture OPC UA outputs");
    bridge
}

fn build_opcua_worker_fixture(
    point_count: usize,
) -> (
    Runtime,
    OpcUaClientBridge,
    OpcUaClientWorker<MockOpcUaClientTransport>,
) {
    let harness = TestHarness::from_source(&build_publish_source(point_count))
        .expect("compile OPC UA fixture");
    let points = (0..point_count)
        .map(|index| OpcUaClientPointConfig {
            var: SmolStr::new(format!("p{index}")),
            node_id: format!("ns=2;i={}", index + 1),
            data_type: OpcUaDataType::Float,
            access: OpcUaClientPointAccess::ReadWrite,
        })
        .collect::<Vec<_>>();
    let bindings = points
        .iter()
        .map(|point| {
            let reference = harness
                .runtime()
                .storage()
                .ref_for_global(point.var.as_str())
                .expect("global ref");
            OpcUaClientBinding {
                point: point.clone(),
                reference,
            }
        })
        .collect::<Vec<_>>();
    let connection = OpcUaClientConnectionConfig {
        name: SmolStr::new("phase10"),
        endpoint_url: "opc.tcp://127.0.0.1:4840/trust".to_string(),
        security: OpcUaSecurityProfile {
            policy: OpcUaSecurityPolicy::None,
            mode: OpcUaMessageSecurityMode::None,
            allow_anonymous: true,
        },
        auth: OpcUaClientAuthConfig::Anonymous,
        trust_server_certificate: true,
        poll_interval_ms: 10,
        timeout_ms: 100,
        points,
    };
    let (bridge, mut worker) = OpcUaClientBridge::with_transport(
        connection,
        MockOpcUaClientTransport::default(),
        bindings,
    )
    .expect("OPC UA bridge with worker");
    worker.tick(0).expect("connect OPC UA worker");
    (harness.into_runtime(), bridge, worker)
}

fn build_point_names(point_count: usize) -> Vec<String> {
    (0..point_count).map(|index| format!("p{index}")).collect()
}

fn write_publish_values(runtime: &mut Runtime, point_count: usize) {
    let generation = runtime.cycle_counter() as f32 + 1.0;
    for index in 0..point_count {
        runtime.storage_mut().set_global(
            SmolStr::new(format!("p{index}")),
            Value::Real(generation + index as f32),
        );
    }
}

fn real_symbol(name: &str) -> SymbolDescriptor {
    SymbolDescriptor::new(
        name,
        AdsDataTypeDescriptor::scalar("REAL", IecDataType::Real),
        0x4020,
        0,
        4,
    )
    .with_flag(SymbolFlag::Read)
    .with_flag(SymbolFlag::Write)
}

#[derive(Default)]
struct MockOpcUaClientTransport {
    write_batches: usize,
    last_write_count: usize,
}

impl OpcUaClientTransport for MockOpcUaClientTransport {
    fn connect(
        &mut self,
        connection: &OpcUaClientConnectionConfig,
        _sink: OpcUaClientEventSink,
    ) -> Result<OpcUaClientSessionInfo, OpcUaClientBridgeError> {
        Ok(OpcUaClientSessionInfo {
            requested_timeout_ms: connection.timeout_ms,
            revised_timeout_ms: Some(connection.timeout_ms),
            recovery_detail: None,
        })
    }

    fn subscribe_read_points(
        &mut self,
        _points: &[OpcUaClientPointConfig],
        _sink: OpcUaClientEventSink,
    ) -> Result<(), OpcUaClientBridgeError> {
        Ok(())
    }

    fn write_values(
        &mut self,
        values: &[(OpcUaClientPointConfig, Value)],
    ) -> Result<(), OpcUaClientBridgeError> {
        let batch = values
            .iter()
            .map(|(point, value)| (point.var.clone(), value.clone()))
            .collect::<Vec<_>>();
        self.write_batches = self.write_batches.saturating_add(1);
        self.last_write_count = batch.len();
        std::hint::black_box(batch);
        Ok(())
    }

    fn disconnect(&mut self) -> Result<(), OpcUaClientBridgeError> {
        Ok(())
    }
}

fn summarize(samples_ns: &[u64]) -> Summary {
    assert!(!samples_ns.is_empty(), "benchmark requires samples");
    let mut sorted = samples_ns.to_vec();
    sorted.sort_unstable();
    Summary {
        samples: sorted.len(),
        min_us: ns_to_us(sorted[0]),
        p50_us: percentile_us(&sorted, 0.50),
        p95_us: percentile_us(&sorted, 0.95),
        p99_us: percentile_us(&sorted, 0.99),
        max_us: ns_to_us(*sorted.last().expect("sample")),
    }
}

fn percentile_us(sorted_ns: &[u64], percentile: f64) -> f64 {
    let index = ((sorted_ns.len() as f64 - 1.0) * percentile).ceil() as usize;
    ns_to_us(sorted_ns[index.min(sorted_ns.len() - 1)])
}

fn ns_to_us(ns: u64) -> f64 {
    ns as f64 / 1_000.0
}

fn duration_ns(started: Instant) -> u64 {
    started.elapsed().as_nanos().try_into().unwrap_or(u64::MAX)
}

fn summary_json(summary: &Summary) -> serde_json::Value {
    json!({
        "samples": summary.samples,
        "min_us": summary.min_us,
        "p50_us": summary.p50_us,
        "p95_us": summary.p95_us,
        "p99_us": summary.p99_us,
        "max_us": summary.max_us,
    })
}

fn delta_json(baseline: &Summary, measured: &Summary) -> serde_json::Value {
    json!({
        "p50_us": measured.p50_us - baseline.p50_us,
        "p95_us": measured.p95_us - baseline.p95_us,
        "p99_us": measured.p99_us - baseline.p99_us,
        "max_us": measured.max_us - baseline.max_us,
        "p50_ratio": ratio(measured.p50_us, baseline.p50_us),
        "p95_ratio": ratio(measured.p95_us, baseline.p95_us),
        "p99_ratio": ratio(measured.p99_us, baseline.p99_us),
    })
}

fn ratio(measured: f64, baseline: f64) -> f64 {
    if baseline <= f64::EPSILON {
        0.0
    } else {
        measured / baseline
    }
}

fn unique_temp_path(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "trust-runtime-phase10-{prefix}-{}-{nanos}.bin",
        std::process::id()
    ))
}
