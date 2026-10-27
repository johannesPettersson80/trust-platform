//! The ADS port rule (specification 11, "Communication discovery scope and symbol browsing"),
//! ported from `editors/vscode/src/test/suite/ads-multiport-live-values.test.ts` lines 40-118.

use std::sync::Mutex;
use std::time::Duration;

use serde_json::{json, Value};

use crate::ads::diagnostics::TargetIdentity;

use super::*;

/// What a counting probe observed: the number of runs at the same time, the highest of them, and
/// every port it was asked, in order.
#[derive(Default)]
struct ProbeCounts {
    active: usize,
    max: usize,
    calls: Vec<u16>,
}

/// A probe that answers by its own rule and records how many probes ran at once.
struct FakeProbe {
    answer: Box<dyn Fn(u16) -> Option<Value> + Send + Sync>,
    counts: Mutex<ProbeCounts>,
}

impl FakeProbe {
    fn new(answer: impl Fn(u16) -> Option<Value> + Send + Sync + 'static) -> Self {
        Self {
            answer: Box::new(answer),
            counts: Mutex::new(ProbeCounts::default()),
        }
    }

    fn max_concurrent(&self) -> usize {
        self.counts.lock().expect("probe counts").max
    }

    fn calls(&self) -> Vec<u16> {
        self.counts.lock().expect("probe counts").calls.clone()
    }
}

impl AdsPortProbe for FakeProbe {
    fn probe(&self, _target: &TargetIdentity, port: u16) -> Option<Value> {
        {
            let mut counts = self.counts.lock().expect("probe counts");
            counts.active += 1;
            counts.max = counts.max.max(counts.active);
            counts.calls.push(port);
        }
        // The TypeScript test's probe waits 5 ms, so the assertions still fit the rule.
        std::thread::sleep(Duration::from_millis(2));
        {
            let mut counts = self.counts.lock().expect("probe counts");
            counts.active -= 1;
        }
        (self.answer)(port)
    }
}

fn target(ams_net_id: &str, ip: &str, ams_port: u16) -> TargetIdentity {
    TargetIdentity {
        name: Some("PLC laptop".to_string()),
        ip: ip.to_string(),
        ams_net_id: ams_net_id.to_string(),
        ams_port,
        tc_version: None,
        os_version: None,
        fingerprint: None,
    }
}

fn symbol() -> Value {
    json!({ "id": "MAIN.value", "name": "value", "path": "MAIN.value" })
}

fn browse_ports(params: &Value) -> Vec<u16> {
    params
        .get("ads_port_browse_results")
        .and_then(Value::as_array)
        .expect("browse results")
        .iter()
        .map(|result| {
            result
                .get("port")
                .and_then(Value::as_u64)
                .and_then(|port| u16::try_from(port).ok())
                .expect("browse result port")
        })
        .collect()
}

#[test]
fn ads_ports_probed_one_at_a_time_per_device_with_the_respond_rule() {
    // The respond rule, ports of `adsDiscoveryPorts.ts:79-88` (TypeScript test lines 60-73).
    assert!(responded(&json!({ "tree": [symbol()] })));
    assert!(responded(&json!({
        "tree": [],
        "error": { "code": "symbol_upload_unsupported", "message": "unsupported" }
    })));
    assert!(responded(&json!({
        "tree": [],
        "error": { "code": "empty_symbol_table", "message": "no symbols" }
    })));
    assert!(responded(
        &json!({ "tree": [], "route": { "status": "ok" } })
    ));
    assert!(!responded(&json!({
        "tree": [],
        "error": { "code": "ads_port_unavailable", "message": "not open" }
    })));
    assert!(!responded(
        &json!({ "tree": [], "route": { "status": "missing" } })
    ));

    // Ports [851, 301, 501] are merged and sorted; 301 and 851 answer with a tree, 501 does not.
    let ports = checked_ports(Some(&json!([851, 301, 501]))).expect("checked ports");
    assert_eq!(ports, vec![301, 501, 851]);
    let probe = FakeProbe::new(|port| match port {
        301 | 851 => Some(json!({ "tree": [symbol()] })),
        501 => Some(json!({
            "tree": [],
            "error": { "code": "ads_port_unavailable", "message": "not open" }
        })),
        _ => None,
    });
    let target = target("100.67.6.217.1.1", "192.168.77.11", 851);
    let (params, warnings) = probe_candidate(&target, &ports, Deadline::NONE, &probe);

    assert_eq!(params.get("responding_ads_ports"), Some(&json!([301, 851])));
    assert_eq!(browse_ports(&params), vec![301, 851]);
    assert_eq!(
        params.get("ams_port"),
        Some(&json!(851)),
        "keep an already responding selection"
    );
    assert_eq!(
        probe.max_concurrent(),
        1,
        "ADS port probes must not race the shared route"
    );
    assert_eq!(probe.calls(), vec![301, 501, 851]);
    assert_eq!(warnings, Vec::<String>::new());
}

#[test]
fn an_ads_find_without_an_answering_port_is_kept_with_a_warning() {
    // J28: an ADS find with no answering port is kept, with a warning, not hidden.
    let probe = FakeProbe::new(|_port| {
        Some(json!({
            "tree": [],
            "error": { "code": "ads_port_unavailable", "message": "not open" }
        }))
    });
    let target = target("100.67.6.217.1.1", "192.168.77.11", 851);
    let ports = checked_ports(None).expect("default ports");
    let (params, warnings) = probe_candidate(&target, &ports, Deadline::NONE, &probe);

    assert_eq!(params.get("responding_ads_ports"), Some(&json!([])));
    assert_eq!(params.get("ams_port"), Some(&json!(851)));
    assert_eq!(params.get("ams_net_id"), Some(&json!("100.67.6.217.1.1")));
    assert_eq!(warnings.len(), 1);
    assert!(
        warnings[0].contains("No ADS port answered"),
        "warning must say no ADS port answered: {}",
        warnings[0]
    );
    assert_eq!(probe.calls(), vec![301, 501, 851, 852, 853, 854]);
}

#[test]
fn ads_identity_keeps_os_version_fingerprint_and_ports() {
    // E23: the OS version and fingerprint survive identify, and the answered port is kept, not 851.
    let target = TargetIdentity {
        name: Some("PLC".to_string()),
        ip: "192.168.77.11".to_string(),
        ams_net_id: "100.67.6.217.1.1".to_string(),
        ams_port: 851,
        tc_version: Some("3.1.4026".to_string()),
        os_version: Some("Windows NT 10.0.19045 (Service Pack 0)".to_string()),
        fingerprint: Some("A1B2-C3D4".to_string()),
    };
    let probe = FakeProbe::new(|port| {
        if port == 852 {
            Some(json!({ "tree": [symbol()] }))
        } else {
            None
        }
    });
    let ports = checked_ports(None).expect("default ports");
    let (params, warnings) = probe_candidate(&target, &ports, Deadline::NONE, &probe);

    assert_eq!(
        params.get("os_version"),
        Some(&json!("Windows NT 10.0.19045 (Service Pack 0)"))
    );
    assert_eq!(params.get("fingerprint"), Some(&json!("A1B2-C3D4")));
    assert_eq!(params.get("tc_version"), Some(&json!("3.1.4026")));
    assert_eq!(params.get("ams_net_id"), Some(&json!("100.67.6.217.1.1")));
    assert_eq!(params.get("host"), Some(&json!("192.168.77.11")));
    assert_eq!(params.get("responding_ads_ports"), Some(&json!([852])));
    assert_eq!(params.get("ams_port"), Some(&json!(852)));
    assert_eq!(browse_ports(&params), vec![852]);
    assert_eq!(warnings, Vec::<String>::new());
}
