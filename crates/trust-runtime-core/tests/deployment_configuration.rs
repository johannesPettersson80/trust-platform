//! Section 7 of the deployment record (specification 34 §5.1.1): the settings a device without an
//! operating system needs from `runtime.toml` and `io.toml`, its bytes, and every field a reader
//! refuses.

use trust_runtime_core::bytecode::BytecodeVersion;
use trust_runtime_core::crc32;
use trust_runtime_core::deployment::{
    ApplicationIdentity, Capabilities, Configuration, Contracts, ControlMode, DeploymentRecord,
    DeploymentRecordError as Error, GpioInput, GpioOutput, HangAction, IoDriver, LogLevel,
    NumericContract, ProcessImageSummary, RequestedLimits, SafeStateEntry, SchedulingProfile,
    TaskOverride, TimingEvidence, RECORD_VERSION,
};
use trust_runtime_core::io_address::IoAddress;
use trust_runtime_core::value::{Duration, Value};
use trust_runtime_core::vm::PreparationLimits;
use trust_runtime_core::watchdog::{FaultPolicy, RetainMode, WatchdogAction, WatchdogPolicy};

/// Section 7's identifier.
const CONFIGURATION_SECTION: u16 = 7;

/// Section 7's payload of [`small_configuration`], byte by byte.
const PAYLOAD: [u8; 67] = [
    3, b'R', b'E', b'S', // the resource name, `RES`
    0x80, 0x96, 0x98, 0, 0, 0, 0, 0, // the cycle interval, 10,000,000 ns
    0, // control.debug_enabled
    1, // control.mode: production
    3, // log.level: info
    0, // retain.mode: none
    0, 0, 0, 0, 0, 0, 0, 0, // retain.save_interval_ms
    0, // watchdog.enabled
    0, 0, 0, 0, 0, 0, 0, 0, // watchdog.timeout_ms
    1, // watchdog.action: halt
    1, // watchdog.hang_action: fault
    1, // fault.policy: halt
    0, // no task overrides
    1, 0, // one I/O driver
    4, b'g', b'p', b'i', b'o', // its name, `gpio`
    1,    // enabled
    0, 0, // no inputs
    1, 0, // one output
    2, 1, 0, 0, 0, 0, 0, // its address, `%QX0.0`: output, size X, byte 0, bit 0
    6, b'G', b'P', b'I', b'O', b'1', b'8', // its line, `GPIO18`
    0,    // not inverted
    0,    // initial level false
    0, 0, // no safe state
];

/// A record whose only filled section is section 7; every other section is minimal.
fn record_with(configuration: Configuration) -> DeploymentRecord {
    DeploymentRecord {
        application: ApplicationIdentity {
            program_sha256: [0; 32],
            program_bytes: 0,
            bytecode_version: BytecodeVersion::SOURCE_FREE,
            bytecode_flags: 0,
            configuration_sha256: [0; 32],
            retain_schema_sha256: [0; 32],
        },
        contracts: Contracts {
            scheduling_profile: SchedulingProfile::COOPERATIVE_RESOURCE_V1,
            numeric_contract: NumericContract::V1,
            timing_evidence: TimingEvidence::UNQUALIFIED,
            capabilities: Capabilities::NONE,
        },
        limits: RequestedLimits::from_preparation(&PreparationLimits::default()),
        process_image: ProcessImageSummary::default(),
        tasks: Vec::new(),
        imports: Vec::new(),
        configuration,
    }
}

/// The small configuration of the layout test: resource `RES`, a 10 ms cycle and one `gpio` output
/// `%QX0.0` on line `GPIO18`, with no task overrides and no safe state.
fn small_configuration() -> Configuration {
    Configuration {
        resource_name: "RES".into(),
        cycle_interval_nanos: 10_000_000,
        control_debug_enabled: false,
        control_mode: ControlMode::Production,
        log_level: LogLevel::Info,
        retain_mode: RetainMode::None,
        retain_save_interval: Duration::ZERO,
        watchdog: WatchdogPolicy {
            enabled: false,
            timeout: Duration::ZERO,
            action: WatchdogAction::Halt,
        },
        watchdog_hang_action: HangAction::Fault,
        fault_policy: FaultPolicy::Halt,
        task_overrides: None,
        io_drivers: vec![IoDriver {
            name: "gpio".into(),
            enabled: true,
            inputs: Vec::new(),
            outputs: vec![GpioOutput {
                address: IoAddress::parse("%QX0.0").expect("a flat output bit"),
                line: "GPIO18".into(),
                invert: false,
                initial: false,
            }],
        }],
        safe_state: Vec::new(),
    }
}

/// The configuration of the round-trip test: every kind of field it holds, filled.
fn full_configuration() -> Configuration {
    Configuration {
        resource_name: "Line 4".into(),
        cycle_interval_nanos: 25_000_000,
        control_debug_enabled: true,
        control_mode: ControlMode::Debug,
        log_level: LogLevel::Trace,
        retain_mode: RetainMode::File,
        retain_save_interval: Duration::from_millis(1_000),
        watchdog: WatchdogPolicy {
            enabled: true,
            timeout: Duration::from_millis(50),
            action: WatchdogAction::Restart,
        },
        watchdog_hang_action: HangAction::Abort,
        fault_policy: FaultPolicy::SafeHalt,
        task_overrides: Some(vec![
            TaskOverride {
                name: "Fast".into(),
                interval: Duration::from_millis(5),
                priority: 0,
                single: None,
                programs: vec!["Main".into(), "Inputs".into()],
            },
            TaskOverride {
                name: "Slow".into(),
                interval: Duration::from_millis(100),
                priority: 3,
                single: Some("Ready".into()),
                programs: vec!["Outputs".into()],
            },
        ]),
        io_drivers: vec![
            IoDriver {
                name: "gpio".into(),
                enabled: true,
                inputs: vec![GpioInput {
                    address: IoAddress::parse("%IX0.0").expect("a flat input bit"),
                    line: "PD12".into(),
                    invert: true,
                    debounce_ms: 5,
                }],
                outputs: vec![GpioOutput {
                    address: IoAddress::parse("%QX0.1").expect("a flat output bit"),
                    line: "GPIO18".into(),
                    invert: false,
                    initial: true,
                }],
            },
            IoDriver {
                name: "loopback".into(),
                enabled: false,
                inputs: Vec::new(),
                outputs: Vec::new(),
            },
        ],
        safe_state: vec![
            SafeStateEntry {
                address: IoAddress::parse("%QX0.0").expect("a flat output bit"),
                value: Value::Bool(true),
            },
            SafeStateEntry {
                address: IoAddress::parse("%QW2").expect("a flat output word"),
                value: Value::Int(-2),
            },
        ],
    }
}

fn u16_at(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([bytes[at], bytes[at + 1]])
}

fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

/// The sections of an encoded record, as identifier, reserved field and payload.
fn sections(bytes: &[u8]) -> Vec<(u16, u16, Vec<u8>)> {
    let end = bytes.len() - 4;
    let mut at = 12;
    let mut found = Vec::new();
    while at < end {
        let length = u32_at(bytes, at + 4) as usize;
        found.push((
            u16_at(bytes, at),
            u16_at(bytes, at + 2),
            bytes[at + 8..at + 8 + length].to_vec(),
        ));
        at += 8 + length;
    }
    found
}

/// A record of `version` with `count` in its header and these sections, its length and CRC
/// computed as a writer computes them.
fn raw(version: u16, count: u16, sections: &[(u16, u16, Vec<u8>)]) -> Vec<u8> {
    let mut bytes = b"TRDR".to_vec();
    bytes.extend_from_slice(&version.to_le_bytes());
    bytes.extend_from_slice(&count.to_le_bytes());
    bytes.extend_from_slice(&[0; 4]);
    for (id, reserved, payload) in sections {
        bytes.extend_from_slice(&id.to_le_bytes());
        bytes.extend_from_slice(&reserved.to_le_bytes());
        bytes.extend_from_slice(&u32::try_from(payload.len()).unwrap().to_le_bytes());
        bytes.extend_from_slice(payload);
    }
    let length = u32::try_from(bytes.len() + 4).unwrap();
    bytes[8..12].copy_from_slice(&length.to_le_bytes());
    let crc = crc32::checksum(&bytes);
    bytes.extend_from_slice(&crc.to_le_bytes());
    bytes
}

/// The record of [`small_configuration`] with section 7's payload replaced by `payload`.
fn payload_record(payload: &[u8]) -> Vec<u8> {
    let bytes = record_with(small_configuration())
        .encode()
        .expect("the record encodes");
    let mut parts = sections(&bytes);
    let section = parts
        .iter_mut()
        .find(|part| part.0 == CONFIGURATION_SECTION)
        .expect("the record has section 7");
    section.2 = payload.to_vec();
    raw(RECORD_VERSION, u16::try_from(parts.len()).unwrap(), &parts)
}

/// Why a record whose section 7 payload is `payload` is refused.
fn refused_payload(payload: &[u8]) -> Error {
    DeploymentRecord::decode(&payload_record(payload)).expect_err("a malformed record is refused")
}

/// [`PAYLOAD`] with one byte changed.
fn payload_with(index: usize, byte: u8) -> Vec<u8> {
    let mut payload = PAYLOAD.to_vec();
    payload[index] = byte;
    payload
}

#[test]
fn the_configuration_section_round_trips() {
    let configuration = full_configuration();
    let record = record_with(configuration.clone());
    let bytes = record.encode().expect("section 7 encodes every field");
    assert_eq!(
        bytes.len(),
        record.encoded_len(),
        "the length is the writer's"
    );
    let decoded = DeploymentRecord::decode(&bytes).expect("the record decodes");
    assert_eq!(decoded, record);
    assert_eq!(decoded.configuration, configuration);
    assert_eq!(decoded.encode().expect("the record encodes again"), bytes);
}

#[test]
fn the_configuration_section_has_the_specified_bytes() {
    let record = record_with(small_configuration());
    let bytes = record.encode().expect("section 7 encodes");
    let parts = sections(&bytes);
    assert_eq!(parts.len(), 7, "one section each, in order");
    let (id, reserved, payload) = parts
        .into_iter()
        .find(|part| part.0 == CONFIGURATION_SECTION)
        .expect("the record has section 7");
    assert_eq!(id, CONFIGURATION_SECTION);
    assert_eq!(reserved, 0, "its reserved field is 0");
    assert_eq!(payload.len(), PAYLOAD.len(), "67 bytes");
    assert_eq!(payload, PAYLOAD);
    // The payload's own length and the record's length agree with the 67 bytes.
    assert_eq!(record.encoded_len(), bytes.len());
}

/// Assert that section 7's payload with one byte changed is refused with `Error::Field(field)`.
fn rejected_field(index: usize, byte: u8, field: &'static str) {
    assert_eq!(
        refused_payload(&payload_with(index, byte)),
        Error::Field(field),
        "byte {index} of section 7"
    );
}

#[test]
fn a_configuration_with_a_bad_code_flag_bit_or_text_is_refused() {
    // A code outside its list.
    rejected_field(13, 3, "a control mode");
    rejected_field(14, 6, "a log level");
    rejected_field(15, 2, "a retain mode");
    rejected_field(33, 4, "a watchdog action");
    rejected_field(34, 0, "a watchdog hang action");
    rejected_field(35, 0, "a fault policy");
    rejected_field(36, 2, "the task overrides");
    rejected_field(49, 4, "an I/O address");
    rejected_field(50, 6, "an I/O address");
    // A flag other than 0 or 1.
    rejected_field(12, 2, "the control debug flag");
    rejected_field(44, 2, "the driver enabled flag");
    rejected_field(63, 2, "the GPIO output invert flag");
    rejected_field(64, 2, "the GPIO output initial flag");
    // A bit above 7, and a bit with a size other than X.
    rejected_field(55, 8, "an I/O address");
    let mut wide = PAYLOAD.to_vec();
    wide[50] = 3; // the size W
    wide[55] = 1; // its bit, which only X may set
    assert_eq!(refused_payload(&wide), Error::Field("an I/O address"));
    // An empty name where the section requires one, and an empty line.
    rejected_field(39, 0, "a driver name");
    rejected_field(56, 0, "a GPIO line");
    // A text that is not UTF-8.
    rejected_field(1, 0xFF, "the resource name");
    // A payload the counts do not fit, and a payload of another length.
    assert_eq!(
        refused_payload(&payload_with(65, 1)),
        Error::SectionLength(CONFIGURATION_SECTION),
        "the safe state count does not fit the payload"
    );
    let short = &PAYLOAD[..PAYLOAD.len() - 1];
    let mut longer = PAYLOAD.to_vec();
    longer.push(0);
    for payload in [short, longer.as_slice()] {
        assert_eq!(
            refused_payload(payload),
            Error::SectionLength(CONFIGURATION_SECTION),
            "a payload of another length"
        );
    }
    // A safe-state value: a code outside its list, and a byte above the type's width that is not 0.
    let mut entry = PAYLOAD.to_vec();
    entry[65] = 1; // one safe-state entry
    entry.extend_from_slice(&[2, 1, 0, 0, 0, 0, 0]); // `%QX0.0`
    entry.push(2); // SINT
    entry.extend_from_slice(&[0xFE, 1, 0, 0, 0, 0, 0, 0]);
    assert_eq!(
        refused_payload(&entry),
        Error::Field("an I/O safe-state value"),
        "a byte above the SINT's width"
    );
    entry[74] = 16; // a value type the section has no code for
    assert_eq!(
        refused_payload(&entry),
        Error::Field("an I/O safe-state value")
    );
    entry[74] = 2;
    entry[76] = 0; // zero-extended above the SINT's width
    let decoded = DeploymentRecord::decode(&payload_record(&entry)).expect("a zero-extended value");
    assert_eq!(
        decoded.configuration.safe_state,
        [SafeStateEntry {
            address: IoAddress::parse("%QX0.0").expect("a flat output bit"),
            value: Value::SInt(-2),
        }]
    );
}

#[test]
fn a_writer_refuses_what_the_reader_would_refuse() {
    let mut long_name = small_configuration();
    long_name.resource_name = "R".repeat(256).into();
    assert_eq!(
        record_with(long_name).encode(),
        Err(Error::Field("the resource name")),
        "a text over 255 bytes"
    );

    let mut empty_name = small_configuration();
    empty_name.io_drivers[0].name = "".into();
    assert_eq!(
        record_with(empty_name).encode(),
        Err(Error::Field("a driver name"))
    );

    let mut hierarchical = small_configuration();
    hierarchical.safe_state = vec![SafeStateEntry {
        address: IoAddress::parse("%IW0.1").expect("a hierarchical input word"),
        value: Value::Bool(true),
    }];
    assert_eq!(
        record_with(hierarchical).encode(),
        Err(Error::Field("an I/O address")),
        "an address the seven bytes cannot hold"
    );

    let mut string_value = small_configuration();
    string_value.safe_state = vec![SafeStateEntry {
        address: IoAddress::parse("%QX0.0").expect("a flat output bit"),
        value: Value::String("on".into()),
    }];
    assert_eq!(
        record_with(string_value).encode(),
        Err(Error::Field("an I/O safe-state value"))
    );

    let mut lines_elsewhere = small_configuration();
    lines_elsewhere.io_drivers[0].name = "loopback".into();
    lines_elsewhere.io_drivers[0].inputs = vec![GpioInput {
        address: IoAddress::parse("%IX0.0").expect("a flat input bit"),
        line: "GPIO9".into(),
        invert: false,
        debounce_ms: 0,
    }];
    assert_eq!(
        record_with(lines_elsewhere).encode(),
        Err(Error::Field("the GPIO lines of another driver"))
    );
}

#[test]
fn a_record_without_section_7_is_refused() {
    let bytes = record_with(small_configuration())
        .encode()
        .expect("the record encodes");
    let mut parts = sections(&bytes);
    parts.retain(|part| part.0 != CONFIGURATION_SECTION);
    assert_eq!(
        DeploymentRecord::decode(&raw(RECORD_VERSION, 6, &parts))
            .expect_err("section 7 is missing"),
        Error::MissingSection(CONFIGURATION_SECTION)
    );
}
