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
