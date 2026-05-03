use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use trust_runtime::harness::{CompileSession, SourceFile, TestHarness};
use trust_runtime::plcopen::{
    import_xml_to_project, import_xml_to_project_with_options, PlcopenImportGlobalVarMode,
    PlcopenImportOptions,
};
use trust_runtime::value::Value;

fn temp_dir(prefix: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time went backwards")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("trust-runtime-{prefix}-{stamp}"));
    std::fs::create_dir_all(&dir).expect("create temp directory");
    dir
}

fn write(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create parent");
    }
    std::fs::write(path, content).expect("write file");
}

fn to_i64(value: &Value) -> Option<i64> {
    match value {
        Value::SInt(value) => Some(i64::from(*value)),
        Value::Int(value) => Some(i64::from(*value)),
        Value::DInt(value) => Some(i64::from(*value)),
        Value::LInt(value) => Some(*value),
        Value::USInt(value) => Some(i64::from(*value)),
        Value::UInt(value) => Some(i64::from(*value)),
        Value::UDInt(value) => Some(i64::from(*value)),
        Value::ULInt(value) => i64::try_from(*value).ok(),
        Value::Byte(value) => Some(i64::from(*value)),
        Value::Word(value) => Some(i64::from(*value)),
        Value::DWord(value) => Some(i64::from(*value)),
        Value::LWord(value) => i64::try_from(*value).ok(),
        _ => None,
    }
}

#[test]
fn import_synthesizes_codesys_body_only_and_empty_plaintext_pous() {
    let project = temp_dir("plcopen-import-codesys-shell");
    let xml_path = project.join("input.xml");
    write(
        &xml_path,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://www.plcopen.org/xml/tc6_0200">
  <types>
    <pous>
      <pou name="PLC_PRG" pouType="program">
        <interface>
          <localVars>
            <variable name="waterPump">
              <type>
                <derived name="Pump" />
              </type>
            </variable>
          </localVars>
        </interface>
        <body>
          <ST>
            <xhtml xmlns="http://www.w3.org/1999/xhtml">waterpump();</xhtml>
          </ST>
        </body>
      </pou>
      <pou name="Pump" pouType="program">
        <interface />
        <body>
          <ST>
            <xhtml xmlns="http://www.w3.org/1999/xhtml" />
          </ST>
        </body>
        <addData>
          <data name="http://www.3s-software.com/plcopenxml/interfaceasplaintext" handleUnknown="implementation">
            <InterfaceAsPlainText>
              <xhtml xmlns="http://www.w3.org/1999/xhtml">PROGRAM Pump
VAR
END_VAR
</xhtml>
            </InterfaceAsPlainText>
          </data>
        </addData>
      </pou>
    </pous>
  </types>
</project>
"#,
    );

    let report = import_xml_to_project(&xml_path, &project).expect("import XML");
    assert_eq!(report.imported_pous, 2);
    assert_eq!(report.discovered_pous, 2);
    assert_eq!(report.source_coverage_percent, 100.0);
    assert_eq!(report.compatibility_coverage.verdict, "full");
    assert!(report
        .unsupported_diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "PLCO207"
            && diagnostic.pou.as_deref() == Some("PLC_PRG")));
    assert!(report.unsupported_diagnostics.iter().any(
        |diagnostic| diagnostic.code == "PLCO210" && diagnostic.pou.as_deref() == Some("Pump")
    ));
    assert!(report.unsupported_diagnostics.iter().any(
        |diagnostic| diagnostic.code == "PLCO208" && diagnostic.pou.as_deref() == Some("Pump")
    ));

    let main = std::fs::read_to_string(project.join("src/PLC_PRG.st")).expect("read PLC_PRG");
    assert!(main.contains("PROGRAM PLC_PRG"));
    assert!(main.contains("waterPump : Pump;"));
    assert!(main.contains("waterpump();"));
    assert!(main.contains("END_PROGRAM"));

    let pump = std::fs::read_to_string(project.join("src/Pump.st")).expect("read Pump");
    assert!(pump.contains("FUNCTION_BLOCK Pump"));
    assert!(pump.contains("END_FUNCTION_BLOCK"));
    assert!(!pump.contains("PROGRAM Pump"));

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn import_codesys_global_vars_and_project_structure_into_application_folder() {
    let project = temp_dir("plcopen-import-codesys-gvl-folders");
    let xml_path = project.join("input.xml");
    write(
        &xml_path,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://www.plcopen.org/xml/tc6_0200">
  <types>
    <pous />
  </types>
  <instances>
    <configurations />
  </instances>
  <addData>
    <data name="http://www.3s-software.com/plcopenxml/application" handleUnknown="implementation">
      <resource name="Application">
        <globalVars name="GVL">
          <variable name="start">
            <type>
              <BOOL />
            </type>
          </variable>
          <variable name="number">
            <type>
              <INT />
            </type>
            <initialValue>
              <simpleValue value="100" />
            </initialValue>
          </variable>
          <addData>
            <data name="http://www.3s-software.com/plcopenxml/interfaceasplaintext" handleUnknown="implementation">
              <InterfaceAsPlainText>
                <xhtml xmlns="http://www.w3.org/1999/xhtml">{attribute 'qualified_only'}
VAR_GLOBAL
    start: BOOL;
    number: INT := 100;
END_VAR</xhtml>
              </InterfaceAsPlainText>
            </data>
            <data name="http://www.3s-software.com/plcopenxml/objectid" handleUnknown="discard">
              <ObjectId>gvl-id</ObjectId>
            </data>
          </addData>
        </globalVars>
        <addData>
          <data name="http://www.3s-software.com/plcopenxml/pou" handleUnknown="implementation">
            <pou name="PLC_PRG" pouType="program">
              <body>
                <ST>
                  <xhtml xmlns="http://www.w3.org/1999/xhtml">GVL.start := TRUE;</xhtml>
                </ST>
              </body>
              <addData>
                <data name="http://www.3s-software.com/plcopenxml/objectid" handleUnknown="discard">
                  <ObjectId>pou-id</ObjectId>
                </data>
              </addData>
            </pou>
          </data>
        </addData>
      </resource>
    </data>
    <data name="http://www.3s-software.com/plcopenxml/projectstructure" handleUnknown="discard">
      <ProjectStructure>
        <Object Name="Application" ObjectId="app-id">
          <Object Name="PLC_PRG" ObjectId="pou-id" />
          <Object Name="GVL" ObjectId="gvl-id" />
        </Object>
      </ProjectStructure>
    </data>
  </addData>
</project>
"#,
    );

    let report = import_xml_to_project(&xml_path, &project).expect("import XML");
    assert_eq!(report.detected_ecosystem, "generic-plcopen");
    assert_eq!(report.imported_pous, 1);
    assert_eq!(report.imported_global_var_lists, 1);
    assert!(report.imported_project_structure_nodes >= 3);
    assert_eq!(report.imported_folder_paths, 1);

    let prg = project.join("src/Application/PLC_PRG.st");
    let gvl = project.join("src/Application/GVL.st");
    assert!(prg.is_file(), "expected PLC_PRG in Application folder");
    assert!(gvl.is_file(), "expected GVL in Application folder");

    let prg_text = std::fs::read_to_string(prg).expect("read prg");
    assert!(!prg_text.contains("VAR_EXTERNAL"));
    assert!(prg_text.contains("GVL.start := TRUE;"));

    let gvl_text = std::fs::read_to_string(gvl).expect("read gvl");
    assert!(gvl_text.contains("NAMESPACE GVL"));
    assert!(gvl_text.contains("VAR_GLOBAL"));
    assert!(gvl_text.contains("number : INT := 100;"));
    assert!(gvl_text.contains("END_NAMESPACE"));

    let mut harness =
        TestHarness::from_sources(&[&gvl_text, &prg_text]).expect("compile GVL import");
    let result = harness.cycle();
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert_eq!(harness.get_output("GVL.start"), Some(Value::Bool(true)));
    assert_eq!(harness.get_output("GVL.number"), Some(Value::Int(100)));

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn import_codesys_qualified_globals_into_namespaced_gvl_without_var_external_injection_and_function_result_assignment(
) {
    let project = temp_dir("plcopen-import-codesys-qualified-global-externals");
    let xml_path = project.join("input.xml");
    write(
        &xml_path,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://www.plcopen.org/xml/tc6_0200">
  <types>
    <pous>
      <pou name="PLC_PRG" pouType="program">
        <body>
          <ST>
            <xhtml xmlns="http://www.w3.org/1999/xhtml">GVL.start := TRUE;
dosomthingfunction();</xhtml>
          </ST>
        </body>
      </pou>
      <pou name="dosomthingfunction" pouType="function">
        <interface>
          <returnType>
            <INT />
          </returnType>
        </interface>
        <body>
          <ST>
            <xhtml xmlns="http://www.w3.org/1999/xhtml">IF (GVL.start) THEN
  GVL.number := 200;
END_IF</xhtml>
          </ST>
        </body>
      </pou>
    </pous>
  </types>
  <addData>
    <data name="http://www.3s-software.com/plcopenxml/application" handleUnknown="implementation">
      <resource name="Application">
        <globalVars name="GVL">
          <addData>
            <data name="http://www.3s-software.com/plcopenxml/interfaceasplaintext" handleUnknown="implementation">
              <InterfaceAsPlainText>
                <xhtml xmlns="http://www.w3.org/1999/xhtml">{attribute 'qualified_only'}
VAR_GLOBAL
    start: BOOL;
    number: INT := 100;
END_VAR</xhtml>
              </InterfaceAsPlainText>
            </data>
          </addData>
        </globalVars>
      </resource>
    </data>
  </addData>
</project>
"#,
    );

    let report = import_xml_to_project(&xml_path, &project).expect("import XML");
    assert_eq!(report.imported_pous, 2);
    assert_eq!(report.imported_global_var_lists, 1);

    let prg_text = std::fs::read_to_string(project.join("src/PLC_PRG.st")).expect("read PLC_PRG");
    assert!(!prg_text.contains("VAR_EXTERNAL"));
    assert!(prg_text.contains("GVL.start := TRUE;"));

    let function_text =
        std::fs::read_to_string(project.join("src/dosomthingfunction.st")).expect("read function");
    assert!(!function_text.contains("VAR_EXTERNAL"));
    assert!(function_text.contains("dosomthingfunction := dosomthingfunction;"));

    let gvl_text = std::fs::read_to_string(project.join("src/GVL.st")).expect("read imported GVL");
    assert!(gvl_text.contains("NAMESPACE GVL"));
    assert!(gvl_text.contains("VAR_GLOBAL"));

    let mut harness = TestHarness::from_sources(&[&gvl_text, &prg_text, &function_text])
        .expect("compile imported qualified GVL project");
    let result = harness.cycle();
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert_eq!(harness.get_output("GVL.start"), Some(Value::Bool(true)));
    let number = harness.get_output("GVL.number").expect("read GVL.number");
    assert_eq!(to_i64(&number).expect("numeric GVL.number"), 200);

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn import_injects_var_external_for_qualified_globals_and_function_result_assignment() {
    let project = temp_dir("plcopen-import-codesys-strict-qualified-global-externals");
    let xml_path = project.join("input.xml");
    write(
        &xml_path,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://www.plcopen.org/xml/tc6_0200">
  <types>
    <pous>
      <pou name="PLC_PRG" pouType="program">
        <body>
          <ST>
            <xhtml xmlns="http://www.w3.org/1999/xhtml">GVL.start := TRUE;
dosomthingfunction();</xhtml>
          </ST>
        </body>
      </pou>
      <pou name="dosomthingfunction" pouType="function">
        <interface>
          <returnType>
            <INT />
          </returnType>
        </interface>
        <body>
          <ST>
            <xhtml xmlns="http://www.w3.org/1999/xhtml">IF (GVL.start) THEN
  GVL.number := 200;
END_IF</xhtml>
          </ST>
        </body>
      </pou>
    </pous>
  </types>
  <addData>
    <data name="http://www.3s-software.com/plcopenxml/application" handleUnknown="implementation">
      <resource name="Application">
        <globalVars name="GVL">
          <addData>
            <data name="http://www.3s-software.com/plcopenxml/interfaceasplaintext" handleUnknown="implementation">
              <InterfaceAsPlainText>
                <xhtml xmlns="http://www.w3.org/1999/xhtml">{attribute 'qualified_only'}
VAR_GLOBAL
    start: BOOL;
    number: INT := 100;
END_VAR</xhtml>
              </InterfaceAsPlainText>
            </data>
          </addData>
        </globalVars>
      </resource>
    </data>
  </addData>
</project>
"#,
    );

    let report = import_xml_to_project_with_options(
        &xml_path,
        &project,
        PlcopenImportOptions {
            global_var_mode: PlcopenImportGlobalVarMode::StrictIecAdapter,
        },
    )
    .expect("import XML");
    assert_eq!(report.imported_pous, 2);
    assert_eq!(report.imported_global_var_lists, 1);

    let prg_text = std::fs::read_to_string(project.join("src/PLC_PRG.st")).expect("read PLC_PRG");
    assert!(prg_text.contains("VAR_EXTERNAL"));
    assert!(prg_text.contains("GVL : GVL_TYPE;"));

    let function_text =
        std::fs::read_to_string(project.join("src/dosomthingfunction.st")).expect("read function");
    assert!(function_text.contains("VAR_EXTERNAL"));
    assert!(function_text.contains("GVL : GVL_TYPE;"));

    let gvl_text = std::fs::read_to_string(project.join("src/GVL.st")).expect("read imported GVL");
    assert!(gvl_text.contains("TYPE"));
    assert!(gvl_text.contains("GVL_TYPE : STRUCT"));
    assert!(gvl_text.contains("CONFIGURATION GVL_Globals"));
    assert!(gvl_text.contains("GVL : GVL_TYPE;"));

    let session = CompileSession::from_sources(vec![
        SourceFile::new(gvl_text),
        SourceFile::new(prg_text),
        SourceFile::new(function_text),
    ])
    .with_extra_program_instances(["PLC_PRG"]);
    let mut runtime = session
        .build_runtime()
        .expect("compile imported strict GVL project with explicit PLC_PRG instance");
    runtime.execute_cycle().expect("execute imported PLC_PRG");

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn import_codesys_method_objects_into_function_block_source() {
    let project = temp_dir("plcopen-import-codesys-methods");
    let xml_path = project.join("input.xml");
    write(
        &xml_path,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://www.plcopen.org/xml/tc6_0200">
  <types>
    <pous>
      <pou name="Random" pouType="functionBlock">
        <interface>
          <localVars>
            <variable name="xRandomActive">
              <type>
                <BOOL />
              </type>
              <initialValue>
                <simpleValue value="TRUE" />
              </initialValue>
            </variable>
          </localVars>
        </interface>
        <body>
          <ST>
            <xhtml xmlns="http://www.w3.org/1999/xhtml" />
          </ST>
        </body>
        <addData>
          <data name="http://www.3s-software.com/plcopenxml/method" handleUnknown="implementation">
            <Method name="method1" ObjectId="a3138e68-c2ea-4656-a1dc-8e01cbca04d9">
              <interface>
                <returnType>
                  <BOOL />
                </returnType>
                <inputVars>
                  <variable name="var1">
                    <type>
                      <BOOL />
                    </type>
                  </variable>
                  <variable name="var2">
                    <type>
                      <BOOL />
                    </type>
                  </variable>
                </inputVars>
                <localVars>
                  <variable name="xStartImplementation">
                    <type>
                      <BOOL />
                    </type>
                  </variable>
                </localVars>
              </interface>
              <body>
                <ST>
                  <xhtml xmlns="http://www.w3.org/1999/xhtml">IF var1 AND var2 THEN
    method1 := TRUE;
END_IF;</xhtml>
                </ST>
              </body>
              <addData />
            </Method>
          </data>
          <data name="http://www.3s-software.com/plcopenxml/objectid" handleUnknown="discard">
            <ObjectId>57fd6ef9-f349-4608-8c39-0c8fd7700aa6</ObjectId>
          </data>
        </addData>
      </pou>
    </pous>
  </types>
  <instances>
    <configurations />
  </instances>
  <addData>
    <data name="http://www.3s-software.com/plcopenxml/projectstructure" handleUnknown="discard">
      <ProjectStructure>
        <Object Name="Random" ObjectId="57fd6ef9-f349-4608-8c39-0c8fd7700aa6">
          <Object Name="method1" ObjectId="a3138e68-c2ea-4656-a1dc-8e01cbca04d9" />
        </Object>
      </ProjectStructure>
    </data>
  </addData>
</project>
"#,
    );

    let report = import_xml_to_project(&xml_path, &project).expect("import XML");
    assert_eq!(report.imported_pous, 1);

    let source = std::fs::read_to_string(project.join("src/Random.st")).expect("read Random");
    assert!(source.contains("FUNCTION_BLOCK Random"));
    assert!(source.contains("xRandomActive : BOOL := TRUE;"));
    assert!(source.contains("METHOD PUBLIC method1 : BOOL"));
    assert!(source.contains("VAR_INPUT"));
    assert!(source.contains("var1 : BOOL;"));
    assert!(source.contains("var2 : BOOL;"));
    assert!(source.contains("xStartImplementation : BOOL;"));
    assert!(source.contains("method1 := TRUE;"));
    assert!(source.contains("END_METHOD"));

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn import_tc6_multiple_bodies_and_extended_interface_sections() {
    let project = temp_dir("plcopen-import-tc6-multi-body");
    let xml_path = project.join("input.xml");
    write(
        &xml_path,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://www.plcopen.org/xml/tc6_0200">
  <types>
    <pous>
      <pou name="Main" pouType="program">
        <interface>
          <localVars retain="true">
            <variable name="counter">
              <type>
                <INT />
              </type>
              <initialValue>
                <simpleValue value="1" />
              </initialValue>
            </variable>
          </localVars>
          <globalVars>
            <variable name="shared">
              <type>
                <INT />
              </type>
            </variable>
          </globalVars>
          <accessVars>
            <accessVariable alias="A1" instancePathAndName="Cell_1.Station_1.P1.shared" direction="readOnly">
              <type>
                <INT />
              </type>
            </accessVariable>
          </accessVars>
        </interface>
        <body WorksheetName="Sheet1">
          <ST>
            <xhtml xmlns="http://www.w3.org/1999/xhtml">counter := counter + 1;</xhtml>
          </ST>
        </body>
        <body WorksheetName="Sheet2">
          <ST>
            <xhtml xmlns="http://www.w3.org/1999/xhtml">shared := counter;</xhtml>
          </ST>
        </body>
      </pou>
    </pous>
  </types>
</project>
"#,
    );

    let report = import_xml_to_project(&xml_path, &project).expect("import XML");
    assert_eq!(report.imported_pous, 1);

    let source = std::fs::read_to_string(project.join("src/Main.st")).expect("read Main");
    assert!(source.contains("PROGRAM Main"));
    assert!(source.contains("VAR RETAIN"));
    assert!(source.contains("counter : INT := 1;"));
    assert!(source.contains("VAR_GLOBAL"));
    assert!(source.contains("shared : INT;"));
    assert!(source.contains("VAR_ACCESS"));
    assert!(source.contains("A1 : Cell_1.Station_1.P1.shared : INT READ_ONLY;"));
    assert!(source.contains("counter := counter + 1;"));
    assert!(source.contains("shared := counter;"));
    assert!(source.find("counter := counter + 1;") < source.find("shared := counter;"));

    let _ = std::fs::remove_dir_all(project);
}
