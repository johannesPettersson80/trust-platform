use std::io::Read;
use std::net::{IpAddr, Ipv4Addr, TcpListener};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::json;
use smol_str::SmolStr;

use super::super::model::SourceId;
use super::catalog::default_subnets;
use super::find::{confidence, host_part};
use super::report::{CandidateSink, Deadline, DiscoverCandidate};
use super::scope::DiscoverScope;
use super::{ads, ethercat, gpio, modbus_tcp, mqtt, opcua, trust_runtimes};
use crate::ads::onboarding::RuntimeAddressCandidate;
use crate::connectors::DiscoveryConfidence;
use crate::discovery::{DiscoveryEntry, DiscoveryState};
use crate::opcua::{OpcUaClientEndpointInfo, OpcUaMessageSecurityMode, OpcUaSecurityPolicy};

fn interface(nic: &str, ip: &str) -> RuntimeAddressCandidate {
    RuntimeAddressCandidate {
        ip: ip.to_string(),
        nic: Some(nic.to_string()),
        prefix_len: Some(24),
        broadcast: None,
    }
}

#[test]
fn search_confidence_is_the_closed_set() {
    assert_eq!(confidence("confirmed"), DiscoveryConfidence::Confirmed);
    assert_eq!(confidence("likely"), DiscoveryConfidence::Likely);
    assert_eq!(
        confidence("port_reachable"),
        DiscoveryConfidence::PortReachable
    );
    assert_eq!(confidence("unavailable"), DiscoveryConfidence::Unavailable);
    // `comm.discover` says `observed` for an ADS identify reply, a GetEndpoints answer, an
    // EtherCAT read, a GPIO listing and an mDNS answer; the search reports each as confirmed.
    assert_eq!(confidence("observed"), DiscoveryConfidence::Confirmed);
    for word in ["", "Confirmed", "healthy", "port-reachable"] {
        assert_eq!(
            confidence(word),
            DiscoveryConfidence::Unavailable,
            "word={word:?}"
        );
    }
    for value in [
        DiscoveryConfidence::Confirmed,
        DiscoveryConfidence::Likely,
        DiscoveryConfidence::PortReachable,
        DiscoveryConfidence::Unavailable,
    ] {
        let word = serde_json::to_value(value).expect("serialize");
        assert!(
            [
                json!("confirmed"),
                json!("likely"),
                json!("port_reachable"),
                json!("unavailable")
            ]
            .contains(&word),
            "{word}"
        );
    }
}

#[test]
fn default_subnet_follows_the_decided_rule() {
    let interfaces = [
        interface("lo", "127.0.0.1"),
        interface("eth0", "192.168.1.20"),
        interface("eth0", "192.168.1.21"),
        interface("wlan0", "10.0.5.7"),
        interface("docker0", "172.17.0.1"),
        interface("eth1", "8.8.8.8"),
        interface("eth2", "169.254.10.3"),
        interface("eth0", "fe80::1"),
    ];
    assert_eq!(
        default_subnets(&interfaces, &[]),
        vec!["192.168.1.0/24", "10.0.5.0/24", "172.17.0.0/24"]
    );
    assert_eq!(
        default_subnets(&interfaces, &[SmolStr::new("wlan0")]),
        vec!["10.0.5.0/24"]
    );
    assert!(default_subnets(&interfaces, &[SmolStr::new("eth1")]).is_empty());
}

#[test]
fn known_address_host_drops_only_its_port() {
    assert_eq!(host_part("10.0.0.9:502"), "10.0.0.9");
    assert_eq!(host_part("10.0.0.9"), "10.0.0.9");
    assert_eq!(host_part("plc.local:1883"), "plc.local");
    assert_eq!(host_part("[fe80::1]:4840"), "fe80::1");
    assert_eq!(host_part("[fe80::1]"), "fe80::1");
    assert_eq!(host_part("fe80::1"), "fe80::1");
}

/// A sink that notes when each candidate arrives.
#[derive(Default)]
struct Recorder {
    found: Vec<(Instant, String)>,
    warnings: Vec<String>,
}

impl CandidateSink for Recorder {
    fn found(&mut self, candidate: DiscoverCandidate) {
        self.found.push((Instant::now(), candidate.id));
    }

    fn warn(&mut self, warning: String) {
        self.warnings.push(warning);
    }
}

/// What the sources search besides a known address: the simulated EtherCAT bus, and a runtime's
/// own discovery that holds one runtime.
struct Targets {
    mock_bus: DiscoverScope,
    runtimes: DiscoveryState,
}

impl Targets {
    fn new() -> Self {
        let runtimes = DiscoveryState::new();
        runtimes.replace_entries(vec![DiscoveryEntry {
            id: SmolStr::new("line-1"),
            name: SmolStr::new("line"),
            addresses: vec![IpAddr::V4(Ipv4Addr::LOCALHOST)],
            web_port: None,
            web_tls: false,
            mesh_port: None,
            control: Some(SmolStr::new("tcp://127.0.0.1:5680")),
            host_group: None,
            last_seen_ns: 0,
        }]);
        Self {
            mock_bus: DiscoverScope {
                adapter: Some("mock".to_string()),
                ..DiscoverScope::default()
            },
            runtimes,
        }
    }

    /// Runs the search of `source` once, with the known address of `host`.
    fn discover(
        &self,
        source: SourceId,
        host: &DiscoverScope,
        deadline: Deadline,
        sink: &mut dyn CandidateSink,
    ) -> Result<(), String> {
        match source {
            SourceId::TrustRuntime => {
                trust_runtimes::discover(host, Some(&self.runtimes), deadline, sink)
            }
            SourceId::Ads => ads::discover(host, deadline, sink),
            SourceId::ModbusTcp => modbus_tcp::discover(host, deadline, sink),
            SourceId::Mqtt => mqtt::discover(host, deadline, sink),
            SourceId::Opcua => opcua::discover(host, deadline, sink),
            SourceId::Ethercat => ethercat::discover(&self.mock_bus, None, deadline, sink),
            SourceId::Gpio => {
                gpio::discover(deadline, sink);
                Ok(())
            }
            SourceId::Serial | SourceId::Usb | SourceId::Bluetooth => {
                unreachable!("{} has no comm.discover search", source.as_str())
            }
        }
    }
}

/// A port that takes each connection, reads one byte and closes it: a Modbus TCP or MQTT probe
/// finds only TCP reachability there, at once.
fn closing_port() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind closing port");
    let address = listener
        .local_addr()
        .expect("closing port address")
        .to_string();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else {
                continue;
            };
            let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
            let mut byte = [0_u8; 1];
            let _ = stream.read(&mut byte);
        }
    });
    address
}

/// A port that takes each connection and never answers.
fn silent_port() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind silent port");
    let address = listener
        .local_addr()
        .expect("silent port address")
        .to_string();
    thread::spawn(move || {
        // Each connection stays open, unanswered, until the test ends.
        let mut open = Vec::new();
        for stream in listener.incoming() {
            let Ok(stream) = stream else {
                continue;
            };
            open.push(stream);
        }
    });
    address
}

#[test]
fn each_source_reports_to_the_sink_before_the_deadline() {
    let targets = Targets::new();
    let answering = DiscoverScope {
        host: Some(closing_port()),
        timeout_ms: Some(1_000),
        ..DiscoverScope::default()
    };

    // What a source finds reaches its sink at once, before the deadline.
    for source in [
        SourceId::TrustRuntime,
        SourceId::ModbusTcp,
        SourceId::Mqtt,
        SourceId::Ethercat,
    ] {
        let name = source.as_str();
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut sink = Recorder::default();
        targets
            .discover(source, &answering, Deadline::at(deadline), &mut sink)
            .expect(name);
        assert!(
            !sink.found.is_empty(),
            "{name} found nothing: {:?}",
            sink.warnings
        );
        assert!(
            sink.found.iter().all(|(at, _)| *at < deadline),
            "{name}: {:?}",
            sink.found
        );
    }

    // A source whose deadline has passed starts no probe and reports nothing.
    for source in [
        SourceId::TrustRuntime,
        SourceId::Ads,
        SourceId::ModbusTcp,
        SourceId::Mqtt,
        SourceId::Opcua,
        SourceId::Ethercat,
        SourceId::Gpio,
    ] {
        let name = source.as_str();
        let started = Instant::now();
        let mut sink = Recorder::default();
        targets
            .discover(source, &answering, Deadline::at(started), &mut sink)
            .expect(name);
        assert!(sink.found.is_empty(), "{name}: {:?}", sink.found);
        assert!(started.elapsed() < Duration::from_secs(1), "{name}");
    }

    // A probe that would wait 2 s for an answer ends at a deadline 300 ms away.
    let unanswered = DiscoverScope {
        host: Some(silent_port()),
        timeout_ms: Some(2_000),
        ..DiscoverScope::default()
    };
    for source in [SourceId::ModbusTcp, SourceId::Mqtt] {
        let name = source.as_str();
        let started = Instant::now();
        let deadline = Deadline::at(started + Duration::from_millis(300));
        let mut sink = Recorder::default();
        targets
            .discover(source, &unanswered, deadline, &mut sink)
            .expect(name);
        assert!(
            started.elapsed() < Duration::from_millis(1_500),
            "{name} waited {:?}",
            started.elapsed()
        );
    }
}

/// A sink that keeps the confidence word of every candidate it is handed.
#[derive(Default)]
struct ConfidenceWords {
    words: Vec<&'static str>,
}

impl CandidateSink for ConfidenceWords {
    fn found(&mut self, candidate: DiscoverCandidate) {
        self.words.push(candidate.confidence);
    }

    fn warn(&mut self, _warning: String) {}
}

/// The closed confidence vocabulary (specification 11, "Confidence").
const CLOSED_CONFIDENCE: [&str; 3] = ["confirmed", "likely", "port_reachable"];

#[test]
fn comm_discover_never_reports_observed() {
    let targets = Targets::new();
    let answering = DiscoverScope {
        host: Some(closing_port()),
        timeout_ms: Some(1_000),
        ..DiscoverScope::default()
    };

    let mut words = Vec::new();

    // The C3a sources that prove themselves with a protocol answer.
    for source in [SourceId::ModbusTcp, SourceId::Mqtt] {
        let name = source.as_str();
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut sink = ConfidenceWords::default();
        targets
            .discover(source, &answering, Deadline::at(deadline), &mut sink)
            .expect(name);
        assert!(!sink.words.is_empty(), "{name} found nothing");
        words.extend(sink.words);
    }

    // OPC UA announces endpoints only to a server that answers, so its candidate projection is
    // checked directly; every endpoint is `confirmed`, never `observed`.
    let endpoint = opcua::endpoint_candidate(&OpcUaClientEndpointInfo {
        endpoint_url: "opc.tcp://10.0.0.5:4840".to_string(),
        application_name: Some("Line PLC".to_string()),
        application_uri: None,
        product_uri: None,
        security_policy: Some(OpcUaSecurityPolicy::None),
        security_policy_uri: "http://opcfoundation.org/UA/SecurityPolicy#None".to_string(),
        security_mode: Some(OpcUaMessageSecurityMode::None),
        anonymous_supported: true,
        username_supported: false,
        unsupported: false,
        certificate_sha256: None,
    });
    assert_eq!(endpoint.confidence, "confirmed");
    words.push(endpoint.confidence);

    for word in words {
        assert!(
            CLOSED_CONFIDENCE.contains(&word),
            "a source reported '{word}', outside the closed confidence vocabulary"
        );
    }
}
