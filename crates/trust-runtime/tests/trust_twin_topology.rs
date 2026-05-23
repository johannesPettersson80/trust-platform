use std::collections::BTreeMap;

use serde_json::json;
use trust_runtime::hmi::{
    build_trust_twin_scene, write_trust_twin_static_view_proof, HmiSceneViewPayload,
};
use trust_runtime::value::Value;
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
fn trust_twin_topology_compiles_renders_and_writes_gate_artifact() -> anyhow::Result<()> {
    let library = ComponentLibrary::load_builtin()?;
    let compiled =
        compile_topology_to_view(TOPOLOGY, &library, &TopologyCompileOptions::default())?;
    let freshness = verify_compiled_view_fresh(TOPOLOGY, &compiled.view_toml)?;
    assert!(
        freshness.matches,
        "compiled view header must match topology hash"
    );

    let view: HmiSceneViewPayload = toml::from_str(&compiled.view_toml)?;
    assert_eq!(view.bindings3d.len(), 1);
    assert!(view.nodes.iter().any(|node| node.id == "TK-101.level"));
    assert!(view.nodes.iter().any(|node| node.id == "line-101.pipe"));

    let low_scale_y = tank_level_scale_y(&view, 10.0)?;
    let high_scale_y = tank_level_scale_y(&view, 75.0)?;
    assert!(
        high_scale_y > low_scale_y,
        "higher TK-101 level should increase rendered fill scale: {low_scale_y} -> {high_scale_y}"
    );

    let artifact_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("target/gate-artifacts");
    std::fs::create_dir_all(&artifact_dir)?;
    let artifact_dir = artifact_dir.canonicalize()?;
    let mut values = BTreeMap::new();
    values.insert("Program.TK101.level".to_string(), Value::Real(75.0));
    let rendered = write_trust_twin_static_view_proof(&view, &values, &artifact_dir)?;
    assert_eq!(rendered.binding.applied_count, 1);
    assert!(rendered.visual.non_background_pixels > 0);

    let mut evidence_blockers = Vec::new();
    if !freshness.matches {
        evidence_blockers.push("compiled view hash does not match topology source".to_string());
    }
    if high_scale_y <= low_scale_y {
        evidence_blockers.push("level binding did not change rendered tank fill scale".to_string());
    }
    if rendered.visual.non_background_pixels == 0 {
        evidence_blockers.push("rendered topology proof has no non-background pixels".to_string());
    }

    let topology_artifact = json!({
        "topology_hash": compiled.topology_hash,
        "view_toml_hash": compiled.view_hash,
        "doctor_rule_results": compiled.doctor_results,
        "rendered_proof": {
            "static_view_artifact": artifact_dir.join("trust-twin-p1-static-view.json").display().to_string(),
            "proof_image": rendered.proof_image,
            "frame_sha256": rendered.visual.frame_sha256,
            "non_background_pixels": rendered.visual.non_background_pixels,
        },
        "binding": {
            "source": "Program.TK101.level",
            "low_value": 10.0,
            "low_scale_y": low_scale_y,
            "high_value": 75.0,
            "high_scale_y": high_scale_y,
        },
        "evidence_blockers": evidence_blockers,
    });
    let artifact_path = artifact_dir.join("trust-twin-p1.5-topology.json");
    std::fs::write(
        &artifact_path,
        serde_json::to_string_pretty(&topology_artifact)?,
    )?;
    assert!(
        artifact_path.is_file(),
        "P1.5 topology gate artifact must be written"
    );
    assert!(
        evidence_blockers.is_empty(),
        "P1.5 topology proof blockers: {evidence_blockers:?}"
    );

    Ok(())
}

fn tank_level_scale_y(view: &HmiSceneViewPayload, level: f64) -> anyhow::Result<f32> {
    let mut values = BTreeMap::new();
    values.insert("Program.TK101.level".to_string(), Value::Real(level as f32));
    let mut scene = build_trust_twin_scene(view)?;
    let report = scene.apply_bindings(&view.bindings3d, &values)?;
    assert_eq!(report.applied.len(), 1);
    assert!(report.missing_sources.is_empty());
    assert!(report.errors.is_empty());
    let state = scene
        .node_state("TK-101.level")
        .expect("compiled tank level node remains addressable");
    Ok(state.scale[1])
}
