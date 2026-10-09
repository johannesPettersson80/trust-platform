/// Specification 39, §4.9: a popup's bindings are checked as a page's are.
#[test]
fn hmi_file_check_checks_a_popups_bindings() {
    let root = temp_dir("trustlsp-hmi-popup");
    std::fs::create_dir_all(root.join("src")).expect("create src dir");
    std::fs::write(root.join("src/main.st"), "PROGRAM Main\nVAR\n    Level : REAL;\nEND_VAR\nEND_PROGRAM\n").expect("write source");
    let root_uri = Url::from_directory_path(&root).expect("root uri");
    let context = MockContext {
        workspace_configs: vec![(root_uri.clone(), test_project_config(root.to_string_lossy().as_ref(), "x86_64"))],
        workspace_folders: vec![root_uri],
        ..MockContext::default()
    };
    let text = "format = 1\ntitle = \"Level\"\nsize = [320, 200]\n\n[[object]]\nid = \"gone\"\ntype = \"value\"\nat = [0, 0]\nsize = [160, 48]\n\n[object.bind]\nvalue = \"Main.Gone\"\n";
    let result = hmi_checks::check_value_with_context(&context, vec![json!({ "text": text, "file": "hmi/popups/level.toml" })]);
    assert_eq!(result["ok"], true, "{result}");
    let problems = result["problems"].as_array().expect("problems");
    assert_eq!(problems.len(), 1, "{result}");
    assert_eq!((&problems[0]["object"], &problems[0]["line"]), (&json!("gone"), &json!(12)), "{result}");
}

/// Specification 39, §12.2.3: a control that needs more than the runtime's write policy
/// allows is an error in the editor, on its binding's line.
#[test]
fn hmi_file_check_reports_controls_outside_the_write_policy() {
    let root = temp_dir("trustlsp-hmi-policy");
    std::fs::create_dir_all(root.join("src")).expect("create src dir");
    std::fs::write(
        root.join("src/main.st"),
        "PROGRAM Main\nVAR\n    Setpoint : REAL;\n    Valve : REAL;\nEND_VAR\nEND_PROGRAM\n",
    )
    .expect("write source");
    std::fs::write(
            root.join("runtime.toml"),
            "[[runtime.web.hmi.write]]\npath = \"Main.Setpoint\"\nrole = \"operator\"\nmin = 0.0\nmax = 120.0\n",
        )
        .expect("write runtime.toml");
    let root_uri = Url::from_directory_path(&root).expect("root uri");
    let context = MockContext {
        workspace_configs: vec![(
            root_uri.clone(),
            test_project_config(root.to_string_lossy().as_ref(), "x86_64"),
        )],
        workspace_folders: vec![root_uri],
        ..MockContext::default()
    };
    let text = "format = 1\ntitle = \"Policy\"\n\n[[object]]\nid = \"sp\"\ntype = \"setpoint\"\nat = [0, 0]\nsize = [200, 48]\n\n[object.bind]\nvalue = { path = \"Main.Setpoint\", direction = \"read-write\" }\n\n[[object]]\nid = \"valve\"\ntype = \"setpoint\"\nat = [0, 60]\nsize = [200, 48]\n\n[object.bind]\nvalue = { path = \"Main.Valve\", direction = \"read-write\" }\n";
    let result = hmi_checks::check_value_with_context(
        &context,
        vec![json!({ "text": text, "file": "hmi/pages/policy.toml" })],
    );
    assert_eq!(result["ok"], true, "{result}");
    let problems = result["problems"].as_array().expect("problems");
    assert_eq!(problems.len(), 1, "{result}");
    assert_eq!(
        (
            &problems[0]["object"],
            &problems[0]["property"],
            &problems[0]["line"]
        ),
        (&json!("valve"), &json!("value"), &json!(20))
    );
    assert!(problems[0]["message"]
        .as_str()
        .is_some_and(|message| message.contains("write policy")));
    std::fs::remove_dir_all(root).expect("remove temp dir");
}

    /// Specification 39, §16.2.1 and §16.4: the editor check builds the bundle from the project
    /// and lists its problems before anything is sent.
    #[test]
    fn hmi_bundle_builds_the_project_bundle_and_checks_it() {
        let root = temp_dir("trustlsp-hmi-bundle");
        std::fs::create_dir_all(root.join("src")).expect("create src dir");
        std::fs::create_dir_all(root.join("hmi/pages")).expect("create pages dir");
        std::fs::write(root.join("src/main.st"), "PROGRAM Main\nVAR\n    Level : REAL;\nEND_VAR\nEND_PROGRAM\n").expect("write source");
        std::fs::write(root.join("hmi/screen.toml"), "format = 1\nsize = [1280, 720]\ntheme = \"dark\"\nsymbols = \"flat\"\n").expect("screen");
        std::fs::write(
            root.join("hmi/pages/overview.toml"),
            "format = 1\ntitle = \"Overview\"\n\n[[object]]\nid = \"level\"\ntype = \"value\"\nat = [0, 0]\nsize = [160, 48]\n\n[object.bind]\nvalue = \"Main.Gone\"\n",
        )
        .expect("page");
        let root_uri = Url::from_directory_path(&root).expect("root uri");
        let context = MockContext {
            workspace_configs: vec![(root_uri.clone(), test_project_config(root.to_string_lossy().as_ref(), "x86_64"))],
            workspace_folders: vec![root_uri.clone()],
            ..MockContext::default()
        };
        let result = hmi_checks::bundle_value_with_context(&context, vec![json!({ "root_uri": root_uri })]);
        assert_eq!(result["ok"], true, "{result}");
        assert_eq!(result["bundle"]["format"], 1, "{result}");
        assert!(result["bundle"]["files"]["hmi/pages/overview.toml"].is_string(), "{result}");
        assert_eq!(result["version"], result["bundle"]["version"], "{result}");
        let problems = result["problems"].as_array().expect("problems");
        assert_eq!(problems.len(), 1, "{result}");
        assert_eq!((&problems[0]["file"], &problems[0]["line"]), (&json!("hmi/pages/overview.toml"), &json!(11)));
        assert!(problems[0]["message"].as_str().is_some_and(|message| message.contains("Main.Gone")));
        std::fs::remove_dir_all(root).expect("remove temp dir");
    }

/// Specification 39, §8.7 item 1: a faceplate binds its fields, each checked as the field's kinds;
/// its other bindings are checked against the program, and a control bound to a field is checked
/// against the write policy only when the runtime knows its instance.
#[test]
fn hmi_file_check_checks_a_faceplates_field_bindings_by_their_kinds() {
    let root = temp_dir("trustlsp-hmi-faceplate");
    std::fs::create_dir_all(root.join("src")).expect("create src dir");
    std::fs::write(root.join("src/main.st"), "PROGRAM Main\nVAR\n    Level : REAL;\nEND_VAR\nEND_PROGRAM\n").expect("write source");
    let root_uri = Url::from_directory_path(&root).expect("root uri");
    let context = MockContext {
        workspace_configs: vec![(root_uri.clone(), test_project_config(root.to_string_lossy().as_ref(), "x86_64"))],
        workspace_folders: vec![root_uri],
        ..MockContext::default()
    };
    let text = "format = 1\ntitle = \"Pump\"\nsize = [400, 300]\n\n[fields.Speed]\naccepts = [\"boolean\"]\n\n[fields.Start]\naccepts = [\"boolean\"]\n\n[[object]]\nid = \"speed\"\ntype = \"gauge\"\nat = [0, 0]\nsize = [160, 160]\n\n[object.bind]\nvalue = \"$Speed\"\n\n[[object]]\nid = \"start\"\ntype = \"switch\"\nat = [0, 200]\nsize = [96, 48]\n\n[object.bind]\nvalue = { path = \"$Start\", direction = \"read-write\" }\n\n[[object]]\nid = \"gone\"\ntype = \"value\"\nat = [200, 0]\nsize = [160, 48]\n\n[object.bind]\nvalue = \"Main.Gone\"\n";
    for authoring in [false, true] {
    let result = hmi_checks::check_value_with_context(&context, vec![json!({ "text": text, "file": "hmi/panels/pump.toml", "authoring": authoring })]);
    assert_eq!(result["ok"], true, "{result}");
    let problems: Vec<(String, i64)> = result["problems"]
        .as_array()
        .expect("problems")
        .iter()
        .map(|problem| (problem["message"].as_str().unwrap_or_default().to_string(), problem["line"].as_i64().unwrap_or_default()))
        .collect();
    assert_eq!(problems.len(), 2, "{result}");
    assert_eq!(problems[0], ("the field `Speed` takes a boolean; a gauge's value needs a number".to_string(), 18));
    assert!(problems[1].0.contains("Main.Gone"), "{result}");
    }
    std::fs::remove_dir_all(root).expect("remove temp dir");
}

/// Specification 39, §8.6: a pair is checked against the compiled program: a field whose member
/// the block does not have is an error at the field's line, and a block the program does not have
/// at the `block` line.
#[test]
fn hmi_file_check_checks_a_pair_against_the_program() {
    let root = temp_dir("trustlsp-hmi-pair");
    std::fs::create_dir_all(root.join("src")).expect("create src dir");
    std::fs::create_dir_all(root.join("hmi/panels")).expect("create faceplates dir");
    std::fs::write(
        root.join("src/main.st"),
        "FUNCTION_BLOCK Pump\nVAR_OUTPUT\n    Running : BOOL;\nEND_VAR\nEND_FUNCTION_BLOCK\n\nPROGRAM Main\nVAR\n    P101 : Pump;\nEND_VAR\n    P101();\nEND_PROGRAM\n",
    )
    .expect("write source");
    std::fs::write(
        root.join("hmi/panels/pump.toml"),
        "format = 1\ntitle = \"Pump\"\nsize = [400, 300]\n\n[fields.Running]\naccepts = [\"boolean\"]\n\n[fields.Speed]\naccepts = [\"number\"]\n",
    )
    .expect("write faceplate");
    let root_uri = Url::from_directory_path(&root).expect("root uri");
    let context = MockContext {
        workspace_configs: vec![(root_uri.clone(), test_project_config(root.to_string_lossy().as_ref(), "x86_64"))],
        workspace_folders: vec![root_uri],
        ..MockContext::default()
    };
    let check = |text: &str| -> Vec<(String, i64)> {
        let result = hmi_checks::check_value_with_context(&context, vec![json!({ "text": text, "file": "hmi/pairs/pump.toml" })]);
        assert_eq!(result["ok"], true, "{result}");
        result["problems"]
            .as_array()
            .expect("problems")
            .iter()
            .map(|problem| (problem["message"].as_str().unwrap_or_default().to_string(), problem["line"].as_i64().unwrap_or_default()))
            .collect()
    };
    let pair = "format = 1\nblock = \"Pump\"\nfaceplate = \"pump\"\nsymbol = \"pump\"\n\n[fields]\nSpeed = \"Gone\"\n";
    assert_eq!(check(pair), vec![("`Pump` has no member `Gone`, which the faceplate's field `Speed` needs".to_string(), 7)]);
    let declared = hmi_checks::check_value_with_context(&context, vec![json!({"text":pair,"file":"hmi/pairs/pump.toml","authoring":true})]);
    assert_eq!(declared["ok"], true, "{declared}");
    assert_eq!(declared["problems"].as_array().unwrap().len(), 1, "{declared}");
    assert_eq!(declared["problems"][0]["line"], 7);
    assert!(declared["problems"][0]["message"].as_str().unwrap().contains("Gone"));
    let nowhere = pair.replace("block = \"Pump\"", "block = \"Nope\"");
    assert_eq!(check(&nowhere), vec![("the program has no function block or program `Nope`".to_string(), 2)]);
    std::fs::remove_dir_all(root).expect("remove temp dir");
}
