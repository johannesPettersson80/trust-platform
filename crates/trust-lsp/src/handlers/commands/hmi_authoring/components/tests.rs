use super::*;

fn plant_assets() -> HmiBundle {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../libraries/plant");
    let mut bundle = HmiBundle::default();
    for folder in ["pairs", "panels"] {
        for entry in std::fs::read_dir(root.join("hmi").join(folder)).unwrap() {
            let entry = entry.unwrap();
            if entry.path().extension().is_some_and(|s| s == "toml") {
                bundle.files.insert(
                    format!("hmi/{folder}/{}", entry.file_name().to_string_lossy()),
                    std::fs::read(entry.path()).unwrap(),
                );
            }
        }
    }
    bundle
}

#[test]
fn public_equipment_instruments_controllers_and_services_have_component_definitions() {
    let (definitions, errors) = equipment_definitions(&plant_assets(), Catalogue::shipped());
    assert!(errors.is_empty(), "{errors:?}");
    for name in [
        "Pump",
        "VariableSpeedPump",
        "DosingPump",
        "Motor",
        "VariableSpeedMotor",
        "ReversingMotor",
        "TwoSpeedMotor",
        "Fan",
        "VariableSpeedFan",
        "Blower",
        "Agitator",
        "Conveyor",
        "OnOffValve",
        "MotorValve",
        "ControlValve",
        "Damper",
        "ModulatingDamper",
        "Cylinder",
        "Heater",
        "Tank",
        "AnalogInput",
        "FlowInput",
        "PressureInput",
        "TemperatureInput",
        "LevelInput",
        "AnalyzerInput",
        "WeighScale",
        "DigitalInput",
        "LevelSwitch",
        "PressureSwitch",
        "Counter",
        "Totalizer",
        "OnOffController",
        "DutyStandbyGroup",
        "LeadLagGroup",
        "StagedGroup",
        "AlarmManager",
        "Sequence",
        "SequenceManager",
        "Interlocks",
        "Permissives",
        "Protection",
    ] {
        let block = format!("Plant.FB_{name}");
        assert!(
            definitions.iter().any(|d| d.block == block),
            "Missing visual definition for {block}"
        );
    }
    // A PID loop is ordinary code around Control.PID (specification 39, §10.5).
    assert!(
        definitions.iter().any(|d| d.block == "Control.PID"),
        "Missing visual definition for the PID controller, Control.PID"
    );
}

#[test]
fn all_stock_panel_fields_resolve_against_the_existing_compiler_metadata() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../libraries/plant");
    let mut sources =
        trust_runtime::bundle_builder::collect_project_source_files(&root, None).unwrap();
    sources.push(trust_runtime::harness::SourceFile::new(
        "PROGRAM HmiAssetContract END_PROGRAM",
    ));
    let runtime = trust_runtime::harness::CompileSession::from_sources(sources)
        .declarations()
        .unwrap();
    let bundle = plant_assets();
    for (file, text) in &bundle.files {
        if !file.starts_with("hmi/pairs/") {
            continue;
        }
        let pair = parse_pair(file, &String::from_utf8_lossy(text)).unwrap();
        let panel = super::super::bundle::faceplate(&bundle, &pair.faceplate)
            .unwrap()
            .unwrap();
        let problems =
            trust_runtime::hmi::binding_checks::check_pair(&pair, &panel, runtime.metadata());
        assert!(problems.is_empty(), "{file}: {problems:?}");
        let (panel_file, bytes) =
            super::super::bundle::faceplate_text(&bundle, &pair.faceplate).unwrap();
        let model = super::super::documents::parse(
            "faceplate",
            panel_file,
            &String::from_utf8_lossy(bytes),
        )
        .unwrap();
        let problems = super::super::command_checks::check(runtime.metadata(), &pair, &model);
        assert!(problems.is_empty(), "{file} commands: {problems:?}");
        if pair.block == "Plant.FB_Pump" {
            let mut invalid = model.clone();
            let control = invalid["object"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|o| o.get("command").is_some())
                .unwrap();
            control["command"]["method"] = serde_json::json!("MissingMethod");
            assert!(
                super::super::command_checks::check(runtime.metadata(), &pair, &invalid)
                    .iter()
                    .any(|(_, message)| message.contains("MissingMethod"))
            );
            let control = invalid["object"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|o| o.get("command").is_some())
                .unwrap();
            control["command"]["interface"] = serde_json::json!("Customer.Unimplemented");
            assert!(
                super::super::command_checks::check(runtime.metadata(), &pair, &invalid)
                    .iter()
                    .any(|(_, message)| message.contains("does not implement"))
            );
        }
    }
}

#[test]
fn a_customer_definition_uses_the_same_parser_and_missing_panels_are_visible_errors() {
    let mut bundle = HmiBundle::default();
    bundle.files.insert("hmi/pairs/custom.toml".into(),b"format=1\ntitle='Customer pump'\nblock='Customer.SpecialPump'\nfaceplate='custom'\nsymbol='pump'\n".to_vec());
    let (definitions, errors) = equipment_definitions(&bundle, Catalogue::shipped());
    assert!(definitions.is_empty());
    assert_eq!(errors.len(), 1);
    bundle.files.insert(
        "hmi/panels/custom.toml".into(),
        b"format=1\ntitle='Customer pump'\nsize=[400,300]\n".to_vec(),
    );
    let (definitions, errors) = equipment_definitions(&bundle, Catalogue::shipped());
    assert!(errors.is_empty());
    assert_eq!(definitions[0].block, "Customer.SpecialPump");
}
