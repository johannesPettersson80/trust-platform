use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use trust_twin_compiler::{
    compile_topology_to_view, verify_compiled_view_fresh, ComponentLibrary, TopologyCompileOptions,
};

const TOPOLOGY: &str = r#"
[[components]]
id = "TK-101"
kind = "tank"
at = { grid = "A1" }

[[components]]
id = "P-101"
kind = "pump"
at = { grid = "A3" }

[[components]]
id = "V-101"
kind = "valve"
at = { grid = "A5" }

[[connections]]
id = "line-101"
from = "TK-101.outlet"
to = "P-101.inlet"
medium = "water"
diameter = "DN50"
route = "auto"

[[bindings]]
component = "TK-101"
signal = "level"
source = "Program.TK101.level"
access = "read"
"#;

#[test]
fn builtin_library_loads_from_packaged_data_and_workspace_layout() {
    let packaged = ComponentLibrary::load_builtin().expect("packaged library loads");
    let workspace =
        ComponentLibrary::load_from_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("library/v1"))
            .expect("workspace library loads");

    assert_eq!(packaged, workspace);
    for kind in ["tank", "pump", "valve", "motor", "vfd", "transmitter"] {
        assert!(
            packaged.kind(kind).is_some(),
            "built-in component library should include {kind}"
        );
    }

    let tank = packaged.kind("tank").expect("tank definition");
    assert_eq!(tank.default_mesh_asset, "trust-twin/components/tank.gltf");
    assert!(tank.port("outlet").is_some());
    assert!(tank.signal("level").is_some());
}

#[test]
fn builtin_library_declares_robot_cell_kinds_and_contact_metadata() {
    let library = ComponentLibrary::load_builtin().expect("library");

    for kind in [
        "robot_arm",
        "gripper",
        "workpiece",
        "pickup_zone",
        "drop_zone",
        "safety_light",
    ] {
        assert!(
            library.kind(kind).is_some(),
            "built-in component library should include robot-cell kind {kind}"
        );
    }

    let pickup = library.kind("pickup_zone").expect("pickup zone");
    assert!(
        pickup.surface("top").is_some(),
        "pickup zone exposes top surface"
    );

    let workpiece = library.kind("workpiece").expect("workpiece");
    assert!(
        workpiece.surface("bottom").is_some(),
        "workpiece exposes bottom contact surface"
    );
    assert!(
        workpiece.surface("grip_left").is_some(),
        "workpiece exposes grip surface"
    );

    let robot = library.kind("robot_arm").expect("robot arm");
    assert!(
        robot.mount("tool").is_some(),
        "robot arm exposes tool mount"
    );
    assert!(robot.signal("shoulder_angle").is_some());
    assert!(robot.signal("elbow_angle").is_some());
    assert!(robot.signal("wrist_angle").is_some());

    let gripper = library.kind("gripper").expect("gripper");
    assert!(gripper.mount("wrist").is_some());
    assert!(gripper.signal("open_left").is_some());
    assert!(gripper.signal("open_right").is_some());
}

#[test]
fn compiler_emits_deterministic_view_with_hash_header_and_bind3d() {
    let library = ComponentLibrary::load_builtin().expect("library");
    let compiled = compile_topology_to_view(TOPOLOGY, &library, &TopologyCompileOptions::default())
        .expect("compile topology");

    assert_eq!(compiled.diagnostics, []);
    assert_eq!(compiled.topology_hash.len(), 64);
    assert_eq!(compiled.view_hash.len(), 64);
    assert!(compiled.doctor_results.iter().all(|result| result.passed));
    assert_eq!(compiled.stats.component_count, 3);
    assert_eq!(compiled.stats.connection_count, 1);
    assert_eq!(compiled.stats.binding_count, 1);
    assert_eq!(compiled.stats.generated_node_count, 5);

    let expected = format!(
        "# trust-twin-topology-hash:v1:sha256:{}\n{}",
        compiled.topology_hash, EXPECTED_VIEW_BODY
    );
    assert_eq!(compiled.view_toml, expected);

    let reparsed: toml::Value = toml::from_str(&compiled.view_toml).expect("view TOML parses");
    let nodes = reparsed
        .get("node")
        .and_then(toml::Value::as_array)
        .expect("compiled view nodes");
    assert!(nodes
        .iter()
        .any(|node| node.get("id").and_then(toml::Value::as_str) == Some("TK-101.level")));
    assert!(nodes
        .iter()
        .any(|node| node.get("id").and_then(toml::Value::as_str) == Some("line-101.pipe")));

    let bindings = reparsed
        .get("bind3d")
        .and_then(toml::Value::as_array)
        .expect("compiled bind3d blocks");
    let binding = bindings.first().expect("one binding");
    assert_eq!(
        binding.get("node").and_then(toml::Value::as_str),
        Some("TK-101.level")
    );
    assert_eq!(
        binding.get("property").and_then(toml::Value::as_str),
        Some("transform.scale.y")
    );
    assert_eq!(
        binding.get("source").and_then(toml::Value::as_str),
        Some("Program.TK101.level")
    );

    let freshness =
        verify_compiled_view_fresh(TOPOLOGY, &compiled.view_toml).expect("verify drift hash");
    assert!(freshness.matches);
    assert_eq!(freshness.topology_hash, compiled.topology_hash);
}

#[test]
fn compiler_resolves_top_center_attachment_without_raw_coordinates() {
    let library = ComponentLibrary::load_builtin().expect("library");
    let compiled = compile_topology_to_view(
        ROBOT_CELL_ATTACHMENT_TOPOLOGY,
        &library,
        &TopologyCompileOptions::default(),
    )
    .expect("compile robot-cell attachment topology");

    assert_eq!(compiled.diagnostics, []);
    assert!(compiled.doctor_results.iter().all(|result| result.passed));
    for rule in [
        "attachment-target-exists",
        "workpiece-rests-on-surface",
        "parent-transform-propagates",
        "scale-vs-grid-cell",
        "link-above-floor",
        "gripper-approach-sane",
    ] {
        assert!(
            compiled
                .doctor_results
                .iter()
                .any(|result| result.rule == rule),
            "doctor results should include physical-scene rule {rule}"
        );
    }

    let view: toml::Value = toml::from_str(&compiled.view_toml).expect("view TOML parses");
    let pickup_position = node_position(&view, "PICKUP-1");
    let box_position = node_position(&view, "BOX-1");
    assert_eq!(pickup_position, [0.0, 0.0, 0.0]);
    assert_eq!(box_position, [0.0, 0.35, 0.0]);
    assert!(
        !compiled.view_toml.contains("xyz"),
        "compiled view should not leak topology raw-coordinate escape hatches"
    );
}

#[test]
fn compiler_emits_robot_cell_metadata_child_nodes_and_robot_bindings() {
    let library = ComponentLibrary::load_builtin().expect("library");
    let compiled = compile_topology_to_view(
        ROBOT_CELL_GENERATED_TOPOLOGY,
        &library,
        &TopologyCompileOptions::default(),
    )
    .expect("compile generated robot-cell topology");

    let view: toml::Value = toml::from_str(&compiled.view_toml).expect("view TOML parses");
    let metadata = view
        .get("metadata")
        .and_then(toml::Value::as_table)
        .expect("compiled view metadata");
    assert_eq!(
        metadata.get("asset_state").and_then(toml::Value::as_str),
        Some("procedural_robot")
    );

    for node_id in [
        "ROBOT-1",
        "ROBOT-1.shoulder",
        "ROBOT-1.elbow",
        "ROBOT-1.wrist",
        "GRIPPER-1",
        "GRIPPER-1.left_jaw",
        "GRIPPER-1.right_jaw",
        "BOX-1",
        "PICKUP-1",
        "DROP-1",
    ] {
        node_position(&view, node_id);
    }

    let bindings = view
        .get("bind3d")
        .and_then(toml::Value::as_array)
        .expect("compiled bind3d blocks");
    for (node, property, source) in [
        (
            "ROBOT-1.shoulder",
            "transform.rotation.z",
            "Main.RobotShoulderAngle",
        ),
        (
            "ROBOT-1.elbow",
            "transform.rotation.z",
            "Main.RobotElbowAngle",
        ),
        (
            "ROBOT-1.wrist",
            "transform.rotation.z",
            "Main.RobotWristAngle",
        ),
    ] {
        assert!(
            bindings.iter().any(|binding| {
                binding.get("node").and_then(toml::Value::as_str) == Some(node)
                    && binding.get("property").and_then(toml::Value::as_str) == Some(property)
                    && binding.get("source").and_then(toml::Value::as_str) == Some(source)
            }),
            "compiled robot-cell view should bind {source} to {node} {property}"
        );
    }
}

#[test]
fn compiler_emits_operator_write_interaction_on_target_node() {
    let library = ComponentLibrary::load_builtin().expect("library");
    let compiled = compile_topology_to_view(
        TOPOLOGY_WITH_INTERACTION,
        &library,
        &TopologyCompileOptions::default(),
    )
    .expect("compile topology interaction");

    assert!(compiled.view_toml.contains("[[node.interaction]]"));
    assert!(compiled.view_toml.contains("event = \"click\""));
    assert!(compiled.view_toml.contains("action = \"hmi.write\""));
    assert!(compiled
        .view_toml
        .contains("id = \"resource/RESOURCE/program/Main/field/run\""));
    assert!(compiled.view_toml.contains("value = true"));
    assert!(compiled.view_toml.contains("required_role = \"Engineer\""));
    assert!(compiled
        .view_toml
        .contains("confirmation = { title = \"Start pump\", message = \"Write Main.run TRUE\" }"));
}

#[test]
fn compiler_cli_dry_run_emits_json_without_writing_view() {
    let root = std::env::temp_dir().join(format!(
        "trust-twin-compiler-cli-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    fs::create_dir_all(&root).expect("create temp root");
    let topology_path = root.join("cell.topology.toml");
    let view_path = root.join("cell.view.toml");
    fs::write(&topology_path, TOPOLOGY).expect("write topology");

    let compiler_exe =
        std::env::var("CARGO_BIN_EXE_trust-twin-compiler").expect("compiler binary path");
    let output = Command::new(compiler_exe)
        .arg("--dry-run")
        .arg("--input")
        .arg(&topology_path)
        .arg("--json")
        .output()
        .expect("run trust-twin compiler CLI");

    assert!(
        output.status.success(),
        "compiler CLI failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !view_path.exists(),
        "dry-run validation must not write a compiled view file"
    );

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("compiler CLI JSON");
    assert_eq!(json["ok"], true);
    assert_eq!(json["input"], topology_path.to_string_lossy().as_ref());
    assert_eq!(json["stats"]["component_count"], 3);
    assert_eq!(json["stats"]["connection_count"], 1);
    assert_eq!(json["stats"]["binding_count"], 1);
    assert_eq!(json["diagnostics"], serde_json::json!([]));
    assert!(json["topology_hash"].as_str().unwrap_or_default().len() == 64);
    assert!(json["view_hash"].as_str().unwrap_or_default().len() == 64);
    assert!(json["doctor_results"]
        .as_array()
        .expect("doctor results array")
        .iter()
        .all(|result| result["passed"] == true));
}

#[test]
fn compiler_cli_writes_view_when_output_is_set() {
    let root = std::env::temp_dir().join(format!(
        "trust-twin-compiler-cli-write-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    fs::create_dir_all(&root).expect("create temp root");
    let topology_path = root.join("robot-cell.topology.toml");
    let view_path = root.join("robot-cell.view.toml");
    fs::write(&topology_path, ROBOT_CELL_GENERATED_TOPOLOGY).expect("write topology");

    let compiler_exe =
        std::env::var("CARGO_BIN_EXE_trust-twin-compiler").expect("compiler binary path");
    let output = Command::new(compiler_exe)
        .arg("--input")
        .arg(&topology_path)
        .arg("--output")
        .arg(&view_path)
        .output()
        .expect("run trust-twin compiler CLI");

    assert!(
        output.status.success(),
        "compiler CLI failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let written = fs::read_to_string(&view_path).expect("compiled view was written");
    assert!(written.starts_with("# trust-twin-topology-hash:v1:sha256:"));
    assert!(written.contains("[metadata]"));
    assert!(written.contains("id = \"ROBOT-1.shoulder\""));
    assert!(written.contains("node = \"ROBOT-1.wrist\""));
}

#[test]
fn topology_hash_ignores_whitespace_and_comments() {
    let library = ComponentLibrary::load_builtin().expect("library");
    let canonical =
        compile_topology_to_view(TOPOLOGY, &library, &TopologyCompileOptions::default())
            .expect("compile canonical topology");
    let commented = compile_topology_to_view(
        COMMENTED_TOPOLOGY,
        &library,
        &TopologyCompileOptions::default(),
    )
    .expect("compile commented topology");

    assert_eq!(canonical.topology_hash, commented.topology_hash);
    assert_eq!(canonical.view_toml, commented.view_toml);
}

#[test]
fn topology_lints_run_before_generation() {
    let library = ComponentLibrary::load_builtin().expect("library");
    for (name, topology, expected_code) in [
        (
            "unknown kind",
            r#"
[[components]]
id = "TK-101"
kind = "unknown-tank"
at = { grid = "A1" }
"#,
            "unknown-component-kind",
        ),
        (
            "unknown signal",
            r#"
[[components]]
id = "TK-101"
kind = "tank"
at = { grid = "A1" }

[[bindings]]
component = "TK-101"
signal = "temperature"
source = "Program.TK101.temperature"
"#,
            "unknown-signal",
        ),
        (
            "missing port",
            r#"
[[components]]
id = "TK-101"
kind = "tank"
at = { grid = "A1" }

[[components]]
id = "P-101"
kind = "pump"
at = { grid = "A3" }

[[connections]]
id = "line-101"
from = "TK-101.missing"
to = "P-101.inlet"
medium = "water"
diameter = "DN50"
route = "auto"
"#,
            "unknown-port",
        ),
        (
            "domain mismatch",
            r#"
[[components]]
id = "TK-101"
kind = "tank"
at = { grid = "A1" }

[[components]]
id = "DRV-101"
kind = "vfd"
at = { grid = "A3" }

[[connections]]
id = "line-101"
from = "TK-101.outlet"
to = "DRV-101.power_in"
medium = "water"
diameter = "DN50"
route = "auto"
"#,
            "domain-mismatch",
        ),
        (
            "raw xyz without justification",
            r#"
[[components]]
id = "TK-101"
kind = "tank"
at = { xyz = [0.0, 0.0, 0.0] }
"#,
            "raw-xyz-without-justification",
        ),
        (
            "unsafe write role",
            r#"
[[components]]
id = "P-101"
kind = "pump"
at = { grid = "A1" }

[[interactions]]
component = "P-101"
event = "click"
action = "hmi.write"
id = "resource/RESOURCE/program/Main/field/run"
value = true
required_role = "Viewer"
"#,
            "unsafe-write-interaction-role",
        ),
        (
            "duplicate grid cell",
            r#"
[[components]]
id = "TK-101"
kind = "tank"
at = { grid = "A1" }

[[components]]
id = "P-101"
kind = "pump"
at = { grid = "A1" }
"#,
            "duplicate-grid-cell",
        ),
        (
            "missing attachment target",
            r#"
[[components]]
id = "BOX-1"
kind = "workpiece"
at = { attach_to = "PICKUP-1.top", placement = "top_center" }
"#,
            "attachment-target-exists",
        ),
        (
            "floating workpiece",
            r#"
[[components]]
id = "BOX-1"
kind = "workpiece"
at = { grid = "A1" }
"#,
            "workpiece-rests-on-surface",
        ),
        (
            "oversized grid footprint",
            r#"
[grid]
cell_size = 0.5
origin = [0.0, 0.0, 0.0]

[[components]]
id = "ROBOT-1"
kind = "robot_arm"
at = { grid = "A1" }
"#,
            "scale-vs-grid-cell",
        ),
        (
            "robot below floor",
            r#"
[[components]]
id = "ROBOT-1"
kind = "robot_arm"
at = { grid = "A1" }
params = { min_y = -0.1 }
"#,
            "link-above-floor",
        ),
        (
            "gripper approach from under",
            r#"
[[components]]
id = "ROBOT-1"
kind = "robot_arm"
at = { grid = "A1" }

[[components]]
id = "GRIPPER-1"
kind = "gripper"
at = { attach_to = "ROBOT-1.tool", placement = "mount" }
params = { approach_axis = [0.0, -1.0, 0.0] }
"#,
            "gripper-approach-sane",
        ),
    ] {
        let error =
            compile_topology_to_view(topology, &library, &TopologyCompileOptions::default())
                .expect_err(name);
        assert!(
            error
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.code == expected_code),
            "{name} should report {expected_code}; got {:?}",
            error.diagnostics()
        );
    }
}

fn node_position(view: &toml::Value, node_id: &str) -> [f64; 3] {
    let nodes = view
        .get("node")
        .and_then(toml::Value::as_array)
        .expect("compiled view nodes");
    let node = nodes
        .iter()
        .find(|node| node.get("id").and_then(toml::Value::as_str) == Some(node_id))
        .unwrap_or_else(|| panic!("compiled view should contain node {node_id}"));
    let position = node
        .get("transform")
        .and_then(|transform| transform.get("position"))
        .and_then(toml::Value::as_array)
        .expect("node transform position");
    let mut result = [0.0; 3];
    for (index, value) in position.iter().enumerate().take(3) {
        result[index] = value.as_float().expect("position number");
    }
    result
}

const EXPECTED_VIEW_BODY: &str = r##"[[node]]
id = "TK-101"
primitive = "box"
label = "TK-101"

[node.transform]
position = [0.0, 0.0, 0.0]
scale = [1.2, 2.0, 1.2]

[node.material]
base_color = "#64748b"

[[node]]
id = "TK-101.level"
primitive = "box"
label = "TK-101 level"

[node.transform]
position = [0.0, -0.4, 0.0]
scale = [1.0, 0.2, 1.0]

[node.material]
base_color = "#38bdf8"
opacity = 0.85

[[node]]
id = "P-101"
primitive = "box"
label = "P-101"

[node.transform]
position = [4.0, 0.0, 0.0]
scale = [1.2, 1.0, 1.2]

[node.material]
base_color = "#22c55e"

[[node]]
id = "V-101"
primitive = "box"
label = "V-101"

[node.transform]
position = [8.0, 0.0, 0.0]
scale = [0.8, 0.8, 0.8]

[node.material]
base_color = "#f59e0b"

[[node]]
id = "line-101.pipe"
primitive = "box"
label = "line-101"

[node.transform]
position = [2.0, 0.0, 0.0]
scale = [2.5, 0.08, 0.08]

[node.material]
base_color = "#0ea5e9"

[[camera]]
id = "main"
position = [4.0, 4.0, 8.0]
target = [4.0, 0.0, 0.0]
fov_degrees = 45.0

[[light]]
id = "key"
kind = "directional"
position = [2.0, 4.0, 6.0]
intensity = 1.0

[[bind3d]]
node = "TK-101.level"
property = "transform.scale.y"
source = "Program.TK101.level"
scale = { min = 0.0, max = 100.0, output_min = 0.05, output_max = 1.0 }
"##;

const COMMENTED_TOPOLOGY: &str = r#"
# Comments and whitespace are intentionally ignored by the topology hash.
[[components]]
id = "TK-101"
kind = "tank"
at = { grid = "A1" }

[[components]]
id = "P-101"
kind = "pump"
at = { grid = "A3" }

[[components]]
id = "V-101"
kind = "valve"
at = { grid = "A5" }

[[connections]]
id = "line-101"
from = "TK-101.outlet"
to = "P-101.inlet"
medium = "water"
diameter = "DN50"
route = "auto"

[[bindings]]
component = "TK-101"
signal = "level"
source = "Program.TK101.level"
access = "read"
"#;

const TOPOLOGY_WITH_INTERACTION: &str = r#"
[[components]]
id = "P-101"
kind = "pump"
at = { grid = "A1" }

[[interactions]]
component = "P-101"
event = "click"
action = "hmi.write"
id = "resource/RESOURCE/program/Main/field/run"
value = true
required_role = "Engineer"
confirmation = { title = "Start pump", message = "Write Main.run TRUE" }
"#;

const ROBOT_CELL_ATTACHMENT_TOPOLOGY: &str = r#"
[[components]]
id = "PICKUP-1"
kind = "pickup_zone"
at = { grid = "A1" }

[[components]]
id = "BOX-1"
kind = "workpiece"
at = { attach_to = "PICKUP-1.top", placement = "top_center" }
"#;

const ROBOT_CELL_GENERATED_TOPOLOGY: &str = r#"
[metadata]
asset_state = "procedural_robot"
design_decision = "Q-H"

[[components]]
id = "ROBOT-1"
kind = "robot_arm"
at = { grid = "B2" }

[[components]]
id = "PICKUP-1"
kind = "pickup_zone"
at = { grid = "A1" }

[[components]]
id = "BOX-1"
kind = "workpiece"
at = { attach_to = "PICKUP-1.top", placement = "top_center" }

[[components]]
id = "GRIPPER-1"
kind = "gripper"
at = { attach_to = "ROBOT-1.tool", placement = "mount" }

[[components]]
id = "DROP-1"
kind = "drop_zone"
at = { grid = "A3" }

[[bindings]]
component = "ROBOT-1"
signal = "shoulder_angle"
source = "Main.RobotShoulderAngle"
access = "read"

[[bindings]]
component = "ROBOT-1"
signal = "elbow_angle"
source = "Main.RobotElbowAngle"
access = "read"

[[bindings]]
component = "ROBOT-1"
signal = "wrist_angle"
source = "Main.RobotWristAngle"
access = "read"
"#;
