    #[test]
    fn round_trip_export_import_export_preserves_pou_subset() {
        let source_project = temp_dir("plcopen-roundtrip-src");
        write(
            &source_project.join("src/main.st"),
            r#"
PROGRAM Main
VAR
    speed : REAL := 42.5;
END_VAR
END_PROGRAM
"#,
        );
        write(
            &source_project.join("src/calc.st"),
            r#"
FUNCTION Calc : INT
VAR_INPUT
    A : INT;
END_VAR
Calc := A + 1;
END_FUNCTION
"#,
        );

        let xml_a = source_project.join("build/plcopen.xml");
        let export_a = export_project_to_xml(&source_project, &xml_a).expect("export A");
        assert_eq!(export_a.pou_count, 2);
        assert!(export_a.source_map_path.is_file());

        let import_project = temp_dir("plcopen-roundtrip-import");
        let import = import_xml_to_project(&xml_a, &import_project).expect("import");
        assert_eq!(import.imported_pous, 2);
        assert_eq!(import.discovered_pous, 2);
        assert!(import.migration_report_path.is_file());
        assert_eq!(import.source_coverage_percent, 100.0);
        assert_eq!(import.semantic_loss_percent, 0.0);

        let xml_b = import_project.join("build/plcopen.xml");
        let export_b = export_project_to_xml(&import_project, &xml_b).expect("export B");
        assert_eq!(export_b.pou_count, 2);
        assert!(export_b.source_map_path.is_file());

        let a_text = std::fs::read_to_string(&xml_a).expect("read xml A");
        let b_text = std::fs::read_to_string(&xml_b).expect("read xml B");
        assert_eq!(pou_signatures(&a_text), pou_signatures(&b_text));

        let _ = std::fs::remove_dir_all(source_project);
        let _ = std::fs::remove_dir_all(import_project);
    }

    #[test]
    fn import_reports_unsupported_nodes_and_preserves_vendor_extensions() {
        let project = temp_dir("plcopen-import-unsupported");
        let xml_path = project.join("input.xml");
        write(
            &xml_path,
            r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://www.plcopen.org/xml/tc6_0200">
  <types>
    <dataTypes>
      <dataType name="POINT"/>
    </dataTypes>
    <pous>
      <pou name="Main" pouType="program">
        <body>
          <ST><![CDATA[
PROGRAM Main
VAR
  speed : REAL := 10.0;
END_VAR
END_PROGRAM
]]></ST>
        </body>
      </pou>
    </pous>
  </types>
  <addData>
    <data name="vendor.raw"><text><![CDATA[<vendorNode id="1"/>]]></text></data>
  </addData>
</project>
"#,
        );

        let report = import_xml_to_project(&xml_path, &project).expect("import XML");
        assert_eq!(report.imported_pous, 1);
        assert_eq!(report.discovered_pous, 1);
        assert!(!report.unsupported_nodes.is_empty());
        assert!(report
            .unsupported_nodes
            .iter()
            .any(|entry| entry.contains("types/dataTypes/POINT")));
        assert!(report.migration_report_path.is_file());
        assert!(report.source_coverage_percent > 0.0);
        assert!(report.semantic_loss_percent > 0.0);
        assert_eq!(report.compatibility_coverage.verdict, "partial");
        assert!(report
            .unsupported_diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "PLCO402"));
        let expected_source = project.join("src/Main.st");
        assert_eq!(report.written_sources, vec![expected_source.clone()]);
        let source = std::fs::read_to_string(&expected_source).expect("read source");
        assert!(source.contains("PROGRAM Main"));
        let expected_vendor =
            project.join("plcopen.vendor-extensions.imported.xml");
        let vendor = report
            .preserved_vendor_extensions
            .expect("vendor extension path");
        assert_eq!(vendor, expected_vendor);
        let vendor_text =
            std::fs::read_to_string(&expected_vendor).expect("read vendor ext");
        assert!(vendor_text.contains("vendor.raw"));

        let _ = std::fs::remove_dir_all(project);
    }

    #[test]
    fn import_accepts_st_body_with_body_level_adddata_metadata() {
        let project = temp_dir("plcopen-import-body-adddata");
        let xml_path = project.join("input.xml");
        write(
            &xml_path,
            r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://www.plcopen.org/xml/tc6_0200">
  <types>
    <pous>
      <pou name="Main" pouType="program">
        <body>
          <ST><![CDATA[
PROGRAM Main
VAR
  speed : REAL := 10.0;
END_VAR
END_PROGRAM
]]></ST>
          <addData>
            <data name="vendor.lineMap"><text>line metadata only</text></data>
          </addData>
        </body>
      </pou>
    </pous>
  </types>
</project>
"#,
        );

        let report = import_xml_to_project(&xml_path, &project)
            .expect("ST body with body-level addData should import");
        assert_eq!(report.discovered_pous, 1);
        assert_eq!(report.imported_pous, 1);
        assert!(
            report
                .unsupported_diagnostics
                .iter()
                .all(|diagnostic| diagnostic.code != "PLCO218"),
            "body-level addData must not be reported as an unsupported body: {:?}",
            report.unsupported_diagnostics
        );

        let expected_source = project.join("src/Main.st");
        assert_eq!(report.written_sources, vec![expected_source.clone()]);
        let source = std::fs::read_to_string(&expected_source).expect("read source");
        assert!(source.contains("PROGRAM Main"));
        assert!(source.contains("speed : REAL"));

        let _ = std::fs::remove_dir_all(project);
    }

    #[test]
    fn import_rejects_non_st_bodies_with_named_diagnostics() {
        let project = temp_dir("plcopen-import-non-st-bodies");
        let _fixture_source =
            include_str!("../../../tests/fixtures/plcopen/non_st_bodies.xml");
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("plcopen")
            .join("non_st_bodies.xml");

        let error = import_xml_to_project(&fixture, &project).expect_err("non-ST import fails");
        assert!(
            error
                .to_string()
                .contains("no importable PLCopen ST content"),
            "unexpected error: {error:#}"
        );

        let report_path = project.join("interop/plcopen-migration-report.json");
        assert!(report_path.is_file(), "expected migration report");
        let report_text = std::fs::read_to_string(&report_path).expect("read migration report");
        let report: serde_json::Value =
            serde_json::from_str(&report_text).expect("parse migration report");

        assert_eq!(report["discovered_pous"], 4);
        assert_eq!(report["imported_pous"], 0);
        assert_eq!(report["skipped_pous"], 4);

        for code in ["PLCO215", "PLCO216", "PLCO217", "PLCO218"] {
            assert!(
                report["unsupported_diagnostics"]
                    .as_array()
                    .expect("diagnostics array")
                    .iter()
                    .any(|diagnostic| diagnostic["code"] == code
                        && diagnostic["severity"] == "error"),
                "missing diagnostic code {code}: {report_text}"
            );
        }

        for name in ["FbdPump", "LadderPump", "Sequence", "VendorGraph"] {
            assert!(
                report["entries"]
                    .as_array()
                    .expect("entries array")
                    .iter()
                    .any(|entry| entry["name"] == name
                        && entry["status"] == "skipped"
                        && entry["reason"]
                            .as_str()
                            .is_some_and(|reason| reason.contains("unsupported non-ST body"))),
                "missing skipped entry for {name}: {report_text}"
            );
        }

        assert!(
            !contains_st_file(&project.join("src")),
            "non-ST bodies must not emit scraped ST files"
        );

        let _ = std::fs::remove_dir_all(project);
    }

    #[test]
    fn import_supports_data_type_subset_and_generates_type_source() {
        let project = temp_dir("plcopen-import-datatypes");
        let xml_path = project.join("input.xml");
        write(
            &xml_path,
            r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://www.plcopen.org/xml/tc6_0200">
  <types>
    <dataTypes>
      <dataType name="Speed">
        <baseType>
          <int />
        </baseType>
      </dataType>
      <dataType name="Mode">
        <baseType>
          <enum>
            <values>
              <value name="Off"/>
              <value name="Auto"/>
            </values>
          </enum>
        </baseType>
      </dataType>
      <dataType name="Window">
        <baseType>
          <subrange lower="0" upper="100">
            <baseType><int /></baseType>
          </subrange>
        </baseType>
      </dataType>
      <dataType name="Point">
        <baseType>
          <struct>
            <variable name="X"><type><int /></type></variable>
            <variable name="Y"><type><int /></type></variable>
          </struct>
        </baseType>
      </dataType>
      <dataType name="Samples">
        <baseType>
          <array>
            <dimension lower="0" upper="15"/>
            <baseType><int /></baseType>
          </array>
        </baseType>
      </dataType>
    </dataTypes>
  </types>
</project>
"#,
        );

        let report = import_xml_to_project(&xml_path, &project).expect("import XML");
        assert_eq!(report.imported_pous, 0);
        assert_eq!(report.discovered_pous, 0);
        assert_eq!(report.written_sources.len(), 1);
        assert!(report
            .written_sources
            .iter()
            .any(|path| path.ends_with("plcopen_data_types.st")));
        assert!(!report
            .unsupported_diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "PLCO402"));

        let expected_types = project.join("src/plcopen_data_types.st");
        assert_eq!(report.written_sources, vec![expected_types.clone()]);
        let types_source =
            std::fs::read_to_string(&expected_types).expect("read generated types");
        assert!(types_source.contains("TYPE"));
        assert!(types_source.contains("Speed : INT;"));
        assert!(types_source.contains("Mode : (Off, Auto);"));
        assert!(types_source.contains("Window : INT(0..100);"));
        assert!(types_source.contains("Point : STRUCT"));
        assert!(types_source.contains("X : INT;"));
        assert!(types_source.contains("Y : INT;"));
        assert!(types_source.contains("Samples : ARRAY[0..15] OF INT;"));
        assert!(types_source.contains("END_TYPE"));

        let _ = std::fs::remove_dir_all(project);
    }
