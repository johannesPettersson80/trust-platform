// Specification 35 §17.1 and §17.17: Control Studio's compiled evidence lists a block's inherited
// members, and Keep in project writes applied parameter values into the project.

fn control_project(name: &str, source: &str) -> (std::path::PathBuf, MockContext) {
    let root = temp_dir(name);
    std::fs::create_dir_all(root.join("src")).expect("create src dir");
    std::fs::write(root.join("src").join("main.st"), source).expect("write source");
    let root_uri = Url::from_directory_path(&root).expect("root uri");
    let context = MockContext {
        workspace_configs: vec![(
            root_uri.clone(),
            test_project_config(root.to_string_lossy().as_ref(), "x86_64"),
        )],
        workspace_folders: vec![root_uri],
        ..MockContext::default()
    };
    (root, context)
}

#[test]
fn control_symbols_list_the_members_a_block_inherits_as_its_own() {
    let (root, context) = control_project(
        "trustlsp-control-inherited",
        r#"
FUNCTION_BLOCK AnalogInput
VAR_INPUT Raw : LREAL; END_VAR
VAR_OUTPUT Value : LREAL; END_VAR
Value := Raw;
END_FUNCTION_BLOCK

FUNCTION_BLOCK TemperatureInput EXTENDS AnalogInput
VAR_OUTPUT Alarm : BOOL; END_VAR
END_FUNCTION_BLOCK

PROGRAM Main
VAR TT101 : TemperatureInput; END_VAR
TT101();
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
END_CONFIGURATION
"#,
    );
    let result = hmi_bindings_value_with_context(&context, Vec::new())
        .expect("compiled bindings response");
    let symbols = result["controlSymbols"]["symbols"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let listed = |symbol: &str| symbols.iter().any(|entry| entry["symbol"] == symbol);
    assert!(
        listed("P1.TT101.Value") && listed("P1.TT101.Raw") && listed("P1.TT101.Alarm"),
        "the inherited Value and Raw are TT101's own members beside its Alarm: {symbols:?}"
    );
    let controllers = result["controlSymbols"]["controllers"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert_eq!(
        controllers
            .iter()
            .filter(|entry| entry["instance"] == "P1.TT101")
            .map(|entry| entry["type"].clone())
            .collect::<Vec<_>>(),
        vec![json!("TemperatureInput")],
        "the instance is listed once, as the derived block; its base is not a second instance"
    );
    std::fs::remove_dir_all(root).expect("remove temp dir");
}

const KEEP_LESSON: &str = "PROGRAM Main\nVAR\n    Controller : Control.PID;\n    Parameters : Control.PIDParameters := (\n        ProportionalGain := LREAL#1.0,\n        IntegralGain := LREAL#0.2,\n        DerivativeGain := 0.0\n    );\n    Gains : Control.PIDParameters;\nEND_VAR\nController.Configure(Parameters := Parameters);\nEND_PROGRAM\n\nCONFIGURATION Config\nRESOURCE Res ON PLC\n    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);\n    PROGRAM P1 WITH MainTask : Main;\nEND_RESOURCE\nEND_CONFIGURATION\n";

fn keep_value(context: &MockContext, root: &std::path::Path, record: &str, values: Value) -> Value {
    control_keep::keep(
        context,
        vec![json!({
            "root_uri": Url::from_directory_path(root).expect("root uri"),
            "record": record,
            "values": values,
        })],
    )
}

#[test]
fn keep_in_project_sets_the_applied_values_in_a_hand_written_initializer() {
    let (root, context) = control_project("trustlsp-control-keep", KEEP_LESSON);
    let answer = keep_value(
        &context,
        &root,
        "P1.Parameters",
        json!({ "ProportionalGain": 2.5, "IntegralGain": 0.125, "Bias": 1.0 }),
    );
    assert_eq!(answer["ok"], true, "{answer}");
    let uri = Url::from_file_path(root.join("src").join("main.st"))
        .expect("source uri")
        .to_string();
    assert_eq!(
        answer["expected"][&uri],
        json!(KEEP_LESSON),
        "the edit is made against the text the language server read"
    );
    let kept = with_lsp_edits(KEEP_LESSON, &answer["edit"]["changes"][&uri]);
    assert!(
        kept.contains("ProportionalGain := LREAL#2.5,")
            && kept.contains("IntegralGain := LREAL#0.125,")
            && kept.contains("DerivativeGain := 0.0, Bias := 1.0\n    );"),
        "typed literals stay typed, the other fields stay as written and a missing one is added: {kept}"
    );
    assert!(kept.contains("Gains : Control.PIDParameters;"), "{kept}");
    std::fs::remove_dir_all(root).expect("remove temp dir");
}

#[test]
fn keep_in_project_gives_a_record_without_initializer_one() {
    let (root, context) = control_project("trustlsp-control-keep-new", KEEP_LESSON);
    let answer = keep_value(&context, &root, "P1.Gains", json!({ "ProportionalGain": 3.0 }));
    let uri = Url::from_file_path(root.join("src").join("main.st"))
        .expect("source uri")
        .to_string();
    let kept = with_lsp_edits(KEEP_LESSON, &answer["edit"]["changes"][&uri]);
    assert!(
        kept.contains("Gains : Control.PIDParameters := (ProportionalGain := 3.0);"),
        "{answer} {kept}"
    );
    let nested = keep_value(&context, &root, "P1.Controller.ParametersValue", json!({ "ProportionalGain": 3.0 }));
    assert_eq!(nested["ok"], false, "{nested}");
    assert!(
        nested["errors"][0]["message"]
            .as_str()
            .is_some_and(|message| message.contains("declared in a program")),
        "a record inside an instance is not edited: {nested}"
    );
    std::fs::remove_dir_all(root).expect("remove temp dir");
}

/// The parser keeps a comment inside the node of the name or value it touches; names and edit ranges
/// are the code's own, so the comments stay where they are written.
#[test]
fn keep_in_project_edits_a_record_whose_names_and_values_stand_beside_comments() {
    const COMMENTED: &str = "PROGRAM Main\nVAR\n    (* The oven controller. *)\n    Parameters : Control.PIDParameters := (\n        (* gain *) ProportionalGain := LREAL#1.0 (* first try *),\n        IntegralGain := 0.2 (* slow *)\n    );\nEND_VAR\nEND_PROGRAM\n\nCONFIGURATION Config\nRESOURCE Res ON PLC\n    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);\n    PROGRAM P1 WITH MainTask : Main;\nEND_RESOURCE\nEND_CONFIGURATION\n";
    let (root, context) = control_project("trustlsp-control-keep-comments", COMMENTED);
    let answer = keep_value(&context, &root, "P1.Parameters", json!({ "ProportionalGain": 2.5, "Bias": 1.0 }));
    assert_eq!(answer["ok"], true, "{answer}");
    let uri = Url::from_file_path(root.join("src").join("main.st"))
        .expect("source uri")
        .to_string();
    let kept = with_lsp_edits(COMMENTED, &answer["edit"]["changes"][&uri]);
    assert!(
        kept.contains("(* gain *) ProportionalGain := LREAL#2.5 (* first try *),")
            && kept.contains("IntegralGain := 0.2, Bias := 1.0 (* slow *)\n    );"),
        "{kept}"
    );
    std::fs::remove_dir_all(root).expect("remove temp dir");
}

#[test]
fn keep_in_project_names_the_equipment_settings_of_a_record_create_plc_code_declared() {
    const GENERATED: &str = "USING Control;\nPROGRAM Main\nVAR\n    (* truST plant: declarations sha256:0 *)\n    TIC101 : Control.PID;\n    (* end of truST plant declarations *)\nEND_VAR\nVAR RETAIN\n    (* truST plant: retained declarations sha256:0 *)\n    TIC101_Parameters : Control.PIDParameters := (ProportionalGain := 1.0, IntegralGain := 0.1);\n    (* end of truST plant retained declarations *)\nEND_VAR\n    (* truST plant: calls sha256:0 *)\n    TIC101.Configure(Parameters := TIC101_Parameters);\n    (* end of truST plant calls *)\nEND_PROGRAM\n";
    let (root, context) = control_project("trustlsp-control-keep-generated", GENERATED);
    let answer = keep_value(
        &context,
        &root,
        "Main.TIC101_Parameters",
        json!({ "ProportionalGain": 33.3, "IntegralGain": 0.0667 }),
    );
    assert_eq!(
        answer,
        json!({
            "ok": false,
            "generated": {
                "instance": "Main.TIC101",
                "settings": {
                    "{instance}_Parameters.IntegralGain": 0.0667,
                    "{instance}_Parameters.ProportionalGain": 33.3
                }
            }
        }),
        "the generated declaration stays the generator's own; the equipment edit keeps the values"
    );
    std::fs::remove_dir_all(root).expect("remove temp dir");
}

/// A program with REAL and LREAL variables, an INT variable and a declaration that names two variables.
const SCALARS: &str = "PROGRAM Main\nVAR\n    Kp : LREAL := LREAL#1.0;\n    Ki : LREAL;\n    Kd : REAL := 0.5;\n    Count : INT := 3;\n    Kx, Ky : LREAL := 1.0;\nEND_VAR\nEND_PROGRAM\n\nCONFIGURATION Config\nRESOURCE Res ON PLC\n    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);\n    PROGRAM P1 WITH MainTask : Main;\nEND_RESOURCE\nEND_CONFIGURATION\n";

/// The sentence of a refused keep.
fn refusal_of(answer: &Value) -> &str {
    answer["errors"][0]["message"].as_str().unwrap_or_default()
}

#[test]
fn keep_in_project_writes_a_program_variables_initial_values_and_nothing_else() {
    // C2, B6.5: the gain variables of a program instance take their values as initial values. A typed initializer keeps its
    // prefix, a variable without one gets an initializer, and the other declarations stay as written.
    let (root, context) = control_project("trustlsp-control-keep-scalars", SCALARS);
    let answer = keep_value(
        &context,
        &root,
        "P1",
        json!({ "Kp": 2.5, "Ki": 0.125, "Kd": 3.0 }),
    );
    assert_eq!(answer["ok"], true, "{answer}");
    assert_eq!(answer["kept"], json!(["Kd", "Ki", "Kp"]), "{answer}");
    let uri = Url::from_file_path(root.join("src").join("main.st"))
        .expect("source uri")
        .to_string();
    let kept = with_lsp_edits(SCALARS, &answer["edit"]["changes"][&uri]);
    assert!(
        kept.contains("    Kp : LREAL := LREAL#2.5;\n")
            && kept.contains("    Ki : LREAL := 0.125;\n")
            && kept.contains("    Kd : REAL := 3.0;\n")
            && kept.contains("    Count : INT := 3;\n")
            && kept.contains("    Kx, Ky : LREAL := 1.0;\n"),
        "{kept}"
    );
    std::fs::remove_dir_all(root).expect("remove temp dir");
}

#[test]
fn keep_in_project_refuses_a_program_variable_it_cannot_write_by_its_name() {
    let (root, context) = control_project("trustlsp-control-keep-scalar-refusals", SCALARS);
    let not_real = keep_value(&context, &root, "P1", json!({ "Count": 1.0 }));
    assert_eq!(not_real["ok"], false, "{not_real}");
    assert!(
        refusal_of(&not_real).starts_with("P1.Count is not a REAL or LREAL variable"),
        "{not_real}"
    );
    let shared = keep_value(&context, &root, "P1", json!({ "Kx": 2.0 }));
    assert!(
        refusal_of(&shared).starts_with("P1.Kx shares its declaration with other variables"),
        "{shared}"
    );
    let missing = keep_value(&context, &root, "P1", json!({ "Gain": 2.0 }));
    assert!(
        refusal_of(&missing).starts_with("P1.Gain is not declared in PROGRAM Main"),
        "{missing}"
    );
    std::fs::remove_dir_all(root).expect("remove temp dir");
}

#[test]
fn a_scalar_named_as_a_record_takes_its_own_name_as_its_one_value() {
    let (root, context) = control_project("trustlsp-control-keep-scalar-record", SCALARS);
    let answer = keep_value(&context, &root, "P1.Kp", json!({ "Kp": 2.5 }));
    assert_eq!(answer["ok"], true, "{answer}");
    let uri = Url::from_file_path(root.join("src").join("main.st"))
        .expect("source uri")
        .to_string();
    let kept = with_lsp_edits(SCALARS, &answer["edit"]["changes"][&uri]);
    assert!(kept.contains("    Kp : LREAL := LREAL#2.5;\n"), "{kept}");
    let two = keep_value(&context, &root, "P1.Kp", json!({ "Kp": 2.5, "Ki": 1.0 }));
    assert!(
        refusal_of(&two).starts_with("P1.Kp keeps one value, its own."),
        "{two}"
    );
    let other = keep_value(&context, &root, "P1.Kp", json!({ "Ki": 1.0 }));
    assert!(
        refusal_of(&other).starts_with("P1.Kp keeps only its own value, not Ki."),
        "{other}"
    );
    let integer = keep_value(&context, &root, "P1.Count", json!({ "Count": 1.0 }));
    assert!(
        refusal_of(&integer).starts_with("P1.Count is not a record"),
        "{integer}"
    );
    std::fs::remove_dir_all(root).expect("remove temp dir");
}
