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
    assert_eq!(speed.label, "speed");
    assert_eq!(speed.alarm_label.as_deref(), Some("Speed Alarm"));
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
fn allowlisted_hmi_write_targets_are_marked_writable_in_schema() {
    let root = temp_dir("trust-runtime-hmi-write-schema");
    write_file(
        &root.join("hmi/_config.toml"),
        r#"
[write]
enabled = true
allow = ["Main.speed"]
"#,
    );
    write_file(
        &root.join("hmi/control.toml"),
        r#"
title = "Setpoints"

[[section]]
title = "Allowed"
span = 12

[[section.widget]]
type = "slider"
bind = "Main.speed"
label = "Speed SP"
min = 0
max = 100

[[section.widget]]
type = "slider"
bind = "Main.pressure"
label = "Pressure SP"
min = 0
max = 10
"#,
    );
    let source = r#"
PROGRAM Main
VAR
    speed : REAL := 25.0;
    pressure : REAL := 3.0;
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

    assert!(!schema.read_only);
    assert_eq!(schema.mode, "read_write");

    let speed = schema
        .widgets
        .iter()
        .find(|widget| widget.path == "Main.speed")
        .expect("speed widget");
    assert_eq!(speed.access, "read_write");
    assert!(speed.writable);
    assert_eq!(speed.widget, "slider");

    let pressure = schema
        .widgets
        .iter()
        .find(|widget| widget.path == "Main.pressure")
        .expect("pressure widget");
    assert_eq!(pressure.access, "read");
    assert!(!pressure.writable);

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
