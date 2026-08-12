struct SemanticImportFixture {
    root: PathBuf,
    xml_path: PathBuf,
}

impl SemanticImportFixture {
    fn new(label: &str, types_xml: &str) -> Self {
        let root = temp_dir(&format!("plcopen-semantic-{label}"));
        let xml_path = root.join("input.xml");
        write(
            &xml_path,
            &format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="{PLCOPEN_NAMESPACE}">
  <types>
    {types_xml}
  </types>
</project>
"#
            ),
        );
        Self { root, xml_path }
    }

    fn import(&self) -> anyhow::Result<PlcopenImportReport> {
        import_xml_to_project(&self.xml_path, &self.root)
    }

    fn source(&self, name: &str) -> String {
        std::fs::read_to_string(self.root.join("src").join(name)).expect("read imported source")
    }

    fn migration_report(&self) -> serde_json::Value {
        let text = std::fs::read_to_string(
            self.root
                .join("interop")
                .join("plcopen-migration-report.json"),
        )
        .expect("read migration report");
        serde_json::from_str(&text).expect("parse migration report")
    }
}

impl Drop for SemanticImportFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn semantic_pou(name: &str, pou_type: &str, interface: &str, body: &str) -> String {
    format!(
        r#"<pous>
  <pou name="{name}" pouType="{pou_type}">
    <interface>{interface}</interface>
    {body}
  </pou>
</pous>"#
    )
}

fn semantic_st_body(source: &str) -> String {
    format!("<body><ST><![CDATA[{source}]]></ST></body>")
}

fn parse_type_fixture(xml: &str) -> Option<String> {
    let document = roxmltree::Document::parse(xml).expect("parse type fixture");
    parse_type_expression_container(document.root_element())
}

#[test]
fn semantic_pou_kind_aliases_are_case_and_punctuation_insensitive() {
    for spelling in ["program", "PROGRAM", "prg", "P-R_G"] {
        assert_eq!(
            PlcopenPouType::from_xml(spelling),
            Some(PlcopenPouType::Program),
            "{spelling}"
        );
    }
    for spelling in ["function", "FUNCTION", "fun", "FC", "f-c"] {
        assert_eq!(
            PlcopenPouType::from_xml(spelling),
            Some(PlcopenPouType::Function),
            "{spelling}"
        );
    }
    for spelling in ["functionBlock", "FUNCTION_BLOCK", "function-block", "fb"] {
        assert_eq!(
            PlcopenPouType::from_xml(spelling),
            Some(PlcopenPouType::FunctionBlock),
            "{spelling}"
        );
    }
    assert_eq!(PlcopenPouType::from_xml("class"), None);
}

#[test]
fn semantic_elementary_type_matrix_maps_to_canonical_st_names() {
    for name in [
        "bool", "byte", "word", "dword", "lword", "sint", "int", "dint", "lint", "usint",
        "uint", "udint", "ulint", "real", "lreal", "time", "ltime", "date", "ldate", "tod",
        "ltod", "dt", "ldt", "char", "wchar",
    ] {
        let xml = format!("<type><{name}/></type>");
        assert_eq!(
            parse_type_fixture(&xml),
            Some(name.to_ascii_uppercase()),
            "{name}"
        );
    }
}

#[test]
fn semantic_string_types_preserve_positive_declared_length() {
    assert_eq!(
        parse_type_fixture(r#"<type><string length="80"/></type>"#),
        Some("STRING[80]".to_string())
    );
    assert_eq!(
        parse_type_fixture(r#"<type><wstring maxLength="120"/></type>"#),
        Some("WSTRING[120]".to_string())
    );
    assert_eq!(
        parse_type_fixture("<type><string/></type>"),
        Some("STRING".to_string())
    );
}

#[test]
fn semantic_string_types_reject_zero_negative_and_nonnumeric_lengths() {
    for length in ["0", "-1", "many", "1 + 1"] {
        let xml = format!(r#"<type><string length="{length}"/></type>"#);
        assert_eq!(parse_type_fixture(&xml), None, "{length}");
    }
}

#[test]
fn semantic_derived_type_requires_nonblank_identity() {
    assert_eq!(
        parse_type_fixture(r#"<type><derived name="  Motor_State  "/></type>"#),
        Some("Motor_State".to_string())
    );
    assert_eq!(
        parse_type_fixture(r#"<type><derived name="   "/></type>"#),
        None
    );
    assert_eq!(parse_type_fixture("<type><derived/></type>"), None);
}

#[test]
fn semantic_array_type_preserves_all_dimensions_in_order() {
    let xml = r#"<type>
  <array>
    <dimension lower="-2" upper="2"/>
    <dimension lowerLimit="1" upperLimit="4"/>
    <baseType><derived name="Sample"/></baseType>
  </array>
</type>"#;

    assert_eq!(
        parse_type_fixture(xml),
        Some("ARRAY[-2..2, 1..4] OF Sample".to_string())
    );
}

#[test]
fn semantic_array_type_rejects_incomplete_dimensions_or_base_type() {
    for xml in [
        r#"<type><array><dimension lower="0"/><baseType><int/></baseType></array></type>"#,
        r#"<type><array><dimension upper="3"/><baseType><int/></baseType></array></type>"#,
        r#"<type><array><dimension lower="0" upper="3"/></array></type>"#,
        r#"<type><array><baseType><int/></baseType></array></type>"#,
    ] {
        assert_eq!(parse_type_fixture(xml), None, "{xml}");
    }
}

#[test]
fn semantic_struct_type_preserves_field_order_types_and_initializers() {
    let xml = r#"<type><struct>
  <variable name="Enabled">
    <type><bool/></type>
    <initialValue><simpleValue value="TRUE"/></initialValue>
  </variable>
  <member><name>Count</name><baseType><int/></baseType></member>
</struct></type>"#;

    assert_eq!(
        parse_type_fixture(xml),
        Some(
            "STRUCT\n    Enabled : BOOL := TRUE;\n    Count : INT;\nEND_STRUCT".to_string()
        )
    );
}

#[test]
fn semantic_struct_type_rejects_empty_or_partially_malformed_fields() {
    for xml in [
        "<type><struct/></type>",
        r#"<type><struct><variable name="MissingType"/></struct></type>"#,
        r#"<type><struct><variable><type><int/></type></variable></struct></type>"#,
    ] {
        assert_eq!(parse_type_fixture(xml), None, "{xml}");
    }
}

#[test]
fn semantic_enum_type_preserves_order_and_explicit_values() {
    let xml = r#"<type><enum><values>
  <value name="Idle" value="0"/>
  <value name="Run" value="4"/>
  <value name="Fault"/>
</values></enum></type>"#;

    assert_eq!(
        parse_type_fixture(xml),
        Some("(Idle := 0, Run := 4, Fault)".to_string())
    );
}

#[test]
fn semantic_enum_type_rejects_empty_and_duplicate_case_insensitive_names() {
    assert_eq!(
        parse_type_fixture("<type><enum><values/></enum></type>"),
        None
    );
    assert_eq!(
        parse_type_fixture(
            r#"<type><enum><values><value name="Run"/><value name="RUN"/></values></enum></type>"#
        ),
        None
    );
}

#[test]
fn semantic_subrange_supports_direct_and_nested_range_bounds() {
    assert_eq!(
        parse_type_fixture(
            r#"<type><subrange lower="-10" upper="10"><baseType><int/></baseType></subrange></type>"#
        ),
        Some("INT(-10..10)".to_string())
    );
    assert_eq!(
        parse_type_fixture(
            r#"<type><subrange><range lower="1" upper="8"/><type><uint/></type></subrange></type>"#
        ),
        Some("UINT(1..8)".to_string())
    );
}

#[test]
fn semantic_subrange_rejects_missing_bound_or_base_type() {
    for xml in [
        r#"<type><subrange lower="0"><baseType><int/></baseType></subrange></type>"#,
        r#"<type><subrange upper="9"><baseType><int/></baseType></subrange></type>"#,
        r#"<type><subrange lower="0" upper="9"/></type>"#,
    ] {
        assert_eq!(parse_type_fixture(xml), None, "{xml}");
    }
}

#[test]
fn semantic_interface_imports_all_supported_variable_sections_in_stable_order() {
    let interface = r#"
      <inputVars><variable name="In"><type><bool/></type></variable></inputVars>
      <outputVars><variable name="Out"><type><int/></type></variable></outputVars>
      <inOutVars><variable name="Shared"><type><dint/></type></variable></inOutVars>
      <externalVars><variable name="External"><type><real/></type></variable></externalVars>
      <localVars><variable name="Local"><type><word/></type></variable></localVars>
      <tempVars><variable name="Temp"><type><byte/></type></variable></tempVars>
      <globalVars><variable name="Global"><type><lint/></type></variable></globalVars>
    "#;
    let fixture = SemanticImportFixture::new(
        "all-interface-sections",
        &semantic_pou(
            "FB_All",
            "functionBlock",
            interface,
            &semantic_st_body("Out := 1;"),
        ),
    );

    fixture.import().expect("import interface");
    let source = fixture.source("FB_All.st");
    let headers = [
        "VAR_INPUT",
        "VAR_OUTPUT",
        "VAR_IN_OUT",
        "VAR_EXTERNAL",
        "VAR\n",
        "VAR_TEMP",
        "VAR_GLOBAL",
    ];
    let offsets = headers
        .iter()
        .map(|header| source.find(header).expect("section header"))
        .collect::<Vec<_>>();

    assert!(offsets.windows(2).all(|pair| pair[0] < pair[1]), "{source}");
    for declaration in [
        "In : BOOL;",
        "Out : INT;",
        "Shared : DINT;",
        "External : REAL;",
        "Local : WORD;",
        "Temp : BYTE;",
        "Global : LINT;",
    ] {
        assert!(source.contains(declaration), "{declaration}\n{source}");
    }
}

#[test]
fn semantic_interface_preserves_simple_initial_values() {
    let fixture = SemanticImportFixture::new(
        "interface-initializer",
        &semantic_pou(
            "Main",
            "program",
            r#"<localVars><variable name="Count"><type><int/></type><initialValue><simpleValue value="7"/></initialValue></variable></localVars>"#,
            &semantic_st_body("Count := Count + 1;"),
        ),
    );

    fixture.import().expect("import initializer");

    assert!(fixture.source("Main.st").contains("Count : INT := 7;"));
}

#[test]
fn semantic_interface_section_modifiers_map_in_declared_order() {
    let fixture = SemanticImportFixture::new(
        "section-modifiers",
        &semantic_pou(
            "Main",
            "program",
            r#"<localVars constant="true" retain="1" persistent="TRUE"><variable name="Count"><type><int/></type></variable></localVars>"#,
            &semantic_st_body("Count := Count;"),
        ),
    );

    fixture.import().expect("import modifiers");

    assert!(
        fixture
            .source("Main.st")
            .contains("VAR CONSTANT RETAIN PERSISTENT")
    );
}

#[test]
fn semantic_interface_rejects_contradictory_section_modifiers() {
    let fixture = SemanticImportFixture::new(
        "contradictory-modifiers",
        &semantic_pou(
            "Main",
            "program",
            r#"<localVars retain="true" nonretain="true"><variable name="Count"><type><int/></type></variable></localVars>"#,
            &semantic_st_body("Count := Count;"),
        ),
    );

    let error = fixture
        .import()
        .expect_err("contradictory modifiers must reject the POU");

    assert!(error.to_string().contains("no importable PLCopen ST content"));
    assert!(!contains_st_file(&fixture.root.join("src")));
}

#[test]
fn semantic_access_variables_map_readonly_and_default_readwrite() {
    let fixture = SemanticImportFixture::new(
        "access-vars",
        &semantic_pou(
            "Main",
            "program",
            r#"<accessVars>
  <accessVariable alias="View" instancePathAndName="Plant.Main.Value" direction="ReadOnly"><type><int/></type></accessVariable>
  <accessVariable alias="Edit" instancePathAndName="Plant.Main.Enable"><type><bool/></type></accessVariable>
</accessVars>"#,
            &semantic_st_body(""),
        ),
    );

    fixture.import().expect("import access variables");
    let source = fixture.source("Main.st");

    assert!(source.contains("View : Plant.Main.Value : INT READ_ONLY;"));
    assert!(source.contains("Edit : Plant.Main.Enable : BOOL READ_WRITE;"));
}

#[test]
fn semantic_interface_rejects_missing_required_variable_metadata_without_partial_source() {
    for (label, variable) in [
        (
            "missing-name",
            r#"<variable><type><int/></type></variable>"#,
        ),
        ("missing-type", r#"<variable name="Count"/>"#),
        (
            "unsupported-type",
            r#"<variable name="Count"><type><vendorMagic/></type></variable>"#,
        ),
    ] {
        let fixture = SemanticImportFixture::new(
            label,
            &semantic_pou(
                "Main",
                "program",
                &format!("<localVars>{variable}</localVars>"),
                &semantic_st_body(""),
            ),
        );

        let error = fixture
            .import()
            .expect_err("malformed declaration must reject the POU");
        assert!(error.to_string().contains("no importable PLCopen ST content"));
        assert!(!contains_st_file(&fixture.root.join("src")));
    }
}

#[test]
fn semantic_interface_rejects_duplicate_case_insensitive_declaration_names() {
    let fixture = SemanticImportFixture::new(
        "duplicate-vars",
        &semantic_pou(
            "Main",
            "program",
            r#"<localVars>
  <variable name="Count"><type><int/></type></variable>
  <variable name="COUNT"><type><dint/></type></variable>
</localVars>"#,
            &semantic_st_body("Count := 1;"),
        ),
    );

    let error = fixture
        .import()
        .expect_err("duplicate declarations must reject the POU");

    assert!(error.to_string().contains("no importable PLCopen ST content"));
    assert!(!contains_st_file(&fixture.root.join("src")));
}

#[test]
fn semantic_interface_rejects_invalid_st_identifiers() {
    let fixture = SemanticImportFixture::new(
        "invalid-identifiers",
        &semantic_pou(
            "Bad Name",
            "program",
            r#"<localVars><variable name="also-bad"><type><int/></type></variable></localVars>"#,
            &semantic_st_body(""),
        ),
    );

    let error = fixture
        .import()
        .expect_err("invalid identities must not publish invalid ST");

    assert!(error.to_string().contains("no importable PLCopen ST content"));
    assert!(!contains_st_file(&fixture.root.join("src")));
}

#[test]
fn semantic_case_insensitive_xml_tags_and_attributes_import_equivalently() {
    let fixture = SemanticImportFixture::new(
        "case-insensitive-xml",
        r#"<POUS>
  <POU NAME="Main" POUTYPE="PROGRAM">
    <INTERFACE><LOCALVARS><VARIABLE NAME="Count"><TYPE><INT/></TYPE></VARIABLE></LOCALVARS></INTERFACE>
    <BODY><st><![CDATA[Count := 1;]]></st></BODY>
  </POU>
</POUS>"#,
    );

    let report = fixture.import().expect("import mixed-case XML");

    assert_eq!(report.discovered_pous, 1);
    assert_eq!(report.imported_pous, 1);
    assert!(fixture.source("Main.st").contains("Count : INT;"));
}

#[test]
fn semantic_body_with_complete_matching_wrapper_is_preserved() {
    let source = "PROGRAM Main\nVAR x : INT; END_VAR\nx := 1;\nEND_PROGRAM\n";
    let fixture = SemanticImportFixture::new(
        "wrapped-body",
        &semantic_pou("Main", "program", "", &semantic_st_body(source)),
    );

    fixture.import().expect("import wrapped body");

    assert_eq!(fixture.source("Main.st"), source);
}

#[test]
fn semantic_statement_body_is_wrapped_with_interface_and_named_diagnostic() {
    let fixture = SemanticImportFixture::new(
        "statement-body",
        &semantic_pou(
            "Main",
            "program",
            r#"<localVars><variable name="Count"><type><int/></type></variable></localVars>"#,
            &semantic_st_body("Count := Count + 1;"),
        ),
    );

    let report = fixture.import().expect("import statement body");
    let source = fixture.source("Main.st");

    assert!(source.starts_with("PROGRAM Main\n"));
    assert!(source.contains("Count : INT;"));
    assert!(source.contains("Count := Count + 1;"));
    assert!(source.ends_with("END_PROGRAM\n"));
    assert!(report
        .unsupported_diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "PLCO207"));
}

#[test]
fn semantic_missing_body_with_interface_generates_declaration_shell() {
    let fixture = SemanticImportFixture::new(
        "interface-shell",
        &semantic_pou(
            "FB_State",
            "functionBlock",
            r#"<outputVars><variable name="Ready"><type><bool/></type></variable></outputVars>"#,
            "",
        ),
    );

    let report = fixture.import().expect("import declaration shell");
    let source = fixture.source("FB_State.st");

    assert!(source.starts_with("FUNCTION_BLOCK FB_State\n"));
    assert!(source.contains("Ready : BOOL;"));
    assert!(source.ends_with("END_FUNCTION_BLOCK\n"));
    assert!(report
        .unsupported_diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "PLCO208"));
}

#[test]
fn semantic_function_uses_structured_return_type_and_inserts_result_fallback() {
    let fixture = SemanticImportFixture::new(
        "function-return",
        &semantic_pou(
            "Calculate",
            "function",
            r#"<returnType><lreal/></returnType><inputVars><variable name="Value"><type><real/></type></variable></inputVars>"#,
            &semantic_st_body("Value := Value;"),
        ),
    );

    let report = fixture.import().expect("import function");
    let source = fixture.source("Calculate.st");

    assert!(source.starts_with("FUNCTION Calculate : LREAL\n"));
    assert!(source.contains("Calculate := Calculate;"));
    assert!(report
        .unsupported_diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "PLCO212"));
}

#[test]
fn semantic_function_missing_return_type_defaults_to_int_with_named_diagnostic() {
    let fixture = SemanticImportFixture::new(
        "function-default-return",
        &semantic_pou(
            "Calculate",
            "function",
            r#"<inputVars><variable name="Value"><type><int/></type></variable></inputVars>"#,
            &semantic_st_body("Calculate := Value;"),
        ),
    );

    let report = fixture.import().expect("import function");

    assert!(fixture
        .source("Calculate.st")
        .starts_with("FUNCTION Calculate : INT\n"));
    assert!(report
        .unsupported_diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "PLCO211"));
}

#[test]
fn semantic_function_comment_does_not_fake_result_assignment() {
    let fixture = SemanticImportFixture::new(
        "function-comment-result",
        &semantic_pou(
            "Calculate",
            "function",
            r#"<returnType><int/></returnType>"#,
            &semantic_st_body("// Calculate := 1;\nOther := 2;"),
        ),
    );

    let report = fixture.import().expect("import function");

    assert!(fixture
        .source("Calculate.st")
        .contains("Calculate := Calculate;"));
    assert!(report
        .unsupported_diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "PLCO212"));
}

#[test]
fn semantic_function_real_result_assignment_suppresses_fallback() {
    let fixture = SemanticImportFixture::new(
        "function-real-result",
        &semantic_pou(
            "Calculate",
            "function",
            r#"<returnType><int/></returnType>"#,
            &semantic_st_body("calculate := 42;"),
        ),
    );

    let report = fixture.import().expect("import function");
    let source = fixture.source("Calculate.st");

    assert_eq!(source.matches("calculate := 42;").count(), 1);
    assert!(!source.contains("Calculate := Calculate;"));
    assert!(report
        .unsupported_diagnostics
        .iter()
        .all(|diagnostic| diagnostic.code != "PLCO212"));
}

#[test]
fn semantic_program_referenced_as_type_is_promoted_to_function_block() {
    let fixture = SemanticImportFixture::new(
        "program-promotion",
        r#"<pous>
  <pou name="Reusable" pouType="program">
    <interface><localVars><variable name="Count"><type><int/></type></variable></localVars></interface>
    <body><ST><![CDATA[Count := Count + 1;]]></ST></body>
  </pou>
  <pou name="Main" pouType="program">
    <interface><localVars><variable name="Instance"><type><derived name="Reusable"/></type></variable></localVars></interface>
    <body><ST><![CDATA[Instance();]]></ST></body>
  </pou>
</pous>"#,
    );

    let report = fixture.import().expect("import promoted program");

    assert!(fixture
        .source("Reusable.st")
        .starts_with("FUNCTION_BLOCK Reusable\n"));
    assert!(report.unsupported_diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "PLCO210" && diagnostic.pou.as_deref() == Some("Reusable")
    }));
}

#[test]
fn semantic_supported_st_plus_non_st_body_fails_closed() {
    let fixture = SemanticImportFixture::new(
        "mixed-body",
        r#"<pous><pou name="Main" pouType="program">
  <body>
    <ST><![CDATA[PROGRAM Main
END_PROGRAM
]]></ST>
    <LD><network/></LD>
  </body>
</pou></pous>"#,
    );

    let error = fixture
        .import()
        .expect_err("mixed executable body must not choose silently");
    let report = fixture.migration_report();

    assert!(error.to_string().contains("no importable PLCopen ST content"));
    assert_eq!(report["imported_pous"], 0);
    assert_eq!(report["skipped_pous"], 1);
    assert!(report["unsupported_diagnostics"]
        .as_array()
        .expect("diagnostics")
        .iter()
        .any(|diagnostic| diagnostic["code"] == "PLCO216"));
}

#[test]
fn semantic_codesys_method_plaintext_header_and_interface_are_preserved() {
    let fixture = SemanticImportFixture::new(
        "method-plaintext",
        r#"<pous><pou name="FB_Counter" pouType="functionBlock">
  <body><ST><![CDATA[FUNCTION_BLOCK FB_Counter
END_FUNCTION_BLOCK
]]></ST></body>
  <addData><data name="http://www.3s-software.com/plcopenxml/method">
    <Method name="Reset">
      <interface><inputVars><variable name="Hard"><type><bool/></type></variable></inputVars></interface>
      <body><ST><![CDATA[IF Hard THEN
END_IF]]></ST></body>
      <addData><data name="http://www.3s-software.com/plcopenxml/interfaceasplaintext"><text>METHOD PRIVATE Reset : BOOL</text></data></addData>
    </Method>
  </data></addData>
</pou></pous>"#,
    );

    fixture.import().expect("import CODESYS method");
    let source = fixture.source("FB_Counter.st");

    assert!(source.contains("METHOD PRIVATE Reset : BOOL"));
    assert!(source.contains("VAR_INPUT"));
    assert!(source.contains("Hard : BOOL;"));
    assert!(source.contains("END_METHOD"));
    assert!(
        source.find("METHOD PRIVATE").expect("method")
            < source
                .rfind("END_FUNCTION_BLOCK")
                .expect("function block end")
    );
}

#[test]
fn semantic_codesys_method_without_header_defaults_public_with_diagnostic() {
    let fixture = SemanticImportFixture::new(
        "method-default-header",
        r#"<pous><pou name="FB_Counter" pouType="functionBlock">
  <interface/>
  <addData><data name="plcopenxml/method">
    <Method name="Reset">
      <interface><returnType><bool/></returnType></interface>
      <body><ST><![CDATA[Reset := TRUE;]]></ST></body>
    </Method>
  </data></addData>
</pou></pous>"#,
    );

    let report = fixture.import().expect("import defaulted method");

    assert!(fixture
        .source("FB_Counter.st")
        .contains("METHOD PUBLIC Reset : BOOL"));
    assert!(report.unsupported_diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "PLCO214" && diagnostic.pou.as_deref() == Some("FB_Counter")
    }));
}

#[test]
fn semantic_codesys_method_on_program_is_skipped_with_named_diagnostic() {
    let fixture = SemanticImportFixture::new(
        "method-wrong-owner",
        r#"<pous><pou name="Main" pouType="program">
  <body><ST><![CDATA[PROGRAM Main
END_PROGRAM
]]></ST></body>
  <addData><data name="plcopenxml/method">
    <Method name="Reset"><body><ST><![CDATA[]]></ST></body></Method>
  </data></addData>
</pou></pous>"#,
    );

    let report = fixture.import().expect("import program without method");
    let source = fixture.source("Main.st");

    assert!(!source.contains("METHOD"));
    assert!(report
        .unsupported_diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "PLCO213"));
}

#[test]
fn semantic_codesys_method_already_present_is_not_duplicated() {
    let fixture = SemanticImportFixture::new(
        "method-dedup",
        r#"<pous><pou name="FB_Counter" pouType="functionBlock">
  <body><ST><![CDATA[FUNCTION_BLOCK FB_Counter
METHOD PUBLIC Reset
END_METHOD
END_FUNCTION_BLOCK
]]></ST></body>
  <addData><data name="plcopenxml/method">
    <Method name="RESET">
      <body><ST><![CDATA[]]></ST></body>
      <addData><data name="interfaceasplaintext"><text>METHOD PUBLIC RESET</text></data></addData>
    </Method>
  </data></addData>
</pou></pous>"#,
    );

    fixture.import().expect("import method metadata");
    let source = fixture.source("FB_Counter.st");

    assert_eq!(
        source
            .lines()
            .filter(|line| line.trim().to_ascii_uppercase().starts_with("METHOD PUBLIC RESET"))
            .count(),
        1,
        "{source}"
    );
}

#[test]
fn semantic_data_type_import_rejects_empty_struct_instead_of_publishing_invalid_st() {
    let fixture = SemanticImportFixture::new(
        "empty-struct",
        r#"<dataTypes><dataType name="Empty"><baseType><struct/></baseType></dataType></dataTypes>"#,
    );

    let error = fixture
        .import()
        .expect_err("empty structure must be unsupported");

    assert!(error.to_string().contains("no importable PLCopen ST content"));
    assert!(!contains_st_file(&fixture.root.join("src")));
}

#[test]
fn semantic_data_type_import_rejects_duplicate_enum_elements() {
    let fixture = SemanticImportFixture::new(
        "duplicate-enum",
        r#"<dataTypes><dataType name="Mode"><baseType><enum><values><value name="Run"/><value name="RUN"/></values></enum></baseType></dataType></dataTypes>"#,
    );

    let error = fixture
        .import()
        .expect_err("duplicate enumeration identity must be unsupported");

    assert!(error.to_string().contains("no importable PLCopen ST content"));
    assert!(!contains_st_file(&fixture.root.join("src")));
}
