#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time after epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "trust-conformance-{name}-{}-{stamp}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).expect("create conformance test directory");
        path
    }

    fn case_with_manifest(manifest: CaseManifest) -> CaseDefinition {
        CaseDefinition {
            id: "cfm_timers_matrix_case_001".to_string(),
            category: "timers".to_string(),
            dir: PathBuf::new(),
            manifest,
        }
    }

    #[test]
    fn case_id_validation_matches_naming_rules() {
        assert!(is_valid_case_id("cfm_timers_ton_sequence_001", "timers"));
        assert!(is_valid_case_id(
            "cfm_memory_map_sync_word_123",
            "memory_map"
        ));
        assert!(is_valid_case_id(
            "cfm_comms_determinism_connector_projection_001",
            "comms_determinism"
        ));
        assert!(!is_valid_case_id("CFM_timers_ton_sequence_001", "timers"));
        assert!(!is_valid_case_id("cfm_timers_ton_sequence_01", "timers"));
        assert!(!is_valid_case_id("cfm_edges_case_001", "timers"));
        assert!(!is_valid_case_id("cfm_timers_001", "timers"));
        assert!(!is_valid_case_id("cfm_timers__001", "timers"));
        assert!(!is_valid_case_id("cfm_timers_ton__001", "timers"));
    }

    #[test]
    fn discovery_rejects_case_directories_without_manifests() {
        let root = temp_dir("missing-manifest");
        let case_dir = root
            .join("cases/timers")
            .join("cfm_timers_missing_manifest_001");
        std::fs::create_dir_all(&case_dir).expect("create case directory");
        std::fs::write(case_dir.join("program.st"), "PROGRAM Main\nEND_PROGRAM\n")
            .expect("write program");

        let error = discover_cases(&root).expect_err("missing manifest must fail discovery");
        assert!(error.to_string().contains("manifest.toml"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn discovery_rejects_unknown_category_directories() {
        let root = temp_dir("unknown-category");
        let case_dir = root
            .join("cases/timres")
            .join("cfm_timres_typo_category_001");
        std::fs::create_dir_all(&case_dir).expect("create unknown category case directory");
        std::fs::write(
            case_dir.join("manifest.toml"),
            "id = \"cfm_timres_typo_category_001\"\ncategory = \"timres\"\n",
        )
        .expect("write unknown category manifest");
        std::fs::write(case_dir.join("program.st"), "PROGRAM Main\nEND_PROGRAM\n")
            .expect("write program");

        let error = discover_cases(&root).expect_err("unknown category must fail discovery");
        assert!(error.to_string().contains("unknown conformance category"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn discovery_rejects_manifest_identity_and_missing_suite_root() {
        let missing = temp_dir("missing-suite-root").join("absent");
        assert!(resolve_suite_root(Some(missing))
            .expect_err("missing suite root must fail")
            .to_string()
            .contains("does not exist or is not a directory"));

        let root = temp_dir("manifest-identity");
        let case_dir = root
            .join("cases/timers")
            .join("cfm_timers_manifest_identity_001");
        std::fs::create_dir_all(&case_dir).expect("create case directory");
        std::fs::write(case_dir.join("program.st"), "PROGRAM Main\nEND_PROGRAM\n")
            .expect("write program");
        std::fs::write(
            case_dir.join("manifest.toml"),
            "id = \"cfm_timers_wrong_identity_001\"\ncategory = \"timers\"\n",
        )
        .expect("write mismatched manifest");
        assert!(discover_cases(&root)
            .expect_err("manifest ID mismatch must fail")
            .to_string()
            .contains("does not match case directory"));

        std::fs::write(
            case_dir.join("manifest.toml"),
            "id = \"cfm_timers_manifest_identity_001\"\ncategory = \"edges\"\n",
        )
        .expect("write category-mismatched manifest");
        assert!(discover_cases(&root)
            .expect_err("manifest category mismatch must fail")
            .to_string()
            .contains("does not match directory category"));

        std::fs::write(
            case_dir.join("manifest.toml"),
            "description = \"missing authority\"\n",
        )
        .expect("write manifest without identity");
        assert!(parse_manifest(&case_dir.join("manifest.toml"))
            .expect_err("missing manifest ID must fail")
            .to_string()
            .contains("missing non-empty `id`"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn case_sources_must_remain_inside_the_case_directory() {
        let root = temp_dir("source-containment");
        let case_dir = root.join("case");
        std::fs::create_dir_all(&case_dir).expect("create case directory");
        std::fs::write(root.join("outside.st"), "PROGRAM Outside\nEND_PROGRAM\n")
            .expect("write outside source");
        let case = CaseDefinition {
            id: "cfm_timers_source_containment_001".to_string(),
            category: "timers".to_string(),
            dir: case_dir,
            manifest: CaseManifest {
                id: "cfm_timers_source_containment_001".to_string(),
                category: "timers".to_string(),
                sources: vec!["../outside.st".to_string()],
                ..CaseManifest::default()
            },
        };

        let error = load_case_sources(&case).expect_err("escaping source path must fail");
        assert!(error.to_string().contains("remain inside"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn compile_error_cases_cannot_bless_successful_compilation() {
        let case = CaseDefinition {
            id: "cfm_memory_map_compile_error_guard_001".to_string(),
            category: "memory_map".to_string(),
            dir: PathBuf::new(),
            manifest: CaseManifest {
                id: "cfm_memory_map_compile_error_guard_001".to_string(),
                category: "memory_map".to_string(),
                kind: CaseKind::CompileError,
                ..CaseManifest::default()
            },
        };
        let sources = vec!["PROGRAM Main\nEND_PROGRAM\n".to_string()];

        let error = execute_compile_error_case(&case, &sources)
            .expect_err("successful compilation must invalidate compile_error case");
        assert!(error.to_string().contains("compiled successfully"));
    }

    #[test]
    fn summary_contract_remains_v1_for_legacy_category_only_suites() {
        let cases = vec![CaseDefinition {
            id: "cfm_timers_ton_sequence_001".to_string(),
            category: "timers".to_string(),
            dir: PathBuf::new(),
            manifest: CaseManifest::default(),
        }];
        let contract = summary_contract_for_cases(&cases);
        assert_eq!(contract.version, 1);
        assert_eq!(contract.profile, "trust-conformance-v1");
    }

    #[test]
    fn summary_contract_moves_to_v2_for_expanded_categories() {
        let cases = vec![CaseDefinition {
            id: "cfm_strings_literal_len_001".to_string(),
            category: "strings".to_string(),
            dir: PathBuf::new(),
            manifest: CaseManifest::default(),
        }];
        let contract = summary_contract_for_cases(&cases);
        assert_eq!(contract.version, 2);
        assert_eq!(contract.profile, "trust-conformance-v2");
    }

    #[test]
    fn parse_typed_values_supports_core_manifest_types() {
        assert_eq!(
            parse_typed_value("BOOL:true").expect("bool"),
            Value::Bool(true)
        );
        assert_eq!(parse_typed_value("INT:-4").expect("int"), Value::Int(-4));
        assert_eq!(parse_typed_value("WORD:41").expect("word"), Value::Word(41));
        assert_eq!(
            parse_typed_value("TIME:10ms").expect("time"),
            Value::Time(Duration::from_millis(10))
        );
    }

    #[test]
    fn series_restart_and_step_validation_cover_closed_partitions() {
        let mut manifest = CaseManifest {
            cycles: 2,
            ..CaseManifest::default()
        };
        assert!(validate_series_lengths(&case_with_manifest(manifest.clone()), 2).is_ok());

        manifest.advance_ms = vec![1];
        assert!(validate_series_lengths(&case_with_manifest(manifest.clone()), 2)
            .expect_err("short advance series must fail")
            .to_string()
            .contains("advance_ms length"));
        manifest.advance_ms = vec![1, 2];

        manifest
            .input_series
            .insert("Input".to_string(), vec!["INT:1".to_string()]);
        assert!(validate_series_lengths(&case_with_manifest(manifest.clone()), 2)
            .expect_err("short named-input series must fail")
            .to_string()
            .contains("input series"));
        manifest.input_series.clear();

        manifest
            .direct_input_series
            .insert("%IX0.0".to_string(), vec!["BOOL:true".to_string()]);
        assert!(validate_series_lengths(&case_with_manifest(manifest.clone()), 2)
            .expect_err("short direct-input series must fail")
            .to_string()
            .contains("direct input series"));
        manifest.direct_input_series.clear();

        for before_cycle in [0, 3] {
            manifest.restarts = vec![RestartDirective {
                before_cycle,
                mode: "warm".to_string(),
            }];
            assert!(validate_series_lengths(&case_with_manifest(manifest.clone()), 2)
                .expect_err("out-of-range restart must fail")
                .to_string()
                .contains("must be within 1..=2"));
        }

        for mode in ["warm", "hot", "fault"] {
            assert_eq!(parse_restart_mode(mode).expect("warm-like mode"), RestartMode::Warm);
        }
        for mode in ["cold", "download"] {
            assert_eq!(parse_restart_mode(mode).expect("cold-like mode"), RestartMode::Cold);
        }
        assert!(parse_restart_mode("resume").is_err());
        assert!(should_skip_step_value("skip"));
        assert!(should_skip_step_value("SKIP"));
        assert!(should_skip_step_value("_"));
        assert!(!should_skip_step_value("BOOL:false"));
    }

    #[test]
    fn typed_value_parser_and_encoder_cover_supported_scalar_contract() {
        let cases = [
            ("BOOL:false", Value::Bool(false), "BOOL"),
            ("SINT:-8", Value::SInt(-8), "SINT"),
            ("INT:-16", Value::Int(-16), "INT"),
            ("DINT:-32", Value::DInt(-32), "DINT"),
            ("LINT:-64", Value::LInt(-64), "LINT"),
            ("USINT:8", Value::USInt(8), "USINT"),
            ("UINT:16", Value::UInt(16), "UINT"),
            ("UDINT:32", Value::UDInt(32), "UDINT"),
            ("ULINT:64", Value::ULInt(64), "ULINT"),
            ("BYTE:8", Value::Byte(8), "BYTE"),
            ("WORD:16", Value::Word(16), "WORD"),
            ("DWORD:32", Value::DWord(32), "DWORD"),
            ("LWORD:64", Value::LWord(64), "LWORD"),
            ("REAL:1.5", Value::Real(1.5), "REAL"),
            ("LREAL:2.5", Value::LReal(2.5), "LREAL"),
            (
                "TIME:3s",
                Value::Time(Duration::from_secs(3)),
                "TIME",
            ),
            (
                "LTIME:4ns",
                Value::LTime(Duration::from_nanos(4)),
                "LTIME",
            ),
            (
                "STRING:hello",
                Value::String("hello".to_string().into()),
                "STRING",
            ),
        ];
        for (raw, expected, encoded_type) in cases {
            let parsed = parse_typed_value(raw).unwrap_or_else(|error| {
                panic!("parse {raw}: {error}");
            });
            assert_eq!(parsed, expected, "{raw}");
            assert_eq!(encode_value(&parsed)["type"], encoded_type, "{raw}");
        }

        for raw in [
            "missing-kind-separator",
            "BOOL:maybe",
            "SINT:128",
            "UINT:-1",
            "TIME:1minute",
            "UNKNOWN:1",
        ] {
            assert!(parse_typed_value(raw).is_err(), "{raw} must fail");
        }
    }

    #[test]
    fn connector_status_trace_rejects_unknown_sources_states_and_expectations() {
        for source in [
            "ads_connection",
            "ads_status",
            "opcua_client",
            "opcua_server",
            "mqtt_session",
            "modbus",
            "ethercat",
            "io_driver",
        ] {
            let step = ConnectorStatusTraceStep {
                source: source.to_string(),
                state: "definitely_unknown".to_string(),
                ..ConnectorStatusTraceStep::default()
            };
            assert!(
                project_connector_status_step(&step).is_err(),
                "{source} must reject an unknown state"
            );
        }

        let unknown_source = ConnectorStatusTraceStep {
            source: "unknown-protocol".to_string(),
            state: "ready".to_string(),
            ..ConnectorStatusTraceStep::default()
        };
        assert!(project_connector_status_step(&unknown_source)
            .expect_err("unknown source must fail")
            .to_string()
            .contains("unsupported connector status source"));

        assert!(ensure_expected("state", "ready", "READY").is_ok());
        assert!(ensure_expected("state", "ready", "not-ready")
            .expect_err("mismatched expected state must fail")
            .to_string()
            .contains("expected connector state"));
    }

    #[test]
    fn unix_split_produces_epoch() {
        let parts = split_unix_utc(0);
        assert_eq!(parts, (1970, 1, 1, 0, 0, 0));
    }

    #[test]
    fn unix_split_handles_pre_epoch_and_leap_day_boundaries() {
        assert_eq!(split_unix_utc(-1), (1969, 12, 31, 23, 59, 59));
        assert_eq!(split_unix_utc(951_782_400), (2000, 2, 29, 0, 0, 0));
    }
}
