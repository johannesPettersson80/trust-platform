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

// Real e!COCKPIT exports nest textual actions structurally under the parent
// POU (`<pou><actions><action name="Plain">`) and emit no pouType="action"
// POUs at all — see plcopensrc/PFC200_2ETH_RS/Application.fbMqttPayloadCreate.xml.

#[test]
fn structurally_nested_actions_merge_into_parent_function_block() {
    let fixture = action_fixture(
        "action-structural-fb",
        r#"<pou name="fbMqttPayloadCreate" pouType="functionBlock">
  <interface>
    <localVars>
      <variable name="count">
        <type><INT /></type>
      </variable>
    </localVars>
  </interface>
  <actions>
    <action name="StatePayload">
      <body><ST><![CDATA[count := count + 1;]]></ST></body>
    </action>
    <action name="EventPayload">
      <body><ST><![CDATA[count := count + 2;]]></ST></body>
    </action>
  </actions>
  <body><ST><![CDATA[StatePayload();
EventPayload();]]></ST></body>
</pou>"#,
    );

    let report = fixture.import().expect("import succeeds");
    let fb_source = fixture.source("fbMqttPayloadCreate.st");
    assert!(
        fb_source.contains("ACTION StatePayload:") && fb_source.contains("ACTION EventPayload:"),
        "both structurally nested actions must merge into the FB:\n{fb_source}"
    );
    assert!(fb_source.contains("count := count + 1;"));
    assert!(fb_source.contains("count := count + 2;"));
    assert!(fb_source.contains("END_ACTION"));
    let entry = migration_entry(&report, "fbMqttPayloadCreate.StatePayload")
        .expect("structural action migration entry present");
    assert_eq!(entry.0, "imported");
    assert_eq!(entry.1.as_deref(), Some("action"));
    assert!(
        !report
            .warnings
            .iter()
            .any(|warning| warning.contains("skipping action pou")),
        "structural actions must not be reported as lost: {:?}",
        report.warnings
    );
}

#[test]
fn structurally_nested_action_without_body_skips_fail_closed() {
    let fixture = action_fixture(
        "action-structural-nobody",
        r#"<pou name="fbDevice" pouType="functionBlock">
  <actions>
    <action name="Empty" />
  </actions>
  <body><ST><![CDATA[Empty();]]></ST></body>
</pou>"#,
    );

    let report = fixture.import().expect("import succeeds");
    let fb_source = fixture.source("fbDevice.st");
    assert!(!fb_source.contains("ACTION Empty:"));
    assert!(
        report
            .unsupported_diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "PLCO204"),
        "body-less structural action keeps the PLCO204 skip: {:?}",
        report.unsupported_diagnostics
    );
    let entry = migration_entry(&report, "fbDevice.Empty").expect("action entry present");
    assert_eq!(entry.0, "skipped");
}

#[test]
fn structurally_nested_actions_route_to_their_own_parents() {
    let fixture = action_fixture(
        "action-structural-routing",
        r#"<pou name="PouOne" pouType="functionBlock">
  <actions>
    <action name="WorkOne">
      <body><ST><![CDATA[a := a + 1;]]></ST></body>
    </action>
  </actions>
  <body><ST><![CDATA[WorkOne();]]></ST></body>
</pou>
<pou name="PouTwo" pouType="functionBlock">
  <actions>
    <action name="WorkTwo">
      <body><ST><![CDATA[b := b + 1;]]></ST></body>
    </action>
  </actions>
  <body><ST><![CDATA[WorkTwo();]]></ST></body>
</pou>"#,
    );

    fixture.import().expect("import succeeds");
    let one = fixture.source("PouOne.st");
    let two = fixture.source("PouTwo.st");
    assert!(
        one.contains("ACTION WorkOne:") && !one.contains("ACTION WorkTwo:"),
        "PouOne must carry only its own action:\n{one}"
    );
    assert!(
        two.contains("ACTION WorkTwo:") && !two.contains("ACTION WorkOne:"),
        "PouTwo must carry only its own action:\n{two}"
    );
}
