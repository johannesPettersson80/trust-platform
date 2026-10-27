/// Specification 39, §10: a page drawn first gets the variables of its value bindings declared in
/// the target program; equipment is left to Create PLC code, which declares and calls it inside
/// its regions. A second run adds nothing.
const ADD_SOURCE: &str = "FUNCTION_BLOCK Pump\nVAR_INPUT\n    AutoRun : BOOL;\nEND_VAR\nVAR_OUTPUT\n    Running : BOOL;\nEND_VAR\n    Running := AutoRun;\nEND_FUNCTION_BLOCK\n\nFUNCTION_BLOCK Sensor\nVAR_OUTPUT\n    Value : REAL;\nEND_VAR\nEND_FUNCTION_BLOCK\n\nPROGRAM Transfer\nVAR\n    Started : BOOL;\nEND_VAR\n    Started := TRUE;\nEND_PROGRAM\n\nCONFIGURATION Config\nRESOURCE Res ON PLC\nTASK Fast(INTERVAL := T#100ms, PRIORITY := 1);\nPROGRAM Main WITH Fast : Transfer;\nEND_RESOURCE\nEND_CONFIGURATION\n";

const ADD_PAGE: &str = "format = 1\ntitle = \"Overview\"\n\n[[object]]\nid = \"p-101\"\ntag = \"P-101\"\ntype = \"pump\"\nat = [40, 40]\nsize = [128, 108]\n\n[object.bind]\nrunning = \"Main.P101.Running\"\n\n[object.click]\nfaceplate = \"pump\"\n\n[[object]]\nid = \"ti-101\"\ntype = \"value\"\nat = [240, 40]\nsize = [160, 48]\n\n[object.bind]\nvalue = \"Main.TI101.Value\"\n\n[object.click]\nfaceplate = \"sensor\"\n\n[[object]]\nid = \"level\"\ntype = \"value\"\nat = [240, 120]\nsize = [160, 48]\n\n[object.bind]\nvalue = \"Main.Level\"\n\n[[object]]\nid = \"started\"\ntype = \"status\"\nat = [240, 200]\nsize = [160, 40]\n\n[object.bind]\nstate = \"Main.Started\"\n";

fn add_missing_project(name: &str, source: &str) -> (std::path::PathBuf, MockContext) {
    let root = temp_dir(name);
    for (path, text) in [
        ("src/main.st", source),
        ("hmi/panels/pump.toml", "format = 1\ntitle = \"Pump\"\nsize = [400, 300]\n\n[fields.Running]\naccepts = [\"boolean\"]\n"),
        ("hmi/pairs/pump.toml", "format = 1\nblock = \"Pump\"\nfaceplate = \"pump\"\nsymbol = \"pump\"\n\n[program]\nposition = \"actuators\"\n"),
        ("hmi/panels/sensor.toml", "format = 1\ntitle = \"Sensor\"\nsize = [400, 300]\n\n[fields.Value]\naccepts = [\"number\"]\n"),
        ("hmi/pairs/sensor.toml", "format = 1\nblock = \"Sensor\"\nfaceplate = \"sensor\"\nsymbol = \"value\"\n\n[program]\nposition = \"measurements\"\n"),
    ] {
        let file = root.join(path);
        std::fs::create_dir_all(file.parent().expect("a folder")).expect("folders");
        std::fs::write(file, text).expect("write");
    }
    let root_uri = Url::from_directory_path(&root).expect("root uri");
    let context = MockContext {
        workspace_configs: vec![(root_uri.clone(), test_project_config(root.to_string_lossy().as_ref(), "x86_64"))],
        workspace_folders: vec![root_uri],
        ..MockContext::default()
    };
    (root, context)
}

/// The text with LSP text edits applied (ASCII text: characters are bytes).
fn with_lsp_edits(text: &str, edits: &Value) -> String {
    let lines: Vec<usize> = std::iter::once(0)
        .chain(text.match_indices('\n').map(|(index, _)| index + 1))
        .collect();
    let offset = |position: &Value| {
        lines[position["line"].as_u64().expect("line") as usize] + position["character"].as_u64().expect("character") as usize
    };
    let mut edits: Vec<(usize, usize, String)> = edits
        .as_array()
        .map(|edits| {
            edits
                .iter()
                .map(|edit| (offset(&edit["range"]["start"]), offset(&edit["range"]["end"]), edit["newText"].as_str().expect("text").to_string()))
                .collect()
        })
        .unwrap_or_default();
    edits.sort_by_key(|edit| std::cmp::Reverse(edit.0));
    let mut out = text.to_string();
    for (start, end, new) in edits {
        out.replace_range(start..end, &new);
    }
    out
}

#[test]
fn hmi_add_missing_variables_declares_a_pages_values_and_leaves_equipment_to_create_plc_code() {
    let (root, context) = add_missing_project("trustlsp-hmi-add", ADD_SOURCE);
    let page = format!("{ADD_PAGE}\n[[object]]\nid = \"p-102\"\ntype = \"pump\"\nblock = \"Pump\"\ninstance = \"Main.P102\"\nat = [40, 300]\nsize = [128, 108]\n");
    let args = || vec![json!({ "text": page, "file": "hmi/pages/overview.toml" })];
    let result = hmi_program_edits::add_missing_variables_value(&context, args());
    assert_eq!(result["ok"], true, "{result}");
    let uri = Url::from_file_path(root.join("src/main.st")).expect("uri").to_string();
    let edited = with_lsp_edits(ADD_SOURCE, &result["edit"]["changes"][uri.as_str()]);
    assert!(edited.contains("VAR\n    Started : BOOL;\n    Level : LREAL;\nEND_VAR"), "{edited}");
    assert!(!edited.contains("P102"), "equipment is not declared here:\n{edited}");
    let skipped = result["skipped"].as_array().expect("skipped");
    assert!(
        skipped.iter().any(|entry| entry["object"] == "p-102"
            && entry["message"] == "equipment is declared and called by Create PLC code"),
        "{result}"
    );
    let mut harness = trust_runtime::harness::TestHarness::from_sources(&[edited.as_str()]).expect("the edited program compiles");
    harness.advance_time(trust_runtime::value::Duration::from_millis(100));
    assert!(harness.cycle().errors.is_empty(), "it runs");
    for path in ["Main.Level", "Main.Started"] {
        let parsed = trust_runtime::hmi::member_paths::MemberPath::parse(path).expect("a path");
        assert!(harness.runtime().with_program_view(|program| trust_runtime::hmi::member_paths::member_value(&parsed, program)).is_some(), "{path}");
    }
    std::fs::write(root.join("src/main.st"), &edited).expect("write the edited program");
    let again = hmi_program_edits::add_missing_variables_value(&context, args());
    assert_eq!(again["added"], json!([]), "{again}");
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn hmi_add_missing_variables_opens_a_var_section_when_the_program_has_none() {
    let source = ADD_SOURCE.replace("PROGRAM Transfer\nVAR\n    Started : BOOL;\nEND_VAR\n    Started := TRUE;\n", "PROGRAM Transfer\n");
    let page = ADD_PAGE.split("[[object]]\nid = \"started\"").next().expect("the page").to_string();
    let (root, context) = add_missing_project("trustlsp-hmi-add-var", &source);
    let result = hmi_program_edits::add_missing_variables_value(&context, vec![json!({ "text": page, "file": "hmi/pages/overview.toml" })]);
    assert_eq!(result["ok"], true, "{result}");
    let uri = Url::from_file_path(root.join("src/main.st")).expect("uri").to_string();
    let edited = with_lsp_edits(&source, &result["edit"]["changes"][uri.as_str()]);
    assert!(edited.contains("PROGRAM Transfer\nVAR\n    Level : LREAL;\nEND_VAR\nEND_PROGRAM"), "{edited}");
    std::fs::remove_dir_all(&root).ok();
}

/// Specification 39, §10.2, §10.3: Create PLC code plans every page once, writes one declaration
/// and one call per instance inside its regions, keeps the program's own code, repeats without
/// change, and refuses a region changed by hand. Planning writes no file.
#[test]
fn hmi_create_plc_code_plans_all_views_once_and_preserves_manual_code() {
    let source = ADD_SOURCE.replace("    Started := TRUE;", "    Started := MissingEngineeringFunction();");
    let (root, context) = add_missing_project("trustlsp-create-plc-code", &source);
    std::fs::create_dir_all(root.join("hmi/pages")).unwrap();
    let page = "format=1\ntitle=\"Overview\"\n[[object]]\nid=\"pump\"\ntype=\"pump\"\nblock=\"Pump\"\ninstance=\"Main.P101\"\nfaceplate=\"pump\"\nat=[80,80]\nsize=[128,108]\n";
    std::fs::write(root.join("hmi/pages/overview.toml"), page).unwrap();
    std::fs::write(root.join("hmi/pages/detail.toml"), page).unwrap();
    let discovery = hmi_bindings_value_with_context(&context, vec![json!({"authoring": true})]).unwrap();
    assert_eq!(discovery["inspectionMode"], "declarations");
    let main = discovery["variables"].as_array().unwrap().iter().find(|node| node["path"] == "Main").unwrap();
    assert_eq!(
        main["children"].as_array().unwrap().iter().find(|node| node["path"] == "Main.P101").unwrap()["pending"],
        true,
        "the equipment not created yet is already editable"
    );
    let generated = hmi_program_edits::add_missing_variables_value(&context, vec![json!({"generate_project": true})]);
    assert_eq!(generated["ok"], true, "{generated}");
    assert_eq!(std::fs::read_to_string(root.join("src/main.st")).unwrap(), source, "planning writes no file");
    let uri = Url::from_file_path(root.join("src/main.st")).unwrap().to_string();
    let edited = with_lsp_edits(&source, &generated["edit"]["changes"][&uri]);
    assert_eq!(edited.matches("    P101 : Pump;\n").count(), 1, "two views make one instance:\n{edited}");
    assert_eq!(edited.matches("    P101();\n").count(), 1, "and one call:\n{edited}");
    assert!(edited.contains("Started := MissingEngineeringFunction();"), "{edited}");
    assert!(edited.contains("(* truST plant: declarations sha256:"), "{edited}");
    let repeat = hmi_program_edits::add_missing_variables_value(&context, vec![json!({"generate_project": true, "open": {"src/main.st": edited}})]);
    assert_eq!(repeat["ok"], true, "{repeat}");
    assert!(repeat["edit"]["changes"].as_object().unwrap().is_empty(), "repeating changes nothing: {repeat}");
    let changed = edited.replace("    P101();\n", "    P101(AutoRun := FALSE);\n");
    let refused = hmi_program_edits::add_missing_variables_value(&context, vec![json!({"generate_project": true, "open": {"src/main.st": changed}})]);
    assert_eq!(refused["ok"], false, "a region changed by hand is never overwritten: {refused}");
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn hmi_create_plc_code_asks_for_a_control_cycle_and_the_result_compiles() {
    let source = ADD_SOURCE.split("PROGRAM Transfer").next().unwrap();
    let (root, context) = add_missing_project("trustlsp-create-plc-code-cycle", source);
    let page = "format=1\ntitle=\"Overview\"\n[[object]]\nid=\"pump\"\ntype=\"pump\"\nblock=\"Pump\"\ninstance=\"Line.P101\"\nfaceplate=\"pump\"\nat=[0,0]\nsize=[128,108]\n";
    let missing = hmi_program_edits::add_missing_variables_value(&context, vec![json!({"generate_project": true, "open": {"hmi/pages/overview.toml": page}})]);
    assert_eq!(missing["needsControlCycle"], true, "{missing}");
    let ready = hmi_program_edits::add_missing_variables_value(&context, vec![json!({"generate_project": true, "cycle_ms": 20, "open": {"hmi/pages/overview.toml": page}})]);
    assert_eq!(ready["ok"], true, "{ready}");
    let creates = ready["creates"].as_array().unwrap();
    assert!(creates.iter().any(|file| file["uri"].as_str().unwrap().ends_with("/src/Line.st")), "{ready}");
    assert!(creates.iter().any(|file| file["text"].as_str().unwrap().contains("INTERVAL := T#20ms")), "{ready}");
    let mut sources = vec![trust_runtime::harness::SourceFile::new(source)];
    for file in creates.iter().filter(|file| file["uri"].as_str().unwrap().ends_with(".st")) {
        sources.push(trust_runtime::harness::SourceFile::new(file["text"].as_str().unwrap()));
    }
    let session = trust_runtime::harness::CompileSession::from_sources(sources);
    let runtime = trust_runtime::harness::TestHarness::from_session(session).expect("the created PLC code compiles").into_runtime();
    let running = trust_runtime::hmi::member_paths::MemberPath::parse("Line.P101.Running").unwrap();
    assert!(runtime.with_program_view(|program| trust_runtime::hmi::member_paths::locate_member(&running, program)).is_some());
    std::fs::remove_dir_all(root).unwrap();
}

/// Specification 39, §21.3: the inspector offers only signals of the right direction and type;
/// connecting them changes `plant.toml` and the call in the generator's region at once, and the
/// result drives the real I/O.
#[test]
fn equipment_hardware_edits_validate_direction_and_wire_a_real_io_signal() {
    let source = ADD_SOURCE.replace("    Started : BOOL;", "    Started : BOOL;\n    Input AT %IX0.0 : BOOL;\n    Output AT %QX0.0 : BOOL;");
    let (root, context) = add_missing_project("trustlsp-equipment-hardware", &source);
    std::fs::write(root.join("hmi/pairs/pump.toml"), "format=1\nblock=\"Pump\"\nfaceplate=\"pump\"\nsymbol=\"pump\"\n[[io]]\nmember=\"AutoRun\"\ntitle=\"Run input\"\ndirection=\"input\"\n[[io]]\nmember=\"Running\"\ntitle=\"Running output\"\ndirection=\"output\"\n[program]\nposition=\"actuators\"\n").unwrap();
    std::fs::create_dir_all(root.join("hmi/pages")).unwrap();
    let page = "format=1\ntitle=\"Overview\"\n[[object]]\nid=\"pump\"\ntype=\"pump\"\nblock=\"Pump\"\ninstance=\"Main.P101\"\nfaceplate=\"pump\"\nat=[0,0]\nsize=[128,108]\n";
    std::fs::write(root.join("hmi/pages/overview.toml"), page).unwrap();
    let pending = hmi_equipment::inspect(&context, vec![json!({"instance": "Main.P101"})]);
    assert_eq!(pending["ok"], true, "a just-dropped instance is inspected before PLC code exists: {pending}");
    assert_eq!(pending["hardware"], "not-connected", "{pending}");
    let created = hmi_program_edits::add_missing_variables_value(&context, vec![json!({"generate_project": true})]);
    let uri = Url::from_file_path(root.join("src/main.st")).unwrap().to_string();
    let generated = with_lsp_edits(&source, &created["edit"]["changes"][&uri]);
    std::fs::write(root.join("src/main.st"), &generated).unwrap();
    let inspect = hmi_equipment::inspect(&context, vec![json!({"instance": "Main.P101"})]);
    assert_eq!(inspect["ok"], true, "{inspect}");
    assert_eq!(inspect["owned"], true, "the generator's instance is the builder's: {inspect}");
    assert_eq!(inspect["terminals"][0]["choices"][0]["path"], "Main.Input", "{inspect}");
    let configure = |signals: Value| hmi_equipment::inspect(&context, vec![json!({"instance": "Main.P101", "changes": {"signals": signals}})]);
    let invalid = configure(json!({"AutoRun": "Main.Output"}));
    assert_eq!(invalid["ok"], false, "an output cannot feed an input: {invalid}");
    let valid = configure(json!({"AutoRun": "Main.Input", "Running": "Main.Output"}));
    assert_eq!(valid["ok"], true, "{valid}");
    assert!(valid["creates"][0]["text"].as_str().unwrap().contains("AutoRun = \"Main.Input\""), "{valid}");
    let wired = with_lsp_edits(&generated, &valid["edit"]["changes"][&uri]);
    assert!(wired.contains("    P101(AutoRun := Input, Running => Output);\n"), "{wired}");
    let mut harness = trust_runtime::harness::TestHarness::from_source(&wired).expect("the wired code compiles");
    harness.runtime_mut().write_io(&trust_runtime::io::IoAddress::parse("%IX0.0").unwrap(), trust_runtime::value::Value::Bool(true)).unwrap();
    for _ in 0..2 {
        harness.advance_time(trust_runtime::value::Duration::from_millis(100));
        assert!(harness.cycle().errors.is_empty());
    }
    assert_eq!(harness.runtime().read_io(&trust_runtime::io::IoAddress::parse("%QX0.0").unwrap()).unwrap(), trust_runtime::value::Value::Bool(true));
    assert!(wired.contains("Started := TRUE;"), "the program's own logic is kept");
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn hmi_declaration_checks_keep_write_policy_without_compiling_unfinished_logic() {
    let source=ADD_SOURCE.replace("Started := TRUE;","Started := UnfinishedApplicationLogic();");
    let (root,context)=add_missing_project("trustlsp-static-write-policy",&source);
    let page="format=1\ntitle='Settings'\n[[object]]\nid='enabled'\ntype='switch'\nat=[0,0]\nsize=[160,48]\nrole='operator'\n[object.bind.value]\npath='Main.Started'\ndirection='read-write'\n";
    let denied=hmi_checks::check_value_with_context(&context,vec![json!({"authoring":true,"file":"hmi/pages/settings.toml","text":page})]);
    assert_eq!(denied["ok"],true,"declaration checks do not compile the invalid body: {denied}");
    assert!(!denied["problems"].as_array().unwrap().is_empty(),"absence of a write policy is still denied");
    // The editor's quick fix allows the point the problem names (specification 39, §12.2.3);
    // catches an authoring answer that drops it, which leaves the problem without its fix.
    assert_eq!(denied["problems"][0]["point"],json!("Main.Started"),"{denied}");
    std::fs::write(root.join("runtime.toml"),"[[runtime.web.hmi.write]]\npath='Main.Started'\nrole='operator'\n").unwrap();
    let allowed=hmi_checks::check_value_with_context(&context,vec![json!({"authoring":true,"file":"hmi/pages/settings.toml","text":page})]);
    assert_eq!(allowed["problems"],json!([]),"{allowed}");
    std::fs::remove_dir_all(root).unwrap();
}
