//! Runtime-cloud profile, transport, and WAN allowlist policy evaluation.

#![allow(missing_docs)]

use std::collections::BTreeMap;

use crate::config::{RuntimeCloudProfile, RuntimeCloudWanAllowRule, WebAuthMode};
use crate::runtime_cloud::contracts::ReasonCode;
use crate::runtime_cloud::routing::{
    RuntimeCloudActionPreflight, RuntimeCloudActionRequest, RuntimeCloudTargetStatus,
};
use crate::runtime_cloud::sites::runtime_cloud_site_of;

pub(crate) fn runtime_cloud_apply_profile_policy(
    mut preflight: RuntimeCloudActionPreflight,
    action: &RuntimeCloudActionRequest,
    targets: &BTreeMap<String, RuntimeCloudTargetStatus>,
    profile: RuntimeCloudProfile,
    wan_allow_write: &[RuntimeCloudWanAllowRule],
    auth_mode: WebAuthMode,
    web_tls_enabled: bool,
) -> RuntimeCloudActionPreflight {
    if let Some((denial_code, denial_reason)) =
        runtime_cloud_profile_precondition(profile, auth_mode, web_tls_enabled)
    {
        for decision in &mut preflight.decisions {
            decision.allowed = false;
            decision.denial_code = Some(denial_code);
            decision.denial_reason = Some(denial_reason.clone());
        }
        preflight.allowed = false;
        preflight.denial_code = Some(denial_code);
        preflight.denial_reason = Some(denial_reason);
        return preflight;
    }

    for decision in &mut preflight.decisions {
        if !decision.allowed {
            continue;
        }
        let Some((denial_code, denial_reason)) = runtime_cloud_profile_target_denial(
            action,
            decision.runtime_id.as_str(),
            targets,
            profile,
            wan_allow_write,
        ) else {
            continue;
        };
        decision.allowed = false;
        decision.denial_code = Some(denial_code);
        decision.denial_reason = Some(denial_reason);
    }

    preflight.allowed = preflight.decisions.iter().all(|decision| decision.allowed);
    let (denial_code, denial_reason) = preflight
        .decisions
        .iter()
        .find(|decision| !decision.allowed)
        .map(|decision| (decision.denial_code, decision.denial_reason.clone()))
        .unwrap_or((None, None));
    preflight.denial_code = denial_code;
    preflight.denial_reason = denial_reason;
    preflight
}

pub(crate) fn runtime_cloud_profile_precondition(
    profile: RuntimeCloudProfile,
    auth_mode: WebAuthMode,
    web_tls_enabled: bool,
) -> Option<(ReasonCode, String)> {
    if !profile.requires_secure_transport() {
        return None;
    }
    if !matches!(auth_mode, WebAuthMode::Token) {
        return Some((
            ReasonCode::NotConfigured,
            format!(
                "runtime cloud profile '{}' requires runtime.web.auth='token'",
                profile.as_str()
            ),
        ));
    }
    if !web_tls_enabled {
        return Some((
            ReasonCode::NotConfigured,
            format!(
                "runtime cloud profile '{}' requires runtime.web.tls=true",
                profile.as_str()
            ),
        ));
    }
    None
}

fn runtime_cloud_profile_target_denial(
    action: &RuntimeCloudActionRequest,
    runtime_id: &str,
    targets: &BTreeMap<String, RuntimeCloudTargetStatus>,
    profile: RuntimeCloudProfile,
    wan_allow_write: &[RuntimeCloudWanAllowRule],
) -> Option<(ReasonCode, String)> {
    if runtime_id == action.connected_via {
        return None;
    }
    if profile.requires_secure_transport()
        && !targets
            .get(runtime_id)
            .map(|target| target.supports_secure_transport)
            .unwrap_or(false)
    {
        return Some((
            ReasonCode::NotConfigured,
            format!(
                "target runtime '{}' does not advertise secure web transport metadata",
                runtime_id
            ),
        ));
    }
    if matches!(profile, RuntimeCloudProfile::Wan)
        && runtime_cloud_is_write_action(action.action_type.as_str())
        && runtime_cloud_is_cross_site_target(action, runtime_id)
        && !runtime_cloud_wan_allowlist_allows(
            action.action_type.as_str(),
            runtime_id,
            wan_allow_write,
        )
    {
        let local_site = runtime_cloud_site_of(action.connected_via.as_str());
        let target_site = runtime_cloud_site_of(runtime_id);
        return Some((
            ReasonCode::PermissionDenied,
            format!(
                "cross-site write action '{}' from site '{}' to runtime '{}' (site '{}') requires explicit runtime.cloud.wan.allow_write policy",
                action.action_type, local_site, runtime_id, target_site
            ),
        ));
    }
    if matches!(profile, RuntimeCloudProfile::Wan)
        && runtime_cloud_is_write_action(action.action_type.as_str())
        && !runtime_cloud_wan_allowlist_allows(
            action.action_type.as_str(),
            runtime_id,
            wan_allow_write,
        )
    {
        return Some((
            ReasonCode::PermissionDenied,
            format!(
                "runtime cloud profile 'wan' denies action '{}' for target '{}' without explicit allowlist rule",
                action.action_type, runtime_id
            ),
        ));
    }
    None
}

fn runtime_cloud_is_write_action(action_type: &str) -> bool {
    matches!(action_type, "cfg_apply" | "cmd_invoke")
}

/// Whether `runtime_id` is in another site than the runtime the request is connected through: the
/// sites their cloud IDs name (specification 11, "Runtime Cloud sites").
fn runtime_cloud_is_cross_site_target(
    action: &RuntimeCloudActionRequest,
    runtime_id: &str,
) -> bool {
    runtime_cloud_site_of(runtime_id) != runtime_cloud_site_of(action.connected_via.as_str())
}

fn runtime_cloud_wan_allowlist_allows(
    action_type: &str,
    runtime_id: &str,
    rules: &[RuntimeCloudWanAllowRule],
) -> bool {
    rules.iter().any(|rule| {
        rule.action.as_str() == action_type
            && runtime_cloud_wan_target_matches(rule.target.as_str(), runtime_id)
    })
}

fn runtime_cloud_wan_target_matches(pattern: &str, runtime_id: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if let Some(prefix) = pattern.strip_suffix('*') {
        return runtime_id.starts_with(prefix);
    }
    if let Some(suffix) = pattern.strip_prefix('*') {
        return runtime_id.ends_with(suffix);
    }
    if pattern.contains('*') {
        return false;
    }
    pattern == runtime_id
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime_cloud::routing::{RuntimeCloudTargetDecision, RuntimeCloudTargetStatus};
    use smol_str::SmolStr;

    fn action_fixture(action_type: &str, targets: Vec<&str>) -> RuntimeCloudActionRequest {
        RuntimeCloudActionRequest {
            api_version: "1.0".to_string(),
            request_id: "req-policy-1".to_string(),
            connected_via: "runtime-a".to_string(),
            target_runtimes: targets.into_iter().map(str::to_string).collect(),
            actor: "spiffe://trust/default-site/operator-1".to_string(),
            action_type: action_type.to_string(),
            query_budget_ms: None,
            dry_run: true,
            payload: serde_json::json!({ "params": { "log.level": "debug" } }),
        }
    }

    fn preflight_fixture(targets: Vec<&str>) -> RuntimeCloudActionPreflight {
        RuntimeCloudActionPreflight {
            api_version: "1.0".to_string(),
            request_id: "req-policy-1".to_string(),
            connected_via: "runtime-a".to_string(),
            acting_on: targets.iter().map(|target| target.to_string()).collect(),
            allowed: true,
            denial_code: None,
            denial_reason: None,
            decisions: targets
                .into_iter()
                .map(|target| RuntimeCloudTargetDecision {
                    runtime_id: target.to_string(),
                    allowed: true,
                    denial_code: None,
                    denial_reason: None,
                })
                .collect(),
        }
    }

    fn targets_fixture(
        runtime_id: &str,
        supports_secure_transport: bool,
    ) -> BTreeMap<String, RuntimeCloudTargetStatus> {
        let mut map = BTreeMap::new();
        map.insert(
            runtime_id.to_string(),
            RuntimeCloudTargetStatus {
                reachable: true,
                stale: false,
                supports_secure_transport,
            },
        );
        map
    }

    #[test]
    fn wan_profile_denies_write_without_matching_rule() {
        let action = action_fixture("cfg_apply", vec!["runtime-b"]);
        let preflight = preflight_fixture(vec!["runtime-b"]);
        let targets = targets_fixture("runtime-b", true);

        let report = runtime_cloud_apply_profile_policy(
            preflight,
            &action,
            &targets,
            RuntimeCloudProfile::Wan,
            &[],
            WebAuthMode::Token,
            true,
        );
        assert!(!report.allowed);
        assert_eq!(report.denial_code, Some(ReasonCode::PermissionDenied));
    }

    /// Specification 11, "Runtime Cloud sites" and the profile contract: a target is cross-site
    /// when the site its cloud ID names differs from the site of `connected_via`, and the actor's
    /// identity does not choose a site. Catches a bare target read in the local runtime's site
    /// instead of the default site, and an actor's site taken for the site of a runtime.
    #[test]
    fn the_sites_of_the_cloud_ids_decide_what_is_cross_site() {
        let denial = |connected_via: &str, actor: &str, target: &str| {
            let mut action = action_fixture("cfg_apply", vec![target]);
            action.connected_via = connected_via.to_string();
            action.actor = actor.to_string();
            let mut preflight = preflight_fixture(vec![target]);
            preflight.connected_via = connected_via.to_string();
            runtime_cloud_apply_profile_policy(
                preflight,
                &action,
                &targets_fixture(target, true),
                RuntimeCloudProfile::Wan,
                &[],
                WebAuthMode::Token,
                true,
            )
            .denial_reason
            .expect("a write without a rule is denied")
        };
        assert_eq!(
            denial(
                "site-b/runtime_b",
                "spiffe://trust/site-b/engineer-1",
                "runtime_c"
            ),
            "cross-site write action 'cfg_apply' from site 'site-b' to runtime 'runtime_c' \
             (site 'default-site') requires explicit runtime.cloud.wan.allow_write policy"
        );
        assert_eq!(
            denial(
                "runtime_a",
                "spiffe://trust/site-a/engineer-1",
                "site-a/runtime_x"
            ),
            "cross-site write action 'cfg_apply' from site 'default-site' to runtime \
             'site-a/runtime_x' (site 'site-a') requires explicit runtime.cloud.wan.allow_write \
             policy"
        );
        assert_eq!(
            denial("runtime_a", "spiffe://trust/site-a/engineer-1", "runtime_c"),
            "runtime cloud profile 'wan' denies action 'cfg_apply' for target 'runtime_c' without \
             explicit allowlist rule"
        );
    }

    #[test]
    fn dev_and_plant_profiles_do_not_require_wan_write_allowlist() {
        let action = action_fixture("cfg_apply", vec!["site-b/runtime-b"]);
        let preflight = preflight_fixture(vec!["site-b/runtime-b"]);
        let targets = targets_fixture("site-b/runtime-b", true);

        for profile in [RuntimeCloudProfile::Dev, RuntimeCloudProfile::Plant] {
            let report = runtime_cloud_apply_profile_policy(
                preflight.clone(),
                &action,
                &targets,
                profile,
                &[],
                WebAuthMode::Token,
                true,
            );
            assert!(
                report.allowed,
                "profile '{}' must not apply the WAN write allowlist",
                profile.as_str()
            );
        }
    }

    #[test]
    fn secure_profiles_require_token_tls_and_secure_remote_metadata() {
        for profile in [RuntimeCloudProfile::Plant, RuntimeCloudProfile::Wan] {
            assert!(
                runtime_cloud_profile_precondition(profile, WebAuthMode::Local, true).is_some()
            );
            assert!(
                runtime_cloud_profile_precondition(profile, WebAuthMode::Token, false).is_some()
            );

            let action = action_fixture("status_read", vec!["runtime-b"]);
            let report = runtime_cloud_apply_profile_policy(
                preflight_fixture(vec!["runtime-b"]),
                &action,
                &targets_fixture("runtime-b", false),
                profile,
                &[],
                WebAuthMode::Token,
                true,
            );
            assert!(!report.allowed);
            assert_eq!(report.denial_code, Some(ReasonCode::NotConfigured));
        }
    }

    #[test]
    fn dev_profile_allows_remote_target_without_secure_transport_metadata() {
        let action = action_fixture("status_read", vec!["runtime-b"]);
        let report = runtime_cloud_apply_profile_policy(
            preflight_fixture(vec!["runtime-b"]),
            &action,
            &targets_fixture("runtime-b", false),
            RuntimeCloudProfile::Dev,
            &[],
            WebAuthMode::Local,
            false,
        );

        assert!(report.allowed);
    }

    #[test]
    fn wan_read_action_does_not_require_write_allowlist() {
        let action = action_fixture("status_read", vec!["site-b/runtime-b"]);
        let report = runtime_cloud_apply_profile_policy(
            preflight_fixture(vec!["site-b/runtime-b"]),
            &action,
            &targets_fixture("site-b/runtime-b", true),
            RuntimeCloudProfile::Wan,
            &[],
            WebAuthMode::Token,
            true,
        );

        assert!(report.allowed);
    }

    #[test]
    fn allowlist_supports_prefix_and_suffix_patterns() {
        let action = action_fixture("cfg_apply", vec!["site-b/runtime-b"]);
        let preflight = preflight_fixture(vec!["site-b/runtime-b"]);
        let targets = targets_fixture("site-b/runtime-b", true);
        let prefix_rules = vec![RuntimeCloudWanAllowRule {
            action: SmolStr::new("cfg_apply"),
            target: SmolStr::new("site-b/*"),
        }];
        let prefix_report = runtime_cloud_apply_profile_policy(
            preflight.clone(),
            &action,
            &targets,
            RuntimeCloudProfile::Wan,
            &prefix_rules,
            WebAuthMode::Token,
            true,
        );
        assert!(prefix_report.allowed);

        let suffix_rules = vec![RuntimeCloudWanAllowRule {
            action: SmolStr::new("cfg_apply"),
            target: SmolStr::new("*/runtime-b"),
        }];
        let suffix_report = runtime_cloud_apply_profile_policy(
            preflight,
            &action,
            &targets,
            RuntimeCloudProfile::Wan,
            &suffix_rules,
            WebAuthMode::Token,
            true,
        );
        assert!(suffix_report.allowed);
    }

    #[test]
    fn allowlist_supports_global_and_exact_patterns() {
        let action = action_fixture("cfg_apply", vec!["site-b/runtime-b"]);
        let preflight = preflight_fixture(vec!["site-b/runtime-b"]);
        let targets = targets_fixture("site-b/runtime-b", true);

        for target in ["*", "site-b/runtime-b"] {
            let rules = vec![RuntimeCloudWanAllowRule {
                action: SmolStr::new("cfg_apply"),
                target: SmolStr::new(target),
            }];
            let report = runtime_cloud_apply_profile_policy(
                preflight.clone(),
                &action,
                &targets,
                RuntimeCloudProfile::Wan,
                &rules,
                WebAuthMode::Token,
                true,
            );
            assert!(
                report.allowed,
                "documented target pattern '{target}' must match"
            );
        }
    }

    #[test]
    fn wan_allowlist_parser_fuzz_smoke_budget() {
        fn next(state: &mut u64) -> u64 {
            *state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            *state
        }

        fn token(state: &mut u64, len: usize) -> String {
            let mut text = String::with_capacity(len);
            for _ in 0..len {
                let value = (next(state) % 26) as u8;
                text.push((b'a' + value) as char);
            }
            text
        }

        let iterations = std::env::var("TRUST_COMMS_FUZZ_ITERS")
            .ok()
            .and_then(|raw| raw.parse::<usize>().ok())
            .unwrap_or(512);
        let mut state = 0xC0FF_EE00_0000_0001_u64;

        for _ in 0..iterations {
            let lhs = token(&mut state, 4);
            let rhs = token(&mut state, 6);
            let runtime = format!("{lhs}/{rhs}");
            let pattern = match next(&mut state) % 5 {
                0 => format!("{lhs}/*"),
                1 => format!("*/{rhs}"),
                2 => format!("{lhs}/{rhs}"),
                3 => String::from("*"),
                _ => format!("{}/*{}", lhs, rhs),
            };
            let _ = runtime_cloud_wan_target_matches(pattern.as_str(), runtime.as_str());

            let rule = RuntimeCloudWanAllowRule {
                action: SmolStr::new("cfg_apply"),
                target: SmolStr::new(pattern),
            };
            let _ = runtime_cloud_wan_allowlist_allows(
                "cfg_apply",
                runtime.as_str(),
                std::slice::from_ref(&rule),
            );
            let _ = runtime_cloud_wan_allowlist_allows("status_read", runtime.as_str(), &[rule]);
        }
    }
}
