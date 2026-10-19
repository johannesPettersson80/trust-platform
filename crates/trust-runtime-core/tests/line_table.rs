//! LINE_TABLE decoding, validation and optionality (specification 12 §6.13, §10).

#[path = "common/bytecode_helpers.rs"]
mod bytecode_helpers;

use bytecode_helpers::module_with_debug;
use trust_runtime_core::bytecode::{
    AccessBindings, BytecodeModule, BytecodeVersion, ConstructionRoots, DebugEntry,
    InitializerIndex, LineFile, LineStatement, LineTable, Section, SectionData, SectionId,
    StorageLayout, HEADER_FLAG_CRC32, LINE_TABLE_SECTION_ID,
};
use trust_runtime_core::debug::StatementSpan;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::value::{Duration, Value};
use trust_runtime_core::vm::{PreparationLimits, PreparedModule};

/// The 3.0 bounded-C corpus artifact, produced by
/// `cargo run --locked -p trust-runtime --example bounded_c_fixtures -- crates/trust-runtime/tests/fixtures/bounded-c`.
const SCALAR_3_0: &[u8] =
    include_bytes!("../../trust-runtime/tests/fixtures/bounded-c/scalar/program.stbc");

/// The line table for two statements on the two lines of a file: one file label and
/// the code and source spans of both statements (spec 12 §6.13).
fn line_table() -> LineTable {
    LineTable {
        files: vec![LineFile { path_idx: 0 }],
        statements: vec![
            LineStatement {
                pou_id: 1,
                start_code_offset: 0,
                end_code_offset: 1,
                file_index: 0,
                start_line: 1,
                start_column: 1,
                end_line: 1,
                end_column: 2,
            },
            LineStatement {
                pou_id: 1,
                start_code_offset: 1,
                end_code_offset: 2,
                file_index: 0,
                start_line: 2,
                start_column: 1,
                end_line: 2,
                end_column: 2,
            },
        ],
    }
}

/// A valid source-free module with two statements: POU_BODIES holds two NOPs,
/// DEBUG_MAP names both statement starts and LINE_TABLE describes the same two
/// statements. The empty construction layout means the resource names no task.
fn source_free_two_statement_module(version: BytecodeVersion) -> BytecodeModule {
    assert!(version.is_source_free(), "LINE_TABLE lives in 2.0/3.0");
    let mut module = module_with_debug();
    module.version = version;
    module.flags = HEADER_FLAG_CRC32;
    let Some(SectionData::PouBodies(bodies)) = module.section_mut(SectionId::PouBodies) else {
        panic!("fixture POU bodies")
    };
    bodies.push(0x00);
    let Some(SectionData::PouIndex(index)) = module.section_mut(SectionId::PouIndex) else {
        panic!("fixture POU index")
    };
    index.entries[0].code_length = 2;
    let Some(SectionData::DebugMap(map)) = module.section_mut(SectionId::DebugMap) else {
        panic!("fixture DEBUG_MAP")
    };
    map.entries.push(DebugEntry {
        pou_id: 1,
        code_offset: 1,
        file_idx: 0,
        line: 2,
        column: 1,
        kind: 0,
    });
    let Some(SectionData::ResourceMeta(meta)) = module.section_mut(SectionId::ResourceMeta) else {
        panic!("fixture resource metadata")
    };
    // The base resource's task names a program root and a single-trigger global,
    // and the empty construction layout declares neither.
    meta.resources[0].tasks.clear();
    module.sections.extend([
        Section {
            id: SectionId::StorageLayout.as_raw(),
            flags: 0,
            data: SectionData::StorageLayout(StorageLayout::default()),
        },
        Section {
            id: SectionId::ConstructionRoots.as_raw(),
            flags: 0,
            data: SectionData::ConstructionRoots(ConstructionRoots::default()),
        },
        Section {
            id: SectionId::Initializers.as_raw(),
            flags: 0,
            data: SectionData::Initializers(InitializerIndex::default()),
        },
        Section {
            id: SectionId::AccessBindings.as_raw(),
            flags: 0,
            data: SectionData::AccessBindings(AccessBindings::default()),
        },
        Section {
            id: LINE_TABLE_SECTION_ID,
            flags: 0,
            data: SectionData::LineTable(line_table()),
        },
    ]);
    module
}

/// A module whose second POU covers the second statement, so a fault can name
/// another POU that really exists.
fn two_pou_module() -> BytecodeModule {
    let mut module = source_free_two_statement_module(BytecodeVersion::SOURCE_FREE);
    let Some(SectionData::PouIndex(index)) = module.section_mut(SectionId::PouIndex) else {
        panic!("fixture POU index")
    };
    let mut second = index.entries[0].clone();
    second.id = 2;
    second.code_offset = 1;
    second.code_length = 1;
    index.entries.push(second);
    module
}

/// A module whose single instruction is a five-byte JMP to the exact end of the
/// body, so instruction boundaries are the start and that end only.
fn jump_module() -> BytecodeModule {
    let mut module = source_free_two_statement_module(BytecodeVersion::SOURCE_FREE);
    let Some(SectionData::PouBodies(bodies)) = module.section_mut(SectionId::PouBodies) else {
        panic!("fixture POU bodies")
    };
    *bodies = vec![0x02, 0x00, 0x00, 0x00, 0x00];
    let Some(SectionData::PouIndex(index)) = module.section_mut(SectionId::PouIndex) else {
        panic!("fixture POU index")
    };
    index.entries[0].code_length = 5;
    let Some(SectionData::DebugMap(map)) = module.section_mut(SectionId::DebugMap) else {
        panic!("fixture DEBUG_MAP")
    };
    map.entries.truncate(1);
    let Some(table) = module.line_table_mut() else {
        panic!("fixture LINE_TABLE")
    };
    table.statements.truncate(1);
    module
}

/// Serialize a line table exactly as §6.13 lays it out, so a test can cut the wire
/// bytes a decoder must refuse.
fn wire(table: &LineTable) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(
        &u32::try_from(table.files.len())
            .expect("file count")
            .to_le_bytes(),
    );
    for file in &table.files {
        out.extend_from_slice(&file.path_idx.to_le_bytes());
    }
    out.extend_from_slice(
        &u32::try_from(table.statements.len())
            .expect("statement count")
            .to_le_bytes(),
    );
    for statement in &table.statements {
        for field in [
            statement.pou_id,
            statement.start_code_offset,
            statement.end_code_offset,
            statement.file_index,
            statement.start_line,
            statement.start_column,
            statement.end_line,
            statement.end_column,
        ] {
            out.extend_from_slice(&field.to_le_bytes());
        }
    }
    out
}

/// Decode the module's bytes and validate them, requiring the refusal to be the
/// section error §6.13 names.
fn assert_refused(module: &BytecodeModule, expected: &str) {
    let bytes = module.encode().expect("a mutated line table still encodes");
    let error = match BytecodeModule::decode(&bytes) {
        Err(error) => error,
        Ok(decoded) => decoded
            .validate()
            .expect_err("a faulty line table must be refused"),
    };
    assert_eq!(
        error.to_string(),
        format!("invalid section data: {expected}")
    );
}

#[test]
fn line_table_round_trips_its_entries() {
    // §6.13: LINE_TABLE is the optional 0x0011 section of the source-free
    // containers 2.0 and 3.0. Each file carries its DEBUG_STRING_TABLE label index
    // only, and each statement carries its code span (start, end) and its source
    // span (file index, start line/column, end line/column).
    for version in [
        BytecodeVersion::SOURCE_FREE,
        BytecodeVersion::SOURCE_FREE_V2,
    ] {
        let module = source_free_two_statement_module(version);
        let bytes = module.encode().expect("encode");
        let decoded = BytecodeModule::decode(&bytes).expect("decode");
        assert_eq!(decoded.version, version);
        decoded.validate().expect("the two sections agree");

        let table = decoded.line_table().expect("LINE_TABLE section");
        assert_eq!(table.files, vec![LineFile { path_idx: 0 }]);
        assert_eq!(
            table.statements,
            vec![
                LineStatement {
                    pou_id: 1,
                    start_code_offset: 0,
                    end_code_offset: 1,
                    file_index: 0,
                    start_line: 1,
                    start_column: 1,
                    end_line: 1,
                    end_column: 2,
                },
                LineStatement {
                    pou_id: 1,
                    start_code_offset: 1,
                    end_code_offset: 2,
                    file_index: 0,
                    start_line: 2,
                    start_column: 1,
                    end_line: 2,
                    end_column: 2,
                },
            ]
        );
        let Some(SectionData::DebugStringTable(strings)) =
            decoded.section(SectionId::DebugStringTable)
        else {
            panic!("fixture DEBUG_STRING_TABLE")
        };
        assert_eq!(
            strings.entries[table.files[0].path_idx as usize].as_str(),
            "File"
        );
        // §6.13: a decoded table re-encodes to the same bytes.
        let reencoded = decoded.encode().expect("re-encode");
        assert_eq!(reencoded, bytes);
    }
}

#[test]
fn every_line_table_fault_is_refused() {
    // §6.13: a truncated or misordered table is refused, as is an entry outside its
    // POU's code or a file index outside the file table; §6.13 and §10 refuse every
    // disagreement with DEBUG_MAP.
    let mut truncated = source_free_two_statement_module(BytecodeVersion::SOURCE_FREE);
    let mut payload = wire(&line_table());
    payload.truncate(payload.len() - 8);
    let Some(section) = truncated
        .sections
        .iter_mut()
        .find(|section| section.id == LINE_TABLE_SECTION_ID)
    else {
        panic!("fixture LINE_TABLE")
    };
    section.data = SectionData::Raw(payload);
    assert_refused(
        &truncated,
        "LINE_TABLE statement count exceeds section bounds",
    );

    let mut out_of_order = source_free_two_statement_module(BytecodeVersion::SOURCE_FREE);
    let Some(table) = out_of_order.line_table_mut() else {
        panic!("fixture LINE_TABLE")
    };
    table.statements.swap(0, 1);
    assert_refused(&out_of_order, "LINE_TABLE statements are out of order");

    // §6.13: two statements that share a start must nest, so the later one's code
    // span must lie inside the earlier one's.
    let mut shared_start = source_free_two_statement_module(BytecodeVersion::SOURCE_FREE);
    let Some(table) = shared_start.line_table_mut() else {
        panic!("fixture LINE_TABLE")
    };
    table.statements[1].start_code_offset = 0;
    assert_refused(
        &shared_start,
        "LINE_TABLE statements that share a start do not nest",
    );

    // §6.13: the end offsets of a shared start must descend; a third entry is
    // compared against its immediate predecessor.
    let mut wrong_end_order = source_free_two_statement_module(BytecodeVersion::SOURCE_FREE);
    let Some(table) = wrong_end_order.line_table_mut() else {
        panic!("fixture LINE_TABLE")
    };
    table.statements[1].start_code_offset = 0;
    table.statements[1].end_code_offset = 3;
    assert_refused(
        &wrong_end_order,
        "LINE_TABLE statements that share a start do not nest",
    );

    let mut empty_code_span = source_free_two_statement_module(BytecodeVersion::SOURCE_FREE);
    let Some(table) = empty_code_span.line_table_mut() else {
        panic!("fixture LINE_TABLE")
    };
    table.statements[1].end_code_offset = 1;
    assert_refused(&empty_code_span, "LINE_TABLE statement code span is empty");

    let mut not_one_based = source_free_two_statement_module(BytecodeVersion::SOURCE_FREE);
    let Some(table) = not_one_based.line_table_mut() else {
        panic!("fixture LINE_TABLE")
    };
    table.statements[1].start_line = 0;
    assert_refused(
        &not_one_based,
        "LINE_TABLE statement position is not 1-based",
    );

    let mut empty_source_span = source_free_two_statement_module(BytecodeVersion::SOURCE_FREE);
    let Some(table) = empty_source_span.line_table_mut() else {
        panic!("fixture LINE_TABLE")
    };
    table.statements[1].end_line = 2;
    table.statements[1].end_column = 1;
    assert_refused(
        &empty_source_span,
        "LINE_TABLE statement source span is empty",
    );

    let mut unknown_file = source_free_two_statement_module(BytecodeVersion::SOURCE_FREE);
    let Some(table) = unknown_file.line_table_mut() else {
        panic!("fixture LINE_TABLE")
    };
    table.statements[1].file_index = 9;
    assert_refused(
        &unknown_file,
        "LINE_TABLE statement file index is outside the file table",
    );

    let mut outside_body = source_free_two_statement_module(BytecodeVersion::SOURCE_FREE);
    let Some(table) = outside_body.line_table_mut() else {
        panic!("fixture LINE_TABLE")
    };
    table.statements[1].start_code_offset = 5;
    table.statements[1].end_code_offset = 6;
    assert_refused(
        &outside_body,
        "LINE_TABLE statement code span is outside its POU body",
    );

    let mut mid_instruction = jump_module();
    let Some(table) = mid_instruction.line_table_mut() else {
        panic!("fixture LINE_TABLE")
    };
    table.statements[0].start_code_offset = 2;
    table.statements[0].end_code_offset = 5;
    assert_refused(
        &mid_instruction,
        "LINE_TABLE statement start is not an instruction",
    );

    let mut split_instruction = jump_module();
    let Some(table) = split_instruction.line_table_mut() else {
        panic!("fixture LINE_TABLE")
    };
    table.statements[0].end_code_offset = 3;
    assert_refused(
        &split_instruction,
        "LINE_TABLE statement end is not an instruction or the exact end of the POU body",
    );

    let mut missing_counterpart = source_free_two_statement_module(BytecodeVersion::SOURCE_FREE);
    let Some(SectionData::DebugMap(map)) = missing_counterpart.section_mut(SectionId::DebugMap)
    else {
        panic!("fixture DEBUG_MAP")
    };
    map.entries.truncate(1);
    assert_refused(
        &missing_counterpart,
        "LINE_TABLE and DEBUG_MAP describe different statement sets",
    );

    let mut another_pou = two_pou_module();
    let Some(SectionData::DebugMap(map)) = another_pou.section_mut(SectionId::DebugMap) else {
        panic!("fixture DEBUG_MAP")
    };
    map.entries[1].pou_id = 2;
    assert_refused(
        &another_pou,
        "LINE_TABLE and DEBUG_MAP disagree on a statement code position",
    );

    let mut another_offset = source_free_two_statement_module(BytecodeVersion::SOURCE_FREE);
    let Some(SectionData::DebugMap(map)) = another_offset.section_mut(SectionId::DebugMap) else {
        panic!("fixture DEBUG_MAP")
    };
    map.entries[1].code_offset = 2;
    assert_refused(
        &another_offset,
        "LINE_TABLE and DEBUG_MAP disagree on a statement code position",
    );

    let mut another_line = source_free_two_statement_module(BytecodeVersion::SOURCE_FREE);
    let Some(SectionData::DebugMap(map)) = another_line.section_mut(SectionId::DebugMap) else {
        panic!("fixture DEBUG_MAP")
    };
    map.entries[1].line = 5;
    assert_refused(
        &another_line,
        "LINE_TABLE and DEBUG_MAP disagree on a source position",
    );

    let mut another_column = source_free_two_statement_module(BytecodeVersion::SOURCE_FREE);
    let Some(SectionData::DebugMap(map)) = another_column.section_mut(SectionId::DebugMap) else {
        panic!("fixture DEBUG_MAP")
    };
    map.entries[1].column = 2;
    assert_refused(
        &another_column,
        "LINE_TABLE and DEBUG_MAP disagree on a source position",
    );

    let mut another_label = source_free_two_statement_module(BytecodeVersion::SOURCE_FREE);
    let Some(SectionData::DebugStringTable(strings)) =
        another_label.section_mut(SectionId::DebugStringTable)
    else {
        panic!("fixture DEBUG_STRING_TABLE")
    };
    strings.entries.push("Other".into());
    let Some(SectionData::DebugMap(map)) = another_label.section_mut(SectionId::DebugMap) else {
        panic!("fixture DEBUG_MAP")
    };
    map.entries[1].file_idx = 1;
    assert_refused(
        &another_label,
        "LINE_TABLE and DEBUG_MAP disagree on a source position",
    );
}

#[test]
fn a_1x_module_never_carries_a_line_table() {
    // §6.13/§11.5: LINE_TABLE is standardized as 0x0011 only in the source-free
    // versions 2.0 and 3.0, so a 1.x container never carries one and 0x0011 stays an
    // unknown optional extension there (§4.2).
    let mut module = module_with_debug();
    assert!(module.line_table().is_none());
    module.sections.push(Section {
        id: LINE_TABLE_SECTION_ID,
        flags: 0,
        data: SectionData::Raw(vec![1, 2, 3, 4]),
    });
    let bytes = module.encode().expect("a 1.x extension still encodes");
    let decoded = BytecodeModule::decode(&bytes).expect("decode");
    decoded.validate().expect("a 1.x extension is ignored");
    assert!(decoded.line_table().is_none());
    let Some(section) = decoded
        .sections
        .iter()
        .find(|section| section.id == LINE_TABLE_SECTION_ID)
    else {
        panic!("extension section")
    };
    assert_eq!(section.data, SectionData::Raw(vec![1, 2, 3, 4]));

    // A 1.x producer never writes the section, and a hand-built 1.x container that
    // claims the payload is refused instead of resolved.
    let mut forged = module_with_debug();
    forged.sections.push(Section {
        id: LINE_TABLE_SECTION_ID,
        flags: 0,
        data: SectionData::LineTable(line_table()),
    });
    assert!(forged.validate().is_err());
}

#[test]
fn a_module_without_a_line_table_still_runs() {
    // §6.13: the section is optional, so a module without it decodes and validates
    // exactly as one with it, and the artifact the core executes is unchanged.
    let mut module = source_free_two_statement_module(BytecodeVersion::SOURCE_FREE);
    module
        .sections
        .retain(|section| !matches!(section.data, SectionData::LineTable(_)));
    let bytes = module.encode().expect("encode");
    let decoded = BytecodeModule::decode(&bytes).expect("decode");
    decoded.validate().expect("validate");
    assert!(decoded.line_table().is_none());

    // The 3.0 bounded-C artifact keeps running with the optional section removed.
    let mut artifact = BytecodeModule::decode(SCALAR_3_0).expect("decode 3.0 artifact");
    artifact
        .sections
        .retain(|section| !matches!(section.data, SectionData::LineTable(_)));
    assert!(artifact.line_table().is_none());
    let prepared = PreparedModule::from_decoded(&artifact, PreparationLimits::default())
        .expect("prepare the artifact");
    let mut state = prepared.instantiate(0).expect("instantiate the resource");
    state
        .execute_cycle(Duration::from_millis(10))
        .expect("one cycle runs");
    assert!(state.fault().is_none());
    // §6.13 optionality is proven by the program body running: `Scalars` starts
    // `phase` at INT#0 and its first `CASE` writes INT#1.
    let Some(Value::Instance(plant)) = state.storage().get_global("Plant") else {
        panic!("3.0 artifact program root")
    };
    assert_eq!(
        state.storage().get_instance_var(*plant, "phase"),
        Some(&Value::Int(1))
    );
}

/// A POU-1 statement in file 0: its code span `[code_span.0, code_span.1)` and its 1-based
/// source span from `source_start` up to `source_end` (specification 12 §6.13).
fn statement(
    code_span: (u32, u32),
    source_start: (u32, u32),
    source_end: (u32, u32),
) -> LineStatement {
    LineStatement {
        pou_id: 1,
        start_code_offset: code_span.0,
        end_code_offset: code_span.1,
        file_index: 0,
        start_line: source_start.0,
        start_column: source_start.1,
        end_line: source_end.0,
        end_column: source_end.1,
    }
}

/// The two-statement fixture with a body of three NOPs and the given statements, so a test can
/// lay out spans that cross. Its DEBUG_MAP keeps the fixture's statement starts at code offsets 0
/// and 1, which agree with statements that start there.
fn three_nop_module(statements: Vec<LineStatement>) -> BytecodeModule {
    let mut module = source_free_two_statement_module(BytecodeVersion::SOURCE_FREE);
    let Some(SectionData::PouBodies(bodies)) = module.section_mut(SectionId::PouBodies) else {
        panic!("fixture POU bodies")
    };
    bodies.push(0x00);
    let Some(SectionData::PouIndex(index)) = module.section_mut(SectionId::PouIndex) else {
        panic!("fixture POU index")
    };
    index.entries[0].code_length = 3;
    let Some(table) = module.line_table_mut() else {
        panic!("fixture LINE_TABLE")
    };
    table.statements = statements;
    module
}

#[test]
fn a_line_table_with_crossing_or_reversed_spans_is_refused() {
    // Specification 12 §6.13: the code span and the source span of a statement run forwards
    // and are non-empty, and the code spans of one POU nest or touch but never overlap without
    // nesting. The spans [0, 2) and [1, 3) overlap, and neither contains the other, so an index
    // that answers both statements at offset 1 is wrong.
    let crossing = three_nop_module(vec![
        statement((0, 2), (1, 1), (2, 1)),
        statement((1, 3), (2, 1), (3, 1)),
    ]);
    assert_refused(&crossing, "LINE_TABLE statement code spans cross");

    // Specification 12 §6.13: a code span that ends before it starts is reversed, not empty.
    let reversed_code = three_nop_module(vec![
        statement((0, 1), (1, 1), (2, 1)),
        statement((2, 1), (2, 1), (3, 1)),
    ]);
    assert_refused(&reversed_code, "LINE_TABLE statement code span is reversed");

    // Specification 12 §6.13: a source span that ends before it starts is reversed as well.
    let reversed_source = three_nop_module(vec![
        statement((0, 1), (1, 1), (2, 1)),
        statement((1, 2), (3, 2), (2, 1)),
    ]);
    assert_refused(
        &reversed_source,
        "LINE_TABLE statement source span is reversed",
    );

    // Specification 12 §6.13: a statement nested in another may end with it, and that is not a
    // crossing. The fixture's DEBUG_MAP agrees with these two starts, so the table is accepted.
    let nested = three_nop_module(vec![
        statement((0, 3), (1, 1), (3, 1)),
        statement((1, 3), (2, 1), (3, 1)),
    ]);
    let bytes = nested.encode().expect("encode the nested pair");
    BytecodeModule::decode(&bytes)
        .expect("decode the nested pair")
        .validate()
        .expect("a nested pair is accepted");
}

#[test]
fn a_prepared_module_keeps_the_statement_starts_of_its_line_table() {
    // Specification 12 §6.13: each statement entry names the statement that starts at its code
    // offset, with its source span. The fixture's statements start at offsets 0 and 1 of POU 1
    // and span 1:1-1:2 and 2:1-2:2 of file 0. An index that drops the table answers none, and one
    // that pairs an offset with another statement's span answers the wrong span here.
    let module = source_free_two_statement_module(BytecodeVersion::SOURCE_FREE);
    let prepared = PreparedModule::from_decoded(&module, PreparationLimits::default())
        .expect("prepare the two-statement fixture");
    assert!(prepared.has_statement_index());
    assert_eq!(
        prepared.statement_start(1, 0),
        Some(StatementSpan {
            file_index: 0,
            start: (1, 1),
            end: (1, 2),
        })
    );
    assert_eq!(
        prepared.statement_start(1, 1),
        Some(StatementSpan {
            file_index: 0,
            start: (2, 1),
            end: (2, 2),
        })
    );
    // Specification 12 §6.13: the fixture has no POU 2, so no statement starts there.
    assert_eq!(prepared.statement_start(2, 0), None);
}

#[test]
fn statement_start_is_none_inside_a_statement() {
    // Specification 12 §6.13: a statement starts at its first instruction only. The one
    // statement here spans both NOPs of the body, [0, 2), so offset 1 lies inside it and starts
    // nothing. An index that answers the containing statement at every offset gives its span at
    // offset 1.
    let mut module = source_free_two_statement_module(BytecodeVersion::SOURCE_FREE);
    let Some(table) = module.line_table_mut() else {
        panic!("fixture LINE_TABLE")
    };
    table.statements.truncate(1);
    table.statements[0].end_code_offset = 2;
    table.statements[0].end_line = 2;
    table.statements[0].end_column = 2;
    let Some(SectionData::DebugMap(map)) = module.section_mut(SectionId::DebugMap) else {
        panic!("fixture DEBUG_MAP")
    };
    map.entries.truncate(1);
    let prepared = PreparedModule::from_decoded(&module, PreparationLimits::default())
        .expect("prepare the one-statement fixture");
    assert!(prepared.statement_start(1, 0).is_some());
    assert_eq!(prepared.statement_start(1, 1), None);
}

#[test]
fn the_index_is_charged_before_it_is_reserved() {
    // Specification 34 (preparation charges): the statement index is charged to the preparation
    // budget before it is reserved. The fixture's two statements in one POU take 2 x 104 + 76 =
    // 284 bytes of index at most, and the artifact's LINE_TABLE adds at least that much to the
    // same artifact without one. An allowance one byte short of the total refuses the artifact
    // with PreparationLimit, and the exact total admits it.
    let with_table = source_free_two_statement_module(BytecodeVersion::SOURCE_FREE)
        .encode()
        .expect("encode the fixture");
    let mut without = source_free_two_statement_module(BytecodeVersion::SOURCE_FREE);
    without
        .sections
        .retain(|section| !matches!(section.data, SectionData::LineTable(_)));
    let without_table = without
        .encode()
        .expect("encode the fixture without a table");
    let total = PreparedModule::from_bytes(&with_table, PreparationLimits::default())
        .expect("prepare the fixture")
        .preparation_usage()
        .bytes;
    let table_free = PreparedModule::from_bytes(&without_table, PreparationLimits::default())
        .expect("prepare the fixture without a table")
        .preparation_usage()
        .bytes;
    assert!(total - table_free >= 2 * 104 + 76);
    let short = PreparationLimits {
        max_preparation_bytes: total - 1,
        ..PreparationLimits::default()
    };
    assert!(matches!(
        PreparedModule::from_bytes(&with_table, short),
        Err(RuntimeError::PreparationLimit)
    ));
    let exact = PreparationLimits {
        max_preparation_bytes: total,
        ..PreparationLimits::default()
    };
    assert!(PreparedModule::from_bytes(&with_table, exact).is_ok());
}

#[test]
fn an_artifact_without_a_line_table_has_no_index() {
    // Specification 12 §6.13: LINE_TABLE is optional, and an artifact without one prepares as
    // before. Preparation keeps no statement index, so a debugging request can tell that the
    // artifact has no statement starts and answer the rebuild error rather than an empty answer.
    // A LINE_TABLE with no statements is still a LINE_TABLE: its index exists and starts nothing.
    let mut module = source_free_two_statement_module(BytecodeVersion::SOURCE_FREE);
    module
        .sections
        .retain(|section| !matches!(section.data, SectionData::LineTable(_)));
    let prepared = PreparedModule::from_decoded(&module, PreparationLimits::default())
        .expect("prepare the artifact without a table");
    assert!(!prepared.has_statement_index());
    assert_eq!(prepared.statement_start(1, 0), None);

    let mut empty = source_free_two_statement_module(BytecodeVersion::SOURCE_FREE);
    let Some(table) = empty.line_table_mut() else {
        panic!("fixture LINE_TABLE")
    };
    table.files.clear();
    table.statements.clear();
    let Some(SectionData::DebugMap(map)) = empty.section_mut(SectionId::DebugMap) else {
        panic!("fixture DEBUG_MAP")
    };
    map.entries.clear();
    let prepared = PreparedModule::from_decoded(&empty, PreparationLimits::default())
        .expect("prepare the empty table");
    assert!(prepared.has_statement_index());
    assert_eq!(prepared.statement_start(1, 0), None);
}
