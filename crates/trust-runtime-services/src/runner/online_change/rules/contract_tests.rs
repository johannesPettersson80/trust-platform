//! The rules of an online change (specification 34 §11.3; UPDATE-06; specification 44 §12.3): the
//! memory a candidate may use and the refusal when it needs more, and the match of the candidate's
//! bindings with the lines of the running program's record. Every expected value is worked out here
//! from the specification, not taken from the code under test.

use alloc::string::ToString;
use alloc::vec::Vec;

use smol_str::SmolStr;
use trust_runtime_core::deployment::{GpioInput, GpioOutput};
use trust_runtime_core::io_address::IoAddress;
use trust_runtime_core::io_image::PreparedIoBinding;
use trust_runtime_core::memory::IoArea;
use trust_runtime_core::native::image::Target;
use trust_runtime_core::native::live::io_bindings;
use trust_runtime_core::native::program::{admit, Device};

use super::{covers, unwired_binding, ChangeMemory};
use crate::io_drivers::EntryBytes;
use crate::tests::runner_ports::board_probe_image;

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
    let image = board_probe_image();
    let device = Device {
        target: Target::this_cpu().unwrap_or(Target::X86_64),
        program_memory: 1 << 20,
        task_stack: 1 << 20,
        max_statements: 1_000_000,
        trusted_keys: &[],
    };
    let admitted = admit(&image, &device).expect("the board probe is admitted");
    io_bindings(&admitted)
        .expect("the board probe's bindings read")
        .0
}

#[test]
fn a_candidate_whose_areas_exceed_the_heap_beside_its_image_is_refused_with_its_numbers() {
    // Specification 44 §12.3, step 2, and specification 34 §11.3 (UPDATE-06): the new image stays in
    // the heap for as long as the changed program runs, so the candidate's areas, bookkeeping and
    // retain capture, its memory figure less its stack bound, must fit in the room less the reserve.
    // Worked by hand: a change of 6,000 bytes when 40,000 bytes were free and 8,192 are kept leaves
    // 40,000 - 8,192 = 31,808 bytes, so 31,808 bytes fit and 31,809 are refused, naming the numbers.
    // The wrong behaviour: the image's bytes counted as freed (31,809 admitted against 37,808), the
    // reserve not subtracted (31,809 admitted against 40,000), or a figure equal to the heap refused.
    let memory = ChangeMemory::for_change(6_000, 40_000, 8_192);
    assert_eq!(memory.held_figure_refusal(31_808), None);
    assert_eq!(
        memory.held_figure_refusal(31_809).as_deref(),
        Some(
            "insufficient memory: a change of 6000 bytes needs 31809 bytes of memory, more than \
             the 31808 bytes of heap that remain for it beside its image (40000 bytes are free, \
             8192 are kept in reserve)"
        )
    );
}

#[test]
fn a_change_that_needs_more_than_the_heap_that_remains_names_its_numbers() {
    // Specification 34 §11.3: with 200,000 bytes free and 100,000 kept in reserve, 100,000 bytes of
    // heap remain, and a change of 141,320 bytes that needs more is refused with its length, the heap
    // that remains, the room and the reserve; a room below the reserve leaves 0 bytes and does not
    // underflow. Wrong behaviour caught: a refusal that names no number, and a room less than the
    // reserve that wraps around.
    assert_eq!(
        ChangeMemory::for_change(141_320, 200_000, 100_000).needs_more(),
        "insufficient memory: a change of 141320 bytes needs more than the 100000 bytes of heap \
         that remain for it (200000 bytes are free, 100000 are kept in reserve)"
    );
    assert_eq!(
        ChangeMemory::for_change(6_412, 4_000, 10_000).needs_more(),
        "insufficient memory: a change of 6412 bytes needs more than the 0 bytes of heap that \
         remain for it (4000 bytes are free, 10000 are kept in reserve)"
    );
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
        found.push(binding.address.to_string());
    }
    found.sort();
    assert_eq!(found, ["%IX0.0", "%QX0.0"], "the probe's two bindings");

    let both = unwired_binding(&bindings, &[input("%IX0.0")], &[output("%QX0.0")], &[]);
    assert_eq!(both, None);
    let no_output = unwired_binding(&bindings, &[input("%IX0.0")], &[], &[]);
    assert_eq!(no_output, Some(address("%QX0.0")));
    let no_input = unwired_binding(&bindings, &[], &[output("%QX0.0")], &[]);
    assert_eq!(no_input, Some(address("%IX0.0")));
    let other_bit = unwired_binding(&bindings, &[input("%IX0.1")], &[output("%QX0.0")], &[]);
    assert_eq!(other_bit, Some(address("%IX0.0")));
    let other_byte = unwired_binding(&bindings, &[input("%IX0.0")], &[output("%QX1.0")], &[]);
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
fn a_driver_entry_wires_the_bindings_that_share_its_bytes() {
    // Specification 34 §11.3: a driver's entry covers a binding that shares one of its bytes. The
    // probe binds `%IX0.0` and `%QX0.0`. A CAN receive at `%IB0` of 2 bytes takes input bytes 0 and
    // 1, so it wires `%IX0.0`; a PWM output at `%QW0` takes output bytes 0 and 1, so it wires
    // `%QX0.0`: with both, and no line, nothing is unwired. An input entry at byte 1 (`%IB1`, one
    // byte) shares no byte with `%IX0.0`, and an output entry does not wire an input binding.
    // Wrong behaviour caught: a change refused although a driver feeds its binding; a binding
    // taken as wired by an entry of the other area or of another byte.
    let bindings = probe_bindings();
    let input_bytes = EntryBytes {
        area: IoArea::Input,
        bytes: 0..2,
    };
    let output_bytes = EntryBytes {
        area: IoArea::Output,
        bytes: 0..2,
    };
    assert_eq!(
        unwired_binding(
            &bindings,
            &[],
            &[],
            &[input_bytes.clone(), output_bytes.clone()]
        ),
        None
    );
    let next_byte = EntryBytes {
        area: IoArea::Input,
        bytes: 1..2,
    };
    assert_eq!(
        unwired_binding(&bindings, &[], &[output("%QX0.0")], &[next_byte]),
        Some(address("%IX0.0"))
    );
    assert_eq!(
        unwired_binding(&bindings, &[], &[], &[output_bytes]),
        Some(address("%IX0.0"))
    );
}
