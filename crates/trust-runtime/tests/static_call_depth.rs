//! A program's static call depth sizes the engine's per-call reservations (spec 34 MEM-02,
//! MEM-09). Construction reserves the deepest chain of activations a program can reach, not
//! the admitted `max_call_depth`, so a chip does not have to hold the whole ceiling's memory;
//! a recursive call graph has no static depth and reserves for the ceiling, as before.
use trust_runtime::bytecode::BytecodeVersion;
use trust_runtime::harness::CompileSession;
use trust_runtime_core::bytecode::CallDepth;
use trust_runtime_core::vm::{PreparationLimits, PreparedModule};
use trust_runtime_services::runner::engine::DeviceMemory;
use trust_runtime_services::runner::{prepare_candidate, stack_shortage, too_deep};

/// Compile a source-free artifact.
fn artifact(source: &str) -> Vec<u8> {
    CompileSession::from_source(source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .unwrap()
        .encode()
        .unwrap()
}

/// Compile a source-free artifact and prepare it under the given call-depth ceiling.
fn prepared(source: &str, max_call_depth: usize) -> PreparedModule {
    let bytes = artifact(source);
    PreparedModule::from_bytes(
        &bytes,
        PreparationLimits {
            max_call_depth,
            ..PreparationLimits::default()
        },
    )
    .unwrap()
}

const NO_CALLS: &str = r#"
PROGRAM Main
VAR value : INT; END_VAR
value := value + INT#1;
END_PROGRAM
"#;

/// Spec 34 MEM-02: a program that calls nothing stays on the entry activation, so the
/// reserved call depth is one and never the ceiling. The wrong behaviour it catches is a
/// reservation that follows `max_call_depth` for every program.
#[test]
fn a_program_without_calls_has_static_call_depth_one() {
    let prepared = prepared(NO_CALLS, 8);
    assert_eq!(prepared.static_call_depth(), CallDepth::Static(1));
    assert_eq!(prepared.reserved_call_levels_for_test(), 1);
}

/// Main calls FB1, which calls FB2, FB3 and FB4: five simultaneously active frames. The
/// wrong behaviour it catches is a graph walk that misses an edge and reports fewer levels.
const FIVE_LEVEL_CHAIN: &str = r#"
FUNCTION_BLOCK FB4
VAR result : INT; END_VAR
result := INT#4;
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB3
VAR next : FB4; END_VAR
next();
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB2
VAR next : FB3; END_VAR
next();
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB1
VAR next : FB2; END_VAR
next();
END_FUNCTION_BLOCK

PROGRAM Main
VAR next : FB1; END_VAR
next();
END_PROGRAM
"#;

/// Spec 34 MEM-02: a five-level function-block chain packs five frames, the program entry
/// and the four it nests. The wrong behaviour it catches is counting the chain as four.
#[test]
fn a_five_level_function_block_chain_has_static_call_depth_five() {
    let prepared = prepared(FIVE_LEVEL_CHAIN, 8);
    assert_eq!(prepared.static_call_depth(), CallDepth::Static(5));
    assert_eq!(prepared.reserved_call_levels_for_test(), 5);
}

/// Two `Advance` implementers: the shallow one calls nothing, the deep one nests two function
/// calls. An interface call must count the deeper implementer. The wrong behaviour it
/// catches is resolving an interface call to only the first or the shallow implementer.
const INTERFACE_IMPLEMENTERS: &str = r#"
INTERFACE ICounter
METHOD Advance : INT
END_METHOD
END_INTERFACE

FUNCTION_BLOCK ImplShallow IMPLEMENTS ICounter
VAR ticks : INT; END_VAR
ticks := ticks + INT#1;
METHOD PUBLIC Advance : INT
Advance := INT#1;
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK ImplDeep IMPLEMENTS ICounter
VAR ticks : INT; END_VAR
ticks := ticks + INT#1;
METHOD PUBLIC Advance : INT
Advance := Deep1();
END_METHOD
END_FUNCTION_BLOCK

FUNCTION Deep1 : INT
Deep1 := Deep2() + INT#1;
END_FUNCTION

FUNCTION Deep2 : INT
Deep2 := INT#2;
END_FUNCTION

PROGRAM Main
VAR counter : ICounter; result : INT; END_VAR
result := counter.Advance();
END_PROGRAM
"#;

/// Spec 34 MEM-09: the interface call reaches both implementers, so the module depth is the
/// program entry plus the deeper implementer's three frames. The wrong behaviour it catches
/// is missing one implementer and reserving two levels instead of four.
#[test]
fn an_interface_call_counts_its_deepest_implementer() {
    let prepared = prepared(INTERFACE_IMPLEMENTERS, 8);
    assert_eq!(prepared.static_call_depth(), CallDepth::Static(4));
    assert_eq!(prepared.reserved_call_levels_for_test(), 4);
}

/// `Recur` calls itself, so its call graph has a cycle and no static depth (the program of
/// `call_path_recursive_register_callee.rs`).
fn recursion_source() -> &'static str {
    r#"
NAMESPACE Demo
FUNCTION Recur : INT
VAR_INPUT level : INT; END_VAR
IF level = INT#0 THEN
    Recur := INT#1;
ELSE
    Recur := Demo.Recur(level := level - INT#1) + INT#1;
END_IF;
END_FUNCTION
END_NAMESPACE

PROGRAM Main
VAR answer : INT; END_VAR
answer := Demo.Recur(level := INT#2);
END_PROGRAM
"#
}

/// Spec 34 MEM-02 and MEM-09: a recursive program keeps the ceiling reservation, exactly as
/// before the change. The wrong behaviour it catches is reporting a finite depth for a call
/// graph with a cycle and under-reserving it.
#[test]
fn a_recursive_function_falls_back_to_the_ceiling() {
    let prepared = prepared(recursion_source(), 8);
    assert_eq!(prepared.static_call_depth(), CallDepth::Recursive);
    assert_eq!(prepared.reserved_call_levels_for_test(), 8);
}

/// Spec 34 MEM-02 and MEM-09: the five-level program reserves five levels' worth even under
/// a host-scale ceiling. The wrong behaviour it catches is a reservation that still follows
/// `max_call_depth` (1,024) for an acyclic program.
#[test]
fn reservations_follow_the_static_depth_not_the_ceiling() {
    let prepared = prepared(FIVE_LEVEL_CHAIN, 1_024);
    assert_eq!(prepared.reserved_call_levels_for_test(), 5);
}

/// Spec 34 MEM-03 (owner, 21 October 2026, "Stack decides"): a device refuses a program whose static
/// depth is more than the call levels its main stack holds, before it constructs it, with both
/// numbers, and admits one whose depth it holds. A recursive program has no static depth: it is
/// admitted, and its deep call is refused when the call is made, as on a computer. The five-level
/// chain needs 5 levels, so a stack of 5 holds it and one of 4 refuses it, also as the candidate of
/// an online change. Wrong behaviour caught: a check on the wrong side of the limit (refusing 5 of
/// 5), a refusal of every recursive program, and a refusal that names no number.
#[test]
fn a_device_refuses_a_program_deeper_than_its_stack_holds() {
    let chain = prepared(FIVE_LEVEL_CHAIN, 8);
    assert_eq!(too_deep(&chain, 5), None);
    assert_eq!(too_deep(&chain, 4), Some(5));
    assert_eq!(too_deep(&prepared(recursion_source(), 8), 1), None);
    assert_eq!(
        stack_shortage("a program", 9_000, 5, 4),
        "insufficient stack: a program of 9000 bytes needs 5 call levels, more than the 4 that \
         the device's main stack holds"
    );

    let bytes = artifact(FIVE_LEVEL_CHAIN);
    let length = bytes.len();
    let memory = |call_depth| DeviceMemory {
        region_bytes: 1024 * 1024,
        heap_bytes: 144 * 1024,
        call_depth,
    };
    assert!(prepare_candidate(bytes.clone(), memory(5), 200_000, 8_192).is_ok());
    assert_eq!(
        prepare_candidate(bytes, memory(4), 200_000, 8_192).err(),
        Some(format!(
            "insufficient stack: a change of {length} bytes needs 5 call levels, more than the 4 \
             that the device's main stack holds"
        ))
    );
}
