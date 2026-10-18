//! The compiler writes LINE_TABLE for a module built from ST source
//! (specification 12 §6.13, §7.5.1).

use trust_runtime::bytecode::{
    BytecodeVersion, LineFile, LineStatement, PouKind, SectionData, SectionId,
    LINE_TABLE_SECTION_ID,
};
use trust_runtime::harness::CompileSession;

/// One source file with an IF, a FOR and a function call. Every line starts at
/// column 1 (spec 12 §6.13).
const SOURCE: &str = concat!(
    "FUNCTION Add : INT\n",    // 1
    "VAR_INPUT\n",             // 2
    "x : INT;\n",              // 3
    "END_VAR\n",               // 4
    "Add := x + INT#1;\n",     // 5
    "END_FUNCTION\n",          // 6
    "PROGRAM Main\n",          // 7
    "VAR\n",                   // 8
    "f : BOOL;\n",             // 9
    "i : INT;\n",              // 10
    "END_VAR\n",               // 11
    "f := TRUE;\n",            // 12
    "IF f THEN\n",             // 13
    "i := 1;\n",               // 14
    "END_IF;\n",               // 15
    "FOR i := 0 TO 1 DO\n",    // 16
    "f := FALSE;\n",           // 17
    "END_FOR;\n",              // 18
    "i := Add(x := INT#1);\n", // 19
    "END_PROGRAM\n",           // 20
);

/// One source file with a REPEAT and a labelled statement: a nested statement
/// shares its start code offset with the statement that owns it (spec 12 §6.13).
const NESTED_SOURCE: &str = concat!(
    "PROGRAM Main\n",    // 1
    "VAR\n",             // 2
    "i : INT;\n",        // 3
    "f : BOOL;\n",       // 4
    "END_VAR\n",         // 5
    "REPEAT\n",          // 6
    "i := i + INT#1;\n", // 7
    "UNTIL i > INT#3\n", // 8
    "END_REPEAT;\n",     // 9
    "Top: f := TRUE;\n", // 10
    "END_PROGRAM\n",     // 11
);

/// The innermost entry that contains a code offset: the last one in table order,
/// exactly as §6.13 resolves a breakpoint or a step.
fn innermost(statements: &[LineStatement], offset: u32) -> Option<&LineStatement> {
    statements.iter().rev().find(|statement| {
        statement.start_code_offset <= offset && offset < statement.end_code_offset
    })
}

#[test]
fn the_compiler_writes_one_line_table_entry_per_statement() {
    // §6.13 writes LINE_TABLE as the optional 0x0011 section for every module the
    // source-free producer builds from ST source, with one entry per DEBUG_MAP
    // statement entry: the statement's code span and its source span, one file entry
    // per file, ordered by ascending `pou_id` and then by `start_code_offset`. The
    // default producer is STBC 3.0 (`build_bytecode_module`), where §11.5 keeps
    // 0x0011 free.
    let module = CompileSession::from_source(SOURCE)
        .build_bytecode_module()
        .expect("the default 3.0 ST producer compiles the program");
    assert_eq!(module.version, BytecodeVersion::SOURCE_FREE);

    let table = module.line_table().expect("LINE_TABLE section");
    let Some(SectionData::PouIndex(pou_index)) = module.section(SectionId::PouIndex) else {
        panic!("module POU index")
    };
    let Some(SectionData::DebugMap(debug_map)) = module.section(SectionId::DebugMap) else {
        panic!("module DEBUG_MAP")
    };
    let Some(SectionData::StringTable(strings)) = module.section(SectionId::StringTable) else {
        panic!("module STRING_TABLE")
    };
    let Some(SectionData::DebugStringTable(debug_strings)) =
        module.section(SectionId::DebugStringTable)
    else {
        panic!("module DEBUG_STRING_TABLE")
    };
    let named_pou = |kind: PouKind, name: &str| {
        let idx = strings
            .entries
            .iter()
            .position(|entry| entry.as_str() == name)
            .expect("POU name in STRING_TABLE") as u32;
        pou_index
            .entries
            .iter()
            .find(|entry| entry.kind == kind && entry.name_idx == idx)
            .map(|entry| entry.id)
            .expect("POU identity")
    };
    let main = named_pou(PouKind::Program, "Main");
    let add = named_pou(PouKind::Function, "Add");
    assert_ne!(main, add);

    // §6.13: one file, its label the fallback `file_<file_id>` of §7.5.1.4, and the
    // entry carries that label index only.
    assert_eq!(table.files, vec![LineFile { path_idx: 0 }]);
    assert_eq!(
        debug_strings.entries[table.files[0].path_idx as usize].as_str(),
        "file_0"
    );

    // The source spans, read off the source above: the statement's first byte and the
    // byte one past its last byte, so an IF and a FOR end on their END_IF/END_FOR
    // line and not on the line after it. The `;` of the IF and the FOR is outside
    // the statement node, as it is for the lowering's own statement range, so those
    // spans end at the keyword.
    let spans: Vec<(u32, u32, u32, u32, u32, u32)> = table
        .statements
        .iter()
        .map(|statement| {
            (
                statement.pou_id,
                statement.start_line,
                statement.start_column,
                statement.end_line,
                statement.end_column,
                statement.file_index,
            )
        })
        .collect();
    assert_eq!(
        spans,
        vec![
            (main, 12, 1, 12, 11, 0), // f := TRUE;
            (main, 13, 1, 15, 7, 0),  // IF f THEN ... END_IF
            (main, 14, 1, 14, 8, 0),  // i := 1;
            (main, 16, 1, 18, 8, 0),  // FOR i := 0 TO 1 DO ... END_FOR
            (main, 17, 1, 17, 12, 0), // f := FALSE;
            (main, 19, 1, 19, 22, 0), // i := Add(x := INT#1);
            (add, 5, 1, 5, 18, 0),    // Add := x + INT#1;
        ]
    );

    // §6.13: one entry per DEBUG_MAP statement entry, the same statements in the
    // specification's order, so a statement start has exactly one source location.
    let mut expected: Vec<(u32, u32)> = debug_map
        .entries
        .iter()
        .filter(|entry| entry.kind == 0)
        .map(|entry| (entry.pou_id, entry.code_offset))
        .collect();
    expected.sort_unstable();
    let written: Vec<(u32, u32)> = table
        .statements
        .iter()
        .map(|statement| (statement.pou_id, statement.start_code_offset))
        .collect();
    assert_eq!(written, expected);

    // §6.13: a code span is non-empty and lies inside its POU's body.
    for statement in &table.statements {
        assert!(statement.start_code_offset < statement.end_code_offset);
    }
}

#[test]
fn nested_statements_may_share_a_start_offset() {
    // §6.13: a REPEAT's first instruction is its first body statement's and a label
    // that owns a statement emits its own entry, so two statements may share a start
    // code offset. They must nest, they are ordered outermost first, and a code
    // offset belongs to the innermost (last) entry whose span contains it.
    let module = CompileSession::from_source(NESTED_SOURCE)
        .build_bytecode_module()
        .expect("the default 3.0 ST producer compiles the program");
    let bytes = module.encode().expect("encode");
    let decoded = trust_runtime::bytecode::BytecodeModule::decode(&bytes)
        .expect("decode the compiled module");
    decoded.validate().expect("the two sections agree");
    let table = decoded.line_table().expect("LINE_TABLE section");

    // The four statements of `Main`, read off the source above: the REPEAT (line 6),
    // its body `i := i + INT#1;` (line 7), the label `Top:` (line 10, column 1) and
    // the labelled `f := TRUE;` (line 10, after `Top: `, column 6).
    assert_eq!(table.statements.len(), 4);
    let repeat = &table.statements[0];
    let body = &table.statements[1];
    let label = &table.statements[2];
    let labelled = &table.statements[3];
    assert_eq!(repeat.start_line, 6);
    assert_eq!(body.start_line, 7);
    assert_eq!(label.start_line, 10);
    assert_eq!(labelled.start_line, 10);
    assert_eq!(label.start_column, 1);
    assert_eq!(labelled.start_column, 6);

    // §6.13: a REPEAT and its first body statement share a start, and the REPEAT's
    // span contains its body's because it also holds the UNTIL check.
    assert_eq!(repeat.start_code_offset, body.start_code_offset);
    assert!(repeat.end_code_offset > body.end_code_offset);
    // §6.13: a label owns exactly the span of its statement, so the two share both
    // ends, and the label comes first as the outer entry.
    assert_eq!(label.start_code_offset, labelled.start_code_offset);
    assert_eq!(label.end_code_offset, labelled.end_code_offset);
    assert_eq!(label.end_line, labelled.end_line);
    assert_eq!(label.end_column, labelled.end_column);

    // §6.13: the innermost containing entry owns the code offset, so the shared
    // REPEAT start resolves to its body statement and the label start to the
    // labelled statement, never to the outer statement.
    let resolved_repeat = innermost(&table.statements, repeat.start_code_offset)
        .expect("the REPEAT start belongs to a statement");
    assert_eq!(resolved_repeat.start_line, 7);
    let resolved_label = innermost(&table.statements, label.start_code_offset)
        .expect("the label start belongs to a statement");
    assert_eq!(resolved_label.start_column, 6);

    // §6.13: the shared starts still round-trip through the wire format.
    let reencoded = decoded.encode().expect("re-encode");
    assert_eq!(reencoded, bytes);
}

#[test]
fn a_1x_build_never_carries_a_line_table() {
    // §6.13/§11.5: LINE_TABLE is the source-free `0x0011` section, so the 1.x
    // producer writes none even though the same ST source still emits DEBUG_MAP.
    let module = CompileSession::from_source(SOURCE)
        .build_bytecode_module_for_version(BytecodeVersion::LEGACY)
        .expect("the 1.x ST producer compiles the program");
    assert_eq!(module.version, BytecodeVersion::LEGACY);
    assert!(module.line_table().is_none());
    assert!(module
        .sections
        .iter()
        .all(|section| section.id != LINE_TABLE_SECTION_ID));
    let Some(SectionData::DebugMap(debug_map)) = module.section(SectionId::DebugMap) else {
        panic!("the 1.x producer still writes DEBUG_MAP")
    };
    assert!(debug_map.entries.iter().any(|entry| entry.kind == 0));
}
