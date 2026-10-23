//! TIME-08 and TIME-09: the scan allowance charges executed instructions and the
//! values a scan copies or traverses. Declaration lookups are bookkeeping, and
//! construction runs under its own allowance rather than the per-entry scan limit.
use trust_runtime::bytecode::BytecodeVersion;
use trust_runtime::harness::CompileSession;
use trust_runtime_core::error::{ExecutionLimitKind, RuntimeError};
use trust_runtime_core::value::Duration;
use trust_runtime_core::vm::{PreparationLimits, PreparedModule};

fn artifact(source: &str) -> Vec<u8> {
    CompileSession::from_source(source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .expect("accepted source")
        .encode()
        .unwrap()
}

fn scheduled(body: &str) -> String {
    format!(
        "{body}\nCONFIGURATION Conf RESOURCE Controller ON PLC TASK Periodic (INTERVAL := T#10ms, \
         PRIORITY := 0); PROGRAM Plant WITH Periodic : Main; END_RESOURCE END_CONFIGURATION"
    )
}

/// Runs the first scan under `max_work`. Only a work refusal of that scan may
/// return false, because instantiation uses the separate construction allowance.
fn first_scan_admitted(bytes: &[u8], max_work: usize) -> bool {
    let limits = PreparationLimits {
        max_work,
        ..Default::default()
    };
    let prepared = PreparedModule::from_bytes(bytes, limits).expect("admit the artifact");
    let mut state = prepared
        .instantiate(0)
        .expect("the default construction allowance admits the artifact");
    match state.execute_cycle(Duration::from_millis(10)) {
        Ok(()) => true,
        Err(error) => {
            assert!(
                matches!(
                    error,
                    RuntimeError::ExecutionLimit {
                        kind: ExecutionLimitKind::Work,
                        ..
                    }
                ),
                "a refused first scan must be a work refusal, got {error:?}"
            );
            false
        }
    }
}

/// The smallest scan allowance that admits the first scan, which is the work
/// that scan charges. Admission is monotone in the allowance.
fn first_scan_work(source: &str) -> usize {
    let bytes = artifact(&scheduled(source));
    let (mut low, mut high) = (1usize, 1_000_000usize);
    assert!(
        first_scan_admitted(&bytes, high),
        "the upper probe must admit the first scan"
    );
    while low < high {
        let middle = low + (high - low) / 2;
        if first_scan_admitted(&bytes, middle) {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    low
}

fn padded_main(extra_globals: usize, extra_locals: usize) -> String {
    let mut source = String::new();
    if extra_globals > 0 {
        source.push_str("VAR_GLOBAL\n");
        for index in 0..extra_globals {
            source.push_str(&format!("global_{index} : INT;\n"));
        }
        source.push_str("END_VAR\n");
    }
    source.push_str("PROGRAM Main\nVAR x : INT; y : INT;\n");
    for index in 0..extra_locals {
        source.push_str(&format!("local_{index} : INT;\n"));
    }
    source.push_str("END_VAR\ny := x + INT#1;\nEND_PROGRAM\n");
    source
}

#[test]
fn scan_work_ignores_declaration_table_size() {
    // TIME-08: name, index and member lookups are bookkeeping. Both programs
    // execute the same instructions for `y := x + INT#1`.
    let small = first_scan_work(&padded_main(0, 0));
    let large = first_scan_work(&padded_main(500, 2_000));
    assert_eq!(large, small);
}

fn array_copy_program(element_type: &str) -> String {
    format!(
        "PROGRAM Main\nVAR a : {element_type}; b : {element_type}; END_VAR\nb := a;\nEND_PROGRAM\n"
    )
}

#[test]
fn array_copy_charges_one_unit_per_element() {
    // Whole-variable assignment lowers through the same type-agnostic `emit_assign`
    // and `emit_expr` path for an array and for a scalar, so the instruction count
    // is equal and only the copy charge differs. TIME-08: the
    // load copies the array for 1 + elements.len(); a whole-variable store charges
    // no clone, and a BYTE array's lifetime check is skipped. ARRAY[0..255] is
    // 1 + 256 and ARRAY[0..0] is 1 + 1, so their difference is 255.
    let wide = first_scan_work(&array_copy_program("ARRAY[0..255] OF BYTE"));
    let single = first_scan_work(&array_copy_program("ARRAY[0..0] OF BYTE"));
    assert_eq!(wide, single + 255);

    // A scalar copy charges nothing beyond its instructions, so the one-element
    // array costs exactly its array unit and its element unit more than INT.
    let scalar =
        first_scan_work("PROGRAM Main\nVAR x : INT; y : INT; END_VAR\ny := x;\nEND_PROGRAM\n");
    assert_eq!(single, scalar + 2);
}

#[test]
fn construction_uses_its_own_allowance() {
    // TIME-09: construction runs under max_construction_work. A 100-unit scan
    // allowance admits the trivial scan, and the default construction allowance
    // admits 5,000 elements. Only the 100-unit construction allowance refuses them.
    let bytes = artifact(&scheduled(
        "VAR_GLOBAL big : ARRAY[0..4999] OF INT; END_VAR\nPROGRAM Main\nEND_PROGRAM\n",
    ));
    let scan_only = PreparationLimits {
        max_work: 100,
        ..Default::default()
    };
    let prepared = PreparedModule::from_bytes(&bytes, scan_only).unwrap();
    let mut state = prepared
        .instantiate(0)
        .expect("the default construction allowance admits 5,000 elements");
    state
        .execute_cycle(Duration::from_millis(10))
        .expect("a 100-unit scan allowance admits the empty program");

    let construction_refused = PreparationLimits {
        max_work: 100,
        max_construction_work: 100,
        ..Default::default()
    };
    let error = PreparedModule::from_bytes(&bytes, construction_refused)
        .unwrap()
        .instantiate(0)
        .err()
        .expect("100 construction units cannot build 5,000 elements");
    assert!(
        matches!(
            error,
            RuntimeError::ExecutionLimit {
                kind: ExecutionLimitKind::Work,
                limit: 100,
                ..
            }
        ),
        "construction refusal must name the construction allowance, got {error:?}"
    );
}

fn in_out_program(array_type: &str) -> String {
    format!(
        "FUNCTION Poke : BOOL\nVAR_IN_OUT buf : {array_type}; END_VAR\nbuf[0] := BYTE#1;\nPoke := TRUE;\nEND_FUNCTION\nPROGRAM Main\nVAR data : {array_type}; done : BOOL; END_VAR\ndone := Poke(buf := data);\nEND_PROGRAM\n"
    )
}

#[test]
fn in_out_array_binding_charges_no_copy() {
    // TIME-08: IEC passes VAR_IN_OUT by reference. The engine's private snapshot and
    // copy-back are call-transaction bookkeeping, so the scan's work does not grow with
    // the bound array's length.
    let wide = first_scan_work(&in_out_program("ARRAY[0..1023] OF BYTE"));
    let single = first_scan_work(&in_out_program("ARRAY[0..0] OF BYTE"));
    assert_eq!(wide, single);
}
