struct ProjectModelFixture {
    root: PathBuf,
    xml_path: PathBuf,
}

impl ProjectModelFixture {
    fn new(label: &str, instances_xml: &str) -> Self {
        let root = temp_dir(&format!("plcopen-project-model-{label}"));
        let xml_path = root.join("input.xml");
        write(
            &xml_path,
            &format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="{PLCOPEN_NAMESPACE}">
  <types/>
  {instances_xml}
</project>
"#
            ),
        );
        Self { root, xml_path }
    }

    fn import(&self) -> anyhow::Result<PlcopenImportReport> {
        import_xml_to_project(&self.xml_path, &self.root)
    }

    fn configuration_sources(&self) -> Vec<(PathBuf, String)> {
        let mut sources = std::fs::read_dir(self.root.join("src"))
            .expect("read source directory")
            .map(|entry| entry.expect("source entry").path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("plcopen_configuration_"))
            })
            .map(|path| {
                let text = std::fs::read_to_string(&path).expect("read configuration source");
                (path, text)
            })
            .collect::<Vec<_>>();
        sources.sort_by(|left, right| left.0.cmp(&right.0));
        sources
    }

    fn only_configuration_source(&self) -> String {
        let sources = self.configuration_sources();
        assert_eq!(sources.len(), 1, "{sources:#?}");
        sources[0].1.clone()
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

impl Drop for ProjectModelFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn project_model_interval_normalization_preserves_iec_literals() {
    for literal in ["T#10ms", "TIME#2s", "LTIME#1d2h", " t#5ms "] {
        assert_eq!(
            normalize_task_interval_literal(literal),
            literal.trim(),
            "{literal}"
        );
    }
}

#[test]
fn project_model_interval_normalization_converts_supported_iso_seconds() {
    for (source, expected) in [
        ("PT1S", "T#1s"),
        ("pt12s", "T#12s"),
        ("PT0.5S", "T#500ms"),
        ("PT1.25S", "T#1250ms"),
        ("PT250MS", "T#250ms"),
    ] {
        assert_eq!(normalize_task_interval_literal(source), expected, "{source}");
    }
}

#[test]
fn project_model_interval_normalization_rejects_negative_nonfinite_and_overflow() {
    for literal in [
        "PT-1S",
        "PT-1MS",
        "PTNaNS",
        "PTinfS",
        "PT999999999999999999999999999999999999999S",
    ] {
        assert_eq!(
            normalize_task_interval_literal(literal),
            "",
            "invalid interval must not enter generated ST: {literal}"
        );
    }
}

#[test]
fn project_model_task_format_supplies_deterministic_defaults() {
    let task = TaskDecl {
        name: "MainTask".to_string(),
        ..TaskDecl::default()
    };

    assert_eq!(
        format_task_declaration(&task),
        "TASK MainTask (INTERVAL := T#100ms, PRIORITY := 1);"
    );
}

#[test]
fn project_model_task_format_preserves_event_mode_without_interval_default() {
    let task = TaskDecl {
        name: "EventTask".to_string(),
        single: Some("StartEvent".to_string()),
        priority: Some("3".to_string()),
        ..TaskDecl::default()
    };

    assert_eq!(
        format_task_declaration(&task),
        "TASK EventTask (SINGLE := StartEvent, PRIORITY := 3);"
    );
}

#[test]
fn project_model_program_binding_formats_bound_and_unbound_forms() {
    assert_eq!(
        format_program_binding(&ProgramBindingDecl {
            instance_name: "MainInstance".to_string(),
            task_name: Some("MainTask".to_string()),
            type_name: "Main".to_string(),
        }),
        "PROGRAM MainInstance WITH MainTask : Main;"
    );
    assert_eq!(
        format_program_binding(&ProgramBindingDecl {
            instance_name: "MainInstance".to_string(),
            task_name: None,
            type_name: "Main".to_string(),
        }),
        "PROGRAM MainInstance : Main;"
    );
}

#[test]
fn project_model_identifier_normalization_is_deterministic() {
    for (source, fallback, expected) in [
        ("Plant A", "Fallback", "Plant_A"),
        ("1st", "Fallback", "_1st"),
        ("A-B.C", "Fallback", "A_B_C"),
        ("", "Fallback", "Fallback"),
        ("_Valid9", "Fallback", "_Valid9"),
    ] {
        assert_eq!(sanitize_st_identifier(source, fallback), expected, "{source}");
    }
}

#[test]
fn project_model_unique_identifier_suffixes_case_insensitive_collisions() {
    let mut used = HashSet::new();

    assert_eq!(unique_identifier("Task".to_string(), &mut used), "Task");
    assert_eq!(unique_identifier("TASK".to_string(), &mut used), "TASK_2");
    assert_eq!(unique_identifier("Task".to_string(), &mut used), "Task_3");
}

#[test]
fn project_model_imports_configuration_level_task_and_program() {
    let fixture = ProjectModelFixture::new(
        "configuration-task",
        r#"<instances><configurations><configuration name="Plant">
  <task name="Fast" interval="PT0.01S" priority="2"/>
  <program name="MainInstance" typeName="Main" taskName="Fast"/>
</configuration></configurations></instances>"#,
    );

    let report = fixture.import().expect("import configuration model");
    let source = fixture.only_configuration_source();

    assert_eq!(report.discovered_configurations, 1);
    assert_eq!(report.imported_configurations, 1);
    assert_eq!(report.imported_tasks, 1);
    assert_eq!(report.imported_program_instances, 1);
    assert!(source.contains("CONFIGURATION Plant"));
    assert!(source.contains("TASK Fast (INTERVAL := T#10ms, PRIORITY := 2);"));
    assert!(source.contains("PROGRAM MainInstance WITH Fast : Main;"));
    assert!(source.ends_with("END_CONFIGURATION\n"));
}

#[test]
fn project_model_accepts_configuration_directly_under_instances() {
    let fixture = ProjectModelFixture::new(
        "direct-configuration",
        r#"<instances><configuration configurationName="Plant">
  <task taskName="MainTask" cycleTime="T#20ms"/>
</configuration></instances>"#,
    );

    let report = fixture.import().expect("import direct configuration");

    assert_eq!(report.imported_configurations, 1);
    assert!(fixture
        .only_configuration_source()
        .contains("TASK MainTask (INTERVAL := T#20ms, PRIORITY := 1);"));
}

#[test]
fn project_model_wraps_direct_resources_in_synthetic_configuration() {
    let fixture = ProjectModelFixture::new(
        "direct-resource",
        r#"<instances><resource name="Controller" target="ARM">
  <task name="Cycle" interval="T#5ms"/>
</resource></instances>"#,
    );

    let report = fixture.import().expect("import direct resource");
    let source = fixture.only_configuration_source();

    assert_eq!(report.imported_configurations, 1);
    assert_eq!(report.imported_resources, 1);
    assert!(source.starts_with("CONFIGURATION ImportedConfiguration\n"));
    assert!(source.contains("RESOURCE Controller ON ARM"));
    assert!(source.contains("TASK Cycle (INTERVAL := T#5ms, PRIORITY := 1);"));
}

#[test]
fn project_model_imports_resources_holder_and_preserves_xml_order() {
    let fixture = ProjectModelFixture::new(
        "resource-order",
        r#"<instances><configuration name="Plant"><resources>
  <resource name="First" type="CPU_A"/>
  <resource name="Second" on="CPU_B"/>
</resources></configuration></instances>"#,
    );

    fixture.import().expect("import resources");
    let source = fixture.only_configuration_source();

    assert!(
        source.find("RESOURCE First").expect("first resource")
            < source.find("RESOURCE Second").expect("second resource"),
        "{source}"
    );
}

#[test]
fn project_model_nested_program_inherits_owning_task() {
    let fixture = ProjectModelFixture::new(
        "nested-program",
        r#"<instances><configuration name="Plant"><resource name="Controller">
  <task name="Cycle" interval="T#10ms">
    <program name="MainInstance" typeName="Main"/>
  </task>
</resource></configuration></instances>"#,
    );

    fixture.import().expect("import nested program");
    let source = fixture.only_configuration_source();

    assert!(source.contains("PROGRAM MainInstance WITH Cycle : Main;"));
}

#[test]
fn project_model_program_aliases_and_child_values_are_supported() {
    let fixture = ProjectModelFixture::new(
        "program-aliases",
        r#"<instances><configuration name="Plant">
  <task name="Cycle"/>
  <pouInstance task="Cycle"><name>MainInstance</name><type name="Main"/></pouInstance>
  <programInstance instanceName="AuxInstance" programType="Aux" withTask="Cycle"/>
</configuration></instances>"#,
    );

    let report = fixture.import().expect("import program aliases");
    let source = fixture.only_configuration_source();

    assert_eq!(report.imported_program_instances, 2);
    assert!(source.contains("PROGRAM MainInstance WITH Cycle : Main;"));
    assert!(source.contains("PROGRAM AuxInstance WITH Cycle : Aux;"));
}

#[test]
fn project_model_normalizes_and_reports_invalid_identifiers() {
    let fixture = ProjectModelFixture::new(
        "normalized-identities",
        r#"<instances><configuration name="Plant A">
  <resource name="CPU-1" target="Linux ARM">
    <task name="10 ms" interval="T#10ms"/>
    <program name="Main Instance" typeName="Main Program" taskName="10 ms"/>
  </resource>
</configuration></instances>"#,
    );

    let report = fixture.import().expect("import normalized identities");
    let source = fixture.only_configuration_source();

    assert!(source.contains("CONFIGURATION Plant_A"));
    assert!(source.contains("RESOURCE CPU_1 ON Linux_ARM"));
    assert!(source.contains("TASK _10_ms"));
    assert!(source.contains("PROGRAM Main_Instance WITH _10_ms : Main_Program;"));
    assert!(
        report
            .warnings
            .iter()
            .filter(|warning| warning.contains("normalized"))
            .count()
            >= 3,
        "{:#?}",
        report.warnings
    );
}

#[test]
fn project_model_suffixes_duplicate_configuration_identities() {
    let fixture = ProjectModelFixture::new(
        "duplicate-configurations",
        r#"<instances><configurations>
  <configuration name="Plant"><task name="A"/></configuration>
  <configuration name="PLANT"><task name="B"/></configuration>
  <configuration name="Plant"><task name="C"/></configuration>
</configurations></instances>"#,
    );

    let report = fixture.import().expect("import duplicate configurations");
    let sources = fixture.configuration_sources();

    assert_eq!(report.imported_configurations, 3);
    assert_eq!(sources.len(), 3);
    assert!(sources.iter().any(|(_, text)| text.contains("CONFIGURATION Plant\n")));
    assert!(sources.iter().any(|(_, text)| text.contains("CONFIGURATION PLANT_2\n")));
    assert!(sources.iter().any(|(_, text)| text.contains("CONFIGURATION Plant_3\n")));
}

#[test]
fn project_model_suffixes_duplicate_names_within_each_scope() {
    let fixture = ProjectModelFixture::new(
        "duplicate-scope-identities",
        r#"<instances><configuration name="Plant">
  <task name="Cycle"/><task name="CYCLE"/><task name="Cycle"/>
  <program name="Main" typeName="P"/><program name="MAIN" typeName="P"/>
  <resource name="CPU"/><resource name="cpu"/>
</configuration></instances>"#,
    );

    fixture.import().expect("import duplicate scope identities");
    let source = fixture.only_configuration_source();

    for expected in [
        "TASK Cycle ",
        "TASK CYCLE_2 ",
        "TASK Cycle_3 ",
        "PROGRAM Main ",
        "PROGRAM MAIN_2 ",
        "RESOURCE CPU ",
        "RESOURCE cpu_2 ",
    ] {
        assert!(source.contains(expected), "{expected}\n{source}");
    }
}

#[test]
fn project_model_configuration_program_without_tasks_gets_named_auto_task() {
    let fixture = ProjectModelFixture::new(
        "configuration-auto-task",
        r#"<instances><configuration name="Plant">
  <program name="MainInstance" typeName="Main"/>
</configuration></instances>"#,
    );

    let report = fixture.import().expect("import auto task");
    let source = fixture.only_configuration_source();

    assert!(source.contains("TASK AutoTask (INTERVAL := T#100ms, PRIORITY := 1);"));
    assert!(source.contains("PROGRAM MainInstance WITH AutoTask : Main;"));
    assert!(report
        .unsupported_diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "PLCO507"));
}

#[test]
fn project_model_resource_program_without_tasks_gets_named_auto_task() {
    let fixture = ProjectModelFixture::new(
        "resource-auto-task",
        r#"<instances><configuration name="Plant"><resource name="CPU">
  <program name="MainInstance" typeName="Main"/>
</resource></configuration></instances>"#,
    );

    let report = fixture.import().expect("import resource auto task");
    let source = fixture.only_configuration_source();

    assert!(source.contains("TASK AutoTask (INTERVAL := T#100ms, PRIORITY := 1);"));
    assert!(source.contains("PROGRAM MainInstance WITH AutoTask : Main;"));
    assert!(report
        .unsupported_diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "PLCO506"));
}

#[test]
fn project_model_explicit_unknown_task_reference_fails_closed() {
    let fixture = ProjectModelFixture::new(
        "unknown-task",
        r#"<instances><configuration name="Plant">
  <task name="Known"/>
  <program name="MainInstance" typeName="Main" taskName="Missing"/>
</configuration></instances>"#,
    );

    let error = fixture
        .import()
        .expect_err("unknown task reference must not be redirected");

    assert!(error.to_string().contains("no importable PLCopen ST content"));
    assert!(fixture.configuration_sources().is_empty());
}

#[test]
fn project_model_task_with_both_interval_and_event_fails_closed() {
    let fixture = ProjectModelFixture::new(
        "ambiguous-task-mode",
        r#"<instances><configuration name="Plant">
  <task name="Ambiguous" interval="T#10ms" single="Start"/>
</configuration></instances>"#,
    );

    let error = fixture
        .import()
        .expect_err("ambiguous scheduling mode must fail");

    assert!(error.to_string().contains("no importable PLCopen ST content"));
    assert!(fixture.configuration_sources().is_empty());
}

#[test]
fn project_model_invalid_explicit_interval_fails_closed() {
    for interval in ["PT-1S", "PTNaNS", "tomorrow", ""] {
        let fixture = ProjectModelFixture::new(
            "invalid-interval",
            &format!(
                r#"<instances><configuration name="Plant"><task name="Cycle" interval="{interval}"/></configuration></instances>"#
            ),
        );

        let error = fixture
            .import()
            .expect_err("invalid explicit interval must fail");
        assert!(error.to_string().contains("no importable PLCopen ST content"));
        assert!(fixture.configuration_sources().is_empty());
    }
}

#[test]
fn project_model_invalid_explicit_priority_fails_closed() {
    for priority in ["-1", "not-a-number", "999999999999999999999"] {
        let fixture = ProjectModelFixture::new(
            "invalid-priority",
            &format!(
                r#"<instances><configuration name="Plant"><task name="Cycle" priority="{priority}"/></configuration></instances>"#
            ),
        );

        let error = fixture
            .import()
            .expect_err("invalid priority must fail");
        assert!(error.to_string().contains("no importable PLCopen ST content"));
        assert!(fixture.configuration_sources().is_empty());
    }
}

#[test]
fn project_model_incomplete_task_is_reported_and_not_rendered() {
    let fixture = ProjectModelFixture::new(
        "incomplete-task",
        r#"<instances><configuration name="Plant">
  <task interval="T#10ms"/>
  <task name="Valid" interval="T#20ms"/>
</configuration></instances>"#,
    );

    let report = fixture.import().expect("import valid remainder");
    let source = fixture.only_configuration_source();

    assert_eq!(report.imported_tasks, 1);
    assert!(source.contains("TASK Valid"));
    assert!(
        report
            .unsupported_diagnostics
            .iter()
            .any(|diagnostic| diagnostic.node.contains("task")),
        "{:#?}",
        report.unsupported_diagnostics
    );
}

#[test]
fn project_model_incomplete_program_is_reported_and_not_rendered() {
    let fixture = ProjectModelFixture::new(
        "incomplete-program",
        r#"<instances><configuration name="Plant">
  <task name="Cycle"/>
  <program name="MissingType"/>
  <program typeName="MissingName"/>
  <program name="Valid" typeName="Main" taskName="Cycle"/>
</configuration></instances>"#,
    );

    let report = fixture.import().expect("import valid remainder");
    let source = fixture.only_configuration_source();

    assert_eq!(report.imported_program_instances, 1);
    assert!(source.contains("PROGRAM Valid WITH Cycle : Main;"));
    assert!(
        report
            .unsupported_diagnostics
            .iter()
            .any(|diagnostic| diagnostic.node.contains("program")),
        "{:#?}",
        report.unsupported_diagnostics
    );
}

#[test]
fn project_model_empty_instances_emits_named_loss_and_no_source() {
    let fixture = ProjectModelFixture::new("empty-instances", "<instances/>");

    let error = fixture
        .import()
        .expect_err("empty instances has no importable source");
    let report = fixture.migration_report();

    assert!(error.to_string().contains("no importable PLCopen ST content"));
    assert_eq!(report["imported_configurations"], 0);
    assert!(report["unsupported_diagnostics"]
        .as_array()
        .expect("diagnostics")
        .iter()
        .any(|diagnostic| diagnostic["code"] == "PLCO501"));
}

#[test]
fn project_model_empty_configuration_is_explicitly_reported() {
    let fixture = ProjectModelFixture::new(
        "empty-configuration",
        r#"<instances><configuration name="Plant"/></instances>"#,
    );

    let report = fixture.import().expect("import explicit empty configuration");
    let source = fixture.only_configuration_source();

    assert_eq!(source, "CONFIGURATION Plant\nEND_CONFIGURATION\n");
    assert!(report
        .unsupported_diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "PLCO508"));
    assert!(report.semantic_loss_percent > 0.0);
}

#[test]
fn project_model_case_insensitive_elements_and_alias_attributes_are_supported() {
    let fixture = ProjectModelFixture::new(
        "case-insensitive",
        r#"<INSTANCES><CONFIGURATIONS><CONFIGURATION CONFIGURATIONNAME="Plant">
  <RESOURCE RESOURCENAME="Controller" TYPE="CPU">
    <TASK TASKNAME="Cycle" PERIOD="PT0.1S" PRIORITY="4">
      <INSTANCE INSTANCENAME="MainInstance" PROGRAMTYPE="Main"/>
    </TASK>
  </RESOURCE>
</CONFIGURATION></CONFIGURATIONS></INSTANCES>"#,
    );

    let report = fixture.import().expect("import case-insensitive model");
    let source = fixture.only_configuration_source();

    assert_eq!(report.imported_resources, 1);
    assert_eq!(report.imported_tasks, 1);
    assert_eq!(report.imported_program_instances, 1);
    assert!(source.contains("TASK Cycle (INTERVAL := T#100ms, PRIORITY := 4);"));
    assert!(source.contains("PROGRAM MainInstance WITH Cycle : Main;"));
}

#[test]
fn project_model_render_order_is_tasks_programs_then_resources() {
    let fixture = ProjectModelFixture::new(
        "render-order",
        r#"<instances><configuration name="Plant">
  <program name="MainInstance" typeName="Main" taskName="TopTask"/>
  <resource name="CPU"><program name="AuxInstance" typeName="Aux"/></resource>
  <task name="TopTask"/>
</configuration></instances>"#,
    );

    fixture.import().expect("import ordering fixture");
    let source = fixture.only_configuration_source();
    let task = source.find("TASK TopTask").expect("top task");
    let program = source.find("PROGRAM MainInstance").expect("top program");
    let resource = source.find("RESOURCE CPU").expect("resource");

    assert!(task < program && program < resource, "{source}");
}
