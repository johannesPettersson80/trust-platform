fn export_loaded(path: &str, text: &str) -> LoadedSource {
    LoadedSource {
        path: PathBuf::from(path),
        text: text.to_string(),
    }
}

struct SemanticExportFixture {
    root: PathBuf,
    output: PathBuf,
}

impl SemanticExportFixture {
    fn new(label: &str) -> Self {
        let root = temp_dir(&format!("plcopen-semantic-export-{label}"));
        std::fs::create_dir_all(root.join("src")).expect("create export src");
        let output = root.join("out/project.xml");
        Self { root, output }
    }

    fn write_source(&self, relative: &str, text: &str) {
        write(&self.root.join("src").join(relative), text);
    }

    fn export(&self) -> anyhow::Result<PlcopenExportReport> {
        export_project_to_xml(&self.root, &self.output)
    }

    fn xml(&self) -> String {
        std::fs::read_to_string(&self.output).expect("read exported XML")
    }

    fn source_map(&self) -> serde_json::Value {
        let path = self.output.with_extension("source-map.json");
        let text = std::fs::read_to_string(path).expect("read source map");
        serde_json::from_str(&text).expect("parse source map")
    }
}

impl Drop for SemanticExportFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn semantic_export_pou_extraction_supports_three_standard_kinds() {
    let source = export_loaded(
        "src/all.st",
        "PROGRAM Main\nEND_PROGRAM\n\
         FUNCTION Calc : INT\nCalc := 1;\nEND_FUNCTION\n\
         FUNCTION_BLOCK Counter\nEND_FUNCTION_BLOCK\n",
    );

    let (declarations, warnings) = extract_pou_declarations(&source);

    assert!(warnings.is_empty(), "{warnings:#?}");
    assert_eq!(declarations.len(), 3);
    assert!(declarations
        .iter()
        .any(|decl| decl.name == "Main" && decl.pou_type == PlcopenPouType::Program));
    assert!(declarations
        .iter()
        .any(|decl| decl.name == "Calc" && decl.pou_type == PlcopenPouType::Function));
    assert!(declarations.iter().any(|decl| {
        decl.name == "Counter" && decl.pou_type == PlcopenPouType::FunctionBlock
    }));
}

#[test]
fn semantic_export_pou_body_normalizes_line_endings_and_trailing_newline() {
    let source = export_loaded(
        "src/main.st",
        "PROGRAM Main\r\nVAR\rx : INT;\rEND_VAR\r\nEND_PROGRAM",
    );

    let (declarations, _) = extract_pou_declarations(&source);

    assert_eq!(declarations.len(), 1);
    assert!(!declarations[0].body.contains('\r'));
    assert!(declarations[0].body.ends_with('\n'));
}

#[test]
fn semantic_export_pou_source_line_is_one_based_physical_declaration_line() {
    let source = export_loaded(
        "src/lines.st",
        "\n// heading\n\nPROGRAM Main\nEND_PROGRAM\n\nFUNCTION Calc : INT\nCalc := 1;\nEND_FUNCTION\n",
    );

    let (declarations, _) = extract_pou_declarations(&source);
    let main = declarations
        .iter()
        .find(|decl| decl.name == "Main")
        .expect("Main");
    let calc = declarations
        .iter()
        .find(|decl| decl.name == "Calc")
        .expect("Calc");

    assert_eq!(main.line, 4);
    assert_eq!(calc.line, 7);
    assert_eq!(main.source, "src/lines.st");
}

#[test]
fn semantic_export_test_pous_map_to_standard_kinds_with_warning() {
    let source = export_loaded(
        "src/tests.st",
        "TEST_PROGRAM CheckProgram\nEND_PROGRAM\n\
         TEST_FUNCTION_BLOCK CheckBlock\nEND_FUNCTION_BLOCK\n",
    );

    let (declarations, warnings) = extract_pou_declarations(&source);

    assert_eq!(declarations.len(), 2);
    assert_eq!(declarations[0].pou_type, PlcopenPouType::Program);
    assert_eq!(declarations[1].pou_type, PlcopenPouType::FunctionBlock);
    assert_eq!(
        warnings
            .iter()
            .filter(|warning| warning.contains("exported as standard"))
            .count(),
        2,
        "{warnings:#?}"
    );
}

#[test]
fn semantic_export_unsupported_top_level_kind_has_path_and_line_warning() {
    let source = export_loaded(
        "src/mixed.st",
        "CLASS Unsupported\nEND_CLASS\n\nPROGRAM Main\nEND_PROGRAM\n",
    );

    let (declarations, warnings) = extract_pou_declarations(&source);

    assert_eq!(declarations.len(), 1);
    assert!(warnings.iter().any(|warning| {
        warning.starts_with("src/mixed.st:1")
            && warning.contains("unsupported top-level node")
    }));
}

#[test]
fn semantic_export_type_extraction_preserves_source_identity_and_order() {
    let source = export_loaded(
        "src/types.st",
        "TYPE\n  Speed : REAL;\n  Mode : (Idle, Run);\n  Alias : Speed;\nEND_TYPE\n",
    );

    let (types, warnings) = extract_data_type_declarations(&source);

    assert!(warnings.is_empty(), "{warnings:#?}");
    assert_eq!(
        types
            .iter()
            .map(|decl| (decl.name.as_str(), decl.type_expr.as_str(), decl.line))
            .collect::<Vec<_>>(),
        vec![
            ("Speed", "REAL", 2),
            ("Mode", "(Idle, Run)", 3),
            ("Alias", "Speed", 4)
        ]
    );
    assert!(types.iter().all(|decl| decl.source == "src/types.st"));
}

#[test]
fn semantic_export_type_extraction_preserves_multiline_struct() {
    let source = export_loaded(
        "src/types.st",
        "TYPE\nPoint : STRUCT\n  X : INT := 1;\n  Y : INT;\nEND_STRUCT;\nEND_TYPE\n",
    );

    let (types, warnings) = extract_data_type_declarations(&source);

    assert!(warnings.is_empty(), "{warnings:#?}");
    assert_eq!(types.len(), 1);
    assert_eq!(
        types[0].type_expr,
        "STRUCT\n  X : INT := 1;\n  Y : INT;\nEND_STRUCT"
    );
}

#[test]
fn semantic_export_unfinished_type_declaration_is_warned_and_not_returned() {
    let source = export_loaded(
        "src/types.st",
        "TYPE\n  Valid : INT;\n  Broken : ARRAY[0..3] OF\nEND_TYPE\n",
    );

    let (types, warnings) = extract_data_type_declarations(&source);

    assert_eq!(types.len(), 1);
    assert_eq!(types[0].name, "Valid");
    assert!(warnings
        .iter()
        .any(|warning| warning.contains("unfinished TYPE declaration")));
}

#[test]
fn semantic_export_elementary_type_matrix_maps_to_lowercase_xml_tags() {
    for name in [
        "BOOL", "BYTE", "WORD", "DWORD", "LWORD", "SINT", "INT", "DINT", "LINT", "USINT",
        "UINT", "UDINT", "ULINT", "REAL", "LREAL", "TIME", "LTIME", "DATE", "LDATE", "TOD",
        "LTOD", "DT", "LDT", "CHAR", "WCHAR",
    ] {
        assert_eq!(
            type_expression_to_plcopen_base_type_xml(name),
            Some(format!("<{} />", name.to_ascii_lowercase())),
            "{name}"
        );
    }
}

#[test]
fn semantic_export_string_types_map_lengths_and_unbounded_forms() {
    for (source, expected) in [
        ("STRING", "<string />"),
        ("WSTRING", "<wstring />"),
        ("STRING[80]", r#"<string length="80"/>"#),
        ("WSTRING[120]", r#"<wstring length="120"/>"#),
    ] {
        assert_eq!(
            type_expression_to_plcopen_base_type_xml(source),
            Some(expected.to_string()),
            "{source}"
        );
    }
}

#[test]
fn semantic_export_string_types_reject_invalid_lengths() {
    for source in ["STRING[]", "STRING[0]", "STRING[-1]", "STRING[many]"] {
        assert_eq!(
            type_expression_to_plcopen_base_type_xml(source),
            None,
            "{source}"
        );
    }
}

#[test]
fn semantic_export_derived_type_requires_valid_qualified_identifier() {
    assert_eq!(
        type_expression_to_plcopen_base_type_xml("Vendor.MotorState"),
        Some(r#"<derived name="Vendor.MotorState"/>"#.to_string())
    );
    for source in ["", ".Bad", "Bad.", "Bad-Name", "Bad Name"] {
        assert_eq!(
            type_expression_to_plcopen_base_type_xml(source),
            None,
            "{source}"
        );
    }
}

#[test]
fn semantic_export_array_type_preserves_dimension_and_base_order() {
    let xml = type_expression_to_plcopen_base_type_xml(
        "ARRAY[-2..2, 1..4] OF ARRAY[0..3] OF UINT",
    )
    .expect("array XML");

    assert!(xml.contains(r#"<dimension lower="-2" upper="2"/>"#));
    assert!(xml.contains(r#"<dimension lower="1" upper="4"/>"#));
    assert!(xml.contains(r#"<dimension lower="0" upper="3"/>"#));
    assert!(xml.contains("<uint />"));
    assert!(
        xml.find(r#"lower="-2""#).expect("first")
            < xml.find(r#"lower="1""#).expect("second")
    );
}

#[test]
fn semantic_export_array_type_rejects_incomplete_shape() {
    for source in [
        "ARRAY[] OF INT",
        "ARRAY[0] OF INT",
        "ARRAY[0..3] INT",
        "ARRAY[0..3] OF",
        "ARRAY[0..3, 1] OF INT",
    ] {
        assert_eq!(
            type_expression_to_plcopen_base_type_xml(source),
            None,
            "{source}"
        );
    }
}

#[test]
fn semantic_export_struct_type_preserves_fields_and_initializers() {
    let xml = type_expression_to_plcopen_base_type_xml(
        "STRUCT\n  Enabled : BOOL := TRUE;\n  Count : DINT;\nEND_STRUCT",
    )
    .expect("struct XML");

    assert!(xml.contains(r#"<variable name="Enabled">"#));
    assert!(xml.contains("<bool />"));
    assert!(xml.contains(r#"<simpleValue value="TRUE"/>"#));
    assert!(xml.contains(r#"<variable name="Count">"#));
    assert!(
        xml.find("Enabled").expect("Enabled") < xml.find("Count").expect("Count")
    );
}

#[test]
fn semantic_export_struct_type_rejects_empty_duplicate_or_malformed_fields() {
    for source in [
        "STRUCT\nEND_STRUCT",
        "STRUCT\n  A : INT;\n  a : DINT;\nEND_STRUCT",
        "STRUCT\n  MissingType : ;\nEND_STRUCT",
        "STRUCT\n  : INT;\nEND_STRUCT",
    ] {
        assert_eq!(
            type_expression_to_plcopen_base_type_xml(source),
            None,
            "{source}"
        );
    }
}

#[test]
fn semantic_export_enum_type_preserves_explicit_values() {
    let xml = type_expression_to_plcopen_base_type_xml("(Idle := 0, Run := 4, Fault)")
        .expect("enum XML");

    assert!(xml.contains(r#"<value name="Idle" value="0"/>"#));
    assert!(xml.contains(r#"<value name="Run" value="4"/>"#));
    assert!(xml.contains(r#"<value name="Fault"/>"#));
}

#[test]
fn semantic_export_enum_type_rejects_empty_duplicate_or_invalid_elements() {
    for source in ["()", "(A, a)", "(,)", "(Bad-Name)", "(A := )"] {
        assert_eq!(
            type_expression_to_plcopen_base_type_xml(source),
            None,
            "{source}"
        );
    }
}

#[test]
fn semantic_export_subrange_type_preserves_base_and_bounds() {
    let xml =
        type_expression_to_plcopen_base_type_xml("INT(-10..10)").expect("subrange XML");

    assert!(xml.starts_with(r#"<subrange lower="-10" upper="10">"#));
    assert!(xml.contains("<int />"));
}

#[test]
fn semantic_export_subrange_type_rejects_incomplete_bounds() {
    for source in ["INT(..10)", "INT(0..)", "INT(0)", "(0..10)"] {
        assert_eq!(
            type_expression_to_plcopen_base_type_xml(source),
            None,
            "{source}"
        );
    }
}

#[test]
fn semantic_export_global_extraction_preserves_attribute_prefix_and_modifiers() {
    let source = export_loaded(
        "src/GVL.st",
        "{attribute 'qualified_only'}\nVAR_GLOBAL RETAIN\n  Count : INT := 1;\nEND_VAR\n",
    );

    let (globals, warnings) = extract_global_var_declarations(&source);

    assert!(warnings.is_empty(), "{warnings:#?}");
    assert_eq!(globals.len(), 1);
    assert_eq!(globals[0].name, "GVL");
    assert_eq!(globals[0].line, 1);
    assert!(globals[0].body.starts_with("{attribute 'qualified_only'}"));
    assert_eq!(globals[0].variables.len(), 1);
    assert_eq!(globals[0].variables[0].name, "Count");
    assert_eq!(
        globals[0].variables[0].initial_value.as_deref(),
        Some("1")
    );
}

#[test]
fn semantic_export_multiple_global_blocks_get_stable_ordinal_names() {
    let source = export_loaded(
        "src/Globals.st",
        "VAR_GLOBAL\n  A : INT;\nEND_VAR\n\nVAR_GLOBAL\n  B : BOOL;\nEND_VAR\n",
    );

    let (globals, _) = extract_global_var_declarations(&source);

    assert_eq!(
        globals
            .iter()
            .map(|global| global.name.as_str())
            .collect::<Vec<_>>(),
        vec!["Globals", "Globals_2"]
    );
}

#[test]
fn semantic_export_global_extraction_expands_comma_names_in_order() {
    let source = export_loaded(
        "src/GVL.st",
        "VAR_GLOBAL\n  A, B, C : INT := 5;\nEND_VAR\n",
    );

    let (globals, _) = extract_global_var_declarations(&source);

    assert_eq!(
        globals[0]
            .variables
            .iter()
            .map(|variable| variable.name.as_str())
            .collect::<Vec<_>>(),
        vec!["A", "B", "C"]
    );
    assert!(globals[0]
        .variables
        .iter()
        .all(|variable| variable.initial_value.as_deref() == Some("5")));
}

#[test]
fn semantic_export_unterminated_global_block_is_warned_and_not_returned() {
    let source = export_loaded("src/GVL.st", "VAR_GLOBAL\n  A : INT;\n");

    let (globals, warnings) = extract_global_var_declarations(&source);

    assert!(globals.is_empty());
    assert!(warnings
        .iter()
        .any(|warning| warning.contains("unterminated VAR_GLOBAL")));
}

#[test]
fn semantic_export_partially_malformed_global_block_is_not_shrunk() {
    let source = export_loaded(
        "src/GVL.st",
        "VAR_GLOBAL\n  Valid : INT;\n  malformed declaration;\nEND_VAR\n",
    );

    let (globals, warnings) = extract_global_var_declarations(&source);

    assert!(globals.is_empty(), "{globals:#?}");
    assert!(warnings
        .iter()
        .any(|warning| warning.contains("malformed VAR_GLOBAL")));
}

#[test]
fn semantic_export_duplicate_global_identity_is_rejected() {
    let source = export_loaded(
        "src/GVL.st",
        "VAR_GLOBAL\n  Count : INT;\n  COUNT : DINT;\nEND_VAR\n",
    );

    let (globals, warnings) = extract_global_var_declarations(&source);

    assert!(globals.is_empty(), "{globals:#?}");
    assert!(warnings
        .iter()
        .any(|warning| warning.contains("duplicate")));
}

#[test]
fn semantic_export_configuration_extraction_preserves_complete_model() {
    let source = export_loaded(
        "src/config.st",
        "CONFIGURATION Plant\n\
           TASK Fast (INTERVAL := T#10ms, PRIORITY := 2);\n\
           PROGRAM MainInstance WITH Fast : Main;\n\
           RESOURCE Controller ON ARM\n\
             TASK Slow (INTERVAL := T#1s, PRIORITY := 4);\n\
             PROGRAM AuxInstance WITH Slow : Aux;\n\
           END_RESOURCE\n\
         END_CONFIGURATION\n",
    );

    let (configurations, warnings) = extract_configuration_declarations(&source);

    assert!(warnings.is_empty(), "{warnings:#?}");
    assert_eq!(configurations.len(), 1);
    assert_eq!(configurations[0].name, "Plant");
    assert_eq!(configurations[0].tasks.len(), 1);
    assert_eq!(configurations[0].programs.len(), 1);
    assert_eq!(configurations[0].resources.len(), 1);
    assert_eq!(configurations[0].resources[0].tasks.len(), 1);
    assert_eq!(configurations[0].resources[0].programs.len(), 1);
}

#[test]
fn semantic_export_configuration_parsers_accept_case_and_retain_binding_prefixes() {
    let task = parse_task_declaration_line(
        "task Cycle (interval := PT0.1S, priority := 3);",
    )
    .expect("task");
    let retained =
        parse_program_binding_line("PROGRAM RETAIN Main WITH Cycle : MainType;")
            .expect("retained program");
    let nonretained =
        parse_program_binding_line("program non_retain Aux : AuxType;")
            .expect("non-retained program");

    assert_eq!(task.name, "Cycle");
    assert_eq!(task.interval.as_deref(), Some("T#100ms"));
    assert_eq!(retained.instance_name, "Main");
    assert_eq!(retained.task_name.as_deref(), Some("Cycle"));
    assert_eq!(retained.type_name, "MainType");
    assert_eq!(nonretained.instance_name, "Aux");
    assert_eq!(nonretained.task_name, None);
}

#[test]
fn semantic_export_configuration_rejects_ambiguous_or_invalid_task_values() {
    for line in [
        "TASK Both (INTERVAL := T#10ms, SINGLE := Event, PRIORITY := 1);",
        "TASK Negative (INTERVAL := PT-1S, PRIORITY := 1);",
        "TASK BadPriority (INTERVAL := T#10ms, PRIORITY := nope);",
        "TASK MissingValue (INTERVAL := , PRIORITY := 1);",
    ] {
        assert!(parse_task_declaration_line(line).is_none(), "{line}");
    }
}

#[test]
fn semantic_export_configuration_rejects_incomplete_resource_scope() {
    let source = export_loaded(
        "src/config.st",
        "CONFIGURATION Plant\nRESOURCE Controller ON CPU\nTASK Cycle (INTERVAL := T#10ms);\nEND_CONFIGURATION\n",
    );

    let (configurations, warnings) = extract_configuration_declarations(&source);

    assert!(configurations.is_empty(), "{configurations:#?}");
    assert!(warnings
        .iter()
        .any(|warning| warning.contains("missing END_RESOURCE")));
}

#[test]
fn semantic_export_configuration_rejects_unknown_task_reference() {
    let source = export_loaded(
        "src/config.st",
        "CONFIGURATION Plant\n\
           TASK Known (INTERVAL := T#10ms);\n\
           PROGRAM MainInstance WITH Missing : Main;\n\
         END_CONFIGURATION\n",
    );

    let (configurations, warnings) = extract_configuration_declarations(&source);

    assert!(configurations.is_empty(), "{configurations:#?}");
    assert!(warnings
        .iter()
        .any(|warning| warning.contains("unknown task")));
}

#[test]
fn semantic_export_function_block_method_metadata_is_extracted_in_source_order() {
    let source = export_loaded(
        "src/fb.st",
        "FUNCTION_BLOCK Counter\n\
           METHOD PRIVATE Reset : BOOL\n\
           VAR_INPUT CONSTANT\nHard : BOOL;\nEND_VAR\n\
           Reset := Hard;\n\
           END_METHOD\n\
           METHOD PUBLIC Read : INT\nRead := 1;\nEND_METHOD\n\
         END_FUNCTION_BLOCK\n",
    );

    let (declarations, warnings) = extract_pou_declarations(&source);
    let methods = &declarations[0].methods;

    assert!(warnings.is_empty(), "{warnings:#?}");
    assert_eq!(
        methods
            .iter()
            .map(|method| method.name.as_str())
            .collect::<Vec<_>>(),
        vec!["Reset", "Read"]
    );
    assert_eq!(methods[0].return_type.as_deref(), Some("BOOL"));
    assert_eq!(methods[0].sections.len(), 1);
    assert_eq!(methods[0].sections[0].xml_name, "inputVars");
    assert!(methods[0].sections[0].constant);
    assert_eq!(methods[0].source, "src/fb.st");
}

#[test]
fn semantic_export_duplicate_method_identity_blocks_export() {
    let fixture = SemanticExportFixture::new("duplicate-method");
    fixture.write_source(
        "fb.st",
        "FUNCTION_BLOCK Counter\n\
         METHOD PUBLIC Reset\nEND_METHOD\n\
         METHOD PRIVATE RESET\nEND_METHOD\n\
         END_FUNCTION_BLOCK\n",
    );

    let error = fixture
        .export()
        .expect_err("duplicate method identity must fail");

    assert!(error.to_string().contains("duplicate"));
    assert!(!fixture.output.exists());
}

#[test]
fn semantic_export_rejects_parser_errors_before_publication() {
    let fixture = SemanticExportFixture::new("parser-error");
    fixture.write_source("valid.st", "PROGRAM Main\nEND_PROGRAM\n");
    fixture.write_source("broken.st", "PROGRAM Broken\nVAR x : ;\nEND_PROGRAM\n");

    let error = fixture
        .export()
        .expect_err("parser errors must block the export transaction");

    assert!(error.to_string().contains("broken.st"), "{error:#}");
    assert!(!fixture.output.exists());
    assert!(!fixture.output.with_extension("source-map.json").exists());
}

#[test]
fn semantic_export_rejects_duplicate_type_identity_before_publication() {
    let fixture = SemanticExportFixture::new("duplicate-type");
    fixture.write_source("a.st", "TYPE\n  Speed : REAL;\nEND_TYPE\n");
    fixture.write_source("b.st", "TYPE\n  SPEED : LREAL;\nEND_TYPE\n");

    let error = fixture
        .export()
        .expect_err("duplicate type authority must fail");

    assert!(error.to_string().contains("duplicate"), "{error:#}");
    assert!(!fixture.output.exists());
}

#[test]
fn semantic_export_rejects_partial_global_block_before_publication() {
    let fixture = SemanticExportFixture::new("partial-global");
    fixture.write_source(
        "GVL.st",
        "VAR_GLOBAL\n  Valid : INT;\n  malformed declaration;\nEND_VAR\n",
    );

    let error = fixture
        .export()
        .expect_err("partial global list must fail");

    assert!(error.to_string().contains("VAR_GLOBAL"), "{error:#}");
    assert!(!fixture.output.exists());
}

#[test]
fn semantic_export_rejects_invalid_configuration_before_publication() {
    let fixture = SemanticExportFixture::new("invalid-config");
    fixture.write_source(
        "config.st",
        "CONFIGURATION Plant\n\
         TASK Cycle (INTERVAL := PT-1S);\n\
         END_CONFIGURATION\n",
    );

    let error = fixture
        .export()
        .expect_err("invalid scheduling model must fail");

    assert!(error.to_string().contains("Cycle"), "{error:#}");
    assert!(!fixture.output.exists());
}

#[test]
fn semantic_export_report_counts_equal_published_xml_nodes() {
    let fixture = SemanticExportFixture::new("report-counts");
    fixture.write_source(
        "model.st",
        "TYPE\n  Speed : REAL;\nEND_TYPE\n\
         FUNCTION_BLOCK Counter\nEND_FUNCTION_BLOCK\n\
         PROGRAM Main\nEND_PROGRAM\n\
         VAR_GLOBAL\n  Enabled : BOOL;\nEND_VAR\n\
         CONFIGURATION Plant\n\
           TASK Cycle (INTERVAL := T#10ms);\n\
           PROGRAM MainInstance WITH Cycle : Main;\n\
           RESOURCE CPU ON ARM\n\
             TASK Slow (INTERVAL := T#1s);\n\
             PROGRAM CounterInstance WITH Slow : Counter;\n\
           END_RESOURCE\n\
         END_CONFIGURATION\n",
    );

    let report = fixture.export().expect("export complete model");
    let xml = fixture.xml();
    let document = roxmltree::Document::parse(&xml).expect("parse output");

    assert_eq!(report.pou_count, 2);
    assert_eq!(report.data_type_count, 1);
    assert_eq!(report.exported_global_var_lists, 1);
    assert_eq!(report.configuration_count, 1);
    assert_eq!(report.resource_count, 1);
    assert_eq!(report.task_count, 2);
    assert_eq!(report.program_instance_count, 2);
    assert_eq!(
        document
            .descendants()
            .filter(|node| is_element_named_ci(*node, "pou"))
            .count(),
        report.pou_count
    );
    assert_eq!(
        document
            .descendants()
            .filter(|node| is_element_named_ci(*node, "dataType"))
            .count(),
        report.data_type_count
    );
    assert_eq!(
        document
            .descendants()
            .filter(|node| is_element_named_ci(*node, "task"))
            .count(),
        report.task_count
    );
}

#[test]
fn semantic_export_pou_and_source_map_order_is_deterministic() {
    let fixture = SemanticExportFixture::new("pou-order");
    fixture.write_source("z.st", "PROGRAM Zed\nEND_PROGRAM\n");
    fixture.write_source(
        "a.st",
        "FUNCTION_BLOCK Block\nEND_FUNCTION_BLOCK\nFUNCTION Alpha : INT\nAlpha := 1;\nEND_FUNCTION\nPROGRAM Main\nEND_PROGRAM\n",
    );

    fixture.export().expect("export ordered POUs");
    let xml = fixture.xml();
    let source_map = fixture.source_map();
    let names = source_map["entries"]
        .as_array()
        .expect("entries")
        .iter()
        .map(|entry| entry["name"].as_str().expect("name"))
        .collect::<Vec<_>>();

    assert_eq!(names, vec!["Alpha", "Block", "Main", "Zed"]);
    assert!(
        xml.find(r#"name="Alpha""#).expect("Alpha")
            < xml.find(r#"name="Block""#).expect("Block")
    );
}

#[test]
fn semantic_export_source_map_preserves_project_relative_paths_and_lines() {
    let fixture = SemanticExportFixture::new("source-map");
    fixture.write_source(
        "nested/main.st",
        "\n\nPROGRAM Main\nEND_PROGRAM\n\nFUNCTION Calc : INT\nCalc := 1;\nEND_FUNCTION\n",
    );

    fixture.export().expect("export source map");
    let source_map = fixture.source_map();
    let entries = source_map["entries"].as_array().expect("entries");

    assert_eq!(entries[0]["name"], "Calc");
    assert_eq!(entries[0]["source"], "src/nested/main.st");
    assert_eq!(entries[0]["line"], 6);
    assert_eq!(entries[1]["name"], "Main");
    assert_eq!(entries[1]["line"], 3);
}

#[test]
fn semantic_export_xml_escapes_names_without_changing_cdata_source() {
    let fixture = SemanticExportFixture::new("escaping");
    fixture.write_source(
        "main.st",
        "PROGRAM Main\nmessage := 'a < b & c > d';\nEND_PROGRAM\n",
    );

    fixture.export().expect("export escaped source");
    let xml = fixture.xml();

    assert!(xml.contains("message := 'a < b & c > d';"));
    assert!(roxmltree::Document::parse(&xml).is_ok());
}

#[test]
fn semantic_export_generic_roundtrip_recovers_supported_identity_set() {
    let fixture = SemanticExportFixture::new("roundtrip-identities");
    fixture.write_source(
        "model.st",
        "TYPE\n  Mode : (Idle, Run);\nEND_TYPE\n\
         FUNCTION_BLOCK Counter\n\
         METHOD PUBLIC Reset\nEND_METHOD\n\
         END_FUNCTION_BLOCK\n\
         PROGRAM Main\nEND_PROGRAM\n\
         CONFIGURATION Plant\nTASK Cycle (INTERVAL := T#10ms);\nPROGRAM MainInstance WITH Cycle : Main;\nEND_CONFIGURATION\n",
    );

    let exported = fixture.export().expect("export model");
    let imported_root = temp_dir("plcopen-semantic-export-roundtrip-import");
    let imported =
        import_xml_to_project(&exported.output_path, &imported_root).expect("re-import model");
    let imported_text = source_files_recursive(&imported_root.join("src"))
        .into_iter()
        .map(|path| std::fs::read_to_string(path).expect("read imported source"))
        .collect::<Vec<_>>()
        .join("\n");

    assert_eq!(imported.imported_pous, 2);
    assert_eq!(imported.imported_data_types, 1);
    assert_eq!(imported.imported_configurations, 1);
    for identity in [
        "Mode : (Idle, Run);",
        "FUNCTION_BLOCK Counter",
        "METHOD PUBLIC Reset",
        "PROGRAM Main",
        "CONFIGURATION Plant",
        "PROGRAM MainInstance WITH Cycle : Main;",
    ] {
        assert!(imported_text.contains(identity), "{identity}\n{imported_text}");
    }

    let _ = std::fs::remove_dir_all(imported_root);
}
