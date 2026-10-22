//! What the I/O snapshot reads of a compiled program (specification 44 §13.3; specification 11,
//! "Live Values"): the image's process-image bindings, each with the wire type of its variable and
//! named by the variable's declared name, as the snapshot of a prepared program renders them.

use alloc::vec::Vec;

use smol_str::SmolStr;

use super::image::{type_kind as kind, Image, ImageError, IoBindRecord, SymbolRecord, TypeRecord};
use crate::io_address::{IoAddress, IoSize};
use crate::io_image::{IoValueCodec, PreparedIoBinding};
use crate::memory::IoArea;
use crate::stdlib::conversions::ConversionType;

/// The bindings of `image` in its table's order, with the declared name of each bound variable,
/// parallel. A binding's codec has its variable's elementary type, the base type of an alias or a
/// subrange, or the integer of an enumeration's size; any other variable's binding reads raw.
///
/// # Errors
///
/// The [`ImageError`] of a table that does not read.
pub fn io_bindings(
    image: &Image<'_>,
) -> Result<(Vec<PreparedIoBinding>, Vec<Option<SmolStr>>), ImageError> {
    let symbols = image.table::<SymbolRecord>()?;
    let types = image.table::<TypeRecord>()?;
    let records = image.table::<IoBindRecord>()?;
    let mut bindings = Vec::with_capacity(records.len() as usize);
    let mut names = Vec::with_capacity(records.len() as usize);
    for record in records.iter() {
        let area = match record.area {
            1 => IoArea::Input,
            2 => IoArea::Output,
            _ => IoArea::Memory,
        };
        let size = match record.size {
            0 => IoSize::Bit,
            1 => IoSize::Byte,
            2 => IoSize::Word,
            3 => IoSize::DWord,
            _ => IoSize::LWord,
        };
        let symbol = symbols.get(record.symbol);
        let wire_type = symbol.and_then(|symbol| {
            let mut ty = types.get(symbol.type_index)?;
            // An alias or a subrange names its base type in its detail; the chain ends at an
            // elementary type within as many steps as the table has types.
            for _ in 0..types.len() {
                match ty.kind {
                    kind::ALIAS | kind::SUBRANGE => {
                        let detail = image.section_bytes(super::image::SectionId::Types);
                        let at = usize::try_from(ty.detail).ok()?;
                        let base = u32::from_le_bytes(detail.get(at..at + 4)?.try_into().ok()?);
                        ty = types.get(base)?;
                    }
                    _ => break,
                }
            }
            wire_type(&ty)
        });
        bindings.push(PreparedIoBinding {
            // A flat address keeps its byte as its one path component, as the parser makes it.
            address: IoAddress {
                area,
                size,
                byte: record.byte,
                bit: record.bit,
                path: alloc::vec![record.byte],
                wildcard: false,
            },
            ref_idx: record.symbol,
            type_id: None,
            codec: IoValueCodec {
                wire_type,
                enum_type: None,
                string_capacity: None,
            },
        });
        names.push(
            symbol
                .and_then(|symbol| image.string(symbol.display))
                .map(SmolStr::new),
        );
    }
    Ok((bindings, names))
}

/// The wire type of a value of `ty`: its elementary type, or the integer of an enumeration's size.
fn wire_type(ty: &TypeRecord) -> Option<ConversionType> {
    Some(match ty.kind {
        kind::BOOL => ConversionType::Bool,
        kind::SINT => ConversionType::SInt,
        kind::INT => ConversionType::Int,
        kind::DINT => ConversionType::DInt,
        kind::LINT => ConversionType::LInt,
        kind::USINT => ConversionType::USInt,
        kind::UINT => ConversionType::UInt,
        kind::UDINT => ConversionType::UDInt,
        kind::ULINT => ConversionType::ULInt,
        kind::REAL => ConversionType::Real,
        kind::LREAL => ConversionType::LReal,
        kind::BYTE => ConversionType::Byte,
        kind::WORD => ConversionType::Word,
        kind::DWORD => ConversionType::DWord,
        kind::LWORD => ConversionType::LWord,
        kind::TIME => ConversionType::Time,
        kind::LTIME => ConversionType::LTime,
        kind::LDATE => ConversionType::LDate,
        kind::LTOD => ConversionType::LTod,
        kind::LDT => ConversionType::Ldt,
        kind::CHAR => ConversionType::Char,
        kind::WCHAR => ConversionType::WChar,
        kind::ENUM => match ty.size {
            1 => ConversionType::SInt,
            2 => ConversionType::Int,
            4 => ConversionType::DInt,
            _ => ConversionType::LInt,
        },
        _ => return None,
    })
}

#[cfg(test)]
mod tests;
