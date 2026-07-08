#[cfg(test)]
mod tests {
    use super::*;

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
}
