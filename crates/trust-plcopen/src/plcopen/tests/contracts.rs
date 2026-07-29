#[test]
fn contracts_export_target_identity_table_is_stable() {
    let cases = [
        (
            PlcopenExportTarget::Generic,
            "generic-plcopen",
            "generic",
            "Generic PLCopen XML",
            "generic",
        ),
        (
            PlcopenExportTarget::AllenBradley,
            "allen-bradley",
            "ab",
            "Allen-Bradley / Studio 5000",
            "allen-bradley",
        ),
        (
            PlcopenExportTarget::Siemens,
            "siemens-tia",
            "siemens",
            "Siemens TIA Portal",
            "siemens",
        ),
        (
            PlcopenExportTarget::Schneider,
            "schneider-ecostruxure",
            "schneider",
            "Schneider EcoStruxure",
            "schneider",
        ),
    ];

    for (target, id, suffix, label, serialized) in cases {
        assert_eq!(target.id(), id);
        assert_eq!(target.file_suffix(), suffix);
        assert_eq!(target.label(), label);
        assert_eq!(
            serde_json::to_value(target).expect("serialize export target"),
            serde_json::Value::String(serialized.to_string())
        );
    }
}

#[test]
fn contracts_import_option_defaults_and_explicit_modes_are_stable() {
    assert_eq!(
        PlcopenImportOptions::default().global_var_mode,
        PlcopenImportGlobalVarMode::NativeVendorParity
    );
    assert_eq!(
        PlcopenImportOptions {
            global_var_mode: PlcopenImportGlobalVarMode::StrictIecAdapter,
        }
        .global_var_mode,
        PlcopenImportGlobalVarMode::StrictIecAdapter
    );
}

#[test]
fn contracts_pou_type_alias_and_keyword_table_is_stable() {
    let cases = [
        ("program", PlcopenPouType::Program),
        ("prg", PlcopenPouType::Program),
        ("function", PlcopenPouType::Function),
        ("fc", PlcopenPouType::Function),
        ("fun", PlcopenPouType::Function),
        ("functionBlock", PlcopenPouType::FunctionBlock),
        ("function_block", PlcopenPouType::FunctionBlock),
        ("fb", PlcopenPouType::FunctionBlock),
    ];

    for (input, expected) in cases {
        assert_eq!(PlcopenPouType::from_xml(input), Some(expected), "{input}");
    }
    assert_eq!(PlcopenPouType::from_xml("unsupported"), None);

    assert_eq!(PlcopenPouType::Program.as_xml(), "program");
    assert_eq!(PlcopenPouType::Program.declaration_keyword(), "PROGRAM");
    assert_eq!(PlcopenPouType::Program.end_keyword(), "END_PROGRAM");
    assert_eq!(PlcopenPouType::Function.as_xml(), "function");
    assert_eq!(PlcopenPouType::Function.declaration_keyword(), "FUNCTION");
    assert_eq!(PlcopenPouType::Function.end_keyword(), "END_FUNCTION");
    assert_eq!(PlcopenPouType::FunctionBlock.as_xml(), "functionBlock");
    assert_eq!(
        PlcopenPouType::FunctionBlock.declaration_keyword(),
        "FUNCTION_BLOCK"
    );
    assert_eq!(
        PlcopenPouType::FunctionBlock.end_keyword(),
        "END_FUNCTION_BLOCK"
    );
}
