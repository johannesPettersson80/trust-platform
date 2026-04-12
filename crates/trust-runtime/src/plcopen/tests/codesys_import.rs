    use crate::harness::TestHarness;
    use crate::numeric::to_i64;
    use crate::plcopen::{
        import_xml_to_project_with_options, PlcopenImportGlobalVarMode, PlcopenImportOptions,
    };
    use crate::value::Value;

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
        assert!(report
            .unsupported_diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "PLCO210"
                && diagnostic.pou.as_deref() == Some("Pump")));
        assert!(report
            .unsupported_diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "PLCO208"
                && diagnostic.pou.as_deref() == Some("Pump")));

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

        let mut harness = TestHarness::from_sources(&[&gvl_text, &prg_text]).expect("compile GVL import");
        let result = harness.cycle();
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert_eq!(harness.get_output("GVL.start"), Some(Value::Bool(true)));
        assert_eq!(harness.get_output("GVL.number"), Some(Value::Int(100)));

        let _ = std::fs::remove_dir_all(project);
    }

    #[test]
    fn import_codesys_qualified_globals_into_namespaced_gvl_without_var_external_injection_and_function_result_assignment() {
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

        let prg_text =
            std::fs::read_to_string(project.join("src/PLC_PRG.st")).expect("read PLC_PRG");
        assert!(!prg_text.contains("VAR_EXTERNAL"));
        assert!(prg_text.contains("GVL.start := TRUE;"));

        let function_text = std::fs::read_to_string(project.join("src/dosomthingfunction.st"))
            .expect("read function");
        assert!(!function_text.contains("VAR_EXTERNAL"));
        assert!(function_text.contains("dosomthingfunction := dosomthingfunction;"));

        let gvl_text =
            std::fs::read_to_string(project.join("src/GVL.st")).expect("read imported GVL");
        assert!(gvl_text.contains("NAMESPACE GVL"));
        assert!(gvl_text.contains("VAR_GLOBAL"));

        let mut harness =
            TestHarness::from_sources(&[&gvl_text, &prg_text, &function_text]).expect("compile imported qualified GVL project");
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

        let prg_text =
            std::fs::read_to_string(project.join("src/PLC_PRG.st")).expect("read PLC_PRG");
        assert!(prg_text.contains("VAR_EXTERNAL"));
        assert!(prg_text.contains("GVL : GVL_TYPE;"));

        let function_text = std::fs::read_to_string(project.join("src/dosomthingfunction.st"))
            .expect("read function");
        assert!(function_text.contains("VAR_EXTERNAL"));
        assert!(function_text.contains("GVL : GVL_TYPE;"));

        let gvl_text =
            std::fs::read_to_string(project.join("src/GVL.st")).expect("read imported GVL");
        assert!(gvl_text.contains("TYPE"));
        assert!(gvl_text.contains("GVL_TYPE : STRUCT"));
        assert!(gvl_text.contains("CONFIGURATION GVL_Globals"));
        assert!(gvl_text.contains("GVL : GVL_TYPE;"));

        let mut harness =
            TestHarness::from_sources(&[&gvl_text, &prg_text, &function_text]).expect("compile imported strict GVL project");
        let result = harness.cycle();
        assert!(result.errors.is_empty(), "{:?}", result.errors);

        let _ = std::fs::remove_dir_all(project);
    }
