//! The state of the candidate of an online change on the C6 (specification 34 §11.3 and UPDATE-04;
//! specification 11 §6.7.1), built by the C6's loader over fake hardware as the resource loop builds
//! it: `load`, then `prepare_change` and `commit_change`. The program is the corpus fixture
//! `retained_blocks` (`crates/trust-runtime/tests/fixtures/bounded-c/retained_blocks`): a 10 ms task
//! that calls the function-block instance `kept` with `delta := DINT#1`, keeps a reference `follow` to
//! `kept.count`, and counts `total` and the program variable `runs` by one, all retained. Its scans at
//! 10, 20 and 30 ms leave `count`, `total` and `runs` at 3. Every expected value is worked out from that.

#[path = "online_change/support.rs"]
mod support;

use smol_str::SmolStr;
use support::{booted, configuration, Dog, Hardware, TestClock};
use trust_platform_esp32::loader::Esp32Loader;
use trust_platform_esp32::profile::{runtime_id, MAIN_STACK};
use trust_platform_esp32::Platform;
use trust_runtime_core::bytecode::{BytecodeModule, SectionData, SectionId};
use trust_runtime_core::deployment::ControlMode;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::memory::{Generation, VariableStorage};
use trust_runtime_core::retain::RestartMode;
use trust_runtime_core::value::{Duration, Value};
use trust_runtime_core::vm::{CarriedValueEvent, CycleHooks};
use trust_runtime_core::watchdog::RetainMode;
use trust_runtime_services::capabilities::{LastReset, ResetCauseName};
use trust_runtime_services::chip::platform::status_settings;
use trust_runtime_services::live_values::var_requests::VariableQueuePort;
use trust_runtime_services::runner::engine::DeviceMemory;
use trust_runtime_services::runner::{EngineLiveValues, Loaded, ProgramLoader, ResourceCycle};

const BLOCKS: &[u8] =
    include_bytes!("../../trust-runtime/tests/fixtures/bounded-c/retained_blocks/program.stbc");

const MEMORY: DeviceMemory = DeviceMemory {
    region_bytes: 64 * 1024,
    heap_bytes: 136 * 1024,
    call_depth: MAIN_STACK.call_depth(),
};

/// A heap with room for any change of these tests.
fn room() -> usize {
    1 << 20
}

/// The hooks of a scan that observe nothing.
struct NoHooks;

impl CycleHooks for NoHooks {}

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

/// The program with the argument of its call of `kept` changed from DINT#1 to DINT#2: the same
/// declarations, types and identities with another executable body, which is congruent.
fn blocks_that_count_by_two() -> Vec<u8> {
    let mut module = BytecodeModule::decode(BLOCKS).expect("the fixture decodes");
    let Some(SectionData::ConstPool(constants)) = module.section_mut(SectionId::ConstPool) else {
        panic!("constants")
    };
    assert_eq!(
        constants.entries[0].payload,
        1_i32.to_le_bytes(),
        "the first constant is the argument DINT#1 of the call of kept"
    );
    constants.entries[0].payload = 2_i32.to_le_bytes().to_vec();
    module.encode().expect("the program encodes")
}

/// The program with the global `total` renamed `sum`: another declaration, so it is not congruent.
fn blocks_with_another_name() -> Vec<u8> {
    let mut module = BytecodeModule::decode(BLOCKS).expect("the fixture decodes");
    let Some(SectionData::StringTable(strings)) = module.section_mut(SectionId::StringTable) else {
        panic!("strings")
    };
    let entry = strings
        .entries
        .iter_mut()
        .find(|entry| entry.as_str() == "total")
        .expect("the program names total");
    *entry = SmolStr::new_static("sum");
    module.encode().expect("the program encodes")
}

fn scan(cycle: &mut impl ResourceCycle, at_ms: i64) {
    cycle
        .run_scan(Duration::from_millis(at_ms), None, &mut NoHooks)
        .expect("the scan runs");
}

/// The `count` of the function-block instance `kept`.
fn count(storage: &VariableStorage) -> Option<Value> {
    let Some(Value::Instance(kept)) = storage.get_global("kept") else {
        return None;
    };
    storage.get_instance_var(*kept, "count").cloned()
}

/// What the reference `follow` reads.
fn followed(storage: &VariableStorage) -> Option<Value> {
    match storage.get_global("follow") {
        Some(Value::Reference(Some(reference))) => storage.read_by_ref_ref(reference).cloned(),
        _ => None,
    }
}

#[test]
fn an_online_change_keeps_a_retained_function_block_instance_of_a_congruent_candidate() {
    // Specification 34 §11.3 and specification 11 §6.7.1: the chip builds the candidate through the
    // engine's replacement, as a computer does. A candidate that declares the same retained storage
    // keeps the retained function-block instance and the retained reference live. The candidate calls
    // `kept` with `delta := DINT#2`: the same declarations with another body, which is congruent.
    // The program has run three scans, so `count` is 3. The candidate shows the live instance, with
    // `count` 3 (an instance built anew counts 0) and the reference reading it, and `total` and `runs`
    // 3. It continues the lineage (generation 2) and keeps the logical time, and it carries the task's
    // schedule (decision OC-2): a scan at 35 ms ran nothing, since the task last ran at 30 ms, and the
    // change at 35 ms leaves the task due at 40 ms. Its scan at 40 ms runs the new body after the old
    // program is gone: `count` 3 + 2 = 5, `total` 4, and `follow` reads 5. Wrong behaviour caught: a
    // retained instance or reference that starts from its declared initial value, as a snapshot would
    // give it (count 0, follow null); a candidate that keeps the old body; a candidate that borrows the
    // old program's memory, which is freed before the scan; a schedule that starts again at the
    // change, which makes the task wait until 45 ms (`count` still 3 at 40 ms): the period of every
    // output would stretch at each change.
    let hardware = Hardware::erased();
    let mut loader = Esp32Loader::new(hardware, TestClock, MEMORY, room);
    let mut platform = platform();
    let Loaded {
        cycle: mut running, ..
    } = loader
        .load(
            &mut platform,
            booted(BLOCKS, configuration(RetainMode::None, Vec::new())),
            RestartMode::Cold,
        )
        .expect("the program loads");
    for at in [10, 20, 30, 35] {
        scan(&mut running, at);
    }
    assert_eq!(
        count(running.storage()),
        Some(Value::DInt(3)),
        "35 ms ran nothing"
    );
    assert_eq!(followed(running.storage()), Some(Value::DInt(3)));

    let mut candidate = loader
        .prepare_change(&mut platform, &mut running, blocks_that_count_by_two())
        .expect("a congruent candidate is prepared");
    loader.commit_change(&mut platform, &mut running, &mut candidate);
    drop(running);

    let mut cycle = candidate.cycle;
    assert!(candidate.events.is_empty());
    assert_eq!(cycle.generation(), Generation::FIRST.next().unwrap());
    assert_eq!(
        count(cycle.storage()),
        Some(Value::DInt(3)),
        "the live instance"
    );
    assert_eq!(
        followed(cycle.storage()),
        Some(Value::DInt(3)),
        "the live reference"
    );
    assert_eq!(cycle.storage().get_global("total"), Some(&Value::DInt(3)));
    scan(&mut cycle, 40);
    assert_eq!(
        count(cycle.storage()),
        Some(Value::DInt(5)),
        "the new body, on the carried schedule"
    );
    assert_eq!(followed(cycle.storage()), Some(Value::DInt(5)));
    assert_eq!(cycle.storage().get_global("total"), Some(&Value::DInt(4)));
}

#[test]
fn an_identity_taken_before_an_online_change_is_stale_after_it() {
    // Specification 34 §11.3, UPDATE-04 and §4.3.1: the change installs the next generation of the
    // running program's lineage. A reference to a global and a reference to a member of the instance
    // `kept`, taken in the first generation, are refused with `ReferenceLifetime` after the change,
    // while references taken after it read the carried values (count 2, total 2). The candidate's Live
    // Values carry the second generation, so a force that names an instance by its identity after the
    // change is resolved in it. Wrong behaviour caught: a candidate that starts the first generation
    // again (the stale references would read the new state, and could name an unrelated instance), and
    // Live Values that stay in the first generation (every instance force would be refused).
    let hardware = Hardware::erased();
    let mut loader = Esp32Loader::new(hardware, TestClock, MEMORY, room);
    let mut platform = platform();
    let Loaded {
        cycle: mut running, ..
    } = loader
        .load(
            &mut platform,
            booted(BLOCKS, configuration(RetainMode::None, Vec::new())),
            RestartMode::Cold,
        )
        .expect("the program loads");
    for at in [10, 20] {
        scan(&mut running, at);
    }
    let Some(Value::Instance(kept)) = running.storage().get_global("kept") else {
        panic!("kept is an instance");
    };
    let stale_member = running
        .storage()
        .ref_for_instance(*kept, "count")
        .expect("the instance has a count");
    let stale_global = running
        .storage()
        .ref_for_global("total")
        .expect("the program declares total");
    assert_eq!(running.generation(), Generation::FIRST);

    let mut candidate = loader
        .prepare_change(&mut platform, &mut running, BLOCKS.to_vec())
        .expect("the candidate is prepared");
    loader.commit_change(&mut platform, &mut running, &mut candidate);
    drop(running);

    let second = Generation::FIRST.next().unwrap();
    let live = candidate.live.clone();
    let mut cycle = candidate.cycle;
    assert_eq!(cycle.generation(), second);
    assert_eq!(live.generation(), second, "the Live Values");
    for stale in [&stale_member, &stale_global] {
        assert_eq!(
            cycle.read_reference(stale),
            Err(RuntimeError::ReferenceLifetime),
            "a reference of the first generation"
        );
    }
    let Some(Value::Instance(kept)) = cycle.storage().get_global("kept") else {
        panic!("kept is an instance");
    };
    let fresh_member = cycle
        .storage()
        .ref_for_instance(*kept, "count")
        .expect("the candidate's instance has a count");
    let fresh_global = cycle
        .storage()
        .ref_for_global("total")
        .expect("the candidate declares total");
    assert_eq!(fresh_member.generation, second);
    assert_eq!(cycle.read_reference(&fresh_member), Ok(Value::DInt(2)));
    assert_eq!(cycle.read_reference(&fresh_global), Ok(Value::DInt(2)));
}

#[test]
fn an_online_change_keeps_the_values_of_a_program_that_renames_a_retained_global() {
    // Specification 11 §6.7.1 and specification 34 §11.3 (decision OC-1 and "keep what fits"): the
    // chip carries values as a computer does, and no value refuses a change. The candidate renames the
    // retained global `total` to `sum`. The program has run two scans: count, total and runs 2, and
    // `follow` reads count. The candidate keeps the instance `kept` at its place (count 2), names
    // `kept.count` with `follow` (it reads 2), keeps `runs` 2 and starts `sum` at its initial 0, which
    // the change records as new; it continues the lineage (generation 2). Its scan at 30 ms counts on:
    // count 3, `sum` 1. Wrong behaviour caught: a chip that refuses the change because a retained
    // instance or reference cannot be put into a snapshot (the rule before checklist C1-24), and a
    // reference that still names the old program's storage.
    let hardware = Hardware::erased();
    let mut loader = Esp32Loader::new(hardware, TestClock, MEMORY, room);
    let mut platform = platform();
    let Loaded {
        cycle: mut running, ..
    } = loader
        .load(
            &mut platform,
            booted(BLOCKS, configuration(RetainMode::None, Vec::new())),
            RestartMode::Cold,
        )
        .expect("the program loads");
    for at in [10, 20] {
        scan(&mut running, at);
    }

    let mut candidate = loader
        .prepare_change(&mut platform, &mut running, blocks_with_another_name())
        .expect("a value refuses no change");
    loader.commit_change(&mut platform, &mut running, &mut candidate);
    drop(running);

    assert_eq!(
        candidate.events,
        [CarriedValueEvent::New { name: "sum".into() }]
    );
    let mut cycle = candidate.cycle;
    assert_eq!(cycle.generation(), Generation::FIRST.next().unwrap());
    assert_eq!(count(cycle.storage()), Some(Value::DInt(2)), "the instance");
    assert_eq!(
        followed(cycle.storage()),
        Some(Value::DInt(2)),
        "the reference"
    );
    assert_eq!(cycle.storage().get_global("sum"), Some(&Value::DInt(0)));
    assert_eq!(cycle.storage().get_global("total"), None);
    scan(&mut cycle, 30);
    assert_eq!(count(cycle.storage()), Some(Value::DInt(3)));
    assert_eq!(cycle.storage().get_global("sum"), Some(&Value::DInt(1)));
}
