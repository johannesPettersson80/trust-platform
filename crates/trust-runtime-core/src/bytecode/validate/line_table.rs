use super::*;

/// Validate the LINE_TABLE section's own references and its DEBUG_MAP agreement.
///
/// Spec 12 §6.13: a statement's code span lies inside its POU's body and starts and
/// ends at decoded instructions, or at the exact end of that body; `file_index`
/// indexes the file table; each file's label resolves in DEBUG_STRING_TABLE, is
/// non-empty, and ascends by label bytes without repeating. §6.13 and §10: when
/// both sections are present they describe the same statements, with the same POU,
/// code offset, file label, line and column; a missing counterpart or a
/// disagreement rejects the candidate.
pub(super) fn validate_line_table(
    tables: &ValidationContext<'_>,
    bodies: &[u8],
    table: &LineTable,
    debug_strings: &StringTable,
    debug_map: Option<&DebugMap>,
    budget: &mut ValidationBudget,
) -> Result<(), BytecodeError> {
    let mut previous_label: Option<&str> = None;
    for file in &table.files {
        budget.work(1)?;
        ensure_string_index(debug_strings, file.path_idx)?;
        let label = debug_strings
            .entries
            .get(file.path_idx as usize)
            .ok_or(BytecodeError::InvalidIndex {
                kind: "debug string".into(),
                index: file.path_idx,
            })?
            .as_str();
        if label.is_empty() {
            return Err(line_table_fault("LINE_TABLE file label is empty"));
        }
        if let Some(previous) = previous_label
            && previous.as_bytes() >= label.as_bytes()
        {
            return Err(line_table_fault(
                "LINE_TABLE file labels are not unique and ascending",
            ));
        }
        previous_label = Some(label);
    }

    // A source-built container reaches the validator without the decoder, so the
    // decoder's ordering and nesting rule of §6.13 is checked here as well.
    table.check_statement_order()?;

    let mut boundaries: Vec<u32> = Vec::new();
    let mut boundaries_pou: Option<u32> = None;
    for statement in &table.statements {
        budget.work(1)?;
        if table.files.get(statement.file_index as usize).is_none() {
            return Err(line_table_fault(
                "LINE_TABLE statement file index is outside the file table",
            ));
        }
        let pou = tables
            .pou(statement.pou_id, budget)?
            .ok_or(BytecodeError::InvalidPouId(statement.pou_id))?;
        let body_end = pou
            .code_offset
            .checked_add(pou.code_length)
            .ok_or_else(|| BytecodeError::from(RejectionReason::PouCodeRangeOverflow))?;
        if statement.start_code_offset < pou.code_offset
            || statement.end_code_offset > body_end
            || statement.start_code_offset >= statement.end_code_offset
        {
            return Err(line_table_fault(
                "LINE_TABLE statement code span is outside its POU body",
            ));
        }
        if boundaries_pou != Some(statement.pou_id) {
            boundaries = instruction_boundaries(pou, bodies, budget)?;
            boundaries_pou = Some(statement.pou_id);
        }
        if !is_instruction_boundary(&boundaries, statement.start_code_offset, budget)? {
            return Err(line_table_fault(
                "LINE_TABLE statement start is not an instruction",
            ));
        }
        if !is_instruction_boundary(&boundaries, statement.end_code_offset, budget)? {
            return Err(line_table_fault(
                "LINE_TABLE statement end is not an instruction or the exact end of the POU body",
            ));
        }
    }

    if let Some(debug_map) = debug_map {
        validate_line_table_agreement(table, debug_strings, debug_map, budget)?;
    }
    Ok(())
}

/// Every instruction offset of one POU body, plus the exact end of that body.
///
/// Spec 12 §6.13 admits a statement start at a decoded instruction and a statement
/// end at a decoded instruction or the exact end of the body, so the boundary set
/// is projected once per POU and searched by offset.
fn instruction_boundaries(
    pou: &PouEntry,
    bodies: &[u8],
    budget: &mut ValidationBudget,
) -> Result<Vec<u32>, BytecodeError> {
    let start = pou.code_offset as usize;
    let end = start
        .checked_add(pou.code_length as usize)
        .ok_or_else(|| BytecodeError::from(RejectionReason::PouCodeRangeOverflow))?;
    let code = bodies
        .get(start..end)
        .ok_or_else(|| BytecodeError::from(RejectionReason::PouCodeOutOfBounds))?;
    budget.work(code.len())?;
    let mut boundaries = Vec::new();
    budget.push(&mut boundaries, pou.code_offset)?;
    let mut reader = BytecodeReader::new(code);
    while reader.remaining() > 0 {
        let opcode = reader.read_u8()?;
        let width =
            crate::vm::opcode_operand_len(opcode).ok_or(BytecodeError::InvalidOpcode(opcode))?;
        reader.read_bytes(width)?;
        let offset = u32::try_from(reader.pos())
            .ok()
            .and_then(|position| pou.code_offset.checked_add(position))
            .ok_or_else(|| BytecodeError::from(RejectionReason::CodePositionOverflow))?;
        budget.push(&mut boundaries, offset)?;
    }
    Ok(boundaries)
}

fn is_instruction_boundary(
    boundaries: &[u32],
    offset: u32,
    budget: &mut ValidationBudget,
) -> Result<bool, BytecodeError> {
    Ok(budget
        .search_by(boundaries, |value, _| Ok(value.cmp(&offset)))?
        .is_ok())
}

/// Check one-to-one agreement between the line table and the DEBUG_MAP (§6.13, §10).
fn validate_line_table_agreement(
    table: &LineTable,
    debug_strings: &StringTable,
    debug_map: &DebugMap,
    budget: &mut ValidationBudget,
) -> Result<(), BytecodeError> {
    let mut debug_statements = Vec::new();
    for (position, entry) in debug_map.entries.iter().enumerate() {
        budget.work(1)?;
        if entry.kind == 0 {
            budget.push(&mut debug_statements, position)?;
        }
    }
    if debug_statements.len() != table.statements.len() {
        return Err(line_table_fault(
            "LINE_TABLE and DEBUG_MAP describe different statement sets",
        ));
    }
    // §6.13 identifies a statement by its POU, code offset and source position. Order
    // both sides by that identity before the one-to-one comparison, so a nested pair
    // that shares a code offset is matched by its source position and never rejected
    // on the order a sort happens to leave equal offsets in.
    budget.sort_by(&mut debug_statements, &mut |left, right, _| {
        Ok(debug_position(&debug_map.entries[*left])
            .cmp(&debug_position(&debug_map.entries[*right])))
    })?;
    let mut line_statements = Vec::new();
    for position in 0..table.statements.len() {
        budget.push(&mut line_statements, position)?;
    }
    budget.sort_by(&mut line_statements, &mut |left, right, _| {
        Ok(line_position(&table.statements[*left]).cmp(&line_position(&table.statements[*right])))
    })?;
    for (line_index, debug_index) in line_statements.into_iter().zip(debug_statements) {
        budget.work(1)?;
        let line_statement = &table.statements[line_index];
        let entry = &debug_map.entries[debug_index];
        if entry.pou_id != line_statement.pou_id
            || entry.code_offset != line_statement.start_code_offset
        {
            return Err(line_table_fault(
                "LINE_TABLE and DEBUG_MAP disagree on a statement code position",
            ));
        }
        let file = table
            .files
            .get(line_statement.file_index as usize)
            .ok_or_else(|| {
                line_table_fault("LINE_TABLE statement file index is outside the file table")
            })?;
        let line_label = debug_strings.entries.get(file.path_idx as usize);
        let debug_label = debug_strings.entries.get(entry.file_idx as usize);
        if line_label != debug_label
            || entry.line != line_statement.start_line
            || entry.column != line_statement.start_column
        {
            return Err(line_table_fault(
                "LINE_TABLE and DEBUG_MAP disagree on a source position",
            ));
        }
    }
    Ok(())
}

fn debug_position(entry: &DebugEntry) -> (u32, u32, u32, u32) {
    (entry.pou_id, entry.code_offset, entry.line, entry.column)
}

fn line_position(statement: &LineStatement) -> (u32, u32, u32, u32) {
    (
        statement.pou_id,
        statement.start_code_offset,
        statement.start_line,
        statement.start_column,
    )
}

fn line_table_fault(message: &'static str) -> BytecodeError {
    BytecodeError::section_static(message)
}
