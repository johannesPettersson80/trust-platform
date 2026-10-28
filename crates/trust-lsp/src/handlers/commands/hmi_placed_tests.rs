const PLACED_PROGRAM: &str = "FUNCTION_BLOCK Pump\nVAR_OUTPUT\n    Running : BOOL;\nEND_VAR\nEND_FUNCTION_BLOCK\n\nFUNCTION_BLOCK Tank\nVAR_OUTPUT\n    Level : REAL;\nEND_VAR\nEND_FUNCTION_BLOCK\n\nPROGRAM Main\nVAR\n    P1 : Pump;\n    T1 : Tank;\n    P2 : Pump;\nEND_VAR\n    P1();\n    T1();\n    P2();\nEND_PROGRAM\n";

const PUMP_CARD: &str = "format = 1\ntitle = \"Pump card\"\nsize = [320, 160]\n\n[fields.Running]\naccepts = [\"boolean\"]\n\n[[object]]\nid = \"running\"\ntype = \"status\"\nat = [16, 16]\nsize = [160, 48]\n\n[object.bind]\nstate = \"$Running\"\n";

fn placed_context(name: &str) -> (PathBuf, Url, MockContext) {
    let root = temp_dir(name);
    std::fs::create_dir_all(root.join("src")).expect("create src dir");
    std::fs::write(root.join("src/main.st"), PLACED_PROGRAM).expect("write source");
    std::fs::create_dir_all(root.join("hmi/panels")).expect("create faceplates dir");
    std::fs::write(root.join("hmi/panels/pump-card.toml"), PUMP_CARD).expect("write faceplate");
    let root_uri = Url::from_directory_path(&root).expect("root uri");
    let context = MockContext {
        workspace_configs: vec![(root_uri.clone(), test_project_config(root.to_string_lossy().as_ref(), "x86_64"))],
        workspace_folders: vec![root_uri.clone()],
        ..MockContext::default()
    };
    (root, root_uri, context)
}

/// Specification 39, §8.9 item 1: placing a faceplate offers only the instances whose block has
/// every field the faceplate needs.
#[test]
fn hmi_faceplate_instances_lists_the_instances_a_faceplate_fits() {
    let (root, root_uri, context) = placed_context("trustlsp-hmi-placed-instances");
    let result = hmi_placed::instances_value_with_context(&context, vec![json!({ "faceplate": "pump-card", "root_uri": root_uri })]);
    assert_eq!(result["ok"], true, "{result}");
    assert_eq!(result["instances"], json!(["Main.P1", "Main.P2"]), "{result}");
    let missing = hmi_placed::instances_value_with_context(&context, vec![json!({ "faceplate": "gone", "root_uri": root_uri })]);
    assert_eq!(missing["ok"], false, "{missing}");
    std::fs::remove_dir_all(root).expect("remove temp dir");
}

/// Specification 39, §8.9 items 1 and 2: a placed faceplate's faceplate must exist and fit its
/// instance; each problem is at its key's line.
#[test]
fn hmi_file_check_checks_a_placed_faceplates_faceplate_and_fit() {
    let (root, _, context) = placed_context("trustlsp-hmi-placed-check");
    let text = "format = 1\ntitle = \"Cards\"\n\n[[object]]\nid = \"tank\"\ntype = \"faceplate\"\nfaceplate = \"pump-card\"\ninstance = \"Main.T1\"\nat = [0, 0]\nsize = [320, 204]\n\n[[object]]\nid = \"gone\"\ntype = \"faceplate\"\nfaceplate = \"gone-card\"\ninstance = \"Main.P1\"\nat = [0, 220]\nsize = [320, 204]\n\n[[object]]\nid = \"pump\"\ntype = \"faceplate\"\nfaceplate = \"pump-card\"\ninstance = \"Main.P1\"\nat = [0, 440]\nsize = [320, 204]\n";
    let result = hmi_checks::check_value_with_context(&context, vec![json!({ "text": text, "file": "hmi/pages/cards.toml" })]);
    assert_eq!(result["ok"], true, "{result}");
    let problems: Vec<(String, i64)> = result["problems"]
        .as_array()
        .expect("problems")
        .iter()
        .map(|problem| (problem["object"].as_str().unwrap_or_default().to_string(), problem["line"].as_i64().unwrap_or_default()))
        .collect();
    assert_eq!(problems, [("tank".to_string(), 8), ("gone".to_string(), 15)], "{result}");
    std::fs::remove_dir_all(root).expect("remove temp dir");
}

/// Specification 39, §8.9 item 3: the editor draws a placed faceplate bound to its instance
/// exactly as the display does, under the header naming the instance.
#[test]
fn hmi_placed_faceplates_binds_each_placement_as_the_display_does() {
    let (root, root_uri, context) = placed_context("trustlsp-hmi-placed-bound");
    let text = "format = 1\ntitle = \"Cards\"\n\n[[object]]\nid = \"card\"\ntype = \"faceplate\"\nfaceplate = \"pump-card\"\ninstance = \"Main.P2\"\nat = [0, 0]\nsize = [320, 204]\n\n[[object]]\nid = \"gone\"\ntype = \"faceplate\"\nfaceplate = \"gone-card\"\ninstance = \"Main.P1\"\nat = [0, 220]\nsize = [320, 204]\n";
    let result = hmi_placed::placed_value_with_context(&context, vec![json!({ "text": text, "file": "hmi/pages/cards.toml", "root_uri": root_uri })]);
    assert_eq!(result["ok"], true, "{result}");
    let model = &result["placed"]["card"]["model"];
    assert_eq!(model["title"], json!("P2"), "{result}");
    assert_eq!(model["size"], json!([320, 160]), "{result}");
    assert_eq!(model["object"][0]["bind"]["state"]["path"], json!("Main.P2.Running"), "{result}");
    assert!(result["placed"].get("gone").is_some_and(Value::is_null), "a faceplate the project lacks is drawn as such: {result}");
    std::fs::remove_dir_all(root).expect("remove temp dir");
}

/// Specification 39, §8.9 item 4: the faceplate list gives each faceplate's own size, which a
/// placement takes.
#[test]
fn hmi_faceplates_lists_each_faceplates_size() {
    let (root, root_uri, _) = placed_context("trustlsp-hmi-placed-size");
    let result = hmi_faceplates::faceplates_value(vec![json!({ "root_uri": root_uri })]);
    assert_eq!(result["faceplates"][0]["name"], json!("pump-card"), "{result}");
    assert_eq!(result["faceplates"][0]["size"], json!([320, 160]), "{result}");
    std::fs::remove_dir_all(root).expect("remove temp dir");
}


/// Specification 43: draft rendering and fitting use unsaved faceplate definitions.
#[test]
fn hmi_placed_faceplates_and_instance_picker_use_unsaved_templates() {
    let (root, root_uri, context) = placed_context("trustlsp-hmi-placed-unsaved");
    let text = "format = 1\ntitle = \"Cards\"\n[[object]]\nid = \"card\"\ntype = \"faceplate\"\nfaceplate = \"pump-card\"\ninstance = \"Main.P1\"\nat = [0, 0]\nsize = [320, 204]\n";
    let changed = PUMP_CARD.replace("type = \"status\"", "type = \"status\"\nlabel = \"Unsaved running\"");
    let result = hmi_placed::placed_value_with_context(&context, vec![json!({
        "root_uri": root_uri, "text": text,
        "open": {"hmi/panels/pump-card.toml": changed}
    })]);
    assert_eq!(result["ok"], true, "{result}");
    assert_eq!(result["placed"]["card"]["model"]["object"][0]["label"], "Unsaved running");
    let incompatible = PUMP_CARD.replace("Running", "MissingMember");
    let fitting = hmi_placed::instances_value_with_context(&context, vec![json!({
        "root_uri": root_uri, "faceplate": "pump-card",
        "open": {"hmi/panels/pump-card.toml": incompatible}
    })]);
    assert_eq!(fitting["ok"], true, "{fitting}");
    assert_eq!(fitting["instances"], json!([]), "{fitting}");
    assert_eq!(std::fs::read_to_string(root.join("hmi/panels/pump-card.toml")).unwrap(), PUMP_CARD);
    std::fs::remove_dir_all(root).unwrap();
}
