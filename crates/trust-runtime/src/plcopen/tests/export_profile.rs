    #[test]
    fn export_emits_codesys_global_vars_and_project_structure_metadata() {
        let project = temp_dir("plcopen-export-codesys-gvl-folders");
        write(
            &project.join("src/Application/PLC_PRG.st"),
            r#"
PROGRAM PLC_PRG
GVL.start := TRUE;
END_PROGRAM
"#,
        );
        write(
            &project.join("src/Application/GVL.st"),
            r#"
{attribute 'qualified_only'}
VAR_GLOBAL
    start: BOOL;
    number: INT := 100;
END_VAR
"#,
        );

        let output = project.join("out/plcopen.xml");
        let report = export_project_to_xml(&project, &output).expect("export XML");
        assert_eq!(report.exported_global_var_lists, 1);
        assert!(report.exported_project_structure_nodes >= 3);
        assert!(report.exported_folder_paths >= 1);

        let xml = std::fs::read_to_string(&output).expect("read xml");
        assert!(xml.contains(CODESYS_APPLICATION_DATA_NAME));
        assert!(xml.contains(CODESYS_PROJECTSTRUCTURE_DATA_NAME));
        assert!(xml.contains("<globalVars name=\"GVL\">"));
        assert!(xml.contains("Object Name=\"GVL\""));

        let imported = temp_dir("plcopen-export-codesys-gvl-folders-import");
        let import_report = import_xml_to_project(&output, &imported).expect("import exported xml");
        assert_eq!(import_report.imported_global_var_lists, 1);
        assert!(imported.join("src/Application/GVL.st").is_file());
        assert!(imported.join("src/Application/PLC_PRG.st").is_file());

        let _ = std::fs::remove_dir_all(project);
        let _ = std::fs::remove_dir_all(imported);
    }

    #[test]
    fn import_rejects_malformed_xml() {
        let project = temp_dir("plcopen-malformed");
        let xml_path = project.join("broken.xml");
        write(&xml_path, "<project><types><pous><pou>");

        let result = import_xml_to_project(&xml_path, &project);
        assert!(result.is_err(), "malformed XML must return error");

        let _ = std::fs::remove_dir_all(project);
    }

    #[test]
    fn export_reinjects_vendor_extension_hook_file() {
        let project = temp_dir("plcopen-export-vendor-hook");
        write(
            &project.join("src/main.st"),
            r#"
PROGRAM Main
END_PROGRAM
"#,
        );
        write(
            &project.join(VENDOR_EXTENSION_HOOK_FILE),
            r#"<vendorData source="external"/>"#,
        );

        let output = project.join("out/plcopen.xml");
        export_project_to_xml(&project, &output).expect("export XML");
        let text = std::fs::read_to_string(output).expect("read output XML");
        assert!(text.contains(VENDOR_EXT_DATA_NAME));
        assert!(text.contains("vendorData"));

        let _ = std::fs::remove_dir_all(project);
    }

    #[test]
    fn export_with_vendor_target_emits_adapter_report_and_metadata() {
        let project = temp_dir("plcopen-export-target-ab");
        write(
            &project.join("src/main.st"),
            r#"
PROGRAM Main
VAR RETAIN
    Counter : INT := 0;
END_VAR
(* address marker for adapter checks: %MW10 *)
END_PROGRAM

CONFIGURATION Plant
TASK MainTask(INTERVAL := T#50ms, PRIORITY := 5);
PROGRAM MainInstance WITH MainTask : Main;
END_CONFIGURATION
"#,
        );

        let output = project.join("out/plcopen.ab.xml");
        let report =
            export_project_to_xml_with_target(&project, &output, PlcopenExportTarget::AllenBradley)
                .expect("export XML with target adapter");

        assert_eq!(report.target, "allen-bradley");
        let adapter_path = report
            .adapter_report_path
            .as_ref()
            .expect("adapter report path");
        assert!(adapter_path.is_file());
        assert!(report
            .adapter_diagnostics
            .iter()
            .any(|entry| entry.code == "PLCO7AB1"));
        assert!(!report.adapter_manual_steps.is_empty());
        assert!(!report.adapter_limitations.is_empty());

        let xml_text = std::fs::read_to_string(&output).expect("read output XML");
        assert!(xml_text.contains(EXPORT_ADAPTER_DATA_NAME));
        assert!(xml_text.contains("allen-bradley"));

        let adapter_text = std::fs::read_to_string(adapter_path).expect("read adapter report");
        assert!(adapter_text.contains("\"target\": \"allen-bradley\""));
        assert!(adapter_text.contains("PLCO7AB1"));

        let _ = std::fs::remove_dir_all(project);
    }

    #[test]
    fn export_siemens_target_emits_scl_bundle_and_program_ob_mapping() {
        let project = temp_dir("plcopen-export-target-siemens-scl");
        write(
            &project.join("src/main.st"),
            r#"
TYPE
    EMode : (Idle, Run);
END_TYPE

FUNCTION_BLOCK FB_Counter
VAR_INPUT
    Enable : BOOL;
END_VAR
VAR_OUTPUT
    Count : INT;
END_VAR
IF Enable THEN
    Count := Count + 1;
END_IF
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    Counter : FB_Counter;
END_VAR
Counter(Enable := TRUE);
END_PROGRAM

CONFIGURATION Plant
TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
PROGRAM MainInstance WITH MainTask : Main;
END_CONFIGURATION
"#,
        );

        let output = project.join("out/plcopen.siemens.xml");
        let report =
            export_project_to_xml_with_target(&project, &output, PlcopenExportTarget::Siemens)
                .expect("export XML with Siemens target");

        let bundle_dir = report
            .siemens_scl_bundle_dir
            .as_ref()
            .expect("siemens scl bundle dir");
        assert!(bundle_dir.is_dir(), "expected Siemens SCL bundle directory");
        assert!(
            report.siemens_scl_files.iter().all(|path| path.is_file()),
            "expected all Siemens SCL files to be written"
        );
        assert!(
            report
                .siemens_scl_files
                .iter()
                .any(|path| path.extension().and_then(|value| value.to_str()) == Some("scl")),
            "expected at least one .scl file"
        );

        let main_scl = report
            .siemens_scl_files
            .iter()
            .find(|path| {
                path.file_name()
                    .and_then(|value| value.to_str())
                    .is_some_and(|name| name.contains("_ob_Main.scl"))
            })
            .expect("main OB file");
        let main_text = std::fs::read_to_string(main_scl).expect("read main scl file");
        assert!(main_text.contains("ORGANIZATION_BLOCK \"Main\""));
        assert!(main_text.contains("END_ORGANIZATION_BLOCK"));

        let adapter_path = report
            .adapter_report_path
            .as_ref()
            .expect("adapter report path");
        let adapter_text = std::fs::read_to_string(adapter_path).expect("read adapter report");
        assert!(adapter_text.contains("\"target\": \"siemens-tia\""));
        assert!(adapter_text.contains("siemens_scl_bundle_dir"));

        let _ = std::fs::remove_dir_all(project);
    }

    #[test]
    fn profile_declares_strict_subset_contract() {
        let profile = supported_profile();
        assert_eq!(profile.namespace, PLCOPEN_NAMESPACE);
        assert_eq!(profile.profile, PROFILE_NAME);
        assert!(profile
            .strict_subset
            .iter()
            .any(|item| item.contains("types/pous/pou")));
        assert!(profile
            .compatibility_matrix
            .iter()
            .any(|entry| entry.status == "supported"));
        assert!(profile.compatibility_matrix.iter().any(|entry| {
            entry.status == "partial" && entry.capability.contains("compatibility shims")
        }));
        assert!(!profile.round_trip_limits.is_empty());
        assert!(!profile.known_gaps.is_empty());
    }

    #[test]
    fn export_emits_codesys_pou_add_data_metadata() {
        let project = temp_dir("plcopen-export-codesys-pou-add-data");
        write(
            &project.join("src/Application/PLC_PRG.st"),
            r#"
PROGRAM PLC_PRG
VAR
    x : INT := 0;
END_VAR
x := x + 1;
END_PROGRAM
"#,
        );
        write(
            &project.join("src/Application/doThing.st"),
            r#"
FUNCTION doThing : INT
doThing := 1;
END_FUNCTION
"#,
        );

        let output = project.join("out/plcopen.xml");
        export_project_to_xml(&project, &output).expect("export XML");

        let xml = std::fs::read_to_string(&output).expect("read xml");
        assert!(xml.contains(CODESYS_APPLICATION_DATA_NAME));
        assert!(xml.contains("http://www.3s-software.com/plcopenxml/pou"));
        assert!(
            xml.contains("<pou name=\"PLC_PRG\" pouType=\"program\">"),
            "expected CODESYS addData pou metadata for PLC_PRG"
        );
        assert!(
            xml.contains("<pou name=\"doThing\" pouType=\"function\">"),
            "expected CODESYS addData pou metadata for doThing"
        );
        assert!(xml.contains(CODESYS_INTERFACE_PLAINTEXT_DATA_NAME));
        assert!(xml.contains(CODESYS_OBJECT_ID_DATA_NAME));

        let _ = std::fs::remove_dir_all(project);
    }

    #[test]
    fn export_emits_codesys_method_metadata_on_standard_pous_and_roundtrips() {
        let project = temp_dir("plcopen-export-codesys-methods");
        write(
            &project.join("src/Application/Random.st"),
            r#"
FUNCTION_BLOCK Random
VAR
    xRandomActive : BOOL := TRUE;
END_VAR

METHOD PUBLIC method1 : BOOL
VAR_INPUT
    var1 : BOOL;
    var2 : BOOL;
END_VAR
IF var1 AND var2 THEN
    method1 := TRUE;
END_IF
END_METHOD

END_FUNCTION_BLOCK
"#,
        );

        let output = project.join("out/plcopen.xml");
        let report = export_project_to_xml(&project, &output).expect("export XML");
        assert!(report.exported_project_structure_nodes >= 3);

        let xml = std::fs::read_to_string(&output).expect("read xml");
        assert!(xml.contains(CODESYS_METHOD_DATA_NAME));
        assert!(xml.contains("<Method name=\"method1\""));
        assert!(xml.contains("Object Name=\"method1\""));
        assert!(xml.contains(CODESYS_OBJECT_ID_DATA_NAME));
        assert!(xml.contains(CODESYS_INTERFACE_PLAINTEXT_DATA_NAME));

        let imported = temp_dir("plcopen-export-codesys-methods-import");
        let import_report = import_xml_to_project(&output, &imported).expect("import exported xml");
        assert_eq!(import_report.imported_pous, 1);

        let source = std::fs::read_to_string(imported.join("src/Application/Random.st"))
            .expect("read imported Random");
        assert!(source.contains("FUNCTION_BLOCK Random"));
        assert!(source.contains("METHOD PUBLIC method1 : BOOL"));
        assert!(source.contains("var1 : BOOL;"));
        assert!(source.contains("var2 : BOOL;"));
        assert!(source.contains("method1 := TRUE;"));

        let _ = std::fs::remove_dir_all(project);
        let _ = std::fs::remove_dir_all(imported);
    }
