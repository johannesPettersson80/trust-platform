//! Canonical runtime-cloud keyspace helpers and authority guards.

#![allow(missing_docs)]

use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeZone {
    Meta,
    Io,
    Cmd,
    Cfg,
    Diag,
    Svc,
}

impl RuntimeZone {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Meta => "_meta",
            Self::Io => "io",
            Self::Cmd => "cmd",
            Self::Cfg => "cfg",
            Self::Diag => "diag",
            Self::Svc => "svc",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigDocumentKind {
    Desired,
    Reported,
    Status,
    Meta,
}

impl ConfigDocumentKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Desired => "desired",
            Self::Reported => "reported",
            Self::Status => "status",
            Self::Meta => "meta",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveAuthorityRole {
    Active,
    Standby,
    Candidate,
}

#[must_use]
pub fn canonical_runtime_root(site: &str, runtime: &str) -> String {
    format!("truST/{site}/{runtime}")
}

#[must_use]
pub fn canonical_runtime_zone_prefix(site: &str, runtime: &str, zone: RuntimeZone) -> String {
    format!(
        "{}/{}",
        canonical_runtime_root(site, runtime),
        zone.as_str()
    )
}

#[must_use]
pub fn canonical_active_zone_prefix(site: &str, zone: RuntimeZone) -> String {
    format!("truST/{site}/active/{}", zone.as_str())
}

#[must_use]
pub fn canonical_cfg_authoritative_prefix(
    site: &str,
    kind: ConfigDocumentKind,
    runtime: &str,
) -> String {
    format!("truST/{site}/cfg/{}/{runtime}", kind.as_str())
}

#[must_use]
pub fn canonical_cfg_runtime_alias_prefix(
    site: &str,
    runtime: &str,
    kind: ConfigDocumentKind,
) -> String {
    format!("truST/{site}/{runtime}/cfg/{}", kind.as_str())
}

#[must_use]
pub fn meta_identity_key(site: &str, runtime: &str) -> String {
    format!(
        "{}/identity",
        canonical_runtime_zone_prefix(site, runtime, RuntimeZone::Meta)
    )
}

#[must_use]
pub fn meta_catalog_key(site: &str, runtime: &str) -> String {
    format!(
        "{}/catalog",
        canonical_runtime_zone_prefix(site, runtime, RuntimeZone::Meta)
    )
}

#[must_use]
pub fn meta_shm_channels_key(site: &str, runtime: &str) -> String {
    format!(
        "{}/shm_channels",
        canonical_runtime_zone_prefix(site, runtime, RuntimeZone::Meta)
    )
}

#[must_use]
pub fn meta_config_schema_key(site: &str, runtime: &str) -> String {
    format!(
        "{}/config_schema",
        canonical_runtime_zone_prefix(site, runtime, RuntimeZone::Meta)
    )
}

#[must_use]
pub fn is_reserved_zone(zone: RuntimeZone) -> bool {
    matches!(
        zone,
        RuntimeZone::Meta | RuntimeZone::Svc | RuntimeZone::Diag
    )
}

#[must_use]
pub fn active_publish_allowed(role: ActiveAuthorityRole) -> bool {
    matches!(role, ActiveAuthorityRole::Active)
}

/// Default stale timeout policy for UI/ops keys.
///
/// Spec rule: `max(2x expected update period, 2s)`.
#[must_use]
pub fn default_ui_ops_stale_timeout(expected_period: Duration) -> Duration {
    expected_period
        .saturating_mul(2)
        .max(Duration::from_secs(2))
}

/// Optional retained continuity is allowed for UI read continuity only.
#[must_use]
pub fn ui_retained_last_value_allowed(is_control_path: bool) -> bool {
    !is_control_path
}

#[must_use]
pub fn svc_liveliness_key(site: &str, runtime: &str, runtime_id: &str) -> String {
    format!(
        "{}/liveliness/{runtime_id}",
        canonical_runtime_zone_prefix(site, runtime, RuntimeZone::Svc)
    )
}

#[must_use]
pub fn svc_role_key(site: &str, runtime: &str, group_id: &str) -> String {
    format!(
        "{}/role/{group_id}",
        canonical_runtime_zone_prefix(site, runtime, RuntimeZone::Svc)
    )
}

#[must_use]
pub fn svc_lease_state_key(site: &str, runtime: &str, group_id: &str) -> String {
    format!(
        "{}/ha/{group_id}/lease_state",
        canonical_runtime_zone_prefix(site, runtime, RuntimeZone::Svc)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_key_layout_matches_runtime_site_shape() {
        assert_eq!(
            canonical_runtime_root("site-a", "rt-1"),
            "truST/site-a/rt-1"
        );
        assert_eq!(
            canonical_runtime_zone_prefix("site-a", "rt-1", RuntimeZone::Meta),
            "truST/site-a/rt-1/_meta"
        );
        assert_eq!(
            canonical_runtime_zone_prefix("site-a", "rt-1", RuntimeZone::Svc),
            "truST/site-a/rt-1/svc"
        );
    }

    #[test]
    fn every_runtime_zone_and_active_prefix_has_the_canonical_shape() {
        for (zone, segment) in [
            (RuntimeZone::Meta, "_meta"),
            (RuntimeZone::Io, "io"),
            (RuntimeZone::Cmd, "cmd"),
            (RuntimeZone::Cfg, "cfg"),
            (RuntimeZone::Diag, "diag"),
            (RuntimeZone::Svc, "svc"),
        ] {
            assert_eq!(
                canonical_runtime_zone_prefix("site-a", "rt-1", zone),
                format!("truST/site-a/rt-1/{segment}")
            );
            assert_eq!(
                canonical_active_zone_prefix("site-a", zone),
                format!("truST/site-a/active/{segment}")
            );
        }
    }

    #[test]
    fn reserved_zone_contract_is_meta_svc_and_diag() {
        assert!(is_reserved_zone(RuntimeZone::Meta));
        assert!(is_reserved_zone(RuntimeZone::Svc));
        assert!(is_reserved_zone(RuntimeZone::Diag));
        assert!(!is_reserved_zone(RuntimeZone::Io));
        assert!(!is_reserved_zone(RuntimeZone::Cmd));
        assert!(!is_reserved_zone(RuntimeZone::Cfg));
    }

    #[test]
    fn active_namespace_publish_requires_active_role() {
        assert!(active_publish_allowed(ActiveAuthorityRole::Active));
        assert!(!active_publish_allowed(ActiveAuthorityRole::Standby));
        assert!(!active_publish_allowed(ActiveAuthorityRole::Candidate));
    }

    #[test]
    fn default_stale_timeout_is_max_of_twice_period_and_two_seconds() {
        assert_eq!(
            default_ui_ops_stale_timeout(Duration::from_millis(200)),
            Duration::from_secs(2)
        );
        assert_eq!(
            default_ui_ops_stale_timeout(Duration::from_secs(3)),
            Duration::from_secs(6)
        );
    }

    #[test]
    fn retained_last_value_is_ui_only_not_control() {
        assert!(ui_retained_last_value_allowed(false));
        assert!(!ui_retained_last_value_allowed(true));
    }

    #[test]
    fn config_hierarchy_and_alias_paths_are_canonical() {
        assert_eq!(
            canonical_cfg_authoritative_prefix("site-a", ConfigDocumentKind::Desired, "rt-1"),
            "truST/site-a/cfg/desired/rt-1"
        );
        assert_eq!(
            canonical_cfg_authoritative_prefix("site-a", ConfigDocumentKind::Reported, "rt-1"),
            "truST/site-a/cfg/reported/rt-1"
        );
        assert_eq!(
            canonical_cfg_runtime_alias_prefix("site-a", "rt-1", ConfigDocumentKind::Status),
            "truST/site-a/rt-1/cfg/status"
        );
        assert_eq!(
            canonical_cfg_runtime_alias_prefix("site-a", "rt-1", ConfigDocumentKind::Meta),
            "truST/site-a/rt-1/cfg/meta"
        );
    }

    #[test]
    fn svc_key_helpers_cover_liveliness_role_and_lease_state() {
        assert_eq!(
            svc_liveliness_key("site-a", "rt-1", "rt-1"),
            "truST/site-a/rt-1/svc/liveliness/rt-1"
        );
        assert_eq!(
            svc_role_key("site-a", "rt-1", "ha-main"),
            "truST/site-a/rt-1/svc/role/ha-main"
        );
        assert_eq!(
            svc_lease_state_key("site-a", "rt-1", "ha-main"),
            "truST/site-a/rt-1/svc/ha/ha-main/lease_state"
        );
    }

    #[test]
    fn meta_key_helpers_cover_identity_catalog_shm_and_config_schema() {
        assert_eq!(
            meta_identity_key("site-a", "rt-1"),
            "truST/site-a/rt-1/_meta/identity"
        );
        assert_eq!(
            meta_catalog_key("site-a", "rt-1"),
            "truST/site-a/rt-1/_meta/catalog"
        );
        assert_eq!(
            meta_shm_channels_key("site-a", "rt-1"),
            "truST/site-a/rt-1/_meta/shm_channels"
        );
        assert_eq!(
            meta_config_schema_key("site-a", "rt-1"),
            "truST/site-a/rt-1/_meta/config_schema"
        );
    }
}
