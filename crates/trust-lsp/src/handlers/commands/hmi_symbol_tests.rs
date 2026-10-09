/// Specification 39, §14.4: the project's own symbols reach the editor with the catalogue, and the
/// checks know them.
const SYMBOL_DEFINITION: &str = "format = 1\ntitle = \"Agitator\"\nsize = [128, 108]\n\n[properties]\nrunning = \"boolean\"\n\n[[part]]\nid = \"rotor\"\nspin = \"running\"\n";
const SYMBOL_DRAWING: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 64 64\"><g id=\"rotor\"><rect width=\"8\" height=\"30\"/></g></svg>";

fn symbol_project(name: &str) -> (std::path::PathBuf, Url) {
    let root = temp_dir(name);
    std::fs::create_dir_all(root.join("src")).expect("create src dir");
    std::fs::create_dir_all(root.join("hmi/symbols")).expect("create symbols dir");
    std::fs::write(root.join("src/main.st"), "PROGRAM Main\nVAR\n    Running : BOOL;\nEND_VAR\nEND_PROGRAM\n").expect("write source");
    std::fs::write(root.join("hmi/symbols/agitator.toml"), SYMBOL_DEFINITION).expect("write definition");
    std::fs::write(root.join("hmi/symbols/agitator.svg"), SYMBOL_DRAWING).expect("write drawing");
    let root_uri = Url::from_directory_path(&root).expect("root uri");
    (root, root_uri)
}

#[test]
fn hmi_catalogue_brings_the_projects_own_symbols() {
    let (_root, root_uri) = symbol_project("trustlsp-hmi-symbols-catalogue");
    let answer = hmi_catalogue::catalogue_value(vec![json!({ "root_uri": root_uri.as_str() })]);
    assert_eq!(answer["ok"], true, "{answer}");
    let components = answer["catalogue"]["components"].as_array().expect("components");
    let symbol = components.iter().find(|component| component["type"] == "my:agitator").expect("the symbol is a component");
    assert_eq!(symbol["group"], "my-symbols", "{symbol}");
    assert!(components.iter().any(|component| component["type"] == "pump"), "the shipped components stay");
    assert_eq!(answer["symbols"][0]["name"], "agitator", "{answer}");
    assert_eq!(answer["symbols"][0]["parts"][0], json!({ "part": "rotor", "spin": "running" }));
    let shipped = hmi_catalogue::catalogue_value(Vec::new());
    assert!(shipped["catalogue"]["components"].as_array().expect("components").iter().all(|component| component["type"] != "my:agitator"));
}

#[test]
fn hmi_file_check_knows_the_projects_symbols_and_checks_their_definitions() {
    let (root, root_uri) = symbol_project("trustlsp-hmi-symbols-check");
    let context = MockContext {
        workspace_configs: vec![(root_uri.clone(), test_project_config(root.to_string_lossy().as_ref(), "x86_64"))],
        workspace_folders: vec![root_uri],
        ..MockContext::default()
    };
    let page = "format = 1\ntitle = \"Mixing\"\n\n[[object]]\nid = \"mixer\"\ntype = \"my:agitator\"\nat = [0, 0]\nsize = [128, 108]\n\n[object.bind]\nrunning = \"Main.Running\"\n";
    let result = hmi_checks::check_value_with_context(&context, vec![json!({ "text": page, "file": "hmi/pages/mixing.toml" })]);
    assert_eq!((&result["ok"], &result["problems"]), (&json!(true), &json!([])), "{result}");
    let broken = SYMBOL_DEFINITION.replace("id = \"rotor\"", "id = \"stirrer\"");
    let result = hmi_checks::check_value_with_context(&context, vec![json!({ "text": broken, "file": "hmi/symbols/agitator.toml" })]);
    assert_eq!(result["ok"], true, "{result}");
    let problems = result["problems"].as_array().expect("problems");
    assert_eq!(problems.len(), 1, "{result}");
    assert_eq!(problems[0]["line"], 9, "{result}");
    assert!(problems[0]["message"].as_str().unwrap_or_default().contains("no element \"stirrer\""), "{result}");
}
