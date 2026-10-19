//! The C6's loader builds the candidate of an online change (specification 34 §11.3, UPDATE-06 and
//! Appendix D.2) on its own program memory, over fake hardware: the pins are claimed through the C6's
//! pin checks, and the checkpoint slots are the real flash owner's. The loader is called as the
//! resource loop calls it, `load` and then `prepare_change` and `commit_change`. Every expected value
//! is worked out here from the specification and `device_limits` at its 72 KiB bring-up heap (512 KiB
//! of preparation bytes, 48 KiB of validation scratch, 4,096 constructed values, 128 KiB of
//! construction bytes, 64 bytes of process image and 4 call levels), which scales with the heap.

#[path = "online_change/support.rs"]
mod support;

use std::cell::Cell;

use smol_str::SmolStr;
use support::{booted, configuration, gpio, Dog, Hardware, TestClock, PROBE, PROGRAM};
use trust_platform_esp32::loader::{Esp32Loader, ONLINE_CHANGE_WORKING_MEMORY_BYTES};
use trust_platform_esp32::process_io::{GpioPin, PinClaims};
use trust_platform_esp32::profile::runtime_id;
use trust_platform_esp32::Platform;
use trust_runtime_core::deployment::ControlMode;
use trust_runtime_core::retain::RestartMode;
use trust_runtime_core::watchdog::RetainMode;
use trust_runtime_services::capabilities::{LastReset, ResetCauseName};
use trust_runtime_services::chip::platform::status_settings;
use trust_runtime_services::runner::engine::DeviceMemory;
use trust_runtime_services::runner::{EngineLiveValues, Loaded, ProgramLoader};

thread_local! {
    /// The bytes of heap that are free now, which the test sets.
    static ROOM: Cell<usize> = const { Cell::new(1 << 20) };
}

fn room() -> usize {
    ROOM.with(Cell::get)
}

/// The device's memory: a 64 KiB region and a heap of 288 KiB, which the candidate's limits must not
/// follow: they follow the heap's room now, less the reserve.
const MEMORY: DeviceMemory = DeviceMemory {
    region_bytes: 64 * 1024,
    heap_bytes: 288 * 1024,
};

/// Whether a line holds `GPIO9`, `GPIO18` and `GPIO19` now.
fn pins(claims: &PinClaims) -> [bool; 3] {
    ["GPIO9", "GPIO18", "GPIO19"]
        .map(|name| claims.holds(GpioPin::parse(name).expect("a pin name")))
}

fn platform() -> Platform<Dog> {
    Platform::new(
        Dog::default(),
        SmolStr::new_static("Controller"),
        status_settings(),
        false,
        ControlMode::Production,
        EngineLiveValues::empty(),
        LastReset {
            cause: ResetCauseName::PowerOn,
            text: SmolStr::default(),
            boot_count: 1,
        },
        runtime_id([0x20, 0x6E, 0xF1, 0x17, 0x04, 0xC0]),
    )
}

#[test]
fn the_c6_builds_a_candidate_within_the_heaps_room_and_hands_over_its_lines() {
    // Specification 34 §11.3 and Appendix D.2: the candidate is prepared with the limits of the heap's
    // room now, less the reserve (the working memory, 8 KiB, for a record that keeps no retained
    // values), not with those of the heap's capacity, and it claims no pin; the commit hands the
    // running program's pins to it, so they are held all the time, by the running program, then by the
    // candidate, and are freed only when the candidate ends. The A4 program runs with GPIO9 as an input
    // and GPIO18 as an output. With the room at the reserve plus 72 KiB the board probe's candidate has
    // the bring-up limits (512 KiB of preparation bytes, 128 KiB of construction bytes, 64 bytes of
    // process image, 4 call levels, an artifact of 64 KiB, the region), although the device's heap is
    // 288 KiB; with the room at the reserve plus 144 KiB it has twice the memory limits and 8 call
    // levels. With a room below the reserve it is refused with the C6's numbers. Wrong behaviour
    // caught: limits taken from the heap's capacity; a reserve that is not subtracted; a candidate that
    // claims a pin ("already claimed"); lines released with the running program, or never released; a
    // refusal that disturbs the running program.
    let (hardware, reader) = Hardware::with_reader();
    let claims: PinClaims = hardware.claims.clone();
    let mut loader = Esp32Loader::new(hardware, TestClock, MEMORY, room, reader);
    let mut platform = platform();
    let record = configuration(RetainMode::None, vec![gpio(&["GPIO9"], &["GPIO18"])]);
    let Loaded {
        cycle: mut running, ..
    } = loader
        .load(&mut platform, booted(PROGRAM, record), RestartMode::Cold)
        .expect("the A4 program loads");
    assert_eq!(
        pins(&claims),
        [true, true, false],
        "the program holds its pins"
    );

    ROOM.with(|room| room.set(ONLINE_CHANGE_WORKING_MEMORY_BYTES + 72 * 1024));
    let at_72 = loader
        .prepare_change(&mut platform, &mut running, PROBE.to_vec())
        .expect("the probe is prepared within 72 KiB");
    let limits = at_72.cycle.limits();
    assert_eq!(limits.max_artifact_bytes, 64 * 1024);
    assert_eq!(limits.max_preparation_bytes, 512 * 1024);
    assert_eq!(limits.validation.max_scratch_bytes, 48 * 1024);
    assert_eq!(limits.max_construction_values, 4096);
    assert_eq!(limits.max_construction_bytes, 128 * 1024);
    assert_eq!(limits.max_process_image_bytes, 64);
    assert_eq!(limits.max_call_depth, 4);
    assert_eq!(
        pins(&claims),
        [true, true, false],
        "preparing the candidate claims no pin and frees none"
    );
    drop(at_72);
    assert_eq!(
        pins(&claims),
        [true, true, false],
        "a dropped candidate frees nothing"
    );

    ROOM.with(|room| room.set(ONLINE_CHANGE_WORKING_MEMORY_BYTES + 144 * 1024));
    let at_144 = loader
        .prepare_change(&mut platform, &mut running, PROBE.to_vec())
        .expect("the probe is prepared within 144 KiB");
    let limits = at_144.cycle.limits();
    assert_eq!(
        limits.max_artifact_bytes,
        64 * 1024,
        "the region still bounds it"
    );
    assert_eq!(limits.max_preparation_bytes, 1024 * 1024);
    assert_eq!(limits.validation.max_scratch_bytes, 96 * 1024);
    assert_eq!(limits.max_construction_values, 8192);
    assert_eq!(limits.max_construction_bytes, 256 * 1024);
    assert_eq!(limits.max_process_image_bytes, 128);
    assert_eq!(limits.max_call_depth, 8);
    drop(at_144);

    ROOM.with(|room| room.set(ONLINE_CHANGE_WORKING_MEMORY_BYTES - 1));
    let refused = loader
        .prepare_change(&mut platform, &mut running, PROBE.to_vec())
        .err()
        .expect("a room below the reserve leaves nothing");
    assert_eq!(PROBE.len(), 3_388, "the board probe");
    assert_eq!(
        refused,
        format!(
            "insufficient memory: a change of 3388 bytes needs more than the 0 bytes of heap that \
             remain for it ({} bytes are free, {} are kept in reserve)",
            ONLINE_CHANGE_WORKING_MEMORY_BYTES - 1,
            ONLINE_CHANGE_WORKING_MEMORY_BYTES
        )
    );
    assert_eq!(
        pins(&claims),
        [true, true, false],
        "the refusal disturbs nothing"
    );

    ROOM.with(|room| room.set(ONLINE_CHANGE_WORKING_MEMORY_BYTES + 72 * 1024));
    let mut candidate = loader
        .prepare_change(&mut platform, &mut running, PROBE.to_vec())
        .expect("the probe is prepared");
    loader.commit_change(&mut platform, &mut running, &mut candidate);
    assert_eq!(
        pins(&claims),
        [true, true, false],
        "the commit claims no pin again and releases none"
    );
    drop(running);
    assert_eq!(
        pins(&claims),
        [true, true, false],
        "the running program ends without its pins: the candidate holds them"
    );
    drop(candidate);
    assert_eq!(
        pins(&claims),
        [false, false, false],
        "the pins are free when the program that holds them ends"
    );
}
