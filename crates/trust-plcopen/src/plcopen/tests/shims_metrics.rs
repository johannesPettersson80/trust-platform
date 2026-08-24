    #[test]
    fn shims_metrics_vendor_library_catalog_rewrites_only_reviewed_positions() {
        let cases = [
            ("siemens-tia", "SFB3", "TP"),
            ("siemens-tia", "SFB4", "TON"),
            ("siemens-tia", "SFB5", "TOF"),
            ("rockwell-studio5000", "TONR", "TON"),
            ("schneider-ecostruxure", "R_EDGE", "R_TRIG"),
            ("codesys", "F_EDGE", "F_TRIG"),
            ("openplc", "R_EDGE", "R_TRIG"),
            ("mitsubishi-gxworks3", "DIFU", "R_TRIG"),
            ("mitsubishi-gxworks3", "DIFD", "F_TRIG"),
        ];

        for (ecosystem, source, replacement) in cases {
            let body = format!(
                "PROGRAM Main\nVAR\n  Instance : {source};\n  {source} : BOOL;\nEND_VAR\n\
                 Instance();\nContainer.{source}();\n{source} := TRUE;\n{source}();\nEND_PROGRAM\n"
            );
            let (rewritten, applications) =
                apply_vendor_library_shims(&body, ecosystem);

            assert!(rewritten.contains(&format!("Instance : {replacement};")));
            assert!(rewritten.contains(&format!("  {source} : BOOL;")));
            assert!(rewritten.contains(&format!("Container.{source}();")));
            assert!(rewritten.contains(&format!("{source} := TRUE;")));
            assert!(rewritten.contains(&format!("{replacement}();")));
            assert_eq!(applications.len(), 1, "{ecosystem} {source}");
            assert_eq!(applications[0].vendor, ecosystem);
            assert_eq!(applications[0].source_symbol, source);
            assert_eq!(applications[0].replacement_symbol, replacement);
            assert_eq!(applications[0].occurrences, 2);
            assert!(!applications[0].notes.is_empty());
        }

        let unchanged = "PROGRAM Main\nUnknownAlias();\nEND_PROGRAM\n";
        for ecosystem in ["beckhoff-twincat", "generic-plcopen", "unknown"] {
            let (body, applications) =
                apply_vendor_library_shims(unchanged, ecosystem);
            assert_eq!(body, unchanged);
            assert!(applications.is_empty());
        }
    }

    #[test]
    fn shims_metrics_vendor_ecosystem_detection_uses_reviewed_precedence_and_fallback() {
        let cases = [
            ("TwinCAT Beckhoff", "beckhoff-twincat"),
            ("OpenPLC Editor", "openplc"),
            ("Schneider EcoStruxure", "schneider-ecostruxure"),
            ("CODESYS 3S-Smart", "codesys"),
            ("Siemens TIA Portal", "siemens-tia"),
            ("Rockwell Studio 5000", "rockwell-studio5000"),
            ("Mitsubishi GX Works MELSOFT", "mitsubishi-gxworks3"),
            ("Independent TC6 producer", "generic-plcopen"),
        ];

        for (hint, expected) in cases {
            let xml = format!(
                r#"<project><fileHeader productName="{hint}"/><addData><data name="{hint}"/></addData></project>"#
            );
            let document = roxmltree::Document::parse(&xml).expect("parse ecosystem fixture");
            assert_eq!(
                detect_vendor_ecosystem(document.root_element(), &xml),
                expected,
                "{hint}"
            );
        }

        let precedence =
            r#"<project><fileHeader productName="Siemens TwinCAT OpenPLC"/></project>"#;
        let document = roxmltree::Document::parse(precedence).expect("parse precedence fixture");
        assert_eq!(
            detect_vendor_ecosystem(document.root_element(), precedence),
            "beckhoff-twincat"
        );
    }

    #[test]
    fn shims_metrics_migration_scoring_covers_zero_full_partial_and_low_partitions() {
        assert_eq!(calculate_source_coverage(0, 0), 0.0);
        assert_eq!(calculate_source_coverage(2, 3), 66.67);

        assert_eq!(calculate_semantic_loss(0, 0, 0, 0), 100.0);
        assert_eq!(calculate_semantic_loss(3, 3, 0, 0), 0.0);
        assert_eq!(calculate_semantic_loss(1, 2, 1, 2), 46.67);

        let none = calculate_compatibility_coverage(0, 0, 0, 0);
        assert_eq!(none.supported_items, 0);
        assert_eq!(none.partial_items, 0);
        assert_eq!(none.unsupported_items, 0);
        assert_eq!(none.support_percent, 0.0);
        assert_eq!(none.verdict, "none");

        let full = calculate_compatibility_coverage(2, 0, 0, 0);
        assert_eq!(full.supported_items, 2);
        assert_eq!(full.partial_items, 0);
        assert_eq!(full.unsupported_items, 0);
        assert_eq!(full.support_percent, 100.0);
        assert_eq!(full.verdict, "full");

        let partial = calculate_compatibility_coverage(2, 0, 1, 2);
        assert_eq!(partial.supported_items, 2);
        assert_eq!(partial.partial_items, 3);
        assert_eq!(partial.unsupported_items, 0);
        assert_eq!(partial.support_percent, 40.0);
        assert_eq!(partial.verdict, "partial");

        let low = calculate_compatibility_coverage(0, 1, 2, 0);
        assert_eq!(low.supported_items, 0);
        assert_eq!(low.partial_items, 2);
        assert_eq!(low.unsupported_items, 1);
        assert_eq!(low.support_percent, 0.0);
        assert_eq!(low.verdict, "low");
        assert_eq!(round_percent(12.3456), 12.35);
    }

    #[test]
    fn shims_metrics_unsupported_structure_inspection_emits_exact_owned_diagnostics() {
        let xml = r#"
<project>
  <unexpected/>
  <types><pous/><dataTypes/><vendorTypes/></types>
  <instances><configuration/><resource/><vendorInstances/></instances>
</project>
"#;
        let document = roxmltree::Document::parse(xml).expect("parse unsupported fixture");
        let mut unsupported_nodes = Vec::new();
        let mut warnings = Vec::new();
        let mut diagnostics = Vec::new();

        inspect_unsupported_structure(
            document.root_element(),
            &mut unsupported_nodes,
            &mut warnings,
            &mut diagnostics,
        );

        assert_eq!(
            unsupported_nodes,
            ["unexpected", "types/vendorTypes", "instances/vendorInstances"]
        );
        assert_eq!(
            diagnostics
                .iter()
                .map(|diagnostic| diagnostic.code.as_str())
                .collect::<Vec<_>>(),
            ["PLCO101", "PLCO102", "PLCO103"]
        );
        assert!(diagnostics
            .iter()
            .all(|diagnostic| diagnostic.severity == "warning"));
        assert!(diagnostics.iter().all(|diagnostic| diagnostic.pou.is_none()));
        assert_eq!(warnings.len(), 3);
    }

    #[test]
    fn shims_metrics_embedded_source_map_accepts_exact_and_rejects_malformed_json() {
        let valid = r#"
<project>
  <addData>
    <data name="trust.sourceMap"><text><![CDATA[
{"profile":"trust-st-complete-v1","namespace":"urn:test","entries":[{"name":"Main","pou_type":"program","source":"src/main.st","line":7}]}
    ]]></text></data>
  </addData>
</project>
"#;
        let document = roxmltree::Document::parse(valid).expect("parse source-map fixture");
        let payload = parse_embedded_source_map(document.root_element())
            .expect("parse valid source-map payload");
        assert_eq!(payload.profile, "trust-st-complete-v1");
        assert_eq!(payload.namespace, "urn:test");
        assert_eq!(payload.entries.len(), 1);
        assert_eq!(payload.entries[0].name, "Main");
        assert_eq!(payload.entries[0].line, 7);

        let malformed = r#"
<project><addData><data name="trust.sourceMap"><text>{not-json}</text></data></addData></project>
"#;
        let document =
            roxmltree::Document::parse(malformed).expect("parse malformed source-map fixture");
        assert!(parse_embedded_source_map(document.root_element()).is_none());

        let absent = r#"<project><addData><data name="vendor.raw"><text>{}</text></data></addData></project>"#;
        let document = roxmltree::Document::parse(absent).expect("parse absent source-map fixture");
        assert!(parse_embedded_source_map(document.root_element()).is_none());
    }

    #[test]
    fn shims_metrics_vendor_extension_preservation_excludes_source_map_data() {
        let project = temp_dir("plcopen-vendor-extension-preservation");
        let xml = r#"
<project>
  <addData>
    <data name="trust.sourceMap"><text>{}</text></data>
    <data name="vendor.raw"><text>opaque payload</text></data>
    <data><text>anonymous opaque payload</text></data>
  </addData>
</project>
"#;
        let document = roxmltree::Document::parse(xml).expect("parse extension fixture");
        let mut warnings = Vec::new();
        let output = preserve_vendor_extensions(
            document.root_element(),
            xml,
            &project,
            &mut warnings,
        )
        .expect("preserve vendor extensions")
        .expect("opaque extension path");

        assert_eq!(output, project.join(IMPORTED_VENDOR_EXTENSION_FILE));
        let content = std::fs::read_to_string(&output).expect("read preserved extensions");
        assert!(content.starts_with("<vendorExtensions>\n"));
        assert!(content.ends_with("</vendorExtensions>\n"));
        assert!(content.contains("vendor.raw"));
        assert!(content.contains("anonymous opaque payload"));
        assert!(!content.contains("trust.sourceMap"));
        assert_eq!(warnings.len(), 1);

        let source_map_only =
            r#"<project><data name="trust.sourceMap"><text>{}</text></data></project>"#;
        let document =
            roxmltree::Document::parse(source_map_only).expect("parse source-map-only fixture");
        assert!(preserve_vendor_extensions(
            document.root_element(),
            source_map_only,
            &project,
            &mut Vec::new(),
        )
        .expect("ignore source-map data")
        .is_none());

        let _ = std::fs::remove_dir_all(project);
    }

    #[test]
    fn shims_metrics_migration_report_writer_creates_pretty_newline_artifact() {
        let project = temp_dir("plcopen-migration-report-writer");
        let report = PlcopenMigrationReport {
            profile: PROFILE_NAME.to_string(),
            namespace: PLCOPEN_NAMESPACE.to_string(),
            source_xml: project.join("input.xml"),
            project_root: project.clone(),
            detected_ecosystem: "generic-plcopen".to_string(),
            discovered_pous: 0,
            importable_pous: 0,
            imported_pous: 0,
            skipped_pous: 0,
            imported_data_types: 0,
            discovered_configurations: 0,
            imported_configurations: 0,
            imported_resources: 0,
            imported_tasks: 0,
            imported_program_instances: 0,
            discovered_global_var_lists: 0,
            imported_global_var_lists: 0,
            imported_project_structure_nodes: 0,
            imported_folder_paths: 0,
            source_coverage_percent: 0.0,
            semantic_loss_percent: 100.0,
            compatibility_coverage: calculate_compatibility_coverage(0, 0, 0, 0),
            unsupported_nodes: Vec::new(),
            unsupported_diagnostics: Vec::new(),
            applied_library_shims: Vec::new(),
            warnings: Vec::new(),
            entries: Vec::new(),
        };

        let output = write_migration_report(&project, &report).expect("write migration report");
        assert_eq!(output, project.join(MIGRATION_REPORT_FILE));
        let content = std::fs::read_to_string(&output).expect("read migration report");
        assert!(content.starts_with("{\n"));
        assert!(content.ends_with("}\n"));
        let value: serde_json::Value =
            serde_json::from_str(&content).expect("parse written migration report");
        assert_eq!(value["profile"], PROFILE_NAME);
        assert_eq!(value["detected_ecosystem"], "generic-plcopen");
        assert_eq!(value["semantic_loss_percent"], 100.0);

        let _ = std::fs::remove_dir_all(project);
    }
