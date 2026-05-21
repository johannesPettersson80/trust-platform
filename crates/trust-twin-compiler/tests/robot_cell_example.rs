use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use trust_twin_compiler::diagnose_scene_view_physical_issues;

const REQUIRED_FILES: &[&str] = &[
    "README.md",
    "src/main.st",
    "runtime.toml",
    "io.toml",
    "simulation.toml",
    "hmi/robot-cell.toml",
    "hmi/views/robot-cell.topology.toml",
    "hmi/views/robot-cell.view.toml",
];

const REQUIRED_NODES: &[&str] = &[
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
    "LIGHT-1",
];

const REQUIRED_PLC_VARIABLES: &[&str] = &[
    "RobotShoulderAngle",
    "RobotElbowAngle",
    "RobotWristAngle",
    "RobotGripperOpen",
    "RobotBoxParentState",
    "RobotEnabled",
    "RobotStatusLight",
];

const REQUIRED_BIND_SOURCES: &[&str] = &[
    "Main.RobotShoulderAngle",
    "Main.RobotElbowAngle",
    "Main.RobotWristAngle",
    "Main.RobotGripperOpen",
    "Main.RobotBoxParentState",
    "Main.RobotStatusLight",
];

#[test]
fn robot_cell_example_has_required_project_files() {
    let root = robot_cell_root();
    for relative in REQUIRED_FILES {
        let path = root.join(relative);
        assert!(
            path.is_file(),
            "robot-cell example must include {}",
            path.display()
        );
    }
}

#[test]
fn robot_cell_scene_declares_packaged_asset_state_assets_and_default_camera() {
    let root = robot_cell_root();
    let page = read_toml(&root.join("hmi/robot-cell.toml"));
    assert_eq!(str_field(&page, "kind"), Some("scene3d"));
    assert_eq!(str_field(&page, "view"), Some("robot-cell.view.toml"));

    let topology = read_toml(&root.join("hmi/views/robot-cell.topology.toml"));
    let metadata = topology
        .get("metadata")
        .and_then(toml::Value::as_table)
        .expect("topology metadata");
    assert_eq!(
        metadata.get("asset_state").and_then(toml::Value::as_str),
        Some("packaged_asset")
    );
    assert_eq!(
        metadata
            .get("design_decision")
            .and_then(toml::Value::as_str),
        Some("Q-H")
    );

    let view = read_toml(&root.join("hmi/views/robot-cell.view.toml"));
    let view_metadata = view
        .get("metadata")
        .and_then(toml::Value::as_table)
        .expect("view metadata");
    assert_eq!(
        view_metadata
            .get("asset_state")
            .and_then(toml::Value::as_str),
        Some("packaged_asset")
    );
    let asset_ids = array(&view, "asset")
        .iter()
        .filter_map(|asset| str_field(asset, "id"))
        .collect::<BTreeSet<_>>();
    for required in [
        "trust-twin/components/ur10/visual/base.gltf",
        "trust-twin/components/ur10/visual/shoulder.gltf",
        "trust-twin/components/ur10/visual/upperarm.gltf",
        "trust-twin/components/ur10/visual/forearm.gltf",
        "trust-twin/components/ur10/visual/wrist1.gltf",
        "trust-twin/components/ur10/visual/wrist2.gltf",
        "trust-twin/components/ur10/visual/wrist3.gltf",
        "trust-twin/components/schunk-wsg50/meshes/wsg_body.gltf",
        "trust-twin/components/schunk-wsg50/meshes/finger_with_tip.gltf",
        "trust-twin/components/ycb/meshes/003_cracker_box_textured.gltf",
    ] {
        assert!(
            asset_ids.contains(required),
            "robot-cell packaged view must declare asset '{required}'"
        );
    }
    assert_eq!(
        view_metadata
            .get("design_decision")
            .and_then(toml::Value::as_str),
        Some("Q-H")
    );

    let main_camera = array(&view, "camera")
        .iter()
        .find(|camera| str_field(camera, "id") == Some("main"))
        .expect("main camera");
    assert!(
        main_camera.get("position").is_some(),
        "main camera must set position"
    );
    assert!(
        main_camera.get("target").is_some(),
        "main camera must set target"
    );
    assert!(
        main_camera.get("fov_degrees").is_some(),
        "main camera must set fov_degrees"
    );
}

#[test]
fn robot_cell_view_contains_recognizable_robot_nodes_without_box_placeholder_escape() {
    let root = robot_cell_root();
    let view = read_toml(&root.join("hmi/views/robot-cell.view.toml"));
    let nodes = array(&view, "node");
    let node_ids = nodes
        .iter()
        .filter_map(|node| str_field(node, "id"))
        .collect::<BTreeSet<_>>();

    for required in REQUIRED_NODES {
        assert!(
            node_ids.contains(required),
            "robot-cell view must include node '{required}'"
        );
    }

    let lower = fs::read_to_string(root.join("hmi/views/robot-cell.view.toml"))
        .expect("read robot-cell view")
        .to_ascii_lowercase();
    for forbidden in ["placeholder", "fallback box", "box-only"] {
        assert!(
            !lower.contains(forbidden),
            "robot-cell view must not contain '{forbidden}'"
        );
    }

    for node in nodes {
        let id = str_field(node, "id").unwrap_or_default();
        let label = str_field(node, "label").unwrap_or_default();
        assert!(
            !label.trim().is_empty(),
            "node '{id}' must carry an operator-readable label"
        );
    }
}

#[test]
fn robot_cell_bindings_are_plc_driven_from_main_st() {
    let root = robot_cell_root();
    let main_st = fs::read_to_string(root.join("src/main.st")).expect("read robot-cell ST");
    assert!(
        main_st.contains("PROGRAM Main"),
        "robot-cell ST must define PROGRAM Main"
    );
    for variable in REQUIRED_PLC_VARIABLES {
        assert!(
            main_st.contains(variable),
            "PROGRAM Main must expose PLC variable '{variable}'"
        );
    }

    for sequence_step in [
        "approach pickup",
        "close gripper",
        "lift",
        "translate",
        "lower",
        "open gripper",
        "return",
    ] {
        assert!(
            main_st.to_ascii_lowercase().contains(sequence_step),
            "PROGRAM Main must document the pick-and-place step '{sequence_step}'"
        );
    }

    let view = read_toml(&root.join("hmi/views/robot-cell.view.toml"));
    let bindings = array(&view, "bind3d");
    let sources = bindings
        .iter()
        .filter_map(|binding| str_field(binding, "source"))
        .collect::<BTreeSet<_>>();
    for source in REQUIRED_BIND_SOURCES {
        assert!(
            sources.contains(source),
            "robot-cell view must bind PLC source '{source}'"
        );
    }
    for source in ["Main.RobotBoxX", "Main.RobotBoxY", "Main.RobotBoxZ"] {
        assert!(
            !sources.contains(source),
            "BOX-1 must not be driven by independent world-position source '{source}'"
        );
    }

    for (node, property) in [
        ("ROBOT-1.shoulder", "transform.rotation.z"),
        ("ROBOT-1.elbow", "transform.rotation.z"),
        ("ROBOT-1.wrist", "transform.rotation.z"),
        ("GRIPPER-1.left_jaw", "transform.position.x"),
        ("GRIPPER-1.right_jaw", "transform.position.x"),
        ("BOX-1", "parent"),
        ("LIGHT-1", "material.emissive"),
    ] {
        assert!(
            bindings.iter().any(|binding| {
                str_field(binding, "node") == Some(node)
                    && str_field(binding, "property") == Some(property)
            }),
            "robot-cell view must bind {node} {property}"
        );
    }

    for property in [
        "transform.position.x",
        "transform.position.y",
        "transform.position.z",
    ] {
        assert!(
            !bindings.iter().any(|binding| {
                str_field(binding, "node") == Some("BOX-1")
                    && str_field(binding, "property") == Some(property)
            }),
            "BOX-1 must not bind {property}; workpiece carry is a parent change"
        );
    }
}

#[test]
fn robot_cell_view_schema_carries_parent_edges_and_local_frames() {
    let root = robot_cell_root();
    let view = read_toml(&root.join("hmi/views/robot-cell.view.toml"));
    let metadata = view
        .get("metadata")
        .and_then(toml::Value::as_table)
        .expect("view metadata");
    assert_eq!(
        metadata
            .get("view_contract_version")
            .and_then(toml::Value::as_integer),
        Some(2),
        "robot-cell view must use the parented view contract"
    );
    assert_eq!(
        metadata
            .get("renderer_contract_version")
            .and_then(toml::Value::as_integer),
        Some(2),
        "renderer and view contracts must be bumped together"
    );

    let nodes = array(&view, "node");
    assert!(!nodes.is_empty(), "robot-cell view must declare nodes");
    for node in nodes {
        let id = str_field(node, "id").unwrap_or_default();
        assert!(
            node.get("parent").is_some(),
            "node '{id}' must carry parent, using an empty string for the scene root"
        );
        assert!(
            node.get("local_position").is_some(),
            "node '{id}' must carry local_position in its parent frame"
        );
        assert!(
            node.get("pivot").is_some(),
            "node '{id}' must carry pivot in its parent frame"
        );
    }

    let parent_of = |id: &str| {
        nodes
            .iter()
            .find(|node| str_field(node, "id") == Some(id))
            .and_then(|node| str_field(node, "parent"))
            .unwrap_or_else(|| panic!("missing parent for {id}"))
            .to_string()
    };
    assert_eq!(parent_of("ROBOT-1"), "");
    assert_eq!(parent_of("ROBOT-1.shoulder"), "ROBOT-1");
    assert_eq!(parent_of("ROBOT-1.elbow"), "ROBOT-1.shoulder");
    assert_eq!(parent_of("ROBOT-1.forearm"), "ROBOT-1.elbow");
    assert_eq!(parent_of("ROBOT-1.wrist"), "ROBOT-1.forearm");
    assert_eq!(parent_of("GRIPPER-1"), "ROBOT-1.wrist");
    assert_eq!(parent_of("GRIPPER-1.left_jaw"), "GRIPPER-1");
    assert_eq!(parent_of("GRIPPER-1.right_jaw"), "GRIPPER-1");
    assert_eq!(parent_of("BOX-1"), "PICKUP-1");

    let box_node = nodes
        .iter()
        .find(|node| str_field(node, "id") == Some("BOX-1"))
        .expect("BOX-1 node");
    let parent_poses = array(box_node, "parent_pose");
    for parent in ["PICKUP-1", "GRIPPER-1", "DROP-1"] {
        assert!(
            parent_poses
                .iter()
                .any(|pose| str_field(pose, "parent") == Some(parent)),
            "BOX-1 must define a local pose for reparenting under {parent}"
        );
    }
}

#[test]
fn robot_cell_readme_requires_assistant_playwright_review_before_johannes_review() {
    let readme =
        fs::read_to_string(robot_cell_root().join("README.md")).expect("read robot-cell README");
    for required in [
        "Playwright",
        "assistant visual review",
        "Johannes visual review",
        "asset_state = \"packaged_asset\"",
        "Universal Robots ROS2 Description commit",
        "18dea90dedf24adea05d920d0d441e4523a41e50",
        "UR10",
        "Schunk WSG-50",
    ] {
        assert!(
            readme.contains(required),
            "robot-cell README must mention '{required}'"
        );
    }
}

#[test]
fn robot_cell_rejected_view_fails_physical_scene_doctor_with_reported_defects() {
    let view_source = fs::read_to_string(robot_cell_root().join("hmi/views/robot-cell.view.toml"))
        .expect("read robot-cell view");
    let view_source =
        fs::read_to_string(robot_cell_root().join("hmi/views/robot-cell.rejected.view.toml"))
            .unwrap_or(view_source);
    let diagnostics =
        diagnose_scene_view_physical_issues(&view_source).expect("diagnose scene view");
    let codes = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect::<BTreeSet<_>>();

    for expected in [
        "workpiece-rests-on-surface",
        "gripper-approach-sane",
        "link-above-floor",
    ] {
        assert!(
            codes.contains(expected),
            "rejected robot-cell view should report {expected}; got {diagnostics:?}"
        );
    }
}

fn robot_cell_root() -> PathBuf {
    workspace_root().join("examples/trust-twin/robot-cell")
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf()
}

fn read_toml(path: &Path) -> toml::Value {
    let text = fs::read_to_string(path).unwrap_or_else(|err| {
        panic!("failed to read {}: {err}", path.display());
    });
    toml::from_str(&text).unwrap_or_else(|err| {
        panic!("failed to parse {}: {err}", path.display());
    })
}

fn array<'a>(value: &'a toml::Value, key: &str) -> &'a [toml::Value] {
    value
        .get(key)
        .and_then(toml::Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

fn str_field<'a>(value: &'a toml::Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(toml::Value::as_str)
}
