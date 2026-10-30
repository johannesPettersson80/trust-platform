fn dialect_fixture(label: &str, types_xml: &str) -> SemanticImportFixture {
    SemanticImportFixture::new(label, types_xml)
}

fn unsupported_diagnostic_codes(report: &PlcopenImportReport) -> Vec<String> {
    report
        .unsupported_diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.clone())
        .collect()
}

// e!COCKPIT derived-name projection: `REFERENCE TO` / `POINTER TO` fused
// into the derived name must import verbatim instead of failing the POU
// closed (s4 ask 1 — pointer variables previously lost their POUs).
#[test]
fn pointer_and_reference_derived_names_import_verbatim() {
    assert_eq!(
        parse_type_fixture(r#"<type><derived name="REFERENCE TO stClickEvent" /></type>"#),
        Some("REFERENCE TO stClickEvent".to_string())
    );
    assert_eq!(
        parse_type_fixture(r#"<type><derived name="POINTER TO WORD" /></type>"#),
        Some("POINTER TO WORD".to_string())
    );
    assert_eq!(
        parse_type_fixture(r#"<type><derived name="pointer to app.stCfg" /></type>"#),
        Some("POINTER TO app.stCfg".to_string())
    );
    assert_eq!(
        parse_type_fixture(r#"<type><derived name="REFERENCE TO POINTER TO WORD" /></type>"#),
        Some("REFERENCE TO POINTER TO WORD".to_string())
    );
    // Sized-string bases pass through verbatim: the grammar accepts
    // STRING(<expr>) and STRING[<expr>] with a full constant expression
    // inside (s4 fbMqttPayload* shape).
    assert_eq!(
        parse_type_fixture(
            r#"<type><derived name="REFERENCE TO STRING(GVLMqtt.uiLenMsgState)" /></type>"#
        ),
        Some("REFERENCE TO STRING(GVLMqtt.uiLenMsgState)".to_string())
    );
    assert_eq!(
        parse_type_fixture(r#"<type><derived name="POINTER TO STRING(240)" /></type>"#),
        Some("POINTER TO STRING(240)".to_string())
    );
    assert_eq!(
        parse_type_fixture(r#"<type><derived name="POINTER TO WSTRING[cuiLen]" /></type>"#),
        Some("POINTER TO WSTRING[cuiLen]".to_string())
    );
    // A derived name that is not a projectable identifier still fails closed.
    assert_eq!(
        parse_type_fixture(r#"<type><derived name="REFERENCE TO" /></type>"#),
        None
    );
    // The fused multi-declaration export artifact stays rejected.
    assert_eq!(
        parse_type_fixture(r#"<type><derived name="REFERENCE TO STRING(80); N2 : T2" /></type>"#),
        None
    );

    let fixture = dialect_fixture(
        "pointer-fb",
        r#"<pous>
  <pou name="fbSen" pouType="functionBlock">
    <interface>
      <localVars>
        <variable name="pClick">
          <type><derived name="REFERENCE TO stClickEvent" /></type>
        </variable>
        <variable name="pRaw">
          <type><derived name="POINTER TO WORD" /></type>
        </variable>
      </localVars>
    </interface>
  </pou>
</pous>"#,
    );

    // Body-less FB: this is the shape that previously failed closed
    // (unprojectable pointer types marked the interface invalid, the POU was
    // skipped) — s4's 17/93 missing POUs.
    let report = fixture.import().expect("import succeeds");
    assert_eq!(report.imported_pous, 1, "{:?}", report.warnings);
    let source = fixture.source("fbSen.st");
    assert!(
        source.contains("pClick : REFERENCE TO stClickEvent;"),
        "reference declaration must be carried verbatim:\n{source}"
    );
    assert!(
        source.contains("pRaw : POINTER TO WORD;"),
        "pointer declaration must be carried verbatim:\n{source}"
    );

    let gvl_fixture = dialect_fixture(
        "pointer-gvl",
        r#"<addData>
  <data name="http://www.3s-software.com/plcopenxml/globalvars" handleUnknown="implementation">
    <globalVars name="GVLEvents">
      <variable name="pLastEvent"><type><derived name="REFERENCE TO stClickEvent" /></type></variable>
    </globalVars>
  </data>
</addData>"#,
    );
    let gvl_report = gvl_fixture.import().expect("GVL import succeeds");
    assert_eq!(gvl_report.imported_global_var_lists, 1);
    let gvl_source = gvl_fixture.source("GVLEvents.st");
    assert!(
        gvl_source.contains("pLastEvent : REFERENCE TO stClickEvent;"),
        "GVL reference declarations must be carried verbatim:\n{gvl_source}"
    );

    // s4 fbMqttPayload* shape: pointer to a sized string whose length is a
    // qualified constant expression — the construct that previously failed
    // the FB closed (5 missing POUs).
    let payload_fixture = dialect_fixture(
        "pointer-sized-string",
        r#"<pous>
  <pou name="fbMqttPayloadLight" pouType="functionBlock">
    <interface>
      <localVars>
        <variable name="pPayload">
          <type><derived name="REFERENCE TO STRING(GVLMqtt.uiLenMsgState)" /></type>
        </variable>
      </localVars>
    </interface>
  </pou>
</pous>"#,
    );
    let payload_report = payload_fixture.import().expect("import succeeds");
    assert_eq!(payload_report.imported_pous, 1, "{:?}", payload_report.warnings);
    let payload_source = payload_fixture.source("fbMqttPayloadLight.st");
    assert!(
        payload_source
            .contains("pPayload : REFERENCE TO STRING(GVLMqtt.uiLenMsgState);"),
        "sized-string reference must be carried verbatim:\n{payload_source}"
    );
}

// CODESYS DUT inheritance: the `datatypeinheritance` addData marker must
// produce a native `TYPE x EXTENDS y` declaration (s4 ask 2 — base struct
// fields were previously dropped silently).
#[test]
fn dut_datatypeinheritance_emits_extends_clause() {
    let fixture = dialect_fixture(
        "dut-extends",
        r#"<dataTypes>
  <dataType name="stEntity">
    <baseType>
      <struct>
        <variable name="sID"><type><string length="255" /></type></variable>
      </struct>
    </baseType>
  </dataType>
  <dataType name="stEntityBinSen">
    <baseType>
      <struct>
        <variable name="pClick"><type><derived name="REFERENCE TO stClickEvent" /></type></variable>
      </struct>
    </baseType>
    <addData>
      <data name="http://www.3s-software.com/plcopenxml/datatypeinheritance" handleUnknown="implementation">
        <Inheritance>
          <Extends>stEntity</Extends>
        </Inheritance>
      </data>
    </addData>
  </dataType>
</dataTypes>"#,
    );

    let report = fixture.import().expect("import succeeds");
    assert_eq!(report.imported_data_types, 2);
    let data_types_file = std::fs::read_dir(fixture.root.join("src"))
        .expect("list src")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| {
            path.file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with("plcopen_data_types"))
        })
        .expect("generated data types file");
    let source = std::fs::read_to_string(data_types_file).expect("read data types");
    assert!(
        source.contains("stEntityBinSen EXTENDS stEntity : STRUCT"),
        "DUT inheritance must be emitted as TYPE x EXTENDS y:\n{source}"
    );
    assert!(unsupported_diagnostic_codes(&report).contains(&"PLCO220".to_string()));
}

#[test]
fn dut_with_unprojectable_extends_marker_fails_closed() {
    let fixture = dialect_fixture(
        "dut-extends-invalid",
        r#"<dataTypes>
  <dataType name="stBroken">
    <baseType>
      <struct>
        <variable name="x"><type><BOOL /></type></variable>
      </struct>
    </baseType>
    <addData>
      <data name="http://www.3s-software.com/plcopenxml/datatypeinheritance" handleUnknown="implementation">
        <Inheritance>
          <Extends>not a base</Extends>
        </Inheritance>
      </data>
    </addData>
  </dataType>
</dataTypes>"#,
    );

    let import_error = fixture.import().expect_err("import must fail closed");
    assert!(
        format!("{import_error:#}").contains("no importable PLCopen ST content"),
        "unprojectable inheritance marker must fail the file closed: {import_error:#}"
    );
    let report = fixture.migration_report();
    let codes: Vec<&str> = report["unsupported_diagnostics"]
        .as_array()
        .expect("diagnostics")
        .iter()
        .filter_map(|diagnostic| diagnostic["code"].as_str())
        .collect();
    assert!(
        codes.contains(&"PLCO404"),
        "unprojectable inheritance marker must be reported: {codes:?}"
    );
}

// e!COCKPIT GVL duplicate shapes (s4 ask 3): the MixedAttrsVarList
// attributes view and identical whole-list copies must neither fail nor
// multiply into `_N.st` mirror files.
#[test]
fn gvl_duplicate_copies_do_not_multiply_into_files() {
    let fixture = dialect_fixture(
        "gvl-dedup",
        r#"<addData>
  <data name="http://www.3s-software.com/plcopenxml/globalvars" handleUnknown="implementation">
    <globalVars name="GVLRetainPersist" retain="true" persistent="true">
      <variable name="xBoot"><type><BOOL /></type><initialValue><simpleValue value="TRUE" /></initialValue></variable>
    </globalVars>
  </data>
  <data name="http://www.3s-software.com/plcopenxml/mixedattrsvarlist" handleUnknown="implementation">
    <MixedAttrsVarList>
      <globalVars name="GVLRetainPersist">
        <variable name="xBoot"><type><BOOL /></type><initialValue><simpleValue value="TRUE" /></initialValue></variable>
      </globalVars>
      <globalVars name="GVLRetainPersist" constant="true">
        <variable name="uiLen"><type><UINT /></type><initialValue><simpleValue value="4" /></initialValue></variable>
      </globalVars>
    </MixedAttrsVarList>
  </data>
  <data name="http://www.3s-software.com/plcopenxml/globalvars" handleUnknown="implementation">
    <globalVars name="GVLEntities">
      <variable name="astEntities"><type><array><dimension lower="0" upper="uiLenBinSen" /><baseType><derived name="stEntityBinSen" /></baseType></array></type></variable>
    </globalVars>
  </data>
  <data name="http://www.3s-software.com/plcopenxml/globalvars" handleUnknown="implementation">
    <globalVars name="GVLEntities" constant="true">
      <variable name="astEntities"><type><array><dimension lower="0" upper="uiLenBinSen" /><baseType><derived name="stEntityBinSen" /></baseType></array></type></variable>
    </globalVars>
  </data>
</addData>"#,
    );

    let report = fixture.import().expect("import succeeds");
    assert_eq!(report.imported_global_var_lists, 2, "{:?}", report.warnings);
    // Main GVLRetainPersist + the two MixedAttrsVarList copies + GVLEntities
    // and its identical `constant="true"` copy.
    assert_eq!(report.discovered_global_var_lists, 5);
    let sources = source_files_recursive(&fixture.root.join("src"));
    assert_eq!(
        sources.len(),
        2,
        "duplicate lists must not multiply into files: {sources:?}"
    );

    let retain_source = fixture.source("GVLRetainPersist.st");
    assert!(
        retain_source.contains("VAR_GLOBAL RETAIN PERSISTENT"),
        "container qualifiers must survive import:\n{retain_source}"
    );
    assert!(unsupported_diagnostic_codes(&report).contains(&"PLCO223".to_string()));
}

// s4 ask 4: `<globalVars retain="true" persistent="true">` must render a
// qualified VAR_GLOBAL header.
#[test]
fn gvl_container_qualifiers_survive_import() {
    let fixture = dialect_fixture(
        "gvl-qualifiers",
        r#"<addData>
  <data name="http://www.3s-software.com/plcopenxml/globalvars" handleUnknown="implementation">
    <globalVars name="GVLRetainPersist" retain="true" persistent="true">
      <variable name="xBoot"><type><BOOL /></type><initialValue><simpleValue value="TRUE" /></initialValue></variable>
    </globalVars>
    <globalVars name="GVLConstants" constant="true">
      <variable name="uiLen"><type><UINT /></type><initialValue><simpleValue value="4" /></initialValue></variable>
    </globalVars>
  </data>
</addData>"#,
    );

    let report = fixture.import().expect("import succeeds");
    assert_eq!(report.imported_global_var_lists, 2);
    let retain_source = fixture.source("GVLRetainPersist.st");
    assert!(
        retain_source.contains("VAR_GLOBAL RETAIN PERSISTENT"),
        "RETAIN/PERSISTENT must be carried into the header:\n{retain_source}"
    );
    let constant_source = fixture.source("GVLConstants.st");
    assert!(
        constant_source.contains("VAR_GLOBAL CONSTANT"),
        "CONSTANT must be carried into the header:\n{constant_source}"
    );
    assert!(unsupported_diagnostic_codes(&report).contains(&"PLCO224".to_string()));
}

// s4 ask 5: array dimension bounds are constant expressions for the
// analyzer; the projection must pass them through verbatim (numerics are
// not required).
#[test]
fn symbolic_array_bounds_pass_through_verbatim() {
    let fixture = dialect_fixture(
        "gvl-symbolic-bounds",
        r#"<addData>
  <data name="http://www.3s-software.com/plcopenxml/globalvars" handleUnknown="implementation">
    <globalVars name="GVLEntities">
      <variable name="astEntitiesBinSen"><type><array><dimension lower="0" upper="uiLenBinSen" /><baseType><derived name="stEntityBinSen" /></baseType></array></type></variable>
      <variable name="aiGrid"><type><array><dimension lower="iOffset" upper="iOffset + 7" /><baseType><INT /></baseType></array></type></variable>
    </globalVars>
  </data>
</addData>"#,
    );

    let report = fixture.import().expect("import succeeds");
    assert_eq!(report.imported_global_var_lists, 1);
    let source = fixture.source("GVLEntities.st");
    assert!(
        source.contains("ARRAY[0..uiLenBinSen] OF stEntityBinSen"),
        "symbolic bounds must pass through verbatim:\n{source}"
    );
    assert!(
        source.contains("ARRAY[iOffset..iOffset + 7] OF INT"),
        "bound expressions must pass through verbatim:\n{source}"
    );
}

// s4 ask 7: CODESYS FB inheritance (`pouinheritance` addData) must land on
// the FUNCTION_BLOCK declaration, on both emission paths.
#[test]
fn fb_implements_is_spliced_into_raw_body_declarations() {
    let fixture = dialect_fixture(
        "fb-implements",
        r#"<pous>
  <pou name="fbDrive" pouType="functionBlock">
    <interface>
      <addData>
        <data name="http://www.3s-software.com/plcopenxml/pouinheritance" handleUnknown="implementation">
          <Inheritance>
            <Implements>
              iDrive
              iStatus
            </Implements>
          </Inheritance>
        </data>
      </addData>
    </interface>
    <body><ST><![CDATA[FUNCTION_BLOCK fbDrive
VAR_INPUT
    xOn : BOOL;
END_VAR
END_FUNCTION_BLOCK]]></ST></body>
  </pou>
</pous>"#,
    );

    let report = fixture.import().expect("import succeeds");
    assert_eq!(report.imported_pous, 1);
    let source = fixture.source("fbDrive.st");
    assert!(
        source.contains("FUNCTION_BLOCK fbDrive IMPLEMENTS iDrive, iStatus"),
        "IMPLEMENTS must be spliced onto the declaration line:\n{source}"
    );
    assert!(unsupported_diagnostic_codes(&report).contains(&"PLCO221".to_string()));
}

#[test]
fn fb_extends_and_implements_reach_synthesized_headers() {
    let fixture = dialect_fixture(
        "fb-inheritance-shell",
        r#"<pous>
  <pou name="fbRgb" pouType="functionBlock">
    <interface>
      <addData>
        <data name="http://www.3s-software.com/plcopenxml/pouinheritance" handleUnknown="implementation">
          <Inheritance>
            <Extends>fbLight</Extends>
            <Implements>iLightRgb</Implements>
          </Inheritance>
        </data>
      </addData>
    </interface>
  </pou>
</pous>"#,
    );

    let report = fixture.import().expect("import succeeds");
    assert_eq!(report.imported_pous, 1);
    let source = fixture.source("fbRgb.st");
    assert!(
        source.starts_with("FUNCTION_BLOCK fbRgb EXTENDS fbLight IMPLEMENTS iLightRgb"),
        "declaration shell must carry both clauses:\n{source}"
    );
}

// s4 ask 7: FB `<property>` objects must synthesize METHOD stubs so
// interface completeness checks see the accessor.
#[test]
fn fb_property_accessors_are_synthesized_as_method_stubs() {
    let fixture = dialect_fixture(
        "fb-property",
        r#"<pous>
  <pou name="fbLight" pouType="functionBlock">
    <interface>
      <localVars>
        <variable name="xOn"><type><BOOL /></type></variable>
      </localVars>
    </interface>
    <body><ST><![CDATA[FUNCTION_BLOCK fbLight
END_FUNCTION_BLOCK]]></ST></body>
    <addData>
      <data name="http://www.3s-software.com/plcopenxml/property" handleUnknown="implementation">
        <Property name="Active">
          <interface>
            <returnType><BOOL /></returnType>
          </interface>
        </Property>
      </data>
    </addData>
  </pou>
</pous>"#,
    );

    let report = fixture.import().expect("import succeeds");
    assert_eq!(report.imported_pous, 1);
    let source = fixture.source("fbLight.st");
    let stub_at = source.find("METHOD PUBLIC Active : BOOL")
        .expect("property stub must be synthesized");
    assert!(
        source.contains("Active := FALSE;"),
        "stub must assign a neutral default:\n{source}"
    );
    let end_at = source
        .rfind("END_FUNCTION_BLOCK")
        .expect("FB end keyword");
    assert!(
        stub_at < end_at,
        "property stub must live inside the FUNCTION_BLOCK block:\n{source}"
    );
    assert!(unsupported_diagnostic_codes(&report).contains(&"PLCO225".to_string()));
}

// s4 ask 7: project-level `<Interface>` objects must import as INTERFACE
// declarations instead of failing the whole file as empty.
#[test]
fn interface_objects_import_as_interface_declarations() {
    let fixture = dialect_fixture(
        "interface-object",
        r#"<Interface name="iMqttEntity">
  <Methods>
    <Method name="Publish">
      <interface>
        <returnType><BOOL /></returnType>
        <inputVars>
          <variable name="topic"><type><STRING /></type></variable>
        </inputVars>
        <inOutVars>
          <variable name="payload"><type><derived name="stPayload" /></type></variable>
        </inOutVars>
      </interface>
    </Method>
  </Methods>
  <Properties>
    <Property name="Online">
      <interface>
        <returnType><BOOL /></returnType>
      </interface>
    </Property>
  </Properties>
</Interface>"#,
    );

    let report = fixture.import().expect("import succeeds");
    assert_eq!(report.imported_interfaces, 1);
    let source = fixture.source("iMqttEntity.st");
    assert!(source.contains("INTERFACE iMqttEntity"), "{source}");
    assert!(source.contains("METHOD Publish : BOOL"), "{source}");
    assert!(
        source.contains("    VAR_INPUT topic : STRING; END_VAR"),
        "{source}"
    );
    assert!(
        source.contains("    VAR_IN_OUT payload : stPayload; END_VAR"),
        "{source}"
    );
    assert!(source.contains("METHOD Online : BOOL"), "{source}");
    assert!(source.trim_end().ends_with("END_INTERFACE"), "{source}");
    assert!(unsupported_diagnostic_codes(&report).contains(&"PLCO222".to_string()));

    let entries = fixture.migration_report();
    let entry = entries["entries"]
        .as_array()
        .expect("migration entries")
        .iter()
        .find(|entry| entry["name"] == "iMqttEntity")
        .expect("interface migration entry");
    assert_eq!(entry["status"], "imported");
    assert_eq!(entry["pou_type_raw"], "interface");
}
