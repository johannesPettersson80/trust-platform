#[test]
fn hmi_dir_schema_snapshot_includes_rich_metadata() {
    let root = temp_dir("trust-runtime-hmi-schema-snapshot");
    write_file(
        &root.join("hmi/_config.toml"),
        r##"
[theme]
style = "mint"
accent = "#14b8a6"
"##,
    );
    write_file(
        &root.join("hmi/overview.toml"),
        r##"
title = "Overview"
icon = "activity"
order = 1
kind = "dashboard"

[[section]]
title = "Drive"
span = 8

[[section.widget]]
type = "gauge"
bind = "Main.speed"
label = "Speed"
unit = "rpm"
span = 6
on_color = "#22c55e"
off_color = "#1f2937"

[[section.widget.zones]]
from = 50
to = 100
color = "#ef4444"

[[section.widget.zones]]
from = 0
to = 50
color = "#22c55e"
"##,
    );

    let source = r#"
PROGRAM Main
VAR
    speed : REAL := 25.0;
END_VAR
END_PROGRAM
"#;
    let metadata = metadata_for_source(source);
    let source_path = root.join("src/main.st");
    let source_refs = [HmiSourceRef {
        path: &source_path,
        text: source,
    }];
    let customization = load_customization(Some(&root), &source_refs);
    let schema = build_schema("RESOURCE", &metadata, None, true, Some(&customization));
    let widget_id = "resource/RESOURCE/program/Main/field/speed";

    let overview_page = schema
        .pages
        .iter()
        .find(|page| page.id == "overview")
        .expect("overview page");
    assert_eq!(
        serde_json::to_value(overview_page).expect("serialize overview page"),
        json!({
            "id": "overview",
            "title": "Overview",
            "order": 1,
            "kind": "dashboard",
            "icon": "activity",
            "duration_ms": null,
            "sections": [
                {
                    "title": "Drive",
                    "span": 8,
                    "widget_ids": [widget_id]
                }
            ]
        })
    );

    let speed = schema
        .widgets
        .iter()
        .find(|widget| widget.path == "Main.speed")
        .expect("speed widget");
    assert_eq!(
        serde_json::to_value(speed).expect("serialize speed widget"),
        json!({
            "id": widget_id,
            "path": "Main.speed",
            "label": "Speed",
            "data_type": "REAL",
            "access": "read",
            "writable": false,
            "widget": "gauge",
            "source": "program:Main",
            "page": "overview",
            "group": "Drive",
            "order": 0,
            "zones": [
                { "from": 0.0, "to": 50.0, "color": "#22c55e" },
                { "from": 50.0, "to": 100.0, "color": "#ef4444" }
            ],
            "on_color": "#22c55e",
            "off_color": "#1f2937",
            "section_title": "Drive",
            "widget_span": 6,
            "unit": "rpm",
            "min": null,
            "max": null
        })
    );

    std::fs::remove_dir_all(root).ok();
}

#[test]
fn hmi_dir_alarm_thresholds_map_to_widget_limits() {
    let root = temp_dir("trust-runtime-hmi-dir-alarms");
    write_file(
        &root.join("hmi/_config.toml"),
        r#"
[[alarm]]
bind = "Main.speed"
high = 120.0
low = 10.0
label = "Speed Alarm"
"#,
    );
    write_file(
        &root.join("hmi/overview.toml"),
        r#"
title = "Overview"

[[section]]
title = "Process"
span = 12

[[section.widget]]
type = "value"
bind = "Main.speed"
"#,
    );
    let source = r#"
PROGRAM Main
VAR
    speed : REAL := 0.0;
END_VAR
END_PROGRAM
"#;
    let metadata = metadata_for_source(source);
    let source_path = root.join("src/main.st");
    let source_refs = [HmiSourceRef {
        path: &source_path,
        text: source,
    }];
    let customization = load_customization(Some(&root), &source_refs);
    let schema = build_schema("RESOURCE", &metadata, None, true, Some(&customization));
    let speed = schema
        .widgets
        .iter()
        .find(|widget| widget.path == "Main.speed")
        .expect("speed widget");
    assert_eq!(speed.min, Some(10.0));
    assert_eq!(speed.max, Some(120.0));
    assert_eq!(speed.label, "Speed Alarm");
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn validate_hmi_bindings_reports_unknown_paths_widgets_and_mismatches() {
    let root = temp_dir("trust-runtime-hmi-dir-validate");
    write_file(
        &root.join("hmi/overview.toml"),
        r#"
title = "Overview"

[[section]]
title = "Main"
span = 12

[[section.widget]]
type = "gauge"
bind = "Main.run"

[[section.widget]]
type = "rocket"
bind = "Main.speed"

[[section.widget]]
type = "value"
bind = "Main.unknown"
"#,
    );
    let descriptor = load_hmi_dir(&root).expect("descriptor");
    let source = r#"
PROGRAM Main
VAR
    run : BOOL := FALSE;
    speed : REAL := 0.0;
END_VAR
END_PROGRAM
"#;
    let metadata = metadata_for_source(source);
    let diagnostics = validate_hmi_bindings("RESOURCE", &metadata, None, &descriptor);
    assert!(diagnostics
        .iter()
        .any(|diag| diag.code == HMI_DIAG_TYPE_MISMATCH));
    assert!(diagnostics
        .iter()
        .any(|diag| diag.code == HMI_DIAG_UNKNOWN_WIDGET));
    assert!(diagnostics
        .iter()
        .any(|diag| diag.code == HMI_DIAG_UNKNOWN_BIND));
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn scene3d_descriptor_exports_view_and_bind3d_contract() {
    let root = temp_dir("trust-runtime-hmi-scene3d-contract");
    write_file(
        &root.join("hmi/cell.toml"),
        r##"
title = "Drive Cell 3D"
kind = "scene3d"
view = "drive-cell.view.toml"

[[bind3d]]
node = "motor-1.shaft"
property = "transform.rotation.y"
source = "Main.shaft_angle"
scale = { min = -3.14159265, max = 3.14159265, output_min = -3.14159265, output_max = 3.14159265 }
format = "{:.2f} rad"
"##,
    );
    write_file(
        &root.join("hmi/views/drive-cell.view.toml"),
        r##"
[[asset]]
id = "motor"
uri = "assets/motor.glb"

[[node]]
id = "motor-1.shaft"
asset = "motor"
[node.transform]
position = [0.0, 0.0, 0.0]
rotation = [0.0, 0.0, 0.0]
scale = [1.0, 1.0, 1.0]

[[node.interaction]]
event = "click"
action = "hmi.write"
id = "resource/RESOURCE/program/Main/field/run"
value = true
required_role = "Engineer"
confirmation = { title = "Start motor", message = "Write Main.run TRUE" }

[[camera]]
id = "main"
position = [0.0, 2.0, 6.0]
target = [0.0, 0.0, 0.0]

[[light]]
id = "key"
kind = "directional"
intensity = 1.0

[[bind3d]]
node = "motor-1.shaft"
property = "transform.rotation.y"
source = "Main.shaft_angle"
"##,
    );
    write_file(
        &root.join("hmi/views/ignored.view.toml"),
        r#"[[node]]
id = "ignored"
"#,
    );

    let descriptor = load_hmi_dir_impl(&root).expect("load hmi dir");
    assert_eq!(descriptor.pages.len(), 1, "view payloads must not become pages");
    let page = descriptor.pages.first().expect("scene3d page");
    assert_eq!(page.kind, "scene3d");
    assert_eq!(page.view.as_deref(), Some("views/drive-cell.view.toml"));
    assert_eq!(page.bindings3d.len(), 1);
    assert_eq!(page.bindings3d[0].property, "transform.rotation.y");

    let view = load_hmi_scene_view(&root, page.view.as_deref().expect("view ref"))
        .expect("load scene view");
    assert_eq!(view.nodes.len(), 1);
    assert_eq!(view.nodes[0].interactions.len(), 1);
    assert_eq!(view.nodes[0].interactions[0].action, "hmi.write");
    assert!(
        view.nodes[0].interactions[0]
            .value
            .as_bool()
            .expect("interaction value")
    );
    assert_eq!(view.cameras.len(), 1);
    assert_eq!(view.lights.len(), 1);
    assert_eq!(view.bindings3d.len(), 1);

    let source = r#"
PROGRAM Main
VAR
    shaft_angle : REAL := 0.0;
END_VAR
END_PROGRAM
"#;
    let metadata = metadata_for_source(source);
    let source_path = root.join("src/main.st");
    let source_refs = [HmiSourceRef {
        path: &source_path,
        text: source,
    }];
    let customization = load_customization(Some(&root), &source_refs);
    let schema = build_schema("RESOURCE", &metadata, None, true, Some(&customization));
    let schema_page = schema
        .pages
        .iter()
        .find(|candidate| candidate.id == "cell")
        .expect("schema page");
    assert_eq!(schema_page.kind, "scene3d");
    assert_eq!(schema_page.view.as_deref(), Some("views/drive-cell.view.toml"));
    let scene_view = schema_page
        .scene_view
        .as_ref()
        .expect("scene3d schema should include parsed view payload");
    assert_eq!(scene_view.nodes[0].interactions.len(), 1);
    assert_eq!(schema_page.bindings3d.len(), 1);
    assert_eq!(schema_page.bindings3d[0].source, "Main.shaft_angle");
    assert!(validate_hmi_bindings("RESOURCE", &metadata, None, &descriptor).is_empty());

    std::fs::remove_dir_all(root).ok();
}

#[test]
fn scene3d_rejects_unknown_bind3d_property_with_clear_error() {
    let root = temp_dir("trust-runtime-hmi-scene3d-invalid-property");
    write_file(
        &root.join("hmi/cell.toml"),
        r#"
title = "Drive Cell 3D"
kind = "scene3d"
view = "drive-cell.view.toml"

[[bind3d]]
node = "motor-1.shaft"
property = "transform.skew.x"
source = "Main.shaft_angle"
"#,
    );

    let err = load_hmi_dir_impl(&root).expect_err("unsupported bind3d property must fail");
    let detail = err.to_string();
    assert!(
        detail.contains("unsupported bind3d property 'transform.skew.x'"),
        "unexpected error: {detail}"
    );

    std::fs::remove_dir_all(root).ok();
}

#[test]
fn validate_hmi_bindings_reports_unknown_bind3d_source() {
    let root = temp_dir("trust-runtime-hmi-scene3d-unknown-source");
    write_file(
        &root.join("hmi/cell.toml"),
        r#"
title = "Drive Cell 3D"
kind = "scene3d"
view = "drive-cell.view.toml"

[[bind3d]]
node = "motor-1.shaft"
property = "transform.rotation.y"
source = "Main.missing"
"#,
    );
    let descriptor = load_hmi_dir_impl(&root).expect("load descriptor");
    let source = r#"
PROGRAM Main
VAR
    shaft_angle : REAL := 0.0;
END_VAR
END_PROGRAM
"#;
    let metadata = metadata_for_source(source);

    let diagnostics = validate_hmi_bindings("RESOURCE", &metadata, None, &descriptor);
    assert!(diagnostics.iter().any(|diag| {
        diag.code == HMI_DIAG_UNKNOWN_BIND
            && diag.bind == "Main.missing"
            && diag.widget.as_deref() == Some("scene3d.bind3d")
    }));

    std::fs::remove_dir_all(root).ok();
}

#[test]
fn layout_overrides_keep_widget_ids_stable() {
    let root = temp_dir("trust-runtime-hmi-layout-stable");
    write_file(
        &root.join("hmi.toml"),
        r#"
[[pages]]
id = "controls"

[widgets."Main.run"]
page = "controls"
group = "Commands"
order = 10
"#,
    );

    let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
    let metadata = metadata_for_source(source);
    let source_path = root.join("sources/main.st");
    let source_refs = [HmiSourceRef {
        path: &source_path,
        text: source,
    }];
    let customization = load_customization(Some(&root), &source_refs);

    let baseline = build_schema("RESOURCE", &metadata, None, true, None);
    let customized = build_schema("RESOURCE", &metadata, None, true, Some(&customization));

    let baseline_map = baseline
        .widgets
        .iter()
        .map(|widget| (widget.path.clone(), widget.id.clone()))
        .collect::<BTreeMap<_, _>>();
    let customized_map = customized
        .widgets
        .iter()
        .map(|widget| (widget.path.clone(), widget.id.clone()))
        .collect::<BTreeMap<_, _>>();

    assert_eq!(baseline_map, customized_map);
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn theme_snapshot_uses_default_fallbacks() {
    let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
    let metadata = metadata_for_source(source);
    let schema = build_schema("RESOURCE", &metadata, None, true, None);
    let theme = serde_json::to_value(&schema.theme).expect("serialize theme");
    assert_eq!(
        theme,
        json!({
            "style": "classic",
            "accent": "#0f766e",
            "background": "#f3f5f8",
            "surface": "#ffffff",
            "text": "#142133"
        })
    );
}

#[test]
fn write_customization_parses_enabled_and_allowlist() {
    let root = temp_dir("trust-runtime-hmi-write-config");
    write_file(
        &root.join("hmi.toml"),
        r#"
[write]
enabled = true
allow = [" resource/RESOURCE/program/Main/field/run ", "", "Main.run"]
"#,
    );
    let source_refs: [HmiSourceRef<'_>; 0] = [];
    let customization = load_customization(Some(&root), &source_refs);
    assert!(customization.write_enabled());
    assert_eq!(customization.write_allowlist().len(), 2);
    assert!(customization.write_target_allowed("resource/RESOURCE/program/Main/field/run"));
    assert!(customization.write_target_allowed("Main.run"));
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn resolve_write_point_supports_id_and_path_matches() {
    let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
    let harness = TestHarness::from_source(source).expect("build harness");
    let metadata = harness.runtime().metadata_snapshot();
    let snapshot = crate::debug::DebugSnapshot {
        storage: harness.runtime().storage().clone(),
        now: harness.runtime().current_time(),
    };

    let by_id = resolve_write_point(
        "RESOURCE",
        &metadata,
        Some(&snapshot),
        "resource/RESOURCE/program/Main/field/run",
    )
    .expect("resolve id");
    assert_eq!(by_id.path, "Main.run");
    assert_eq!(
        resolve_write_value_template(&by_id, &snapshot),
        Some(Value::Bool(true))
    );

    let by_path = resolve_write_point("RESOURCE", &metadata, Some(&snapshot), "Main.run")
        .expect("resolve path");
    assert_eq!(by_path.id, "resource/RESOURCE/program/Main/field/run");
}
