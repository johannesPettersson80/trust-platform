//! The `capabilities` operation over the fake platform and the device runner: the descriptor and
//! the last reset its reply renders, and the role that may read it (specification 11, the runtime
//! device capabilities contract; specification 34 §5.1.1).

use serde_json::json;
use smol_str::SmolStr;
use trust_runtime_core::bytecode::BytecodeVersion;
use trust_runtime_core::deployment::{
    Capabilities, DeviceDescriptor, NumericContract, RequestedLimits, SchedulingProfile,
    TimingEvidence,
};

use super::{LastReset, ResetCauseName};
use crate::access::AccessRole;
use crate::dispatch::dispatch_value;
use crate::tests::fake_host::FakeHost;
use crate::tests::runner_ports::{Fixture, TestPlatform};

const NETWORK: Option<&str> = Some("127.0.0.1:9000");

/// A chip's descriptor: the C6's STBC versions, scheduling profile, numeric contract, timing
/// evidence and no capability (specification 34 §5.1.1 and TARGET-03), the chip file's application
/// slot (`[layout] application`, 6112 KiB, that is 6,258,688 bytes) and its event store (`[events]`,
/// 200 by default and 200 at most). Its bounded maxima are distinct fixture values, and three are
/// unbounded (`u64::MAX`, no maximum), so a reply that swaps two limits, renames one or writes an
/// unbounded limit as a number fails the literal comparison below.
fn chip_descriptor() -> DeviceDescriptor {
    DeviceDescriptor {
        bytecode_versions: &[
            BytecodeVersion::SOURCE_FREE_V2,
            BytecodeVersion::SOURCE_FREE,
        ],
        scheduling_profiles: &[SchedulingProfile::COOPERATIVE_RESOURCE_V1],
        numeric_contracts: &[NumericContract::V1],
        timing_evidence: TimingEvidence::UNQUALIFIED,
        capabilities: Capabilities::NONE,
        maxima: RequestedLimits {
            max_artifact_bytes: 1_001,
            max_preparation_bytes: u64::MAX,
            max_preparation_work: 1_003,
            max_validation_scratch_bytes: u64::MAX,
            max_validation_work: 1_005,
            max_construction_values: 1_006,
            max_construction_bytes: 1_007,
            max_process_image_bytes: u64::MAX,
            max_work: 1_009,
            max_construction_work: 1_010,
            max_call_depth: 1_011,
        },
        event_store_default: 200,
        event_store_max: 200,
        application_slot_bytes: Some(6_258_688),
    }
}

#[test]
fn capabilities_answers_the_chips_descriptor_and_its_last_reset() {
    // Specification 11, the runtime device capabilities contract: a device without an operating
    // system answers its own descriptor and the reset its boot read. Here the boot read a watchdog
    // reset with the text "scan" and boot count 3, the chip's counter for the boot that reads it
    // (specification 34, the fixed-size reset record). The request goes through the device runner,
    // the path a chip's requests take. The wrong behaviours it catches: the runner answers the
    // computer's descriptor instead of the platform's; the last reset is dropped or read as null
    // on a chip; a cause is renamed on the way (a watchdog reported as `panic`); the text or the
    // boot count is lost; a limit is rendered under another name or in another field (the bounded
    // maxima are distinct); and an unbounded limit is written as a number instead of `null`.
    let platform = TestPlatform {
        device_descriptor: chip_descriptor(),
        last_reset: Some(LastReset {
            cause: ResetCauseName::Watchdog,
            text: SmolStr::new_static("scan"),
            boot_count: 3,
        }),
        ..TestPlatform::default()
    };
    let fixture = Fixture::board_probe();
    let mut runner = fixture.runner_with(platform);
    let reply = fixture.call(&mut runner, &json!({"id": 1, "type": "capabilities"}));
    assert_eq!(reply["ok"], json!(true));
    assert_eq!(
        reply["result"],
        json!({
            "bytecode_versions": ["2.0", "3.0"],
            "scheduling_profiles": ["cooperative-resource-v1"],
            "numeric_contracts": ["1"],
            "timing_evidence": "unqualified",
            "capabilities": [],
            "maxima": {
                "max_artifact_bytes": 1_001,
                "max_preparation_bytes": null,
                "max_preparation_work": 1_003,
                "validation.max_scratch_bytes": null,
                "validation.max_work": 1_005,
                "max_construction_values": 1_006,
                "max_construction_bytes": 1_007,
                "max_process_image_bytes": null,
                "max_work": 1_009,
                "max_construction_work": 1_010,
                "max_call_depth": 1_011,
            },
            "event_store_default": 200,
            "event_store_max": 200,
            "application_slot_bytes": 6_258_688,
            "last_reset": {"cause": "watchdog", "text": "scan", "boot_count": 3},
        }),
        "the chip's reply names the device's descriptor fields and its watchdog reset"
    );
}

#[test]
fn capabilities_is_read_only_for_a_viewer() {
    // Specification 11, the runtime device capabilities contract: `capabilities` takes no
    // parameters, changes nothing, and its minimum role is Viewer. A paired viewer's token reads it,
    // and so does an unauthenticated network client, which resolves to a viewer. The reply is the
    // descriptor alone (no `access` block, which only `status` carries), and the request leaves one
    // audit record with no operation details (specification 11, the runtime control audit). The
    // wrong behaviours it catches: a registry row that asks for more than Viewer (a viewer refused
    // with `forbidden: requires role ...`); a handler that starts, pauses or restarts the resource,
    // queues a restart or emits an event; an audit record that carries details.
    let mut host = FakeHost::running();
    host.requires_auth = true;
    host.control_token = Some("admin-token");
    host.pairing = Some(("viewer-token", AccessRole::Viewer));
    let response = dispatch_value(
        json!({"id": 1, "type": "capabilities", "auth": "viewer-token"}),
        &host,
        NETWORK,
    );
    assert!(
        response.ok,
        "a viewer reads the descriptor: {:?}",
        response.error
    );
    let result = response.result.expect("capabilities result");
    assert_eq!(result["bytecode_versions"], json!(["2.0", "3.0"]));
    assert_eq!(result["last_reset"], json!(null));
    assert!(
        result.get("access").is_none(),
        "only status carries the access block: {result}"
    );
    assert!(host.actions().is_empty(), "nothing started or paused");
    assert!(host.pending_restart.borrow().is_none(), "no restart queued");
    assert!(host.events.borrow().is_empty(), "no event emitted");
    let audits = host.audits();
    assert_eq!(audits.len(), 1, "one audit record: {audits:?}");
    assert_eq!(audits[0].request_type, "capabilities");
    assert!(audits[0].ok);
    assert!(audits[0].auth_present);
    assert!(audits[0].details.is_none(), "no operation details");

    let unauthenticated = FakeHost::running();
    let response = dispatch_value(
        json!({"id": 2, "type": "capabilities"}),
        &unauthenticated,
        NETWORK,
    );
    assert!(
        response.ok,
        "an unauthenticated network client resolves to a viewer and reads it: {:?}",
        response.error
    );
}
