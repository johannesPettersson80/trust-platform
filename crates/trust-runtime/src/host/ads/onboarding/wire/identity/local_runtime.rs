use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use super::super::ObservedAdsIdentity;
#[cfg(windows)]
use crate::ads::onboarding::errors::OnboardingWireError;

#[cfg(windows)]
use super::local_router_error;
use super::windows_runtime;

#[path = "local_runtime_budget.rs"]
mod budget;
#[cfg(any(windows, test))]
use budget::LocalRuntimeProbeBudget;
use budget::LocalRuntimeProbeResult;
#[cfg(windows)]
use budget::{budgeted_native_runtime_probe, SystemMonotonicClock};
#[cfg(test)]
use budget::{classify_native_runtime_probe, MonotonicClock};

const LOCAL_RUNTIME_PROBE_PORTS: [u16; 7] = [10_000, 851, 852, 853, 854, 301, 501];
const LOCAL_USER_SERVICE_PORTS: [u16; 6] = [851, 852, 853, 854, 301, 501];
const MAX_COLLISION_ID_INCREMENTS: u8 = 16;

#[derive(Debug, Default)]
struct RuntimePortProbeOutcome {
    identity_responded: bool,
    responding_user_ports: Vec<u16>,
    deadline_reached: bool,
    failure: Option<String>,
}

#[derive(Debug, Default)]
pub(super) struct LocalRuntimeIdentityReport {
    pub(super) identities: Vec<ObservedAdsIdentity>,
    pub(super) warnings: Vec<String>,
    deadline_reached: bool,
}

#[cfg(windows)]
pub(super) fn identities(
    target_ip: &str,
    timeout: Duration,
) -> Result<LocalRuntimeIdentityReport, OnboardingWireError> {
    // The deadline starts before native setup and registry enumeration. Those
    // synchronous DLL/filesystem calls cannot be pre-empted, so this is not a
    // hard wall-clock cancellation guarantee; if they consume the budget, no
    // AdsSyncReadStateReqEx request is allowed to start.
    let clock = SystemMonotonicClock;
    let mut probe_budget = LocalRuntimeProbeBudget::new(&clock, timeout);
    let library = trust_ads_windows::TcAdsDll::load_installed().map_err(|error| {
        local_router_error(
            target_ip,
            format!(
                "open installed TcAdsDll.dll: {error}. Start the TwinCAT router/runtime; truST will not create a self-route or fall back to raw ADS/TCP"
            ),
        )
    })?;
    let mut port = library
        .open_port()
        .map_err(|error| local_router_error(target_ip, format!("AdsPortOpenEx: {error}")))?;
    let local_source = port
        .local_address()
        .map_err(|error| local_router_error(target_ip, format!("AdsGetLocalAddressEx: {error}")))?;
    if local_source.net_id.octets.iter().all(|octet| *octet == 0) {
        return Err(local_router_error(
            target_ip,
            "AdsGetLocalAddressEx returned an empty source AMS Net ID".to_string(),
        ));
    }

    let registry_scan = windows_runtime::scan_installed_runtimes();
    let source_net_id = local_source.net_id.to_string();
    let mut report = responding_runtime_targets(
        target_ip,
        registry_scan.runtimes.as_slice(),
        &source_net_id,
        |net_id, ads_port, ports_remaining| {
            let Ok(net_id) = net_id.parse::<trust_ads_windows::AmsNetId>() else {
                return Ok(LocalRuntimeProbeResult::NoResponse);
            };
            budgeted_native_runtime_probe(
                &mut probe_budget,
                &mut port,
                &trust_ads_windows::AmsAddress::new(net_id, ads_port),
                ports_remaining,
            )
        },
    )
    .map_err(|error| local_router_error(target_ip, error))?;
    if report.deadline_reached {
        report.warnings.push(format!(
            "Local ADS identity discovery reached its time budget after {} native probe(s). Only native ADS replies received before the deadline are shown.",
            probe_budget.probes_started()
        ));
    }
    report.warnings.extend(registry_scan.warnings);
    if !report.identities.is_empty() {
        return Ok(report);
    }

    let candidate_ids = registry_scan
        .runtimes
        .iter()
        .map(|candidate| candidate.ams_net_id.as_str())
        .chain(std::iter::once(source_net_id.as_str()))
        .collect::<Vec<_>>()
        .join(", ");
    let warnings = if report.warnings.is_empty() {
        String::new()
    } else {
        format!(" Warnings: {}", report.warnings.join("; "))
    };
    Err(local_router_error(
        target_ip,
        format!(
            "the native router opened, but no configured local runtime ({candidate_ids}) exposed a responding user ADS service on ports 851-854, 301, or 501. Port 10000 is probed only to verify router identity and is never presented as a connectable device service.{warnings} Start the intended ADS runtime and retry"
        ),
    ))
}

fn responding_runtime_targets(
    target_ip: &str,
    configured: &[windows_runtime::ConfiguredRuntime],
    source_net_id: &str,
    mut responds: impl FnMut(&str, u16, usize) -> Result<LocalRuntimeProbeResult, String>,
) -> Result<LocalRuntimeIdentityReport, String> {
    let mut report = LocalRuntimeIdentityReport::default();
    let mut groups = BTreeMap::<String, Vec<&windows_runtime::ConfiguredRuntime>>::new();
    let mut bases = Vec::new();
    for runtime in configured {
        if !groups.contains_key(&runtime.ams_net_id) {
            bases.push(runtime.ams_net_id.clone());
        }
        groups
            .entry(runtime.ams_net_id.clone())
            .or_default()
            .push(runtime);
    }
    let base_set = bases.iter().cloned().collect::<BTreeSet<_>>();
    let mut scheduled_probes_remaining = planned_runtime_probe_slots(&bases, source_net_id).max(1);
    let mut collision_detected =
        groups.values().any(|group| group.len() > 1) || base_set.contains(source_net_id);

    let mut proven_configured = 0_usize;
    let mut seen_probe_ids = base_set.clone();
    let mut scan_failure = None;

    // Configured exact IDs are the strongest Usermode Runtime candidates. Probe
    // each one before the router source identity or collision guesses so a slow
    // source route cannot consume the entire discovery budget first.
    for candidate in &bases {
        if report.deadline_reached {
            break;
        }
        let outcome = probe_runtime_ports(
            candidate,
            &LOCAL_RUNTIME_PROBE_PORTS,
            &mut scheduled_probes_remaining,
            &mut responds,
        );
        if outcome.identity_responded {
            let name = if collision_detected {
                format!("Local ADS runtime {candidate}")
            } else {
                groups
                    .get(candidate)
                    .and_then(|group| group.first())
                    .map_or_else(
                        || format!("Local ADS runtime {candidate}"),
                        |runtime| runtime.name.clone(),
                    )
            };
            push_runtime_identity(
                &mut report.identities,
                target_ip,
                candidate,
                name,
                &outcome.responding_user_ports,
            );
            if outcome.responding_user_ports.is_empty() {
                report.warnings.push(format!(
                    "AMS identity {candidate} answered only on router system port 10000; no connectable user ADS service was found."
                ));
            }
            // When the configured base equals the router source, this response
            // may belong to the source runtime while Usermode was collision-
            // adjusted. Keep scanning before counting it as the configured
            // instance.
            if candidate != source_net_id && !outcome.responding_user_ports.is_empty() {
                proven_configured += 1;
            }
        }
        if let Some(error) = outcome.failure {
            scan_failure = Some(error);
            break;
        }
        if outcome.deadline_reached {
            report.deadline_reached = true;
            break;
        }
    }

    // The router-assigned source can represent a non-Usermode local runtime,
    // but it is a fallback. If it is also a configured base, the full exact-ID
    // probe above already covered every source service port.
    if scan_failure.is_none() && !report.deadline_reached && !base_set.contains(source_net_id) {
        seen_probe_ids.insert(source_net_id.to_string());
        let outcome = probe_runtime_ports(
            source_net_id,
            &LOCAL_USER_SERVICE_PORTS,
            &mut scheduled_probes_remaining,
            &mut responds,
        );
        if outcome.identity_responded {
            push_runtime_identity(
                &mut report.identities,
                target_ip,
                source_net_id,
                "Local ADS runtime".to_string(),
                &outcome.responding_user_ports,
            );
        }
        if outcome.deadline_reached {
            report.deadline_reached = true;
        }
        if let Some(error) = outcome.failure {
            scan_failure = Some(error);
        }
    }

    // Only after exact configured IDs and the source fallback do we fan out to
    // Beckhoff's collision-adjusted second-octet candidates.
    'increments: for increment in 1..=MAX_COLLISION_ID_INCREMENTS {
        for base in &bases {
            if scan_failure.is_some()
                || report.deadline_reached
                || proven_configured >= configured.len()
            {
                break 'increments;
            }
            let Some(candidate) = increment_second_octet(base, increment) else {
                continue;
            };
            if candidate == source_net_id || !seen_probe_ids.insert(candidate.clone()) {
                continue;
            }
            let outcome = probe_runtime_ports(
                &candidate,
                &LOCAL_RUNTIME_PROBE_PORTS,
                &mut scheduled_probes_remaining,
                &mut responds,
            );
            if outcome.identity_responded {
                collision_detected = true;
                push_runtime_identity(
                    &mut report.identities,
                    target_ip,
                    &candidate,
                    format!("Local ADS runtime {candidate}"),
                    &outcome.responding_user_ports,
                );
                if !outcome.responding_user_ports.is_empty() {
                    proven_configured += 1;
                }
                if outcome.responding_user_ports.is_empty() {
                    report.warnings.push(format!(
                        "AMS identity {candidate} answered only on router system port 10000; no connectable user ADS service was found."
                    ));
                }
            }
            if let Some(error) = outcome.failure {
                scan_failure = Some(error);
                break 'increments;
            }
            if outcome.deadline_reached {
                report.deadline_reached = true;
                break 'increments;
            }
        }
    }

    if let Some(failure) = scan_failure {
        if report.identities.is_empty() {
            return Err(failure);
        }
        report.warnings.push(format!(
            "Local ADS identity scanning stopped after a native ADS error: {failure}. Previously verified devices are still shown."
        ));
    }
    if collision_detected && !configured.is_empty() {
        let instances = configured
            .iter()
            .map(|runtime| format!("{} ({})", runtime.name, runtime.ams_net_id))
            .collect::<Vec<_>>()
            .join(", ");
        report.warnings.push(format!(
            "Local ADS runtime identities collide or may be auto-adjusted ({instances}). Only AMS Net IDs that completed a native ADS read-state probe are shown; ambiguous runtime names are not assigned."
        ));
    }
    if proven_configured < configured.len() {
        report.warnings.push(format!(
            "Verified {proven_configured} of {} configured local Usermode Runtime instance(s). {} instance(s) could not be matched to a responding AMS Net ID before the bounded scan ended; open Advanced recovery to inspect the configured AMS Net IDs.",
            configured.len(),
            configured.len() - proven_configured
        ));
    }
    Ok(report)
}

fn probe_runtime_ports(
    net_id: &str,
    ports: &[u16],
    scheduled_probes_remaining: &mut usize,
    responds: &mut impl FnMut(&str, u16, usize) -> Result<LocalRuntimeProbeResult, String>,
) -> RuntimePortProbeOutcome {
    let mut outcome = RuntimePortProbeOutcome::default();
    for port in ports {
        let fairness_slots = (*scheduled_probes_remaining).max(1);
        let result = responds(net_id, *port, fairness_slots);
        *scheduled_probes_remaining = scheduled_probes_remaining.saturating_sub(1);
        let result = match result {
            Ok(result) => result,
            Err(error) => {
                outcome.failure = Some(error);
                break;
            }
        };
        match result {
            LocalRuntimeProbeResult::Responded => {
                outcome.identity_responded = true;
                if LOCAL_USER_SERVICE_PORTS.contains(port) {
                    outcome.responding_user_ports.push(*port);
                }
            }
            LocalRuntimeProbeResult::DeadlineReached => {
                outcome.deadline_reached = true;
                break;
            }
            LocalRuntimeProbeResult::NoResponse => {}
        }
    }
    outcome
}

/// Counts the complete, de-duplicated scan before the first native call.
///
/// The timeout allocator must reserve a fair slot for later source and
/// collision-adjusted identities. Counting only the ports on the current AMS
/// identity lets one unresponsive configured base consume almost the whole
/// discovery deadline before the documented second-octet candidates run.
fn planned_runtime_probe_slots(bases: &[String], source_net_id: &str) -> usize {
    let mut seen = BTreeSet::new();
    let mut slots = 0_usize;
    for base in bases {
        if seen.insert(base.clone()) {
            slots = slots.saturating_add(LOCAL_RUNTIME_PROBE_PORTS.len());
        }
    }
    if seen.insert(source_net_id.to_string()) {
        slots = slots.saturating_add(LOCAL_USER_SERVICE_PORTS.len());
    }
    for increment in 1..=MAX_COLLISION_ID_INCREMENTS {
        for base in bases {
            let Some(candidate) = increment_second_octet(base, increment) else {
                continue;
            };
            if seen.insert(candidate) {
                slots = slots.saturating_add(LOCAL_RUNTIME_PROBE_PORTS.len());
            }
        }
    }
    slots
}

fn increment_second_octet(net_id: &str, increment: u8) -> Option<String> {
    let mut octets = net_id
        .split('.')
        .map(str::parse::<u8>)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    if octets.len() != 6 {
        return None;
    }
    octets[1] = octets[1].checked_add(increment)?;
    Some(
        octets
            .into_iter()
            .map(|octet| octet.to_string())
            .collect::<Vec<_>>()
            .join("."),
    )
}

fn push_runtime_identity(
    identities: &mut Vec<ObservedAdsIdentity>,
    target_ip: &str,
    ams_net_id: &str,
    name: String,
    ports: &[u16],
) {
    if let Some(identity) = identities
        .iter_mut()
        .find(|identity| identity.ams_net_id == ams_net_id)
    {
        for port in ports {
            if !identity.responding_ads_ports.contains(port) {
                identity.responding_ads_ports.push(*port);
            }
        }
        identity.preferred_ams_port = identity.responding_ads_ports.first().copied();
        return;
    }
    identities.push(ObservedAdsIdentity {
        name: Some(name),
        ip: target_ip.to_string(),
        ams_net_id: ams_net_id.to_string(),
        preferred_ams_port: ports.first().copied(),
        responding_ads_ports: ports.to_vec(),
        tc_version: None,
    });
}

#[cfg(test)]
#[path = "local_runtime/tests.rs"]
mod tests;
