use super::*;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_temp_dir(prefix: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "trust-runtime-bench-{prefix}-{}-{nanos}",
        std::process::id()
    ))
}

#[test]
fn summarize_ns_computes_quantiles() {
    let summary = summarize_ns(&[1_000, 2_000, 3_000, 4_000, 5_000]);
    assert_eq!(summary.samples, 5);
    assert!((summary.min_us - 1.0).abs() < f64::EPSILON);
    assert!((summary.p50_us - 3.0).abs() < f64::EPSILON);
    assert!((summary.p95_us - 5.0).abs() < f64::EPSILON);
    assert!((summary.max_us - 5.0).abs() < f64::EPSILON);
}

#[test]
fn histogram_includes_overflow_bucket() {
    let histogram = histogram_from_ns(&[1_000, 2_000, 30_000_000]);
    assert_eq!(histogram.len(), HISTOGRAM_LIMITS_US.len() + 1);
    assert_eq!(histogram[0].count, 2);
    assert_eq!(histogram[histogram.len() - 1].count, 1);
}

fn write_project_bench_fixture(project: &std::path::Path) {
    let sources = project.join("src");
    fs::create_dir_all(&sources).expect("create src");
    fs::write(
        project.join("runtime.toml"),
        r#"[bundle]
version = 1

[resource]
name = "BenchRes"
cycle_interval_ms = 10

[runtime]
execution_backend = "vm"

[runtime.control]
endpoint = "unix:///tmp/trust-runtime.sock"
mode = "production"
debug_enabled = false

[runtime.web]
enabled = false
listen = "127.0.0.1:8080"
auth = "local"
tls = false

[runtime.tls]
mode = "disabled"
require_remote = false

[runtime.discovery]
enabled = false
service_name = "truST"
advertise = false
interfaces = []

[runtime.mesh]
enabled = false
listen = "0.0.0.0:5200"
tls = false
auth_token = ""
publish = []

[runtime.opcua]
enabled = false
listen = "0.0.0.0:4840"
endpoint_path = "/"
namespace_uri = "urn:trust:runtime"
publish_interval_ms = 250
max_nodes = 128
expose = []
security_policy = "basic256sha256"
security_mode = "sign_and_encrypt"
allow_anonymous = false

[runtime.observability]
enabled = false
sample_interval_ms = 1000
mode = "all"
include = []
history_path = "history/historian.jsonl"
max_entries = 20000
prometheus_enabled = true
prometheus_path = "/metrics"

[runtime.log]
level = "info"

[runtime.retain]
mode = "none"
save_interval_ms = 1000

[runtime.watchdog]
enabled = false
timeout_ms = 1000
action = "halt"

[runtime.fault]
policy = "halt"
"#,
    )
    .expect("write runtime.toml");
    fs::write(
        project.join("io.toml"),
        r#"[io]
driver = "simulated"
params = {}

[[io.safe_state]]
address = "%QX0.0"
value = "FALSE"
"#,
    )
    .expect("write io.toml");
    fs::write(
        project.join("trust-lsp.toml"),
        r#"[project]
include_paths = ["src"]
stdlib = "iec"
"#,
    )
    .expect("write trust-lsp.toml");
    fs::write(
        sources.join("main.st"),
        r#"VAR_GLOBAL
    g_cycles : UDINT;
END_VAR

PROGRAM Main
g_cycles := g_cycles + UDINT#1;
END_PROGRAM
"#,
    )
    .expect("write source");
}

fn write_project_bench_sizeof_fixture(project: &std::path::Path) {
    write_project_bench_fixture(project);
    fs::write(
        project.join("src/main.st"),
        r#"VAR_GLOBAL
    g_size : DINT;
END_VAR

PROGRAM Main
g_size := SIZEOF(INT);
END_PROGRAM
"#,
    )
    .expect("write source");
}

#[test]
fn init_bench_json_output_contains_startup_latency_fields() {
    let project =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/init_bench");

    let (report, format) = execute_bench(BenchAction::Init {
        project: project.clone(),
        samples: 2,
        warmup_cycles: 0,
        output: BenchOutputFormat::Json,
    })
    .expect("run init benchmark");
    let rendered = render_bench_output(&report, format).expect("render json");
    let value: serde_json::Value = serde_json::from_str(&rendered).expect("parse bench json");
    assert_eq!(
        value.get("benchmark").and_then(serde_json::Value::as_str),
        Some("init")
    );
    assert_eq!(
        value
            .pointer("/report/init_only_latency/samples")
            .and_then(serde_json::Value::as_u64),
        Some(2)
    );
    assert!(value
        .pointer("/report/init_plus_first_cycle_latency/p95_us")
        .and_then(serde_json::Value::as_f64)
        .is_some());
    assert!(value
        .pointer("/report/first_cycle_latency/p99_us")
        .and_then(serde_json::Value::as_f64)
        .is_some());
    assert!(value
        .pointer("/report/steady_cycle_latency/p50_us")
        .and_then(serde_json::Value::as_f64)
        .is_some());
    assert!(value
        .pointer("/report/first_mutation_latency/p50_us")
        .and_then(serde_json::Value::as_f64)
        .is_some());
    assert!(value
        .pointer("/report/retain_restart_latency/p50_us")
        .and_then(serde_json::Value::as_f64)
        .is_some());
    assert!(value
        .pointer("/report/struct_value_new_latency/p50_us")
        .and_then(serde_json::Value::as_f64)
        .is_some());
    assert!(value
        .pointer("/report/struct_value_untyped_latency/p50_us")
        .and_then(serde_json::Value::as_f64)
        .is_some());
}

#[test]
fn project_bench_json_output_contains_budget_and_watched_globals() {
    let project = unique_temp_dir("project-bench");
    write_project_bench_fixture(&project);

    let (report, format) = execute_bench(BenchAction::Project {
        project: project.clone(),
        samples: 4,
        warmup_cycles: 1,
        watch: vec!["g_cycles".into(), "g_missing".into()],
        tier1: false,
        output: BenchOutputFormat::Json,
    })
    .expect("run project benchmark");
    let rendered = render_bench_output(&report, format).expect("render json");
    let value: serde_json::Value = serde_json::from_str(&rendered).expect("parse bench json");
    assert_eq!(
        value.get("benchmark").and_then(serde_json::Value::as_str),
        Some("project")
    );
    assert_eq!(
        value
            .pointer("/report/resource_name")
            .and_then(serde_json::Value::as_str),
        Some("BenchRes")
    );
    assert_eq!(
        value
            .pointer("/report/execution_backend")
            .and_then(serde_json::Value::as_str),
        Some("vm")
    );
    assert_eq!(
        value
            .pointer("/report/watched_globals/g_cycles")
            .and_then(serde_json::Value::as_u64),
        Some(5)
    );
    assert!(value.pointer("/report/watched_globals/g_missing").is_some());
    assert!(value
        .pointer("/report/cycle_budget_us")
        .and_then(serde_json::Value::as_f64)
        .is_some());
    assert!(value
        .pointer("/report/budget_overruns")
        .and_then(serde_json::Value::as_u64)
        .is_some());
    assert!(value
        .pointer("/report/vm_profile/register_programs_executed")
        .and_then(serde_json::Value::as_u64)
        .is_some());
    assert!(value
        .pointer("/report/vm_profile/hot_blocks")
        .and_then(serde_json::Value::as_array)
        .is_some());
    assert_eq!(
        value
            .pointer("/report/vm_profile/hot_blocks/0/pou_name")
            .and_then(serde_json::Value::as_str),
        Some("Main")
    );
    assert!(value
        .pointer("/report/vm_profile/register_lowering_cache/hit_ratio")
        .and_then(serde_json::Value::as_f64)
        .is_some());
    assert!(value
        .pointer("/report/vm_profile/ref_ops/load_ref")
        .and_then(serde_json::Value::as_u64)
        .is_some());
    assert!(value
        .pointer("/report/vm_profile/call_ops/frame_pushes")
        .and_then(serde_json::Value::as_u64)
        .is_some());
    assert!(value
        .pointer("/report/vm_profile/value_ops/read_value_clones")
        .and_then(serde_json::Value::as_u64)
        .is_some());
    assert!(value
        .pointer("/report/vm_profile/tier1_specialized_executor")
        .is_none());

    let _ = fs::remove_dir_all(project);
}

#[test]
fn project_bench_table_output_contains_tier1_executor_stats_when_enabled() {
    let project = unique_temp_dir("project-bench-table");
    write_project_bench_fixture(&project);

    let (report, format) = execute_bench(BenchAction::Project {
        project: project.clone(),
        samples: 4,
        warmup_cycles: 1,
        watch: vec!["g_cycles".into()],
        tier1: true,
        output: BenchOutputFormat::Table,
    })
    .expect("run project benchmark");
    let rendered = render_bench_output(&report, format).expect("render table");
    assert!(
        rendered.contains("tier1-specialized-executor:"),
        "{rendered}"
    );

    let _ = fs::remove_dir_all(project);
}

#[test]
fn project_bench_json_output_omits_tier1_executor_stats_by_default() {
    let project = unique_temp_dir("project-bench-no-tier1");
    write_project_bench_fixture(&project);

    let (report, format) = execute_bench(BenchAction::Project {
        project: project.clone(),
        samples: 4,
        warmup_cycles: 1,
        watch: vec!["g_cycles".into()],
        tier1: false,
        output: BenchOutputFormat::Json,
    })
    .expect("run project benchmark");
    let rendered = render_bench_output(&report, format).expect("render json");
    let value: serde_json::Value = serde_json::from_str(&rendered).expect("parse bench json");
    assert!(value
        .pointer("/report/vm_profile/tier1_specialized_executor")
        .is_none());

    let _ = fs::remove_dir_all(project);
}

#[test]
fn project_bench_output_contains_tier1_compile_failure_reasons() {
    let project = unique_temp_dir("project-bench-tier1-compile-failure");
    write_project_bench_sizeof_fixture(&project);

    let (report, format) = execute_bench(BenchAction::Project {
        project: project.clone(),
        samples: 80,
        warmup_cycles: 1,
        watch: vec!["g_size".into()],
        tier1: true,
        output: BenchOutputFormat::Json,
    })
    .expect("run project benchmark");
    let rendered = render_bench_output(&report.clone(), format).expect("render json");
    let value: serde_json::Value = serde_json::from_str(&rendered).expect("parse bench json");
    assert_eq!(
        value
            .pointer(
                "/report/vm_profile/tier1_specialized_executor/compile_failure_reasons/0/reason"
            )
            .and_then(serde_json::Value::as_str),
        Some("unsupported_instr:size_of_type")
    );

    let rendered_table =
        render_bench_output(&report, BenchOutputFormat::Table).expect("render table");
    assert!(
        rendered_table.contains("compile-failure-reasons:"),
        "{rendered_table}"
    );
    assert!(
        rendered_table.contains("unsupported_instr:size_of_type"),
        "{rendered_table}"
    );

    let _ = fs::remove_dir_all(project);
}

#[test]
fn t0_shm_bench_json_output_contains_latency_and_overrun_fields() {
    let (report, format) = execute_bench(BenchAction::T0Shm {
        samples: 16,
        payload_bytes: 16,
        output: BenchOutputFormat::Json,
    })
    .expect("run t0 benchmark");
    let rendered = render_bench_output(&report, format).expect("render json");
    let value: serde_json::Value = serde_json::from_str(&rendered).expect("parse bench json");
    assert_eq!(
        value.get("benchmark").and_then(serde_json::Value::as_str),
        Some("t0-shm")
    );
    assert!(value
        .pointer("/report/round_trip_latency/p95_us")
        .and_then(serde_json::Value::as_f64)
        .is_some());
    assert!(value
        .pointer("/report/overruns")
        .and_then(serde_json::Value::as_u64)
        .is_some());
}

#[test]
fn mesh_zenoh_bench_json_output_contains_loss_and_reorder_fields() {
    let (report, format) = execute_bench(BenchAction::MeshZenoh {
        samples: 20,
        payload_bytes: 24,
        loss_rate: 0.1,
        reorder_rate: 0.2,
        output: BenchOutputFormat::Json,
    })
    .expect("run mesh benchmark");
    let rendered = render_bench_output(&report, format).expect("render json");
    let value: serde_json::Value = serde_json::from_str(&rendered).expect("parse bench json");
    assert_eq!(
        value.get("benchmark").and_then(serde_json::Value::as_str),
        Some("mesh-zenoh")
    );
    assert!(value
        .pointer("/report/loss_count")
        .and_then(serde_json::Value::as_u64)
        .is_some());
    assert!(value
        .pointer("/report/reorder_count")
        .and_then(serde_json::Value::as_u64)
        .is_some());
}

#[test]
fn dispatch_bench_table_output_contains_fanout_and_audit_metrics() {
    let (report, format) = execute_bench(BenchAction::Dispatch {
        samples: 12,
        payload_bytes: 8,
        fanout: 3,
        output: BenchOutputFormat::Table,
    })
    .expect("run dispatch benchmark");
    let rendered = render_bench_output(&report, format).expect("render table");
    assert!(rendered.contains("fanout=3"));
    assert!(rendered.contains("audit-correlation latency"));
}

#[test]
fn project_workload_rejects_missing_project_folder() {
    let missing = unique_temp_dir("missing-project");
    let err = ProjectBenchWorkload::normalize(missing, 10, 0, Vec::new(), false)
        .expect_err("missing project should fail");
    assert!(err.to_string().contains("--project"));
}

#[test]
fn project_workload_rejects_zero_samples() {
    let project = unique_temp_dir("zero-samples");
    fs::create_dir_all(&project).expect("create project dir");
    let err = ProjectBenchWorkload::normalize(project.clone(), 0, 0, Vec::new(), false)
        .expect_err("zero samples should fail");
    assert!(err.to_string().contains("--samples"));
    let _ = fs::remove_dir_all(project);
}

#[test]
fn mesh_workload_rejects_out_of_range_rates() {
    let err = MeshBenchWorkload::normalize(10, 32, -0.1, 0.0).expect_err("invalid rate");
    assert!(err.to_string().contains("--loss-rate"));

    let err = MeshBenchWorkload::normalize(10, 32, 0.0, 1.1).expect_err("invalid rate");
    assert!(err.to_string().contains("--reorder-rate"));
}
