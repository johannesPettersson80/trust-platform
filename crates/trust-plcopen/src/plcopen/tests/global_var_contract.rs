fn global_var_fixture(label: &str, global_vars: &str, pous: &str) -> SemanticImportFixture {
    SemanticImportFixture::new(
        label,
        &format!(
            r#"<globalVarLists>{global_vars}</globalVarLists>
{pous}"#
        ),
    )
}

fn import_globals_with_mode(
    fixture: &SemanticImportFixture,
    mode: PlcopenImportGlobalVarMode,
) -> anyhow::Result<PlcopenImportReport> {
    import_xml_to_project_with_options(
        &fixture.xml_path,
        &fixture.root,
        PlcopenImportOptions {
            global_var_mode: mode,
        },
    )
}

fn source_files_recursive(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(directory) = stack.pop() {
        let mut entries = std::fs::read_dir(&directory)
            .expect("read source directory")
            .map(|entry| entry.expect("source entry").path())
            .collect::<Vec<_>>();
        entries.sort();
        for path in entries {
            if path.is_dir() {
                stack.push(path);
            } else if path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("st"))
            {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

fn read_source_named(root: &Path, name: &str) -> String {
    let matches = source_files_recursive(&root.join("src"))
        .into_iter()
        .filter(|path| {
            path.file_name()
                .and_then(|file_name| file_name.to_str())
                .is_some_and(|file_name| file_name.eq_ignore_ascii_case(name))
        })
        .collect::<Vec<_>>();
    assert_eq!(matches.len(), 1, "{matches:#?}");
    std::fs::read_to_string(&matches[0]).expect("read imported source")
}

#[test]
fn global_vars_structured_entries_preserve_order_types_and_initial_values() {
    let fixture = global_var_fixture(
        "structured",
        r#"<globalVars name="PlantGlobals">
  <variable name="Enabled"><type><bool/></type><initialValue><simpleValue value="TRUE"/></initialValue></variable>
  <variable name="Count"><type><dint/></type></variable>
  <variable name="State"><type><derived name="MachineState"/></type></variable>
</globalVars>"#,
        "",
    );

    let report = fixture.import().expect("import structured globals");
    let source = read_source_named(&fixture.root, "PlantGlobals.st");

    assert_eq!(report.discovered_global_var_lists, 1);
    assert_eq!(report.imported_global_var_lists, 1);
    assert_eq!(
        source,
        "VAR_GLOBAL\n    Enabled : BOOL := TRUE;\n    Count : DINT;\n    State : MachineState;\nEND_VAR\n"
    );
}

#[test]
fn global_vars_plaintext_takes_precedence_over_structured_entries() {
    let fixture = global_var_fixture(
        "plaintext-precedence",
        r#"<globalVars name="PlantGlobals">
  <variable name="Structured"><type><bool/></type></variable>
  <addData><data name="interfaceasplaintext"><text><![CDATA[
VAR_GLOBAL
    Plaintext : INT := 7;
END_VAR
]]></text></data></addData>
</globalVars>"#,
        "",
    );

    fixture.import().expect("import plaintext globals");
    let source = read_source_named(&fixture.root, "PlantGlobals.st");

    assert!(source.contains("Plaintext : INT := 7;"));
    assert!(!source.contains("Structured"));
}

#[test]
fn global_vars_name_child_and_stable_unnamed_fallback_are_supported() {
    let fixture = global_var_fixture(
        "name-fallbacks",
        r#"<globalVars>
  <name>ChildName</name>
  <variable name="A"><type><int/></type></variable>
</globalVars>
<globalVars>
  <variable name="B"><type><int/></type></variable>
</globalVars>"#,
        "",
    );

    let report = fixture.import().expect("import named globals");

    assert_eq!(report.imported_global_var_lists, 2);
    assert_eq!(
        read_source_named(&fixture.root, "ChildName.st"),
        "VAR_GLOBAL\n    A : INT;\nEND_VAR\n"
    );
    assert_eq!(
        read_source_named(&fixture.root, "GlobalVars2.st"),
        "VAR_GLOBAL\n    B : INT;\nEND_VAR\n"
    );
}

#[test]
fn global_vars_missing_declarations_emits_plco601_and_is_not_imported() {
    let fixture = global_var_fixture(
        "missing-declarations",
        r#"<globalVars name="Empty"/>"#,
        "",
    );

    let error = fixture
        .import()
        .expect_err("empty global list has no importable content");
    let report = fixture.migration_report();

    assert!(error.to_string().contains("no importable PLCopen ST content"));
    assert_eq!(report["discovered_global_var_lists"], 1);
    assert_eq!(report["imported_global_var_lists"], 0);
    assert!(report["unsupported_diagnostics"]
        .as_array()
        .expect("diagnostics")
        .iter()
        .any(|diagnostic| diagnostic["code"] == "PLCO601"));
}

#[test]
fn global_vars_unparseable_plaintext_emits_plco602_without_structured_fallback() {
    let fixture = global_var_fixture(
        "unparseable-plaintext",
        r#"<globalVars name="Broken">
  <variable name="WouldBeValid"><type><int/></type></variable>
  <addData><data name="interfaceasplaintext"><text>this is not a VAR_GLOBAL block</text></data></addData>
</globalVars>"#,
        "",
    );

    let error = fixture
        .import()
        .expect_err("authoritative malformed plaintext must fail");
    let report = fixture.migration_report();

    assert!(error.to_string().contains("no importable PLCopen ST content"));
    assert_eq!(report["imported_global_var_lists"], 0);
    assert!(report["unsupported_diagnostics"]
        .as_array()
        .expect("diagnostics")
        .iter()
        .any(|diagnostic| diagnostic["code"] == "PLCO602"));
}

#[test]
fn global_vars_native_qualified_only_plaintext_maps_to_namespace() {
    let fixture = global_var_fixture(
        "qualified-native",
        r#"<globalVars name="GVL">
  <addData><data name="interfaceasplaintext"><text><![CDATA[
{attribute 'qualified_only'}
VAR_GLOBAL RETAIN
    Count : INT := 1;
END_VAR
]]></text></data></addData>
</globalVars>"#,
        "",
    );

    fixture.import().expect("import qualified globals");
    let source = read_source_named(&fixture.root, "GVL.st");

    assert!(source.starts_with("{attribute 'qualified_only'}\nNAMESPACE GVL\n"));
    assert!(source.contains("VAR_GLOBAL RETAIN"));
    assert!(source.contains("Count : INT := 1;"));
    assert!(source.ends_with("END_NAMESPACE\n"));
}

#[test]
fn global_vars_structured_qualified_only_attribute_maps_to_namespace() {
    let fixture = global_var_fixture(
        "qualified-structured",
        r#"<globalVars name="GVL">
  <Attribute Name="QuAlIfIeD_OnLy" Value="TRUE"/>
  <variable name="Count"><type><int/></type></variable>
</globalVars>"#,
        "",
    );

    fixture.import().expect("import structured qualified globals");
    let source = read_source_named(&fixture.root, "GVL.st");

    assert!(source.contains("NAMESPACE GVL"));
    assert!(source.contains("Count : INT;"));
}

#[test]
fn global_vars_false_qualified_only_attribute_does_not_create_namespace() {
    let fixture = global_var_fixture(
        "qualified-false",
        r#"<globalVars name="GVL">
  <Attribute Name="qualified_only" Value="false"/>
  <variable name="Count"><type><int/></type></variable>
</globalVars>"#,
        "",
    );

    fixture.import().expect("import ordinary globals");
    let source = read_source_named(&fixture.root, "GVL.st");

    assert!(!source.contains("NAMESPACE"));
    assert!(source.contains("VAR_GLOBAL"));
}

#[test]
fn global_vars_native_plaintext_preserves_leading_attributes_and_header_suffix() {
    let fixture = global_var_fixture(
        "prefix-and-suffix",
        r#"<globalVars name="GVL">
  <addData><data name="interfaceasplaintext"><text><![CDATA[
{attribute 'linkalways'}
{attribute 'symbol' := 'readwrite'}
VAR_GLOBAL CONSTANT PERSISTENT
    Revision : UINT := 3;
END_VAR
]]></text></data></addData>
</globalVars>"#,
        "",
    );

    fixture.import().expect("import prefixed globals");
    let source = read_source_named(&fixture.root, "GVL.st");

    assert!(source.starts_with("{attribute 'linkalways'}\n{attribute 'symbol'"));
    assert!(source.contains("VAR_GLOBAL CONSTANT PERSISTENT"));
    assert!(source.contains("Revision : UINT := 3;"));
}

#[test]
fn global_vars_plaintext_expands_comma_separated_names_in_order() {
    let fixture = global_var_fixture(
        "comma-names",
        r#"<globalVars name="GVL">
  <addData><data name="interfaceasplaintext"><text><![CDATA[
VAR_GLOBAL
    A, B, C : INT := 4;
END_VAR
]]></text></data></addData>
</globalVars>"#,
        "",
    );

    fixture.import().expect("import comma declarations");
    let source = read_source_named(&fixture.root, "GVL.st");

    assert_eq!(
        source,
        "VAR_GLOBAL\n    A : INT := 4;\n    B : INT := 4;\n    C : INT := 4;\nEND_VAR\n"
    );
}

#[test]
fn global_vars_invalid_list_identity_is_normalized_and_reported() {
    let fixture = global_var_fixture(
        "normalized-list",
        r#"<globalVars name="1 Plant GVL">
  <variable name="Count"><type><int/></type></variable>
</globalVars>"#,
        "",
    );

    let report = fixture.import().expect("import normalized list");
    let source = read_source_named(&fixture.root, "1_Plant_GVL.st");

    assert_eq!(source, "VAR_GLOBAL\n    Count : INT;\nEND_VAR\n");
    assert!(report
        .warnings
        .iter()
        .any(|warning| warning.contains("1 Plant GVL") && warning.contains("_1_Plant_GVL")));
}

#[test]
fn global_vars_case_insensitive_list_collisions_get_unique_st_identities() {
    let fixture = global_var_fixture(
        "list-collisions",
        r#"<globalVars name="GVL">
  <Attribute Name="qualified_only" Value="true"/>
  <variable name="A"><type><int/></type></variable>
</globalVars>
<globalVars name="gvl">
  <Attribute Name="qualified_only" Value="true"/>
  <variable name="B"><type><int/></type></variable>
</globalVars>"#,
        "",
    );

    let report = fixture.import().expect("import colliding lists");
    let sources = source_files_recursive(&fixture.root.join("src"))
        .into_iter()
        .map(|path| std::fs::read_to_string(path).expect("read source"))
        .collect::<Vec<_>>();

    assert_eq!(report.imported_global_var_lists, 2);
    assert!(sources.iter().any(|source| source.contains("NAMESPACE GVL\n")));
    assert!(sources
        .iter()
        .any(|source| source.contains("NAMESPACE gvl_2\n")));
}

#[test]
fn global_vars_duplicate_case_insensitive_variable_names_reject_the_list() {
    let fixture = global_var_fixture(
        "duplicate-vars",
        r#"<globalVars name="GVL">
  <variable name="Count"><type><int/></type></variable>
  <variable name="COUNT"><type><dint/></type></variable>
</globalVars>"#,
        "",
    );

    let error = fixture
        .import()
        .expect_err("duplicate globals must not publish ambiguous ST");

    assert!(error.to_string().contains("no importable PLCopen ST content"));
    assert!(!fixture.root.join("src/GVL.st").exists());
}

#[test]
fn global_vars_invalid_variable_identifier_rejects_the_list() {
    let fixture = global_var_fixture(
        "invalid-var-name",
        r#"<globalVars name="GVL">
  <variable name="bad-name"><type><int/></type></variable>
</globalVars>"#,
        "",
    );

    let error = fixture
        .import()
        .expect_err("invalid declaration identity must reject list");

    assert!(error.to_string().contains("no importable PLCopen ST content"));
    assert!(!fixture.root.join("src/GVL.st").exists());
}

#[test]
fn global_vars_one_malformed_structured_entry_rejects_whole_list() {
    let fixture = global_var_fixture(
        "partial-structured",
        r#"<globalVars name="GVL">
  <variable name="Valid"><type><int/></type></variable>
  <variable name="MissingType"/>
</globalVars>"#,
        "",
    );

    let error = fixture
        .import()
        .expect_err("partial structured list must fail atomically");

    assert!(error.to_string().contains("no importable PLCopen ST content"));
    assert!(!fixture.root.join("src/GVL.st").exists());
}

#[test]
fn global_vars_one_malformed_plaintext_declaration_rejects_whole_list() {
    let fixture = global_var_fixture(
        "partial-plaintext",
        r#"<globalVars name="GVL">
  <addData><data name="interfaceasplaintext"><text><![CDATA[
VAR_GLOBAL
    Valid : INT;
    this declaration is malformed;
END_VAR
]]></text></data></addData>
</globalVars>"#,
        "",
    );

    let error = fixture
        .import()
        .expect_err("partial plaintext list must fail atomically");

    assert!(error.to_string().contains("no importable PLCopen ST content"));
    assert!(!fixture.root.join("src/GVL.st").exists());
}

#[test]
fn global_vars_strict_mode_wraps_ordinary_list_in_configuration() {
    let fixture = global_var_fixture(
        "strict-ordinary",
        r#"<globalVars name="GVL">
  <variable name="Count"><type><int/></type></variable>
</globalVars>"#,
        "",
    );

    let report =
        import_globals_with_mode(&fixture, PlcopenImportGlobalVarMode::StrictIecAdapter)
            .expect("import strict globals");
    let source = read_source_named(&fixture.root, "GVL.st");

    assert_eq!(report.imported_global_var_lists, 1);
    assert!(source.starts_with("CONFIGURATION GVL_Globals\n"));
    assert!(source.contains("VAR_GLOBAL"));
    assert!(source.contains("Count : INT;"));
    assert!(source.ends_with("END_CONFIGURATION\n"));
}

#[test]
fn global_vars_strict_qualified_mode_emits_type_and_single_instance() {
    let fixture = global_var_fixture(
        "strict-qualified",
        r#"<globalVars name="GVL">
  <Attribute Name="qualified_only" Value="true"/>
  <variable name="Count"><type><int/></type></variable>
  <variable name="Ready"><type><bool/></type></variable>
</globalVars>"#,
        "",
    );

    import_globals_with_mode(&fixture, PlcopenImportGlobalVarMode::StrictIecAdapter)
        .expect("import strict qualified globals");
    let source = read_source_named(&fixture.root, "GVL.st");

    assert!(source.starts_with("TYPE\nGVL_TYPE : STRUCT\n"));
    assert!(source.contains("Count : INT;"));
    assert!(source.contains("Ready : BOOL;"));
    assert!(source.contains("CONFIGURATION GVL_Globals"));
    assert!(source.contains("GVL : GVL_TYPE;"));
}

#[test]
fn global_vars_strict_mode_injects_required_external_before_statements() {
    let fixture = global_var_fixture(
        "strict-external",
        r#"<globalVars name="GVL">
  <Attribute Name="qualified_only" Value="true"/>
  <variable name="Count"><type><int/></type></variable>
</globalVars>"#,
        &semantic_pou(
            "Main",
            "program",
            r#"<localVars><variable name="Local"><type><int/></type></variable></localVars>"#,
            &semantic_st_body("Local := GVL.Count;"),
        ),
    );

    let report =
        import_globals_with_mode(&fixture, PlcopenImportGlobalVarMode::StrictIecAdapter)
            .expect("import strict external");
    let source = read_source_named(&fixture.root, "Main.st");
    let external = source.find("VAR_EXTERNAL").expect("external section");
    let statement = source.find("Local := GVL.Count;").expect("statement");

    assert!(external < statement, "{source}");
    assert!(source.contains("GVL : GVL_TYPE;"));
    assert!(report.unsupported_diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "PLCO603" && diagnostic.pou.as_deref() == Some("Main")
    }));
}

#[test]
fn global_vars_strict_mode_does_not_duplicate_existing_external() {
    let fixture = global_var_fixture(
        "strict-existing-external",
        r#"<globalVars name="GVL">
  <Attribute Name="qualified_only" Value="true"/>
  <variable name="Count"><type><int/></type></variable>
</globalVars>"#,
        &semantic_pou(
            "Main",
            "program",
            r#"<externalVars><variable name="gvl"><type><derived name="GVL_TYPE"/></type></variable></externalVars>"#,
            &semantic_st_body("GVL.Count := GVL.Count + 1;"),
        ),
    );

    let report =
        import_globals_with_mode(&fixture, PlcopenImportGlobalVarMode::StrictIecAdapter)
            .expect("import existing external");
    let source = read_source_named(&fixture.root, "Main.st");

    assert_eq!(
        source
            .lines()
            .filter(|line| line.trim().to_ascii_lowercase().starts_with("gvl :"))
            .count(),
        1,
        "{source}"
    );
    assert!(report
        .unsupported_diagnostics
        .iter()
        .all(|diagnostic| diagnostic.code != "PLCO603"));
}

#[test]
fn global_vars_strict_mode_does_not_inject_for_unreferenced_list() {
    let fixture = global_var_fixture(
        "strict-unreferenced",
        r#"<globalVars name="GVL">
  <Attribute Name="qualified_only" Value="true"/>
  <variable name="Count"><type><int/></type></variable>
</globalVars>"#,
        &semantic_pou(
            "Main",
            "program",
            "",
            &semantic_st_body("Other := 1;"),
        ),
    );

    let report =
        import_globals_with_mode(&fixture, PlcopenImportGlobalVarMode::StrictIecAdapter)
            .expect("import unreferenced globals");
    let source = read_source_named(&fixture.root, "Main.st");

    assert!(!source.contains("VAR_EXTERNAL"), "{source}");
    assert!(report
        .unsupported_diagnostics
        .iter()
        .all(|diagnostic| diagnostic.code != "PLCO603"));
}

#[test]
fn global_vars_qualified_reference_match_respects_identifier_boundary() {
    assert!(source_uses_qualified_global_list("x := GVL.Count;", "GVL"));
    assert!(source_uses_qualified_global_list("x := (gvl.Count);", "GVL"));
    assert!(!source_uses_qualified_global_list(
        "x := OtherGVL.Count;",
        "GVL"
    ));
    assert!(!source_uses_qualified_global_list(
        "x := GVL_Count;",
        "GVL"
    ));
}

#[test]
fn global_vars_qualified_reference_ignores_comments_and_string_literals() {
    for source in [
        "// GVL.Count\nx := 1;",
        "(* GVL.Count *)\nx := 1;",
        "message := 'GVL.Count';",
        "message := \"GVL.Count\";",
    ] {
        assert!(
            !source_uses_qualified_global_list(source, "GVL"),
            "{source}"
        );
    }
}

#[test]
fn global_vars_existing_external_detection_is_section_and_case_sensitive_in_scope_only() {
    assert!(source_has_var_external_symbol(
        "PROGRAM Main\nVAR_EXTERNAL\n  gVl : GVL_TYPE;\nEND_VAR\nEND_PROGRAM\n",
        "GVL"
    ));
    assert!(!source_has_var_external_symbol(
        "PROGRAM Main\nVAR\n  GVL : GVL_TYPE;\nEND_VAR\nEND_PROGRAM\n",
        "GVL"
    ));
    assert!(!source_has_var_external_symbol(
        "PROGRAM Main\n// VAR_EXTERNAL GVL : GVL_TYPE;\nEND_PROGRAM\n",
        "GVL"
    ));
}

#[test]
fn global_vars_strict_injection_occurs_after_existing_var_sections() {
    let source =
        "PROGRAM Main\nVAR\n    Local : INT;\nEND_VAR\nLocal := GVL.Count;\nEND_PROGRAM\n";
    let externals = vec![QualifiedGlobalListExternalDecl {
        list_name: "GVL".to_string(),
        type_name: "GVL_TYPE".to_string(),
    }];

    let (rendered, inserted) = inject_required_var_external_declarations(source, &externals);

    assert_eq!(inserted, vec!["GVL"]);
    assert!(
        rendered.find("END_VAR").expect("local end")
            < rendered.find("VAR_EXTERNAL").expect("external")
    );
    assert!(
        rendered.find("VAR_EXTERNAL").expect("external")
            < rendered.find("Local := GVL.Count;").expect("statement")
    );
}

#[test]
fn global_vars_report_counts_and_written_sources_match_complete_set() {
    let fixture = global_var_fixture(
        "report-accounting",
        r#"<globalVars name="First"><variable name="A"><type><int/></type></variable></globalVars>
<globalVars name="Empty"/>
<globalVars name="Third"><variable name="C"><type><bool/></type></variable></globalVars>"#,
        "",
    );

    let report = fixture.import().expect("import valid lists around skipped list");
    let sources = source_files_recursive(&fixture.root.join("src"));

    assert_eq!(report.discovered_global_var_lists, 3);
    assert_eq!(report.imported_global_var_lists, 2);
    assert_eq!(report.written_sources.len(), 2);
    assert_eq!(sources.len(), 2);
    assert!(report
        .unsupported_diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "PLCO601"));
    assert!(report.semantic_loss_percent > 0.0);
}

#[test]
fn global_vars_structured_attribute_order_is_deterministic() {
    let fixture = global_var_fixture(
        "attribute-order",
        r#"<globalVars name="GVL">
  <Attribute Name="zeta" Value="last"/>
  <Attribute Name="alpha" Value="first"/>
  <variable name="Count"><type><int/></type></variable>
</globalVars>"#,
        "",
    );

    fixture.import().expect("import attributes");
    let source = read_source_named(&fixture.root, "GVL.st");

    assert!(
        source.find("{attribute 'zeta': 'last'}").expect("zeta")
            < source
                .find("{attribute 'alpha': 'first'}")
                .expect("alpha"),
        "{source}"
    );
}
