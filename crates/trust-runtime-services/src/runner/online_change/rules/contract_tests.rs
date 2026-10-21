//! The rules of an online change (specification 34 §11.3; UPDATE-06): the memory a candidate may use
//! and the refusal when it needs more, and the match of the candidate's bindings with the lines of the
//! running program's record. Every expected value is worked out here from the specification and
//! `device_limits` at its 72 KiB bring-up heap (512 KiB of preparation bytes and 48 KiB of validation
//! scratch, which scale with the heap; trust-runtime's constructed values, construction bytes and
//! process image; and the call levels of the device's main stack, 7 here), not taken from the code
//! under test.

use alloc::format;
use alloc::string::ToString;
use alloc::vec::Vec;

use smol_str::SmolStr;
use trust_runtime_core::bytecode::{BytecodeError, BytecodeModule, ValidationLimits};
use trust_runtime_core::deployment::{GpioInput, GpioOutput};
use trust_runtime_core::error::{ExecutionLimitKind, RuntimeError};
use trust_runtime_core::io_address::IoAddress;
use trust_runtime_core::io_image::PreparedIoBinding;
use trust_runtime_core::vm::{PreparationLimits, PreparedModule, VmTrap};

use super::{
    address_text, covers, is_call_depth_limit, is_memory_limit, prepare_candidate, unwired_binding,
    ChangeMemory,
};
use crate::runner::engine::{device_limits, DeviceMemory};
use crate::tests::program_store::PROGRAM;
use crate::tests::runner_ports::{CHIP_MEMORY, GPIO};

/// The OSCAT core artifact, 141,320 bytes: longer than any heap of a chip.
const OSCAT: &[u8] =
    include_bytes!("../../../../../trust-runtime/tests/fixtures/oscat/core/program.stbc");

fn address(text: &str) -> IoAddress {
    IoAddress::parse(text).expect("an address")
}

fn input(text: &str) -> GpioInput {
    GpioInput {
        address: address(text),
        line: SmolStr::new_static("GPIO9"),
        invert: false,
        debounce_ms: 0,
    }
}

fn output(text: &str) -> GpioOutput {
    GpioOutput {
        address: address(text),
        line: SmolStr::new_static("GPIO18"),
        invert: false,
        initial: false,
    }
}

/// The input and the output binding of the board probe, `%IX0.0` and `%QX0.0`.
fn probe_bindings() -> Vec<PreparedIoBinding> {
    PreparedModule::from_bytes(GPIO, device_limits(CHIP_MEMORY))
        .expect("the board probe is admitted")
        .io_bindings()
        .to_vec()
}

#[test]
fn a_candidates_limits_are_the_devices_limits_for_the_room_less_the_reserve() {
    // Specification 34 §11.3 (the memory check, UPDATE-06): the candidate's limits are what the heap's
    // room allows less the reserve. A room of 90,112 bytes (88 KiB) with a reserve of 16,384 (16 KiB)
    // leaves 73,728 bytes, 72 KiB, whose limits are the bring-up limits; a room of 53,248 (52 KiB)
    // with the same reserve leaves 36,864 (36 KiB), exactly half of them. The artifact is no longer
    // than the application region (16 KiB here), or than the heap that remains. The counts are
    // trust-runtime's and the call levels the stack's at any heap (owner, 20 and 21 October 2026).
    // Wrong behaviour caught: limits taken from the heap's capacity instead of its room; a reserve
    // that is not subtracted; a subtraction that leaves the limits unscaled; counts or call levels
    // that still scale with the heap (4,096 values and 4 levels at 72 KiB, half at 36 KiB).
    let memory = DeviceMemory {
        region_bytes: 16 * 1024,
        heap_bytes: 144 * 1024,
        call_depth: 7,
    };
    let reserve = 16 * 1024;
    let at_72 = ChangeMemory::for_change(6_412, 88 * 1024, reserve)
        .limits(memory)
        .expect("the limits of a 72 KiB heap");
    assert_eq!(at_72.max_artifact_bytes, 16 * 1024);
    assert_eq!(at_72.max_preparation_bytes, 512 * 1024);
    assert_eq!(at_72.validation.max_scratch_bytes, 48 * 1024);
    let host = PreparationLimits::default();
    assert_eq!(at_72.max_construction_values, host.max_construction_values);
    assert_eq!(at_72.max_construction_bytes, host.max_construction_bytes);
    assert_eq!(at_72.max_process_image_bytes, host.max_process_image_bytes);
    assert_eq!(at_72.max_call_depth, 7);

    let at_36 = ChangeMemory::for_change(6_412, 52 * 1024, reserve)
        .limits(memory)
        .expect("the limits of a 36 KiB heap");
    assert_eq!(at_36.max_artifact_bytes, 16 * 1024);
    assert_eq!(at_36.max_preparation_bytes, 256 * 1024);
    assert_eq!(at_36.validation.max_scratch_bytes, 24 * 1024);
    assert_eq!(at_36.max_construction_values, host.max_construction_values);
    assert_eq!(at_36.max_construction_bytes, host.max_construction_bytes);
    assert_eq!(at_36.max_process_image_bytes, host.max_process_image_bytes);
    assert_eq!(at_36.max_call_depth, 7);
}

#[test]
fn an_artifact_longer_than_the_heap_that_remains_is_over_the_limit() {
    // Specification 34 §11.3: the artifact may not be longer than the heap that remains, whatever the
    // region allows. A region of 1 MiB with 40,000 bytes available gives an artifact limit of 40,000.
    // Wrong behaviour caught: an artifact limit taken from the region alone, which would decode an
    // upload that the heap cannot hold.
    let memory = DeviceMemory {
        region_bytes: 1024 * 1024,
        heap_bytes: 144 * 1024,
        call_depth: 7,
    };
    let limits = ChangeMemory::for_change(1, 60_000, 20_000)
        .limits(memory)
        .expect("40,000 bytes allow limits");
    assert_eq!(limits.max_artifact_bytes, 40_000);
}

#[test]
fn a_heap_with_no_byte_left_is_refused_as_insufficient_memory() {
    // Specification 34 §11.3: the preparation bytes scale with the heap that remains, 512 KiB per
    // 72 KiB, so one byte gives 524,288 / 73,728 = 7 bytes and keeps the 7 call levels of the stack,
    // and no byte gives none: no program can be prepared in it, and the refusal names the numbers. A
    // room below the reserve leaves nothing. Wrong behaviour caught: call levels that still come from
    // the heap (one byte would allow none); limits with no preparation byte, which would fail with a
    // bare preparation error instead of the refusal; a room less than the reserve that underflows.
    let memory = CHIP_MEMORY;
    let reserve = 10_000;
    let allows = ChangeMemory::for_change(6_412, 1 + reserve, reserve)
        .limits(memory)
        .expect("one byte gives limits");
    assert_eq!(allows.max_preparation_bytes, 7);
    assert_eq!(allows.max_call_depth, 7);

    let refused = ChangeMemory::for_change(6_412, reserve, reserve).limits(memory);
    assert_eq!(
        refused.err().as_deref(),
        Some(
            "insufficient memory: a change of 6412 bytes needs more than the 0 bytes of heap \
             that remain for it (10000 bytes are free, 10000 are kept in reserve)"
        )
    );

    let below = ChangeMemory::for_change(6_412, 4_000, reserve).limits(memory);
    assert_eq!(
        below.err().as_deref(),
        Some(
            "insufficient memory: a change of 6412 bytes needs more than the 0 bytes of heap that \
             remain for it (4000 bytes are free, 10000 are kept in reserve)"
        )
    );
}

#[test]
fn a_program_the_memory_limits_refuse_is_refused_with_its_numbers() {
    // Specification 34 §11.3: a candidate that needs more than the limits is refused with
    // `insufficient memory` and its numbers. The preparation limit carries the program's length, the
    // heap that remains, the room and the reserve; a construction limit carries what the construction
    // needed after what it had used, and the limit. An allowance that is not memory (the work limit)
    // and any other error keep their own text, as a computer replies with the error of its
    // transaction. Wrong behaviour caught: a memory refusal worded as the error's own text, which names
    // no number; a work limit or another error worded as a memory shortage.
    let memory = ChangeMemory::for_install(9_000, 70_000, 30_000);
    assert_eq!(
        memory.refusal(&RuntimeError::PreparationLimit),
        "insufficient memory: a program of 9000 bytes needs more than the 40000 bytes of heap that \
         remain for it (70000 bytes are free, 30000 are kept in reserve)"
    );
    assert_eq!(
        memory.refusal(&RuntimeError::ExecutionLimit {
            kind: ExecutionLimitKind::AllocationBytes,
            used: 100,
            requested: 500,
            limit: 512,
        }),
        "insufficient memory: building a program of 9000 bytes needs 500 more allocation bytes \
         after 100, above the limit of 512 that the 40000 bytes of heap that remain for it allow \
         (70000 bytes are free, 30000 are kept in reserve)"
    );
    assert_eq!(
        memory.refusal(&RuntimeError::ExecutionLimit {
            kind: ExecutionLimitKind::ConstructedValues,
            used: 4,
            requested: 8,
            limit: 10,
        }),
        "insufficient memory: building a program of 9000 bytes needs 8 more constructed values \
         after 4, above the limit of 10 that the 40000 bytes of heap that remain for it allow \
         (70000 bytes are free, 30000 are kept in reserve)"
    );
    let work = RuntimeError::ExecutionLimit {
        kind: ExecutionLimitKind::Work,
        used: 1,
        requested: 2,
        limit: 2,
    };
    assert_eq!(memory.refusal(&work), work.to_string());
    assert!(!is_memory_limit(&work));
    assert_eq!(memory.refusal(&RuntimeError::TypeMismatch), "type mismatch");
    assert!(!is_memory_limit(&RuntimeError::TypeMismatch));
    assert!(is_memory_limit(&RuntimeError::PreparationLimit));
}

#[test]
fn a_candidate_whose_memory_figure_exceeds_the_heap_that_remains_is_refused_with_its_numbers() {
    // Specification 34 MEM-12 and §11.3 (UPDATE-06): an online change compares the candidate's
    // memory figure with the heap that remains for it once its upload is freed: the room less the
    // reserve, with the upload's bytes. Worked by hand: a change of 6,000 bytes when 40,000 bytes
    // were free and 8,192 are kept leaves 40,000 - 8,192 + 6,000 = 37,808 bytes, so a figure of
    // 37,808 fits and one of 37,809 is refused, naming both numbers. The wrong behaviour: the
    // upload's bytes left out (37,808 refused against 31,808), the reserve not subtracted (37,809
    // admitted against 46,000), or a figure equal to the heap refused.
    let memory = ChangeMemory::for_change(6_000, 40_000, 8_192);
    assert_eq!(memory.figure_refusal(37_808), None);
    assert_eq!(
        memory.figure_refusal(37_809).as_deref(),
        Some(
            "insufficient memory: a change of 6000 bytes needs 37809 bytes of memory, more than \
             the 37808 bytes of heap that remain for it once it is prepared (40000 bytes are free, \
             8192 are kept in reserve)"
        )
    );
}

#[test]
fn a_validation_limit_is_refused_as_insufficient_memory_with_its_numbers() {
    // Specification 34 §11.3 (UPDATE-06): a candidate that needs more than the limits of the heap
    // that remains is refused with `insufficient memory` and its numbers, and the validator's limits
    // are among them. The validator's analysis of the A4 program peaks at a number of bytes that the
    // test measures; the limit for `h` bytes of heap is two thirds of `h` (48 KiB per 72 KiB), so a
    // heap of three halves of that peak less one leaves a limit just below it. The core returns the
    // validator's own error, and the refusal is the same text as for any other limit, with the
    // artifact's length, the heap that remains, the room and the reserve. The validator's work limit
    // is worded the same way. Wrong behaviour caught: the bench's reply `invalid bytecode 'invalid
    // section data: validation analysis storage limit exceeded'`, which names no number and reads as
    // a damaged program, for a program that only needed more memory than the limits allow.
    let memory = DeviceMemory {
        region_bytes: 64 * 1024,
        heap_bytes: 144 * 1024,
        call_depth: 7,
    };
    let reserve = 8_192;
    let peak = BytecodeModule::decode(PROGRAM)
        .expect("the A4 program decodes")
        .validated_with_limits(ValidationLimits::default())
        .expect("the A4 program validates")
        .stats()
        .peak_scratch_bytes;
    // Any heap that remains gives limits (the call levels come from the stack): the premise of the
    // test is that the heap that remains holds the artifact, so that the validator's limit is met
    // and not the artifact's.
    assert!(
        3 * (peak - 1) / 2 >= PROGRAM.len(),
        "the analysis of the A4 program peaks at {peak}"
    );
    let remains = 3 * (peak - 1) / 2;
    let room = remains + reserve;
    let budget = ChangeMemory::for_change(PROGRAM.len(), room, reserve);
    let limits = budget.limits(memory).expect("the limits of the heap");
    assert!(limits.validation.max_scratch_bytes < peak);

    let error = PreparedModule::from_bytes(PROGRAM, limits).expect_err("the analysis is refused");
    assert_eq!(
        error,
        RuntimeError::from(BytecodeError::ValidationStorageLimit)
    );
    assert!(is_memory_limit(&error));
    let expected = format!(
        "insufficient memory: a change of 6412 bytes needs more than the {remains} bytes of heap \
         that remain for it ({room} bytes are free, {reserve} are kept in reserve)"
    );
    assert_eq!(budget.refusal(&error), expected);
    assert_eq!(
        prepare_candidate(PROGRAM.to_vec(), memory, room, reserve).err(),
        Some(expected)
    );

    let work = RuntimeError::from(BytecodeError::ValidationWorkLimit);
    assert!(is_memory_limit(&work));
    assert_eq!(
        ChangeMemory::for_install(9_000, 70_000, 30_000).refusal(&work),
        "insufficient memory: a program of 9000 bytes needs more than the 40000 bytes of heap that \
         remain for it (70000 bytes are free, 30000 are kept in reserve)"
    );
}

#[test]
fn a_change_longer_than_the_heap_that_remains_is_refused_before_it_is_decoded() {
    // Specification 34 §11.3: with 200,000 bytes free and 100,000 kept in reserve, 100,000 bytes of
    // heap remain, and the OSCAT core artifact (141,320 bytes) is longer than that, so it is refused
    // with its numbers; the 6,412 bytes of the A4 program are prepared. The upload is consumed either
    // way (it is freed as soon as the module is prepared). Wrong behaviour caught: an artifact decoded
    // although the heap that remains cannot hold it, which would reset the chip.
    assert_eq!(OSCAT.len(), 141_320);
    let memory = DeviceMemory {
        region_bytes: 1024 * 1024,
        heap_bytes: 144 * 1024,
        call_depth: 7,
    };
    let refused = prepare_candidate(OSCAT.to_vec(), memory, 200_000, 100_000);
    assert_eq!(
        refused.err().as_deref(),
        Some(
            "insufficient memory: a change of 141320 bytes needs more than the 100000 bytes of \
             heap that remain for it (200000 bytes are free, 100000 are kept in reserve)"
        )
    );
    assert_eq!(PROGRAM.len(), 6_412);
    let (prepared, budget) = prepare_candidate(PROGRAM.to_vec(), memory, 200_000, 100_000)
        .expect("the A4 program fits the heap that remains");
    assert_eq!(prepared.limits().max_artifact_bytes, 100_000);
    assert_eq!(
        budget,
        ChangeMemory::for_change(6_412, 200_000, 100_000).with_call_depth(7)
    );
}

#[test]
fn a_program_that_is_not_bytecode_is_refused_with_the_preparations_own_error() {
    // Specification 11 §6.7.1: a malformed byte sequence fails preparation as the typed
    // `RuntimeError::Bytecode`, and the reply is that error's text, not a memory refusal. Wrong
    // behaviour caught: every preparation failure worded as `insufficient memory`.
    let memory = DeviceMemory {
        region_bytes: 16 * 1024,
        heap_bytes: 72 * 1024,
        call_depth: 7,
    };
    let refused = prepare_candidate(Vec::from([0, 1, 2, 3]), memory, 100_000, 20_000)
        .expect_err("four bytes are not a program");
    assert!(
        refused.contains("bytecode"),
        "the error of the decoder: {refused}"
    );
    assert!(!refused.contains("insufficient memory"), "{refused}");
}

#[test]
fn a_binding_without_a_line_is_found() {
    // Specification 34 §11.3: the board probe binds the input `%IX0.0` and the output `%QX0.0`. With a
    // line for each, nothing is unwired. With only the input line the output binding is the one
    // found, and the other way round. A line at another bit (`%IX0.1`) or another byte (`%QX1.0`)
    // covers neither. Wrong behaviour caught: a check that finds no binding because it compares the
    // area alone; one that confuses a bit with its byte; one that takes a line of the other area.
    let bindings = probe_bindings();
    let mut found = Vec::new();
    for binding in &bindings {
        found.push(address_text(&binding.address));
    }
    found.sort();
    assert_eq!(found, ["%IX0.0", "%QX0.0"], "the probe's two bindings");

    let both = unwired_binding(&bindings, &[input("%IX0.0")], &[output("%QX0.0")]);
    assert_eq!(both, None);
    let no_output = unwired_binding(&bindings, &[input("%IX0.0")], &[]);
    assert_eq!(no_output, Some(address("%QX0.0")));
    let no_input = unwired_binding(&bindings, &[], &[output("%QX0.0")]);
    assert_eq!(no_input, Some(address("%IX0.0")));
    let other_bit = unwired_binding(&bindings, &[input("%IX0.1")], &[output("%QX0.0")]);
    assert_eq!(other_bit, Some(address("%IX0.0")));
    let other_byte = unwired_binding(&bindings, &[input("%IX0.0")], &[output("%QX1.0")]);
    assert_eq!(other_byte, Some(address("%QX0.0")));
}

#[test]
fn a_line_covers_the_bits_or_bytes_of_a_binding() {
    // Specification 34 §11.3: a line covers a bit binding when it is that bit, and a binding of a
    // byte or more when it lies in one of the binding's bytes: `%IW2` covers the bytes 2 and 3. A
    // hierarchical address has no line. Wrong behaviour caught: a word binding wired by a line of
    // another byte; a bit binding wired by another bit of its byte.
    assert!(covers(&address("%IX1.3"), &address("%IX1.3")));
    assert!(!covers(&address("%IX1.3"), &address("%IX1.4")));
    assert!(!covers(&address("%IX1.3"), &address("%IX2.3")));
    assert!(covers(&address("%IW2"), &address("%IX2.7")));
    assert!(covers(&address("%IW2"), &address("%IX3.0")));
    assert!(!covers(&address("%IW2"), &address("%IX4.0")));
    assert!(!covers(&address("%IW2"), &address("%IX1.7")));
    assert!(covers(&address("%IB5"), &address("%IX5.0")));
    assert!(!covers(&address("%IB5"), &address("%IX6.0")));
    assert!(!covers(&address("%IX0.1.2"), &address("%IX0.1")));
}

#[test]
fn an_address_is_named_as_the_standard_writes_it() {
    // IEC 61131-3 Ed.3 §6.5.5.3 (direct representation): `%`, the location prefix, the size prefix
    // and the position. Wrong behaviour caught: a refusal that names a debug dump of the address.
    assert_eq!(address_text(&address("%QX0.5")), "%QX0.5");
    assert_eq!(address_text(&address("%IW2")), "%IW2");
    assert_eq!(address_text(&address("%MD4")), "%MD4");
    assert_eq!(address_text(&address("%IB1")), "%IB1");
    assert_eq!(address_text(&address("%QL8")), "%QL8");
    assert_eq!(address_text(&address("%IX0.1.2")), "%IX0.1.2");
}

#[test]
fn a_call_deeper_than_the_stack_holds_is_refused_as_insufficient_stack() {
    // Specification 34 MEM-03 (owner, 21 October 2026, "Stack decides"): a construction that calls
    // deeper than the call levels the device's main stack holds is refused as `insufficient stack`
    // with those levels, and the change's or the program's length. Wrong behaviour caught: the bench's
    // reply `the retained-value check could not run: invalid bytecode 'vm call stack overflow'`,
    // which reads as a damaged program; a call depth that the refusal does not name; a memory limit
    // worded as a stack refusal.
    let overflow = VmTrap::CallStackOverflow.into_runtime_error();
    assert!(is_call_depth_limit(&overflow));
    assert!(!is_memory_limit(&overflow));
    assert_eq!(
        ChangeMemory::for_change(9_008, 60_000, 8_192)
            .with_call_depth(7)
            .refusal(&overflow),
        "insufficient stack: a change of 9008 bytes calls deeper than the 7 call levels that the \
         device's main stack holds"
    );
    assert_eq!(
        ChangeMemory::for_install(9_008, 60_000, 8_192)
            .with_call_depth(7)
            .refusal(&overflow),
        "insufficient stack: a program of 9008 bytes calls deeper than the 7 call levels that the \
         device's main stack holds"
    );
    assert!(!is_call_depth_limit(&RuntimeError::PreparationLimit));
}
