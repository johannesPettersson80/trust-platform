/// Specification 39, §9: Create pages from program proposes an overview page with a symbol for each
/// instance whose block has an equipment pair, bound to the instance and opening its faceplate,
/// and changes nothing in the program.
#[test]
fn hmi_propose_page_places_the_programs_equipment_with_their_faceplates() {
    let source = ADD_SOURCE.replace(
        "    Started : BOOL;\n",
        "    Started : BOOL;\n    P101 : Pump;\n    TI101 : Sensor;\n    Level : REAL;\n",
    );
    let (root, context) = add_missing_project("trustlsp-hmi-propose", &source);
    let result = hmi_page_proposal::propose_page_value(&context, vec![json!({ "root_uri": Url::from_directory_path(&root).expect("uri") })]);
    assert_eq!(result["ok"], true, "{result}");
    let text = result["text"].as_str().expect("the page's text");
    let page = hmi_authoring::documents::page("hmi/pages/overview.toml", text, false).expect("the page loads").0;
    let summary: Vec<(String, String, Option<String>)> = page
        .objects
        .iter()
        .map(|object| (object.kind.clone(), object.tag.clone().unwrap_or_default(), object.bind.values().next().map(|binding| binding.path.clone())))
        .collect();
    assert_eq!(
        summary,
        vec![
            ("value".to_string(), "TI101".to_string(), Some("Main.TI101.Value".to_string())),
            ("pump".to_string(), "P101".to_string(), Some("Main.P101.Running".to_string())),
        ],
        "measurements in the first row, actuators in the last: {text}"
    );
    assert!(page.objects[0].at[1] < page.objects[1].at[1], "{text}");
    assert_eq!(page.objects[1].block.as_deref(), Some("Pump"), "{text}");
    assert_eq!(page.objects[1].faceplate.as_deref(), Some("pump"), "{text}");
    assert!(
        page.objects.iter().all(|object| object.click.is_none()),
        "the equipment opens its panel through `faceplate`, with no click of its own: {text}"
    );
    // The proposal checks clean against the program, which is unchanged.
    let check = hmi_checks::check_value_with_context(&context, vec![json!({ "text": text, "file": "hmi/pages/overview.toml" })]);
    assert_eq!(check["problems"], json!([]), "{check}");
    assert_eq!(std::fs::read_to_string(root.join("src/main.st")).expect("the program"), source);
    std::fs::remove_dir_all(&root).ok();
}

/// Specification 39, §9 item 4: Create settings page offers an entry for each retained variable
/// of a program, for engineers, and lists what no control can set yet.
#[test]
fn hmi_propose_settings_page_offers_an_entry_for_each_retained_setting() {
    let source = ADD_SOURCE.replace(
        "PROGRAM Transfer\nVAR\n    Started : BOOL;\nEND_VAR\n",
        "TYPE FillMode : (Gentle, Fast); END_TYPE\n\nPROGRAM Transfer\nVAR\n    Started : BOOL;\nEND_VAR\nVAR RETAIN\n    HighLevel : REAL := 2.5;\n    PumpEnabled : BOOL;\n    Mode : FillMode;\n    FillTime : TIME;\nEND_VAR\n",
    );
    let (root, context) = add_missing_project("trustlsp-hmi-settings", &source);
    std::fs::write(root.join("runtime.toml"), "[[runtime.web.hmi.write]]\npath = \"Main.*\"\nrole = \"engineer\"\n").expect("write runtime.toml");
    let result = hmi_page_proposal::propose_settings_page_value(
        &context,
        vec![json!({ "root_uri": Url::from_directory_path(&root).expect("uri"), "program": "Main" })],
    );
    assert_eq!(result["ok"], true, "{result}");
    let text = result["text"].as_str().expect("the page's text");
    let page = trust_runtime::hmi::page_files::parse_page("hmi/pages/settings.toml", text).expect("the page loads");
    let entries: Vec<(String, String, String, bool)> = page
        .objects
        .iter()
        .map(|object| {
            let binding = object.bind.get(if object.kind == "mode-selector" { "mode" } else { "value" }).expect("a value binding");
            (
                object.kind.clone(),
                object.label.clone().unwrap_or_default(),
                binding.path.clone(),
                binding.direction == trust_runtime::hmi::page_files::HmiBindingDirection::ReadWrite
                    && object.role == Some(trust_runtime::hmi::page_files::HmiRole::Engineer),
            )
        })
        .collect();
    assert_eq!(
        entries,
        vec![
            ("numeric-input".to_string(), "High level".to_string(), "Main.HighLevel".to_string(), true),
            ("switch".to_string(), "Pump enabled".to_string(), "Main.PumpEnabled".to_string(), true),
            ("selector".to_string(), "Mode".to_string(), "Main.Mode".to_string(), true),
        ],
        "{text}"
    );
    assert_eq!(
        result["skipped"],
        json!([{ "variable": "Main.FillTime", "message": "no control sets a time yet" }]),
        "{result}"
    );
    let check = hmi_checks::check_value_with_context(&context, vec![json!({ "text": text, "file": "hmi/pages/settings.toml" })]);
    assert_eq!(check["problems"], json!([]), "{check}");
    std::fs::remove_dir_all(&root).ok();
}

/// Specification 39, §18 Phase 2 "program first": pages created from the example's program, with
/// pumps, a control valve, a tank and a PID loop, bind every symbol and open each faceplate.
#[test]
fn hmi_propose_page_from_the_equipment_example_places_its_pump_valve_loop_and_tank() {
    let example = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/hmi_equipment");
    let root = temp_dir("trustlsp-hmi-program-first");
    copy_tree(&example, &root);
    if root.join("hmi/pages").is_dir() {std::fs::remove_dir_all(root.join("hmi/pages")).unwrap();}
    let root_uri = Url::from_directory_path(&root).expect("root uri");
    let context = MockContext {
        workspace_configs: vec![(root_uri.clone(), test_project_config(root.to_string_lossy().as_ref(), "x86_64"))],
        workspace_folders: vec![root_uri.clone()],
        ..MockContext::default()
    };
    let result = hmi_page_proposal::propose_page_value(&context, vec![json!({ "root_uri": root_uri })]);
    assert_eq!(result["ok"], true, "{result}");
    let text = result["text"].as_str().expect("the page's text");
    let page = hmi_authoring::documents::page("hmi/pages/overview.toml", text, false).expect("the page loads").0;
    let placed: Vec<(String, String, String)> = page
        .objects
        .iter()
        .map(|object| {
            let faceplate = object.faceplate.clone().unwrap_or_default();
            (object.tag.clone().unwrap_or_default(), object.kind.clone(), faceplate)
        })
        .collect();
    assert_eq!(
        placed,
        vec![
            ("LT101".to_string(), "level-sensor".to_string(), "plant-measurement".to_string()),
            ("T101".to_string(), "tank".to_string(), "plant-tank".to_string()),
            ("LIC101".to_string(), "controller".to_string(), "plant-pid".to_string()),
            ("P101".to_string(), "pump".to_string(), "plant-run".to_string()),
            ("FV101".to_string(), "control-valve".to_string(), "plant-position".to_string()),
        ],
        "measurements, then controllers, then actuators: {text}"
    );
    // Each symbol shows the member its definition names: the tank's level in percent, the loop's
    // measurement and setpoint, variables beside its Control.PID (§10.5).
    let bound = |id: &str, property: &str| {
        page.objects
            .iter()
            .find(|object| object.tag.as_deref().is_some_and(|tag|tag.eq_ignore_ascii_case(id)))
            .and_then(|object| object.bind.get(property))
            .map(|binding| binding.path.clone())
    };
    assert_eq!(bound("t101", "level").as_deref(), Some("Main.T101.Percent"), "{text}");
    assert_eq!(bound("lic101", "pv").as_deref(), Some("Main.LIC101_Input.ProcessValue"), "{text}");
    assert_eq!(bound("lic101", "sp").as_deref(), Some("Main.LIC101_Setpoint"), "{text}");
    let check = hmi_checks::check_value_with_context(&context, vec![json!({ "text": text, "file": "hmi/pages/overview.toml" })]);
    assert_eq!(check["problems"], json!([]), "{check}");
    std::fs::remove_dir_all(&root).ok();
}

fn copy_tree(from: &std::path::Path, to: &std::path::Path) {
    std::fs::create_dir_all(to).expect("a folder");
    for entry in std::fs::read_dir(from).expect("readable").filter_map(Result::ok) {
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).expect("copied");
        }
    }
}
