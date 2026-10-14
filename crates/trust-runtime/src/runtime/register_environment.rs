//! Capture hosted controls once; parsing policy remains in the canonical core.
use trust_runtime_core::vm::register::RegisterExecutionConfiguration;

pub(crate) fn capture() -> RegisterExecutionConfiguration {
    from_resolver(|name| std::env::var(name).ok())
}

pub(crate) fn from_resolver(
    read: impl FnMut(&str) -> Option<String>,
) -> RegisterExecutionConfiguration {
    RegisterExecutionConfiguration::from_environment(read)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hosted_environment_parser_preserves_every_existing_control_without_global_mutation() {
        let mut reads = Vec::new();
        let config = from_resolver(|name| {
            reads.push(name.to_owned());
            Some(
                match name {
                    "TRUST_VM_REGISTER_LOWERING_CACHE" => " OFF ",
                    "TRUST_VM_REGISTER_LOWERING_CACHE_CAP" => " 0 ",
                    "TRUST_VM_TIER1_SPECIALIZED_EXECUTOR" => " YeS ",
                    "TRUST_VM_TIER1_SPECIALIZED_EXECUTOR_HOT_THRESHOLD" => " 0 ",
                    "TRUST_VM_TIER1_SPECIALIZED_EXECUTOR_CACHE_CAP" => " 7 ",
                    _ => panic!("unexpected environment read {name}"),
                }
                .into(),
            )
        });
        assert_eq!(reads.len(), 5);
        assert!(!config.lowering_enabled);
        assert_eq!(config.lowering_capacity, 1);
        assert!(config.tier1_enabled);
        assert_eq!(config.tier1_hot_threshold, 0);
        assert_eq!(config.tier1_capacity, 7);
        assert!(!config.profile_enabled);
    }

    #[test]
    fn absent_malformed_and_overflowing_settings_keep_canonical_defaults() {
        let defaults = from_resolver(|_| None);
        assert!(defaults.lowering_enabled);
        assert_eq!(defaults.lowering_capacity, 256);
        assert!(!defaults.tier1_enabled);
        assert_eq!(defaults.tier1_hot_threshold, 64);
        assert_eq!(defaults.tier1_capacity, 128);
        assert_eq!(from_resolver(|_| Some("not a value".into())), defaults);
        assert_eq!(
            from_resolver(|_| Some("99999999999999999999999999999999999999999999".into())),
            defaults
        );
        for yes in ["1", "true", "yes", "on"] {
            assert!(
                from_resolver(
                    |name| (name == "TRUST_VM_TIER1_SPECIALIZED_EXECUTOR").then(|| yes.into())
                )
                .tier1_enabled
            );
        }
        for no in ["0", "false", "no", "off"] {
            assert!(
                !from_resolver(
                    |name| (name == "TRUST_VM_REGISTER_LOWERING_CACHE").then(|| no.into())
                )
                .lowering_enabled
            );
        }
        let zero_caps = from_resolver(|name| name.ends_with("_CAP").then(|| "0".into()));
        assert_eq!(
            (zero_caps.lowering_capacity, zero_caps.tier1_capacity),
            (1, 1)
        );
        let negative = from_resolver(|name| {
            (!name.ends_with("EXECUTOR") && name != "TRUST_VM_REGISTER_LOWERING_CACHE")
                .then(|| "-1".into())
        });
        assert_eq!(negative, defaults);
    }
}
