use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use trust_runtime::hmi::{build_trust_twin_scene, load_hmi_scene_view, HmiSceneViewPayload};
use trust_runtime::memory::InstanceId;
use trust_runtime::value::Value;
use trust_runtime::{harness::CompileSession, harness::SourceFile, Runtime};
use trust_twin_compiler::{
    compile_topology_to_view, verify_compiled_view_fresh, ComponentLibrary, TopologyCompileOptions,
};

const SAMPLE_CYCLES: &[usize] = &[1, 12, 22, 32, 42, 52, 62];
const VIEW_REF: &str = "robot-cell.view.toml";

#[test]
fn robot_cell_plc_trace_drives_scene_state_deterministically() -> anyhow::Result<()> {
    let first = run_robot_cell_trace()?;
    let second = run_robot_cell_trace()?;

    assert_eq!(
        first.stable_trace_hash, second.stable_trace_hash,
        "robot-cell runtime/HMI trace must be stable across repeated runs"
    );
    assert_eq!(
        first.samples, second.samples,
        "robot-cell stable samples must match across repeated runs"
    );
    assert_robot_motion_is_visible_in_scene_state(&first)?;
    write_robot_cell_gate_artifact(&first, &second)?;

    Ok(())
}

fn run_robot_cell_trace() -> anyhow::Result<RobotCellRun> {
    let root = robot_cell_root();
    let source_path = root.join("src/main.st");
    let source = fs::read_to_string(&source_path)?;
    let robot_fb_path = root.join("src/Robot_P3MinimalArm.fb.st");
    let robot_fb = fs::read_to_string(&robot_fb_path)?;
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path(
            "examples/trust-twin/robot-cell/src/Robot_P3MinimalArm.fb.st",
            robot_fb,
        ),
        SourceFile::with_path("examples/trust-twin/robot-cell/src/main.st", source),
    ])
    .build_runtime()?;
    let view = load_hmi_scene_view(&root, VIEW_REF)?;

    let mut samples = Vec::new();
    for cycle in 1..=*SAMPLE_CYCLES.iter().max().expect("sample cycles") {
        runtime.execute_cycle()?;
        if SAMPLE_CYCLES.contains(&cycle) {
            samples.push(sample_robot_cell_scene(&runtime, &view, cycle)?);
        }
    }

    let stable_trace_hash = stable_hash(&samples)?;
    Ok(RobotCellRun {
        samples,
        stable_trace_hash,
    })
}

fn sample_robot_cell_scene(
    runtime: &Runtime,
    view: &HmiSceneViewPayload,
    cycle: usize,
) -> anyhow::Result<RobotCellSample> {
    let values = robot_cell_values(runtime)?;
    let mut scene = build_trust_twin_scene(view)?;
    let report = scene.apply_bindings(&view.bindings3d, &values)?;
    assert!(
        report.missing_sources.is_empty(),
        "all robot-cell bind3d sources must be present, missing={:?}",
        report.missing_sources
    );
    assert!(
        report.errors.is_empty(),
        "robot-cell bind3d errors must be empty, errors={:?}",
        report.errors
    );

    let step = int_value(values.get("Main.RobotMotionStep").expect("step value"))?;
    let shoulder_angle = real_value(
        values
            .get("Main.RobotShoulderAngle")
            .expect("shoulder value"),
    )?;
    let elbow_angle = real_value(values.get("Main.RobotElbowAngle").expect("elbow value"))?;
    let wrist_angle = real_value(values.get("Main.RobotWristAngle").expect("wrist value"))?;
    let gripper_open = bool_value(values.get("Main.RobotGripperOpen").expect("gripper value"))?;
    let gripper_x = real_value(values.get("Main.RobotGripperX").expect("gripper x"))?;
    let gripper_y = real_value(values.get("Main.RobotGripperY").expect("gripper y"))?;
    let gripper_z = real_value(values.get("Main.RobotGripperZ").expect("gripper z"))?;
    let box_x = real_value(values.get("Main.RobotBoxX").expect("box x"))?;
    let box_y = real_value(values.get("Main.RobotBoxY").expect("box y"))?;
    let box_z = real_value(values.get("Main.RobotBoxZ").expect("box z"))?;

    let shoulder = scene
        .node_state("ROBOT-1.shoulder")
        .expect("shoulder state");
    let elbow = scene.node_state("ROBOT-1.elbow").expect("elbow state");
    let wrist = scene.node_state("ROBOT-1.wrist").expect("wrist state");
    let gripper = scene.node_state("GRIPPER-1").expect("gripper state");
    let left_jaw = scene
        .node_state("GRIPPER-1.left_jaw")
        .expect("left jaw state");
    let right_jaw = scene
        .node_state("GRIPPER-1.right_jaw")
        .expect("right jaw state");
    let workpiece = scene.node_state("BOX-1").expect("workpiece state");
    let status_light = scene.node_state("LIGHT-1").expect("status light state");

    assert_close(shoulder.rotation[2], shoulder_angle, "shoulder rotation");
    assert_close(elbow.rotation[2], elbow_angle, "elbow rotation");
    assert_close(wrist.rotation[2], wrist_angle, "wrist rotation");
    assert_close(gripper.position[0], gripper_x, "gripper x");
    assert_close(gripper.position[1], gripper_y, "gripper y");
    assert_close(gripper.position[2], gripper_z, "gripper z");
    assert_close(workpiece.position[0], box_x, "box x");
    assert_close(workpiece.position[1], box_y, "box y");
    assert_close(workpiece.position[2], box_z, "box z");
    assert!(
        matches!(
            status_light.material.emissive.as_str(),
            "#22c55e" | "#f59e0b"
        ),
        "RobotStatusLight must drive the status light emissive color, got {}",
        status_light.material.emissive
    );

    Ok(RobotCellSample {
        cycle,
        step,
        shoulder_angle: rounded(shoulder_angle),
        elbow_angle: rounded(elbow_angle),
        wrist_angle: rounded(wrist_angle),
        gripper_open,
        gripper_position: [rounded(gripper_x), rounded(gripper_y), rounded(gripper_z)],
        left_jaw_z: rounded(left_jaw.position[2] as f64),
        right_jaw_z: rounded(right_jaw.position[2] as f64),
        box_position: [rounded(box_x), rounded(box_y), rounded(box_z)],
        status_emissive: status_light.material.emissive.clone(),
        applied_binding_count: report.applied.len(),
    })
}

fn robot_cell_values(runtime: &Runtime) -> anyhow::Result<BTreeMap<String, Value>> {
    let main_id = main_instance_id(runtime)?;
    let mut values = BTreeMap::new();
    for variable in [
        "RobotMotionStep",
        "RobotShoulderAngle",
        "RobotElbowAngle",
        "RobotWristAngle",
        "RobotGripperOpen",
        "RobotGripperX",
        "RobotGripperY",
        "RobotGripperZ",
        "RobotBoxX",
        "RobotBoxY",
        "RobotBoxZ",
        "RobotEnabled",
        "RobotStatusLight",
    ] {
        let value = runtime
            .storage()
            .get_instance_var(main_id, variable)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("missing Main.{variable}"))?;
        values.insert(format!("Main.{variable}"), value);
    }
    Ok(values)
}

fn main_instance_id(runtime: &Runtime) -> anyhow::Result<InstanceId> {
    match runtime.storage().get_global("Main") {
        Some(Value::Instance(id)) => Ok(*id),
        other => anyhow::bail!("expected Main program instance, got {other:?}"),
    }
}

fn assert_robot_motion_is_visible_in_scene_state(run: &RobotCellRun) -> anyhow::Result<()> {
    let shoulder_range = value_range(run.samples.iter().map(|sample| sample.shoulder_angle));
    let elbow_range = value_range(run.samples.iter().map(|sample| sample.elbow_angle));
    let wrist_range = value_range(run.samples.iter().map(|sample| sample.wrist_angle));
    let box_x_range = value_range(run.samples.iter().map(|sample| sample.box_position[0]));
    assert!(
        shoulder_range > 1.0,
        "shoulder must move through a visible range, got {shoulder_range}"
    );
    assert!(
        elbow_range > 0.25,
        "elbow must move through a visible range, got {elbow_range}"
    );
    assert!(
        wrist_range > 0.6,
        "wrist must move through a visibly reviewable range, got {wrist_range}"
    );
    assert!(
        box_x_range > 3.5,
        "workpiece must move from pickup toward drop zone, got range {box_x_range}"
    );
    assert!(
        run.samples.iter().any(|sample| sample.gripper_open)
            && run.samples.iter().any(|sample| !sample.gripper_open),
        "gripper must open and close during the sampled pick-and-place trace"
    );
    assert!(
        run.samples
            .iter()
            .any(|sample| sample.status_emissive == "#22c55e")
            && run
                .samples
                .iter()
                .any(|sample| sample.status_emissive == "#f59e0b"),
        "status light must visibly change from a real PLC status signal"
    );
    assert!(
        run.samples
            .iter()
            .all(|sample| sample.applied_binding_count >= 12),
        "all robot-cell scene bindings must apply in every sample"
    );
    assert_surface_aligned_pick_and_drop(run)?;
    Ok(())
}

fn assert_surface_aligned_pick_and_drop(run: &RobotCellRun) -> anyhow::Result<()> {
    let pickup = run
        .samples
        .iter()
        .find(|sample| sample.step == 1)
        .ok_or_else(|| anyhow::anyhow!("missing approach-pickup sample"))?;
    let drop = run
        .samples
        .iter()
        .find(|sample| sample.step == 6)
        .ok_or_else(|| anyhow::anyhow!("missing open-gripper drop sample"))?;
    assert_close_f64(pickup.box_position[0], 0.0, "pickup box x");
    assert_close_f64(
        pickup.box_position[1],
        0.35,
        "pickup box y on pickup surface",
    );
    assert_close_f64(pickup.box_position[2], 0.0, "pickup box z");
    assert_close_f64(drop.box_position[0], 4.0, "drop box x");
    assert_close_f64(drop.box_position[1], 0.35, "drop box y on drop surface");
    assert_close_f64(drop.box_position[2], 0.0, "drop box z");
    Ok(())
}

fn write_robot_cell_gate_artifact(
    first: &RobotCellRun,
    second: &RobotCellRun,
) -> anyhow::Result<()> {
    let artifact_dir = workspace_root().join("target/gate-artifacts");
    fs::create_dir_all(&artifact_dir)?;
    let root = robot_cell_root();
    let topology_source = fs::read_to_string(root.join("hmi/views/robot-cell.topology.toml"))?;
    let view_source = fs::read_to_string(root.join("hmi/views/robot-cell.view.toml"))?;
    let freshness = verify_compiled_view_fresh(&topology_source, &view_source)?;
    let compiled = compile_topology_to_view(
        &topology_source,
        &ComponentLibrary::load_builtin()?,
        &TopologyCompileOptions::default(),
    )?;
    let compiled_view = normalize_line_endings(&compiled.view_toml);
    let checked_out_view = normalize_line_endings(&view_source);
    assert_eq!(
        compiled_view, checked_out_view,
        "robot-cell view must be compiler-generated from topology"
    );
    let blockers = vec![
        "playwright_motion_capture_pending".to_string(),
        "assistant_visual_review_pending".to_string(),
        "runtime_disconnect_stale_visual_pending".to_string(),
    ];
    let artifact = json!({
        "scene_path": "examples/trust-twin/robot-cell/hmi/views/robot-cell.view.toml",
        "topology_path": "examples/trust-twin/robot-cell/hmi/views/robot-cell.topology.toml",
        "topology_hash": compiled.topology_hash,
        "compiled_view_hash": compiled.view_hash,
        "compiler_command": "cargo run -p trust-twin-compiler -- --input examples/trust-twin/robot-cell/hmi/views/robot-cell.topology.toml --output examples/trust-twin/robot-cell/hmi/views/robot-cell.view.toml",
        "drift_doctor_result": {
            "matches": freshness.matches,
            "compiled_hash": freshness.compiled_hash,
            "topology_hash": freshness.topology_hash
        },
        "physical_scene_doctor_result": compiled.doctor_results,
        "renderer_origin": serde_json::Value::Null,
        "runtime_signal_names": [
            "Main.RobotShoulderAngle",
            "Main.RobotElbowAngle",
            "Main.RobotWristAngle",
            "Main.RobotGripperOpen",
            "Main.RobotGripperX",
            "Main.RobotGripperY",
            "Main.RobotGripperZ",
            "Main.RobotBoxX",
            "Main.RobotBoxY",
            "Main.RobotBoxZ",
            "Main.RobotEnabled",
            "Main.RobotStatusLight"
        ],
        "joint_ids": [
            "ROBOT-1",
            "ROBOT-1.shoulder",
            "ROBOT-1.elbow",
            "ROBOT-1.wrist",
            "GRIPPER-1",
            "GRIPPER-1.left_jaw",
            "GRIPPER-1.right_jaw"
        ],
        "frame_hashes_before_after": serde_json::Value::Null,
        "pixel_difference_count": serde_json::Value::Null,
        "screenshot_video_path": serde_json::Value::Null,
        "playwright": {
            "command": serde_json::Value::Null,
            "result": "pending"
        },
        "assistant_visual_verdict": "pending",
        "asset_state": {
            "state": "procedural_robot",
            "source": "repo-authored procedural primitives",
            "license": "workspace license",
            "package_path": "examples/trust-twin/robot-cell/hmi/views/robot-cell.view.toml",
            "version": 1
        },
        "fps_latency": {
            "fps": serde_json::Value::Null,
            "latency_ms": serde_json::Value::Null
        },
        "disconnected_state_result": "pending",
        "determinism_trace_hash": first.stable_trace_hash,
        "stable_trace_match": first.stable_trace_hash == second.stable_trace_hash,
        "trace_samples": first.samples,
        "evidence_blockers": blockers,
    });
    let artifact_path = artifact_dir.join("trust-twin-robot-cell-motion.json");
    fs::write(&artifact_path, serde_json::to_string_pretty(&artifact)?)?;
    assert!(artifact_path.is_file(), "robot-cell motion artifact exists");
    Ok(())
}

fn real_value(value: &Value) -> anyhow::Result<f64> {
    match value {
        Value::Real(value) => Ok(f64::from(*value)),
        Value::LReal(value) => Ok(*value),
        other => anyhow::bail!("expected REAL/LREAL, got {other:?}"),
    }
}

fn int_value(value: &Value) -> anyhow::Result<i32> {
    match value {
        Value::Int(value) => Ok(i32::from(*value)),
        Value::DInt(value) => Ok(*value),
        other => anyhow::bail!("expected INT/DINT, got {other:?}"),
    }
}

fn bool_value(value: &Value) -> anyhow::Result<bool> {
    match value {
        Value::Bool(value) => Ok(*value),
        other => anyhow::bail!("expected BOOL, got {other:?}"),
    }
}

fn assert_close(actual: f32, expected: f64, context: &str) {
    let actual = f64::from(actual);
    assert_close_f64(actual, expected, context);
}

fn assert_close_f64(actual: f64, expected: f64, context: &str) {
    assert!(
        (actual - expected).abs() < 0.000_01,
        "{context}: expected {expected}, got {actual}"
    );
}

fn value_range(values: impl Iterator<Item = f64>) -> f64 {
    let mut min = f64::INFINITY;
    let mut max = f64::NEG_INFINITY;
    for value in values {
        min = min.min(value);
        max = max.max(value);
    }
    max - min
}

fn stable_hash(samples: &[RobotCellSample]) -> anyhow::Result<String> {
    let bytes = serde_json::to_vec(samples)?;
    let digest = Sha256::digest(bytes);
    Ok(digest.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn normalize_line_endings(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

fn rounded(value: f64) -> f64 {
    (value * 1_000_000.0).round() / 1_000_000.0
}

fn robot_cell_root() -> PathBuf {
    workspace_root().join("examples/trust-twin/robot-cell")
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root")
}

#[derive(Debug, Clone, PartialEq, Serialize)]
struct RobotCellRun {
    samples: Vec<RobotCellSample>,
    stable_trace_hash: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
struct RobotCellSample {
    cycle: usize,
    step: i32,
    shoulder_angle: f64,
    elbow_angle: f64,
    wrist_angle: f64,
    gripper_open: bool,
    gripper_position: [f64; 3],
    left_jaw_z: f64,
    right_jaw_z: f64,
    box_position: [f64; 3],
    status_emissive: String,
    applied_binding_count: usize,
}
