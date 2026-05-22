use std::collections::BTreeMap;

use crate::value::Value;

#[test]
fn trust_twin_scene_bridge_applies_rotation_binding_to_scena_node() {
    let view: HmiSceneViewPayload = toml::from_str(
        r##"
[[node]]
id = "motor-1.shaft"
primitive = "box"
[node.transform]
position = [0.0, 0.0, 0.0]
rotation = [0.0, 0.0, 0.0]
scale = [1.0, 0.35, 0.35]
[node.material]
base_color = "#3b82f6"

[[camera]]
id = "main"
position = [0.0, 0.0, 4.0]
target = [0.0, 0.0, 0.0]

[[light]]
id = "key"
kind = "directional"
intensity = 1.0

[[bind3d]]
node = "motor-1.shaft"
property = "transform.rotation.y"
source = "Program.motor1.shaft_angle"
scale = { min = 0.0, max = 10.0, output_min = 0.0, output_max = 3.14159265 }
"##,
    )
    .expect("view fixture parses");

    let mut scene = build_trust_twin_scene(&view).expect("build trust-twin scena bridge");
    assert!(scene.scena_scene().active_camera().is_some());
    assert!(scene.node_key("motor-1.shaft").is_some());

    let mut values = BTreeMap::new();
    values.insert("Program.motor1.shaft_angle".to_string(), Value::Real(5.0));
    let report = scene
        .apply_bindings(&view.bindings3d, &values)
        .expect("apply bind3d values");

    assert_eq!(report.applied.len(), 1);
    assert!(report.missing_sources.is_empty());
    assert!(report.errors.is_empty());
    let state = scene
        .node_state("motor-1.shaft")
        .expect("motor node state remains addressable");
    assert!(
        (state.rotation[1] - std::f32::consts::FRAC_PI_2).abs() < 0.000_01,
        "rotation.y should be scaled into radians, got {}",
        state.rotation[1]
    );
}

#[test]
fn trust_twin_static_view_proof_writes_gate_artifact_with_capabilities() {
    let view: HmiSceneViewPayload = toml::from_str(
        r##"
[[node]]
id = "motor-1.shaft"
primitive = "box"
[node.transform]
scale = [1.0, 0.35, 0.35]
[node.material]
base_color = "#22c55e"

[[camera]]
id = "main"
position = [0.0, 0.0, 4.0]
target = [0.0, 0.0, 0.0]

[[light]]
id = "key"
kind = "directional"
intensity = 1.0

[[bind3d]]
node = "motor-1.shaft"
property = "transform.rotation.y"
source = "Program.motor1.shaft_angle"
"##,
    )
    .expect("view fixture parses");
    let mut values = BTreeMap::new();
    values.insert("Program.motor1.shaft_angle".to_string(), Value::Real(0.75));
    let artifact_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("target/gate-artifacts");

    let artifact = write_trust_twin_static_view_proof(&view, &values, &artifact_dir)
        .expect("write trust-twin P1 proof artifact");

    assert_eq!(artifact.driver, "scena");
    assert_eq!(artifact.binding.applied_count, 1);
    assert!(artifact.binding.errors.is_empty());
    assert!(artifact.visual.non_background_pixels > 0);
    assert!(artifact.visual.frame_sha256.len() == 64);
    assert!(artifact.proof_image.ends_with("trust-twin-p1-static-view.ppm"));
    assert!(
        artifact
            .baseline
            .tolerance
            .contains_key("non_background_pixels_min")
    );
    assert_eq!(artifact.capability_report.backend, "Headless");
    assert!(artifact.evidence_blockers.is_empty());

    let artifact_path = artifact_dir.join("trust-twin-p1-static-view.json");
    assert!(artifact_path.is_file(), "gate artifact must be written");
    let image_path = artifact_dir.join("trust-twin-p1-static-view.ppm");
    assert!(image_path.is_file(), "render proof image must be written");
}
