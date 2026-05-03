    #[test]
    fn import_applies_siemens_library_shims_and_reports_them() {
        let project = temp_dir("plcopen-import-siemens-shims");
        let xml_path = project.join("siemens.xml");
        write(
            &xml_path,
            r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://www.plcopen.org/xml/tc6_0200">
  <fileHeader companyName="Siemens AG" productName="TIA Portal V18" />
  <types>
    <pous>
      <pou name="MainOb1" pouType="PRG">
        <body>
          <ST><![CDATA[
PROGRAM MainOb1
VAR
    PulseTimer : SFB3;
    DelayTimer : SFB4;
END_VAR
PulseTimer(IN := TRUE, PT := T#200ms);
DelayTimer(IN := PulseTimer.Q, PT := T#2s);
END_PROGRAM
]]></ST>
        </body>
      </pou>
    </pous>
  </types>
</project>
"#,
        );

        let report = import_xml_to_project(&xml_path, &project).expect("import XML");
        assert_eq!(report.detected_ecosystem, "siemens-tia");
        assert!(!report.applied_library_shims.is_empty());
        assert!(report
            .applied_library_shims
            .iter()
            .any(|entry| entry.source_symbol == "SFB3" && entry.replacement_symbol == "TP"));
        assert!(report
            .applied_library_shims
            .iter()
            .any(|entry| entry.source_symbol == "SFB4" && entry.replacement_symbol == "TON"));
        assert!(report
            .unsupported_diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "PLCO301"));

        let source = std::fs::read_to_string(&report.written_sources[0]).expect("read source");
        assert!(source.contains("PulseTimer : TP;"));
        assert!(source.contains("DelayTimer : TON;"));
        assert!(!source.contains("SFB3"));
        assert!(!source.contains("SFB4"));

        let _ = std::fs::remove_dir_all(project);
    }

    #[test]
    fn library_shim_rewrites_type_and_call_sites_only() {
        let body = r#"
PROGRAM Main
VAR
    SFB4 : BOOL := FALSE;
    DelayTimer : SFB4;
END_VAR
SFB4 := TRUE;
DelayTimer(IN := SFB4, PT := T#1s);
END_PROGRAM
"#;

        let (shimmed, applied) = apply_vendor_library_shims(body, "siemens-tia");
        assert_eq!(applied.len(), 1);
        assert!(shimmed.contains("SFB4 : BOOL := FALSE;"));
        assert!(shimmed.contains("DelayTimer : TON;"));
        assert!(shimmed.contains("SFB4 := TRUE;"));
    }

