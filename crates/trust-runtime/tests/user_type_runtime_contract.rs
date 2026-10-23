use trust_runtime::bytecode::{
    opcodes, BytecodeModule, InitializationStage, InitializerIndex, PouIndex, RefLocation,
    RefSegment, RefTable, SectionData, SectionId, StorageLayout, StorageOwner, StorageRole,
    StringTable,
};
use trust_runtime::harness::{CompileSession, TestHarness};
use trust_runtime::value::Value;

fn run(source: &str) -> TestHarness {
    let mut harness =
        TestHarness::from_source(source).unwrap_or_else(|error| panic!("fixture: {error}"));
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "{:?}", cycle.errors);
    harness
}

fn array_elements(harness: &TestHarness, name: &str) -> Vec<Value> {
    let Some(Value::Array(values)) = harness.get_output(name) else {
        panic!("{name} must be an array");
    };
    values.elements().to_vec()
}

fn string_index(strings: &StringTable, name: &str) -> u32 {
    strings
        .entries
        .iter()
        .position(|candidate| candidate.eq_ignore_ascii_case(name))
        .and_then(|index| u32::try_from(index).ok())
        .unwrap_or_else(|| panic!("missing bytecode string {name}"))
}

fn rewrite_initializer_to_load_global_member(
    module: &mut BytecodeModule,
    global_name: &str,
    global_member: &str,
    owner_pou_name: &str,
    local_name: &str,
    conversion_opcode: u8,
) {
    let strings = match module.section(SectionId::StringTable) {
        Some(SectionData::StringTable(strings)) => strings,
        _ => panic!("source-free module string table"),
    };
    let global_name_idx = string_index(strings, global_name);
    let member_name_idx = string_index(strings, global_member);
    let owner_pou_name_idx = string_index(strings, owner_pou_name);
    let local_name_idx = string_index(strings, local_name);

    let pou_id = match module.section(SectionId::PouIndex) {
        Some(SectionData::PouIndex(PouIndex { entries })) => entries
            .iter()
            .find(|pou| pou.name_idx == owner_pou_name_idx)
            .map(|pou| pou.id)
            .unwrap_or_else(|| panic!("missing POU {owner_pou_name}")),
        _ => panic!("source-free module POU index"),
    };
    let (global_ref_idx, type_id, declaration_idx) = match module.section(SectionId::StorageLayout)
    {
        Some(SectionData::StorageLayout(StorageLayout { entries })) => {
            let global = entries
                .iter()
                .find(|declaration| {
                    declaration.owner == StorageOwner::Global
                        && declaration.owner_pou_id.is_none()
                        && declaration.role == StorageRole::Variable
                        && declaration.name_idx == global_name_idx
                })
                .unwrap_or_else(|| panic!("missing global declaration {global_name}"));
            let global_ref_idx = global
                .ref_idx
                .unwrap_or_else(|| panic!("global {global_name} has no base reference"));
            let local = entries
                .iter()
                .enumerate()
                .find(|(_, declaration)| {
                    declaration.owner == StorageOwner::Frame
                        && declaration.owner_pou_id == Some(pou_id)
                        && declaration.role == StorageRole::Variable
                        && declaration.name_idx == local_name_idx
                })
                .unwrap_or_else(|| panic!("missing local declaration {local_name}"));
            (
                global_ref_idx,
                local
                    .1
                    .type_id
                    .unwrap_or_else(|| panic!("local {local_name} has no type")),
                u32::try_from(local.0).expect("declaration index fits u32"),
            )
        }
        _ => panic!("source-free module storage layout"),
    };

    let reference_idx = match module.section_mut(SectionId::RefTable) {
        Some(SectionData::RefTable(RefTable { entries })) => {
            let mut reference = entries
                .get(global_ref_idx as usize)
                .cloned()
                .unwrap_or_else(|| panic!("global {global_name} base reference is missing"));
            assert_eq!(reference.location, RefLocation::Global);
            assert!(
                reference.segments.is_empty(),
                "global base reference has a path"
            );
            reference.segments.push(RefSegment::Field {
                name_idx: member_name_idx,
            });
            let reference_idx =
                u32::try_from(entries.len()).expect("reference table index fits u32");
            entries.push(reference);
            reference_idx
        }
        _ => panic!("source-free module reference table"),
    };

    let (initializer_idx, result_ref_idx, code_offset, old_code_length) =
        match module.section(SectionId::Initializers) {
            Some(SectionData::Initializers(InitializerIndex { entries })) => entries
                .iter()
                .enumerate()
                .find(|(_, initializer)| {
                    initializer.declaration_idx == Some(declaration_idx)
                        && initializer.stage == InitializationStage::Explicit
                })
                .map(|(index, initializer)| {
                    (
                        index,
                        initializer.result_ref_idx,
                        initializer.code_offset,
                        initializer.code_length,
                    )
                })
                .unwrap_or_else(|| panic!("missing explicit initializer for {local_name}")),
            _ => panic!("source-free module initializer index"),
        };
    let mut replacement = Vec::with_capacity(15);
    replacement.push(0x20); // LOAD_REF global_name.global_member
    replacement.extend_from_slice(&reference_idx.to_le_bytes());
    replacement.push(conversion_opcode);
    replacement.extend_from_slice(&type_id.to_le_bytes());
    replacement.push(0x21); // STORE_REF initializer result
    replacement.extend_from_slice(&result_ref_idx.to_le_bytes());
    let replacement_len = u32::try_from(replacement.len()).expect("initializer body fits u32");

    let start = code_offset as usize;
    let old_end = start
        .checked_add(old_code_length as usize)
        .expect("initializer code range fits usize");
    assert!(old_code_length != 0, "initializer body is not present");
    let length_delta = replacement.len() as i64 - i64::from(old_code_length);
    let shift_offset = |offset: u32| {
        u32::try_from(i64::from(offset) + length_delta).expect("shifted bytecode offset fits u32")
    };

    match module.section_mut(SectionId::PouBodies) {
        Some(SectionData::PouBodies(bodies)) => {
            assert!(
                old_end <= bodies.len(),
                "initializer body range is in bounds"
            );
            bodies.splice(start..old_end, replacement);
        }
        _ => panic!("source-free module POU bodies"),
    }
    if length_delta != 0 {
        if let Some(SectionData::Initializers(InitializerIndex { entries })) =
            module.section_mut(SectionId::Initializers)
        {
            for (index, initializer) in entries.iter_mut().enumerate() {
                if index == initializer_idx {
                    initializer.code_length = replacement_len;
                } else if initializer.code_offset as usize >= old_end {
                    initializer.code_offset = shift_offset(initializer.code_offset);
                }
            }
        }
        if let Some(SectionData::PouIndex(PouIndex { entries })) =
            module.section_mut(SectionId::PouIndex)
        {
            for pou in entries {
                if pou.code_offset as usize >= old_end {
                    pou.code_offset = shift_offset(pou.code_offset);
                }
            }
        }
    } else if let Some(SectionData::Initializers(InitializerIndex { entries })) =
        module.section_mut(SectionId::Initializers)
    {
        entries[initializer_idx].code_length = replacement_len;
    }
}

#[test]
fn user_type_runtime_ordinary_enum_defaults_to_first_literal() {
    let harness = run(r#"
TYPE Color : (Red, Green, Blue); END_TYPE
PROGRAM Main
VAR color : Color; isRed : BOOL; END_VAR
isRed := color = Color#Red;
END_PROGRAM
"#);
    assert_eq!(harness.get_output("isRed"), Some(Value::Bool(true)));
}

#[test]
fn user_type_runtime_ordinary_enum_uses_explicit_type_default() {
    let harness = run(r#"
TYPE Color : (Red, Green, Blue) := Blue; END_TYPE
PROGRAM Main
VAR color : Color; isBlue : BOOL; END_VAR
isBlue := color = Color#Blue;
END_PROGRAM
"#);
    assert_eq!(harness.get_output("isBlue"), Some(Value::Bool(true)));
}

#[test]
fn user_type_runtime_enum_assignment_preserves_declared_identity() {
    let harness = run(r#"
TYPE Color : (Red, Green, Blue); END_TYPE
PROGRAM Main
VAR color : Color; isGreen : BOOL; END_VAR
color := Color#Green;
isGreen := color = Color#Green;
END_PROGRAM
"#);
    assert_eq!(harness.get_output("isGreen"), Some(Value::Bool(true)));
}

#[test]
fn user_type_runtime_named_values_retain_integer_base_range() {
    let harness = run(r#"
TYPE Signal : INT (Low := 1, High := 2); END_TYPE
PROGRAM Main
VAR signal : Signal; result : INT; END_VAR
signal := 27;
result := signal + Signal#High;
END_PROGRAM
"#);
    assert_eq!(harness.get_output("result"), Some(Value::Int(29)));
}

#[test]
fn user_type_runtime_subrange_without_initializer_defaults_to_lower_bound() {
    let harness = run(r#"
TYPE Limited : INT (4..6); END_TYPE
PROGRAM Main
VAR value : Limited; observed : INT; END_VAR
observed := value;
END_PROGRAM
"#);
    assert_eq!(harness.get_output("observed"), Some(Value::Int(4)));
}

#[test]
fn user_type_runtime_subrange_uses_explicit_type_default() {
    let harness = run(r#"
TYPE Limited : INT (4..6) := 5; END_TYPE
PROGRAM Main
VAR value : Limited; observed : INT; END_VAR
observed := value;
END_PROGRAM
"#);
    assert_eq!(harness.get_output("observed"), Some(Value::Int(5)));
}

#[test]
fn user_type_runtime_rejects_above_subrange_without_mutation() {
    let mut harness = TestHarness::from_source(
        r#"
TYPE Limited : INT (4..6); END_TYPE
PROGRAM Main
VAR source : INT := 7; value : Limited := 5; END_VAR
value := source;
END_PROGRAM
"#,
    )
    .expect("dynamic subrange fixture must compile");
    let cycle = harness.cycle();
    assert_eq!(cycle.errors.len(), 1, "{:?}", cycle.errors);
    assert_eq!(harness.get_output("value"), Some(Value::Int(5)));
}

#[test]
fn user_type_runtime_rejects_below_subrange_without_mutation() {
    let mut harness = TestHarness::from_source(
        r#"
TYPE Limited : INT (4..6); END_TYPE
PROGRAM Main
VAR source : INT := 3; value : Limited := 5; END_VAR
value := source;
END_PROGRAM
"#,
    )
    .expect("dynamic subrange fixture must compile");
    let cycle = harness.cycle();
    assert_eq!(cycle.errors.len(), 1, "{:?}", cycle.errors);
    assert_eq!(harness.get_output("value"), Some(Value::Int(5)));
}

#[test]
fn user_type_runtime_rejects_out_of_range_parameter_copy_in_before_body() {
    let mut harness = TestHarness::from_source(
        r#"
TYPE Limited : INT (4..6); END_TYPE
VAR_GLOBAL trace : INT; END_VAR
FUNCTION Read : INT
VAR_INPUT value : Limited; END_VAR
VAR_EXTERNAL trace : INT; END_VAR
trace := trace + 1; Read := value;
END_FUNCTION
PROGRAM Main
VAR source : INT := 7; result : INT := 9; END_VAR
result := Read(value := source);
END_PROGRAM
"#,
    )
    .expect("dynamic parameter fixture must compile");
    let cycle = harness.cycle();
    assert_eq!(cycle.errors.len(), 1, "{:?}", cycle.errors);
    assert_eq!(harness.get_output("result"), Some(Value::Int(9)));
    assert_eq!(
        harness.runtime().storage().get_global("trace"),
        Some(&Value::Int(0))
    );
}

#[test]
fn user_type_runtime_partial_array_initializer_default_fills_rightmost_values() {
    let harness =
        run("PROGRAM Main\nVAR values : ARRAY[1..4] OF INT := [1, 2]; END_VAR\nEND_PROGRAM");
    assert_eq!(
        array_elements(&harness, "values"),
        [Value::Int(1), Value::Int(2), Value::Int(0), Value::Int(0)]
    );
}

#[test]
fn user_type_runtime_excess_array_initializer_ignores_rightmost_values() {
    let harness =
        run("PROGRAM Main\nVAR values : ARRAY[1..2] OF INT := [1, 2, 3, 4]; END_VAR\nEND_PROGRAM");
    assert_eq!(
        array_elements(&harness, "values"),
        [Value::Int(1), Value::Int(2)]
    );
}

#[test]
fn user_type_runtime_zero_repetition_contributes_no_elements() {
    let harness =
        run("PROGRAM Main\nVAR values : ARRAY[1..2] OF INT := [0(9), 1, 2]; END_VAR\nEND_PROGRAM");
    assert_eq!(
        array_elements(&harness, "values"),
        [Value::Int(1), Value::Int(2)]
    );
}

#[test]
fn user_type_runtime_nested_repetition_expands_in_source_order() {
    let harness =
        run("PROGRAM Main\nVAR values : ARRAY[1..8] OF INT := [2(2(1, 2))]; END_VAR\nEND_PROGRAM");
    assert_eq!(
        array_elements(&harness, "values"),
        [
            Value::Int(1),
            Value::Int(2),
            Value::Int(1),
            Value::Int(2),
            Value::Int(1),
            Value::Int(2),
            Value::Int(1),
            Value::Int(2),
        ]
    );
}

#[test]
fn user_type_runtime_multidimensional_initializer_varies_rightmost_index_fastest() {
    let harness = run(r#"
PROGRAM Main
VAR
    values : ARRAY[1..2, 10..11] OF INT := [1, 2, 3, 4];
    a : INT; b : INT; c : INT; d : INT;
END_VAR
a := values[1, 10];
b := values[1, 11];
c := values[2, 10];
d := values[2, 11];
END_PROGRAM
"#);
    assert_eq!(harness.get_output("a"), Some(Value::Int(1)));
    assert_eq!(harness.get_output("b"), Some(Value::Int(2)));
    assert_eq!(harness.get_output("c"), Some(Value::Int(3)));
    assert_eq!(harness.get_output("d"), Some(Value::Int(4)));
}

#[test]
fn user_type_runtime_whole_array_assignment_is_an_independent_copy() {
    let harness = run(r#"
PROGRAM Main
VAR source : ARRAY[1..2] OF INT := [1, 2]; target : ARRAY[1..2] OF INT; observed : INT; END_VAR
target := source;
source[1] := 9;
observed := target[1];
END_PROGRAM
"#);
    assert_eq!(harness.get_output("observed"), Some(Value::Int(1)));
}

#[test]
fn user_type_runtime_whole_structure_assignment_is_an_independent_copy() {
    let harness = run(r#"
TYPE Point : STRUCT x : INT; y : INT; END_STRUCT; END_TYPE
PROGRAM Main
VAR source : Point := (x := 1, y := 2); target : Point; observed : INT; END_VAR
target := source;
source.x := 9;
observed := target.x;
END_PROGRAM
"#);
    assert_eq!(harness.get_output("observed"), Some(Value::Int(1)));
}

#[test]
fn user_type_runtime_failed_computed_array_read_preserves_destination() {
    let mut harness = TestHarness::from_source(
        r#"
PROGRAM Main
VAR values : ARRAY[1..2] OF INT := [1, 2]; index : INT := 3; result : INT := 9; END_VAR
result := values[index];
END_PROGRAM
"#,
    )
    .expect("computed-index fixture must compile");
    let cycle = harness.cycle();
    assert_eq!(cycle.errors.len(), 1, "{:?}", cycle.errors);
    assert_eq!(harness.get_output("result"), Some(Value::Int(9)));
}

#[test]
fn user_type_runtime_failed_computed_array_write_preserves_every_element() {
    let mut harness = TestHarness::from_source(
        r#"
PROGRAM Main
VAR values : ARRAY[1..2] OF INT := [1, 2]; index : INT := 3; END_VAR
values[index] := 9;
END_PROGRAM
"#,
    )
    .expect("computed-index fixture must compile");
    let cycle = harness.cycle();
    assert_eq!(cycle.errors.len(), 1, "{:?}", cycle.errors);
    assert_eq!(
        array_elements(&harness, "values"),
        [Value::Int(1), Value::Int(2)]
    );
}

#[test]
fn user_type_runtime_array_of_enum_defaults_each_element_to_first_literal() {
    let harness = run(r#"
TYPE Color : (Red, Green); END_TYPE
PROGRAM Main
VAR values : ARRAY[1..2] OF Color; bothRed : BOOL; END_VAR
bothRed := (values[1] = Color#Red) AND (values[2] = Color#Red);
END_PROGRAM
"#);
    assert_eq!(harness.get_output("bothRed"), Some(Value::Bool(true)));
}

#[test]
fn user_type_runtime_array_of_subrange_defaults_each_element_to_lower_bound() {
    let harness = run(r#"
TYPE Limited : INT (4..6); END_TYPE
PROGRAM Main
VAR values : ARRAY[1..2] OF Limited; sum : INT; END_VAR
sum := values[1] + values[2];
END_PROGRAM
"#);
    assert_eq!(harness.get_output("sum"), Some(Value::Int(8)));
}

#[test]
fn user_type_runtime_structure_initializer_precedence_is_recursive() {
    let harness = run(r#"
TYPE
Inner : STRUCT value : INT := 1; flag : BOOL := TRUE; END_STRUCT;
DefaultInner : Inner := (value := 2);
Outer : STRUCT nested : DefaultInner; other : INT := 3; END_STRUCT;
END_TYPE
PROGRAM Main
VAR value : Outer := (nested := (value := 4)); observed : INT; flag : BOOL; other : INT; END_VAR
observed := value.nested.value;
flag := value.nested.flag;
other := value.other;
END_PROGRAM
"#);
    assert_eq!(harness.get_output("observed"), Some(Value::Int(4)));
    assert_eq!(harness.get_output("flag"), Some(Value::Bool(true)));
    assert_eq!(harness.get_output("other"), Some(Value::Int(3)));
}

#[test]
fn user_type_runtime_union_materializes_every_variant_default() {
    let harness = run(r#"
TYPE Choice : UNION count : INT := 7; ready : BOOL := TRUE; END_UNION; END_TYPE
PROGRAM Main
VAR value : Choice; count : INT; ready : BOOL; END_VAR
count := value.count;
ready := value.ready;
END_PROGRAM
"#);
    assert_eq!(harness.get_output("count"), Some(Value::Int(7)));
    assert_eq!(harness.get_output("ready"), Some(Value::Bool(true)));
}

#[test]
fn user_type_runtime_union_partial_initializer_preserves_omitted_variant_default() {
    let harness = run(r#"
TYPE Choice : UNION count : INT := 1; ready : BOOL := TRUE; END_UNION; END_TYPE
PROGRAM Main
VAR value : Choice := (count := 9); count : INT; ready : BOOL; END_VAR
count := value.count;
ready := value.ready;
END_PROGRAM
"#);
    assert_eq!(harness.get_output("count"), Some(Value::Int(9)));
    assert_eq!(harness.get_output("ready"), Some(Value::Bool(true)));
}

#[test]
fn user_type_runtime_union_variant_write_does_not_alias_other_variants() {
    let harness = run(r#"
TYPE Choice : UNION wide : DWORD := DWORD#16#AABBCCDD; low : WORD := WORD#16#1234; END_UNION; END_TYPE
PROGRAM Main
VAR value : Choice; observedWide : DWORD; observedLow : WORD; END_VAR
value.low := WORD#16#5678;
observedWide := value.wide;
observedLow := value.low;
END_PROGRAM
"#);
    assert_eq!(
        harness.get_output("observedWide"),
        Some(Value::DWord(0xAABBCCDD))
    );
    assert_eq!(harness.get_output("observedLow"), Some(Value::Word(0x5678)));
}

#[test]
fn user_type_runtime_whole_union_assignment_is_an_independent_copy() {
    let harness = run(r#"
TYPE Choice : UNION count : INT; ready : BOOL; END_UNION; END_TYPE
PROGRAM Main
VAR
    source : Choice := (count := 1, ready := TRUE);
    target : Choice;
    observedCount : INT;
    observedReady : BOOL;
END_VAR
target := source;
source.count := 9;
source.ready := FALSE;
observedCount := target.count;
observedReady := target.ready;
END_PROGRAM
"#);
    assert_eq!(harness.get_output("observedCount"), Some(Value::Int(1)));
    assert_eq!(harness.get_output("observedReady"), Some(Value::Bool(true)));
}

#[test]
fn user_type_runtime_overlap_fields_share_backing_storage() {
    let harness = run(r#"
TYPE Overlay : STRUCT OVERLAP
    wide AT %B0 : DWORD;
    low AT %B0 : WORD;
END_STRUCT;
END_TYPE
PROGRAM Main
VAR value : Overlay; observed : WORD; observedWide : DWORD; END_VAR
value.wide := DWORD#0;
value.low := WORD#16#1234;
observed := value.low;
observedWide := value.wide;
END_PROGRAM
"#);
    assert_eq!(harness.get_output("observed"), Some(Value::Word(0x1234)));
    assert_eq!(
        harness.get_output("observedWide"),
        Some(Value::DWord(0x1234))
    );
}

#[test]
fn local_initializers_preserve_unnamed_physical_bytes_through_coercion_and_recipes() {
    for type_default in ["", " := (low := BYTE#0, high := BYTE#0)"] {
        let source = format!(
            r#"
TYPE
    Inner : STRUCT
        low AT %B0 : BYTE;
        high AT %B2 : BYTE;
    END_STRUCT{type_default};
    Outer : STRUCT OVERLAP
        raw AT %B0 : DWORD;
        inner AT %B0 : Inner;
    END_STRUCT;
END_TYPE
VAR_GLOBAL source : Outer; observed : DWORD; END_VAR
FUNCTION CopyGap : DWORD
VAR
    current : Inner := (low := BYTE#0, high := BYTE#0);
    roundTrip : Outer;
END_VAR
roundTrip.inner := current;
CopyGap := roundTrip.raw;
END_FUNCTION
PROGRAM Main
source.raw := DWORD#16#00CCBBAA;
observed := CopyGap();
END_PROGRAM
"#
        );
        let session = CompileSession::from_source(&source);
        let mut module = session
            .build_bytecode_module()
            .expect("valid initializer source builds STBC");
        rewrite_initializer_to_load_global_member(
            &mut module,
            "source",
            "inner",
            "CopyGap",
            "current",
            if type_default.is_empty() {
                opcodes::COERCE_INIT_VALUE
            } else {
                opcodes::APPLY_INIT_VALUE
            },
        );
        module
            .validate()
            .expect("runtime-dependent staged initializer is admitted STBC");
        let bytes = module.encode().expect("encode patched initializer STBC");
        let mut harness =
            TestHarness::from_source(&source).expect("valid initializer source builds a runtime");
        harness
            .runtime_mut()
            .apply_bytecode_bytes(&bytes, None)
            .expect("admit encoded patched STBC initializer");
        let cycle = harness.cycle();
        assert!(cycle.errors.is_empty(), "{:?}", cycle.errors);
        assert_eq!(
            harness.runtime().storage().get_global("observed"),
            Some(&Value::DWord(0x00ccbbaa))
        );
    }
}

#[test]
fn user_type_runtime_invalid_excess_initializer_is_not_hidden() {
    let error = CompileSession::from_source(
        "PROGRAM Main\nVAR values : ARRAY[1..2] OF INT := [1, 2, missing]; END_VAR\nEND_PROGRAM",
    )
    .build_runtime()
    .expect_err("invalid excess initializer expression must reject compilation");
    assert!(
        error.to_string().contains("missing"),
        "primary missing-name diagnostic must remain visible: {error}"
    );
}
