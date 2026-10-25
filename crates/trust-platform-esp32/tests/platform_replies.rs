//! The answers the ESP32-C6's platform gives its `capabilities`, `status`, `health` and `io.*` operations
//! (specification 11), now that the platform is the shared chip platform (specification 34, ARCH-11).
//! The bytes must stay the same. Each expected value below comes from the specification, from the C6's
//! chip file (`chips/esp32c6.toml`), from the adapter's existing tests (`chip_file.rs`), or from the
//! definition of the reply, worked out by hand. None is read back from the platform. `status` also reads
//! the runner's records (metrics and events), which the platform does not own, so its platform-owned
//! fields are pinned through `StatusPort` here; `health` reads only `StatusPort` and the resource, and
//! is pinned as a whole reply.

use std::cell::RefCell;
use std::rc::Rc;

use serde_json::json;
use smol_str::SmolStr;
use trust_platform_esp32::profile::runtime_id;
use trust_platform_esp32::Platform;
use trust_runtime_core::deployment::DeviceDescriptor;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::ports::MonotonicInstant;
use trust_runtime_core::retain::RestartMode;
use trust_runtime_core::scheduler::ResourceState;
use trust_runtime_services::capabilities::{self, LastReset, ResetCauseName};
use trust_runtime_services::chip::network::{ChipNetwork, DriverMemory, NetworkFacts};
use trust_runtime_services::chip::platform::status_settings;
use trust_runtime_services::live_values::io_requests::{io_list, io_read, IoQueuePort};
use trust_runtime_services::ports::{ProgramControl, ResourcePort, StatusPort};
use trust_runtime_services::resource::commands::CommandSendError;
use trust_runtime_services::runner::DeviceLiveValues;
use trust_runtime_services::status::{health, IoDriverStatus, StatusSettings};

/// The C6's platform after a power-on boot with boot counter 1, built the way the firmware builds it:
/// the resource `Controller`, a chip's settings, production control, no debug surface, the live values
/// of no program, and the runtime ID of the owned board's base MAC address.
fn platform() -> Platform<()> {
    Platform::new(
        (),
        SmolStr::new_static("Controller"),
        status_settings(),
        DeviceLiveValues::empty(),
        LastReset {
            cause: ResetCauseName::PowerOn,
            text: SmolStr::default(),
            boot_count: 1,
        },
        runtime_id([0x20, 0x6E, 0xF1, 0x17, 0x04, 0xC0]),
    )
}

/// The `capabilities` reply of the C6 is its descriptor and its reset, with the program format 44.1, the
/// compiled program image (specification 44 §10.1), the cooperative resource profile, numeric contract 1, unqualified
/// timing and the wall clock (specification 34 TARGET-06, D.2). Its maxima are the 64 MiB artifact
/// and 1,024 call levels, and every other limit is `null`. Its event store holds 200 events by default and
/// at most 200, and its application slot is one program slot, 2,064 KiB = 2,113,536 bytes
/// (`chips/esp32c6.toml`, `[events]` and `[layout] application_a`; specification 34 §5.1.1: an install
/// writes one of the two slots). Its memory for programs is the number the firmware measured at boot
/// (specification 34 MEM-12). A platform that the boot gave no firmware facts and no network reports
/// both as null (TARGET-04, ARCH-12). Catches a field renamed, a maximum written as a number instead of
/// `null`, a slot or event store of another chip, a slot that is the whole 4,128 KiB region, the wall clock left
/// out, so that a program that calls `CURRENT_DT` is refused at install, a program memory
/// that the reply leaves out or does not take from the platform, and a reset that is not the power-on
/// one.
#[test]
fn the_c6_capabilities_reply_is_the_bytes_the_chip_gives_today() {
    // The firmware measures the memory the C6 has for programs at boot, before it serves the link
    // (specification 34 MEM-12): here 98,304 bytes.
    let mut platform = platform();
    platform.set_program_memory(98_304);
    let reply = capabilities::capabilities(7, &platform);
    assert_eq!(
        serde_json::to_value(&reply).expect("a reply serializes"),
        json!({
            "id": 7,
            "ok": true,
            "result": {
                "program_versions": ["44.1"],
                "scheduling_profiles": ["cooperative-resource-v1"],
                "numeric_contracts": ["1"],
                "timing_evidence": "unqualified",
                "capabilities": ["wall_clock"],
                "maxima": {
                    "max_artifact_bytes": 67_108_864,
                    "max_preparation_bytes": null,
                    "max_preparation_work": null,
                    "validation.max_scratch_bytes": null,
                    "validation.max_work": null,
                    "max_construction_values": null,
                    "max_construction_bytes": null,
                    "max_process_image_bytes": null,
                    "max_work": null,
                    "max_construction_work": null,
                    "max_call_depth": 1_024,
                },
                "event_store_default": 200,
                "event_store_max": 200,
                "application_slot_bytes": 2_113_536,
                "program_memory_bytes": 98_304,
                "last_reset": { "cause": "power_on", "text": "", "boot_count": 1 },
                "firmware": null,
                "network": null,
            },
        })
    );
}

/// A network that joined reports its interface, its link, its address and its control port, and one
/// that did not start reports why (specification 34 ARCH-12; specification 11, the runtime device
/// capabilities contract). Catches a chip whose address cannot be read over its USB link, and a radio
/// that failed silently.
#[test]
fn the_c6_capabilities_report_its_network() {
    struct Joined;
    impl ChipNetwork for Joined {
        fn poll(&mut self) {}
        fn poll_due(&mut self) -> Option<MonotonicInstant> {
            None
        }
        fn facts(&self) -> NetworkFacts {
            NetworkFacts {
                interface: "wifi",
                link_up: true,
                address: Some("192.168.4.23/24".to_string()),
                control_port: Some(9901),
                error: None,
                driver_memory: Some(DriverMemory {
                    bytes: 65_536,
                    peak_bytes: 51_200,
                    failures: 0,
                }),
                time: None,
            }
        }
    }
    let network: Rc<RefCell<dyn ChipNetwork>> = Rc::new(RefCell::new(Joined));
    let joined = platform().with_network(network);
    let reply = serde_json::to_value(capabilities::capabilities(8, &joined)).expect("a reply");
    assert_eq!(
        reply["result"]["network"],
        json!({"interface": "wifi", "link": "up", "address": "192.168.4.23/24",
            "control_port": 9901, "error": null,
            "driver_memory": {"bytes": 65_536, "peak_bytes": 51_200, "failures": 0},
            "time": null})
    );
    let mut failed = platform();
    failed.set_network_failure(NetworkFacts {
        interface: "wifi",
        link_up: false,
        address: None,
        control_port: None,
        error: Some("the radio did not start".to_string()),
        driver_memory: None,
        time: None,
    });
    let reply = serde_json::to_value(capabilities::capabilities(9, &failed)).expect("a reply");
    assert_eq!(
        reply["result"]["network"],
        json!({"interface": "wifi", "link": "down", "address": null, "control_port": null,
            "error": "the radio did not start", "driver_memory": null, "time": null})
    );
}

/// The platform answers `status` with its resource, its chip name (`esp32c6`), its runtime ID (the chip
/// name and the USB serial number), its control after `shutdown`, a chip's settings (compiled programs,
/// specification 44 NX-15), an empty list of its
/// own I/O drivers (the installed program's drivers come from the cycle), and its reset (specification
/// 11, `status`; specification 34, STATE-08). Catches a platform name or runtime ID taken from another
/// source, a chip that gives up control, and settings or a driver list that differ from the C6's.
#[test]
fn the_c6_status_answers_name_the_chip_and_keep_its_control() {
    let platform = platform();
    assert_eq!(StatusPort::resource_name(&platform), "Controller");
    assert_eq!(StatusPort::platform(&platform), "esp32c6");
    assert_eq!(
        StatusPort::runtime_id(&platform),
        "esp32c6-20:6E:F1:17:04:C0"
    );
    assert_eq!(
        StatusPort::program_control(&platform),
        ProgramControl::KeepsControl
    );
    let settings = StatusPort::status_settings(&platform).expect("the C6 gives its settings");
    assert_eq!(settings.execution_backend, "native");
    assert_eq!(settings.execution_backend_source, "default");
    assert!(!settings.simulation_enabled);
    assert_eq!(settings.simulation_time_scale, 1);
    assert_eq!(settings.simulation_mode, "production");
    assert_eq!(settings.simulation_warning, "");
    let drivers = StatusPort::io_drivers(&platform).expect("the C6 lists its drivers");
    assert!(drivers.is_empty(), "the platform runs no driver of its own");
    assert_eq!(StatusPort::realtime_has_errors(&platform), Some(false));
    assert_eq!(
        StatusPort::last_reset(&platform),
        Some(LastReset {
            cause: ResetCauseName::PowerOn,
            text: SmolStr::default(),
            boot_count: 1,
        })
    );
}

/// The realtime block of `status` is the disabled profile with its requested scheduler, and the OpenOT
/// persistence block is the disabled one with no backend and nothing read or committed (specification 11,
/// `status`). A chip has neither, so neither reports an enabled state. Catches a block that reports a
/// profile or a backend, and a count that is not zero.
#[test]
fn the_c6_realtime_and_openot_blocks_are_the_disabled_ones() {
    let platform = platform();
    assert_eq!(
        StatusPort::realtime_status(&platform),
        Some(json!({
            "profile": "disabled",
            "enabled": false,
            "strict": false,
            "require_preempt_rt_kernel": false,
            "requested": {
                "lock_memory": false,
                "scheduler": "other",
                "priority": 70,
                "cpu_affinity": [],
            },
            "observed": {
                "kernel_realtime": null,
                "scheduler": null,
                "priority": null,
                "cpu_affinity": [],
                "memory_locked_kb": null,
            },
            "runtime_applied": {
                "memory_lock": false,
                "affinity": false,
                "scheduler": false,
            },
            "active": false,
            "warnings": [],
            "errors": [],
        }))
    );
    assert_eq!(
        StatusPort::openot_persistence(&platform),
        Some(json!({
            "state": "disabled",
            "backend": null,
            "schema_generation": null,
            "documents_read": 0,
            "documents_committed": 0,
            "documents_duplicated": 0,
            "remote_pending": 0,
            "projection_rows_committed": 0,
            "unclassified_event_count": 0,
            "reconciled_part_count": 0,
            "pending_part_count": 0,
            "documents_retried": 0,
            "pending": 0,
            "rejected": 0,
            "unresolved": 0,
            "loss_range_count": 0,
            "lost_record_count": 0,
            "cursor_abs": 0,
            "head_abs": 0,
            "cursor_lag": 0,
            "last_success_time_ns": null,
            "last_error": null,
            "warnings": [],
        }))
    );
}

/// Before a program runs no I/O snapshot is published, so `io.read` answers a null snapshot and `io.list`
/// answers that none is available (specification 11, "Live Values"). Catches a snapshot the chip makes up,
/// and an `io.list` that reports success.
#[test]
fn the_c6_io_replies_have_no_snapshot_before_a_program_runs() {
    let platform = platform();
    assert_eq!(
        serde_json::to_value(io_read(3, platform.io_snapshot())).expect("a reply serializes"),
        json!({ "id": 3, "ok": true, "result": { "snapshot": null } })
    );
    assert_eq!(
        serde_json::to_value(io_list(4, platform.io_snapshot())).expect("a reply serializes"),
        json!({ "id": 4, "ok": false, "error": "no snapshot available" })
    );
}

/// The resource of the C6 as `health` sees it: the platform answers every `StatusPort` question, and the
/// resource runs with no fault, no hold and no program problem. `health` reads no other port.
struct RunningResource<'a>(&'a Platform<()>);

impl StatusPort for RunningResource<'_> {
    fn resource_name(&self) -> &str {
        StatusPort::resource_name(self.0)
    }

    fn platform(&self) -> &str {
        StatusPort::platform(self.0)
    }

    fn runtime_id(&self) -> &str {
        StatusPort::runtime_id(self.0)
    }

    fn program_control(&self) -> ProgramControl {
        StatusPort::program_control(self.0)
    }

    fn status_settings(&self) -> Option<StatusSettings> {
        StatusPort::status_settings(self.0)
    }

    fn io_drivers(&self) -> Option<Vec<IoDriverStatus>> {
        StatusPort::io_drivers(self.0)
    }

    fn realtime_status(&self) -> Option<serde_json::Value> {
        StatusPort::realtime_status(self.0)
    }

    fn realtime_has_errors(&self) -> Option<bool> {
        StatusPort::realtime_has_errors(self.0)
    }

    fn openot_persistence(&self) -> Option<serde_json::Value> {
        StatusPort::openot_persistence(self.0)
    }

    fn device_descriptor(&self) -> DeviceDescriptor {
        StatusPort::device_descriptor(self.0)
    }

    fn last_reset(&self) -> Option<LastReset> {
        StatusPort::last_reset(self.0)
    }
}

impl ResourcePort for RunningResource<'_> {
    fn resource_state(&self) -> ResourceState {
        ResourceState::Running
    }

    fn last_error(&self) -> Option<RuntimeError> {
        None
    }

    fn is_hung(&self) -> bool {
        false
    }

    fn pause_resource(&self) -> Result<(), CommandSendError> {
        Ok(())
    }

    fn resume_resource(&self) -> Result<(), CommandSendError> {
        Ok(())
    }

    fn debug_control_mode(&self) -> bool {
        false
    }

    fn debugger_pause(&self) {}

    fn debugger_continue(&self) {}

    fn update_pending_restart(&self, _update: impl FnOnce(&mut Option<RestartMode>)) {}

    fn starts_when_stopped(&self) -> bool {
        true
    }

    fn missing_program(&self) -> Option<&str> {
        None
    }
}

/// `health` answers `ok` for a running resource with no fault, no faulted driver and no realtime error:
/// its state label is `running`, its fault is `null` and its driver list is empty (specification 11,
/// `health`). Catches a platform that reports a realtime error or a driver fault of its own, and a reply
/// that names another state or invents a fault.
#[test]
fn the_c6_health_answer_is_ok_for_a_running_resource_with_no_fault() {
    let platform = platform();
    let host = RunningResource(&platform);
    assert_eq!(
        serde_json::to_value(health(5, &host)).expect("a reply serializes"),
        json!({
            "id": 5,
            "ok": true,
            "result": { "ok": true, "state": "running", "fault": null, "io_drivers": [] },
        })
    );
}
