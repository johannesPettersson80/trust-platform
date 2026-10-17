//! The EtherCAT search source's tests (specification 11, "Sources", "Identity", "Confidence" and
//! "Safety"): a fake bus and a literal adapter list replace both seams, so no test touches a real
//! network adapter.

use std::sync::Arc;

use smol_str::SmolStr;

use crate::connectors::DiscoveryConfidence;
use crate::discovery::DiscoveryState;
use crate::error::RuntimeError;
use crate::search::model::{EthercatIdentity, Identity, ProtocolFind};
use crate::search::sources::report::{CandidateSink, Deadline, DiscoverCandidate};
use crate::search::sources::scope::DiscoverScope;
use crate::search::sources::{RuntimeFacts, SourceContext};

use super::{
    adapter_plans, discover_with, refused_warning, AdapterPlan, EthercatIdentification,
    EthercatIdentifiedSubDevice, EthercatSeams, UNKNOWN_ADAPTER_OWNER,
};

/// A sink that maps every candidate to a find and keeps its source and confidence word.
#[derive(Default)]
struct FindRecorder {
    sources: Vec<&'static str>,
    words: Vec<&'static str>,
    finds: Vec<ProtocolFind>,
}

impl CandidateSink for FindRecorder {
    fn found(&mut self, candidate: DiscoverCandidate) {
        self.sources.push(candidate.source);
        self.words.push(candidate.confidence);
        self.finds.push(super::ethercat_find(candidate));
    }

    fn warn(&mut self, _warning: String) {}
}

/// One SubDevice with literal identity values, and the bus's literal byte totals.
fn identification(
    subdevice: EthercatIdentifiedSubDevice,
    input_bytes: usize,
    output_bytes: usize,
) -> EthercatIdentification {
    EthercatIdentification {
        subdevices: vec![subdevice],
        input_bytes,
        output_bytes,
    }
}

/// Runs one identification of `adapter`'s SubDevices through the fake seam.
fn identify(adapter: &str, identification: EthercatIdentification) -> FindRecorder {
    let identify = move |_params: &toml::Value| -> Result<EthercatIdentification, RuntimeError> {
        Ok(identification.clone())
    };
    let local = || -> Vec<SmolStr> { Vec::new() };
    let seams = EthercatSeams {
        local_adapters: &local,
        identify: &identify,
    };
    let scope = DiscoverScope {
        adapter: Some(adapter.to_string()),
        ..DiscoverScope::default()
    };
    let mut recorder = FindRecorder::default();
    discover_with(&scope, None, Deadline::NONE, &mut recorder, &seams).expect("identification");
    recorder
}

/// A runtime context with a literal owned-adapter list and no project.
fn context(adapters: Vec<Option<String>>, owned: Vec<SmolStr>) -> SourceContext {
    SourceContext {
        hosts: Vec::new(),
        cidr: None,
        ethercat_adapters: adapters,
        modbus_unit_ids: Vec::new(),
        ads_ports: Vec::new(),
        runtime: Some(RuntimeFacts {
            project_root: None,
            discovery: Arc::new(DiscoveryState::new()),
            discovery_enabled: true,
            discovery_interfaces: Vec::new(),
            owned_adapters: owned,
        }),
    }
}

#[test]
fn ethercat_finds_carry_vendor_product_revision_and_serial() {
    // Spec 11, "Identity": "for each SubDevice its position, alias address, vendor ID, product
    // code, revision, and serial number". The wrong behaviour it catches: a find that drops the
    // identity read from the bus and reports only a model and slot.
    let recorder = identify(
        "eth0",
        identification(
            EthercatIdentifiedSubDevice {
                slot: 1,
                configured_address: 1001,
                alias: 7,
                model: "EL1008".to_string(),
                channels: 8,
                vendor_id: 0x0000_0002,
                product_id: 0x07D3_0532,
                revision: 0x0011_0000,
                serial: 0x1234_5678,
            },
            4,
            2,
        ),
    );

    assert_eq!(recorder.finds.len(), 1);
    assert_eq!(
        recorder.finds[0].identity,
        Identity::Ethercat(EthercatIdentity {
            adapter: "eth0".to_string(),
            slot: 1,
            configured_address: 1001,
            alias: 7,
            model: "EL1008".to_string(),
            channels: 8,
            vendor_id: 0x0000_0002,
            product_id: 0x07D3_0532,
            revision: 0x0011_0000,
            serial: 0x1234_5678,
            input_bytes: 4,
            output_bytes: 2,
        })
    );
}

#[test]
fn ethercat_finds_report_input_and_output_bytes() {
    // Spec 11, "Sources": the EtherCAT find comes from a "bus scan that reads each SubDevice's
    // identity"; the bus's mapped process data lengths are reported with it. The wrong behaviour
    // it catches: dropping the byte totals that identification read from the mapped process data.
    let recorder = identify(
        "eth0",
        identification(
            EthercatIdentifiedSubDevice {
                slot: 0,
                configured_address: 1000,
                alias: 0,
                model: "EK1100".to_string(),
                channels: 1,
                vendor_id: 0,
                product_id: 0,
                revision: 0,
                serial: 0,
            },
            3,
            5,
        ),
    );

    assert_eq!(recorder.finds.len(), 1);
    let Identity::Ethercat(identity) = &recorder.finds[0].identity else {
        panic!("an EtherCAT candidate must become an EtherCAT find");
    };
    assert_eq!(identity.input_bytes, 3, "the bus's mapped input bytes");
    assert_eq!(identity.output_bytes, 5, "the bus's mapped output bytes");
}

#[test]
fn ethercat_search_refuses_an_adapter_owned_by_a_running_driver() {
    // Spec 11, "Safety": "it refuses an adapter that a running EtherCAT driver of the runtime owns;
    // the driver's own live view of that bus is reported instead". The wrong behaviour it catches:
    // a search that opens an adapter a running driver already owns and disturbs its bus.
    let local = || -> Vec<SmolStr> { Vec::new() };
    let identify = |_params: &toml::Value| -> Result<EthercatIdentification, RuntimeError> {
        panic!("a refused adapter must never be scanned")
    };
    let seams = EthercatSeams {
        local_adapters: &local,
        identify: &identify,
    };
    let context = context(vec![Some("eth0".to_string())], vec![SmolStr::new("eth0")]);

    let plans = adapter_plans(&context, None, &seams).expect("plans");

    assert_eq!(plans.len(), 1, "a refused adapter is never scanned");
    let AdapterPlan::Refused(adapter) = &plans[0] else {
        panic!("an owned adapter must be refused, got {:?}", plans[0]);
    };
    assert_eq!(adapter.as_str(), "eth0");
    assert!(
        refused_warning(adapter).contains("eth0"),
        "the refusal warning must name the adapter"
    );
}

#[test]
fn ethercat_search_lists_the_adapters_it_scans() {
    // O22 and spec 11, "Sources": a search that names no adapter scans this computer's adapters.
    // The wrong behaviour it catches: scanning only the project's driver adapter, or dropping a
    // listed adapter.
    let local = || vec![SmolStr::new("eth0"), SmolStr::new("eth1")];
    let identify = |_params: &toml::Value| -> Result<EthercatIdentification, RuntimeError> {
        Ok(EthercatIdentification {
            subdevices: Vec::new(),
            input_bytes: 0,
            output_bytes: 0,
        })
    };
    let seams = EthercatSeams {
        local_adapters: &local,
        identify: &identify,
    };
    let context = context(vec![None], Vec::new());

    let plans = adapter_plans(&context, None, &seams).expect("plans");

    let scanned: Vec<&str> = plans
        .iter()
        .filter_map(|plan| match plan {
            AdapterPlan::Scan(adapter) => Some(adapter.as_str()),
            AdapterPlan::Refused(_) => None,
        })
        .collect();
    assert_eq!(scanned, ["eth0", "eth1"]);
}

#[test]
fn an_unknown_owner_means_no_ethercat_scan() {
    // Spec 11, "Safety": a search never disturbs a bus that a running EtherCAT driver owns; when a
    // running driver's adapter cannot be determined, no EtherCAT adapter is scanned and a warning
    // says why. The wrong behaviour it catches: scanning adapters anyway when ownership is unknown,
    // which can disturb a bus a running EtherCAT driver owns.
    let local = || vec![SmolStr::new("eth0"), SmolStr::new("eth1")];
    let identify = |_params: &toml::Value| -> Result<EthercatIdentification, RuntimeError> {
        panic!("no adapter may be scanned when the owner is unknown")
    };
    let seams = EthercatSeams {
        local_adapters: &local,
        identify: &identify,
    };
    let context = context(vec![None], vec![SmolStr::new(UNKNOWN_ADAPTER_OWNER)]);

    let error = adapter_plans(&context, None, &seams)
        .expect_err("an unknown EtherCAT owner must refuse every EtherCAT scan");

    assert!(
        error.contains("EtherCAT"),
        "the warning must say why no adapter is scanned: {error}"
    );
}

#[test]
fn mock_ethercat_finds_are_simulated() {
    // Spec 11, "Confidence": "A simulated (mock) result carries simulated = true and is never
    // presented as hardware", and spec 11: "mock discovery ... remains mock evidence". The wrong
    // behaviour it catches: reporting the simulated bus under the hardware source `ethercat_bus`
    // with `simulated = false`.
    let recorder = identify(
        "mock",
        identification(
            EthercatIdentifiedSubDevice {
                slot: 1,
                configured_address: 1,
                alias: 0,
                model: "EL1008".to_string(),
                channels: 8,
                vendor_id: 0,
                product_id: 0,
                revision: 0,
                serial: 0,
            },
            0,
            0,
        ),
    );

    assert_eq!(recorder.sources, ["ethercat_mock"]);
    assert!(recorder.finds[0].simulated, "a mock find is simulated");
    assert_ne!(recorder.finds[0].source, "ethercat_bus");
}

#[test]
fn ethercat_finds_are_confirmed_never_observed() {
    // Spec 11, "Confidence": an EtherCAT identity read is `confirmed`, and "No other value, such
    // as `observed`, is reported". The wrong behaviour it catches: reporting the candidate with
    // the word `observed`, which no longer matches the closed confidence vocabulary.
    let recorder = identify(
        "eth0",
        identification(
            EthercatIdentifiedSubDevice {
                slot: 0,
                configured_address: 1000,
                alias: 0,
                model: "EK1100".to_string(),
                channels: 1,
                vendor_id: 0,
                product_id: 0,
                revision: 0,
                serial: 0,
            },
            0,
            0,
        ),
    );

    assert_eq!(recorder.words, ["confirmed"]);
    assert_eq!(recorder.finds[0].confidence, DiscoveryConfidence::Confirmed);
    assert_ne!(recorder.words[0], "observed");
}
