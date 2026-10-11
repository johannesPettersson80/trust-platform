fn action_fixture(label: &str, pous: &str) -> SemanticImportFixture {
    SemanticImportFixture::new(
        label,
        &format!(
            r#"<pous>
{pous}
</pous>"#
        ),
    )
}

fn migration_entry(
    report: &PlcopenImportReport,
    name: &str,
) -> Option<(String, Option<String>)> {
    let text = std::fs::read_to_string(&report.migration_report_path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    value["entries"]
        .as_array()?
        .iter()
        .find(|entry| entry["name"] == *name)
        .map(|entry| {
            (
                entry["status"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
                entry["resolved_pou_type"]
                    .as_str()
                    .map(str::to_string),
            )
        })
}

#[test]
fn action_pou_with_dotted_name_merges_into_parent_program() {
    let fixture = action_fixture(
        "action-merge",
        r#"<pou name="PLC_PRG" pouType="program">
  <body><ST><![CDATA[count := count + 1;]]></ST></body>
</pou>
<pou name="PLC_PRG.DayPayload" pouType="action">
  <body><ST><![CDATA[payload := payload + 1;]]></ST></body>
</pou>"#,
    );

    let report = fixture.import().expect("import succeeds");
    assert!(
        !report
            .warnings
            .iter()
            .any(|warning| warning.contains("unsupported pouType 'action'")),
        "action POU must not be skipped as unsupported: {:?}",
        report.warnings
    );

    let program_source = fixture.source("PLC_PRG.st");
    assert!(
        program_source.contains("ACTION DayPayload:"),
        "parent program must contain the merged action:\n{program_source}"
    );
    assert!(
        program_source.contains("payload := payload + 1;"),
        "action body must be carried into the parent source"
    );
    assert!(program_source.contains("END_ACTION"));

    let entry = migration_entry(&report, "PLC_PRG.DayPayload").expect("action migration entry present");
    assert_eq!(entry.0, "imported");
    assert_eq!(entry.1.as_deref(), Some("action"));
}

#[test]
fn action_pou_declared_before_parent_still_merges() {
    let fixture = action_fixture(
        "action-order",
        r#"<pou name="PLC_PRG.Early" pouType="action">
  <body><ST><![CDATA[i := i + 1;]]></ST></body>
</pou>
<pou name="PLC_PRG" pouType="program">
  <body><ST><![CDATA[j := j + 1;]]></ST></body>
</pou>"#,
    );

    let report = fixture.import().expect("import succeeds");
    let program_source = fixture.source("PLC_PRG.st");
    assert!(program_source.contains("ACTION Early:"));
    assert!(program_source.contains("i := i + 1;"));
    assert!(
        !report
            .warnings
            .iter()
            .any(|warning| warning.contains("unsupported pouType")),
        "{:?}",
        report.warnings
    );
}

#[test]
fn action_pou_on_function_block_parent_merges() {
    let fixture = action_fixture(
        "action-fb",
        r#"<pou name="FBMachine" pouType="functionBlock">
  <body><ST><![CDATA[step := step + 1;]]></ST></body>
</pou>
<pou name="FBMachine.DoWork" pouType="action">
  <body><ST><![CDATA[work := work + 1;]]></ST></body>
</pou>"#,
    );

    fixture.import().expect("import succeeds");
    let fb_source = fixture.source("FBMachine.st");
    assert!(
        fb_source.contains("ACTION DoWork:"),
        "FB parent must contain the merged action:\n{fb_source}"
    );
}

#[test]
fn orphan_action_without_parent_linkage_is_skipped_with_plco203() {
    let fixture = action_fixture(
        "action-orphan",
        r#"<pou name="PLC_PRG" pouType="program">
  <body><ST><![CDATA[count := count + 1;]]></ST></body>
</pou>
<pou name="LooseAction" pouType="action">
  <body><ST><![CDATA[x := 1;]]></ST></body>
</pou>"#,
    );

    let report = fixture.import().expect("import succeeds");
    assert!(
        report
            .unsupported_diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "PLCO203"
                && diagnostic.pou.as_deref() == Some("LooseAction")),
        "orphan action keeps the PLCO203 skip: {:?}",
        report.unsupported_diagnostics
    );
    let entry = migration_entry(&report, "LooseAction").expect("orphan action entry present");
    assert_eq!(entry.0, "skipped");
}

#[test]
fn action_pou_without_body_is_skipped() {
    let fixture = action_fixture(
        "action-nobody",
        r#"<pou name="PLC_PRG" pouType="program">
  <body><ST><![CDATA[count := count + 1;]]></ST></body>
</pou>
<pou name="PLC_PRG.Empty" pouType="action" />"#,
    );

    let report = fixture.import().expect("import succeeds");
    let program_source = fixture.source("PLC_PRG.st");
    assert!(!program_source.contains("ACTION Empty:"));
    let entry = migration_entry(&report, "PLC_PRG.Empty").expect("action entry present");
    assert_eq!(entry.0, "skipped");
}
