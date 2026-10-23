//! Compare fixed-record reads with the prior scalar decoder, including its cursor.
use super::*;

fn scalar_io_map(
    reader: &mut BytecodeReader<'_>,
    budget: &mut DecodeBudget,
) -> Result<IoMap, BytecodeError> {
    let binding_count = read_bounded_count(reader, 12, "IO_MAP")?;
    let mut bindings = budget.vector(binding_count)?;
    for _ in 0..binding_count {
        let address_str_idx = reader.read_u32()?;
        let ref_idx = reader.read_u32()?;
        let type_id = optional_u32(reader.read_u32()?);
        bindings.push(IoBinding {
            address_str_idx,
            ref_idx,
            type_id,
        });
    }
    Ok(IoMap { bindings })
}

fn scalar_debug_map(
    reader: &mut BytecodeReader<'_>,
    budget: &mut DecodeBudget,
) -> Result<DebugMap, BytecodeError> {
    let entry_count = read_bounded_count(reader, 24, "DEBUG_MAP")?;
    let mut entries = budget.vector(entry_count)?;
    for _ in 0..entry_count {
        let pou_id = reader.read_u32()?;
        let code_offset = reader.read_u32()?;
        let file_idx = reader.read_u32()?;
        let line = reader.read_u32()?;
        let column = reader.read_u32()?;
        let kind = reader.read_u8()?;
        let _reserved = reader.read_bytes(3)?;
        entries.push(DebugEntry {
            pou_id,
            code_offset,
            file_idx,
            line,
            column,
            kind,
        });
    }
    Ok(DebugMap { entries })
}

fn scalar_var_meta(
    reader: &mut BytecodeReader<'_>,
    budget: &mut DecodeBudget,
) -> Result<VarMeta, BytecodeError> {
    let entry_count = read_bounded_count(reader, 20, "VAR_META")?;
    let mut entries = budget.vector(entry_count)?;
    for _ in 0..entry_count {
        let name_idx = reader.read_u32()?;
        let type_id = reader.read_u32()?;
        let ref_idx = reader.read_u32()?;
        let retain = reader.read_u8()?;
        let _flags = reader.read_u8()?;
        let _reserved = reader.read_u16()?;
        let init_const_idx = optional_u32(reader.read_u32()?);
        entries.push(VarMetaEntry {
            name_idx,
            type_id,
            ref_idx,
            retain,
            init_const_idx,
        });
    }
    Ok(VarMeta { entries })
}

fn scalar_retain_init(
    reader: &mut BytecodeReader<'_>,
    budget: &mut DecodeBudget,
) -> Result<RetainInit, BytecodeError> {
    let entry_count = read_bounded_count(reader, 8, "RETAIN_INIT")?;
    let mut entries = budget.vector(entry_count)?;
    for _ in 0..entry_count {
        let ref_idx = reader.read_u32()?;
        let const_idx = reader.read_u32()?;
        entries.push(RetainInitEntry { ref_idx, const_idx });
    }
    Ok(RetainInit { entries })
}

fn run(
    kind: SectionId,
    payload: &[u8],
    bytes: usize,
    work: usize,
    original: bool,
) -> (Result<SectionData, BytecodeError>, DecodeStats, usize) {
    let mut budget = DecodeBudget::new(bytes, work);
    let mut reader = BytecodeReader::new(payload);
    let result = match (original, kind) {
        (true, SectionId::IoMap) => scalar_io_map(&mut reader, &mut budget).map(SectionData::IoMap),
        (false, SectionId::IoMap) => {
            decode_io_map(&mut reader, &mut budget).map(SectionData::IoMap)
        }
        (true, SectionId::DebugMap) => {
            scalar_debug_map(&mut reader, &mut budget).map(SectionData::DebugMap)
        }
        (false, SectionId::DebugMap) => {
            decode_debug_map(&mut reader, &mut budget).map(SectionData::DebugMap)
        }
        (true, SectionId::VarMeta) => {
            scalar_var_meta(&mut reader, &mut budget).map(SectionData::VarMeta)
        }
        (false, SectionId::VarMeta) => {
            decode_var_meta(&mut reader, &mut budget).map(SectionData::VarMeta)
        }
        (true, SectionId::RetainInit) => {
            scalar_retain_init(&mut reader, &mut budget).map(SectionData::RetainInit)
        }
        (false, SectionId::RetainInit) => {
            decode_retain_init(&mut reader, &mut budget).map(SectionData::RetainInit)
        }
        _ => unreachable!(),
    };
    (result, budget.stats(), reader.pos())
}

#[test]
fn fixed_metadata_records_preserve_values_limits_and_cursor() {
    let mut seed = 0x701b_37a1_f073_811bu64;
    for (kind, width) in [
        (SectionId::IoMap, 12),
        (SectionId::DebugMap, 24),
        (SectionId::VarMeta, 20),
        (SectionId::RetainInit, 8),
    ] {
        let mut payload = alloc::vec![0; 4 + width * 2];
        payload[..4].copy_from_slice(&2u32.to_le_bytes());
        for iteration in 0..1024 {
            for byte in &mut payload[4..] {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                *byte = seed as u8;
            }
            for (bytes, work) in [(usize::MAX, usize::MAX), (iteration % 128, iteration % 4)] {
                assert_eq!(
                    run(kind, &payload, bytes, work, false),
                    run(kind, &payload, bytes, work, true)
                );
            }
        }
        for length in 0..payload.len() {
            assert_eq!(
                run(kind, &payload[..length], usize::MAX, usize::MAX, false),
                run(kind, &payload[..length], usize::MAX, usize::MAX, true)
            );
        }
        for count in [0, 1, 3, u32::MAX] {
            payload[..4].copy_from_slice(&count.to_le_bytes());
            assert_eq!(
                run(kind, &payload, usize::MAX, usize::MAX, false),
                run(kind, &payload, usize::MAX, usize::MAX, true)
            );
        }
    }
}
