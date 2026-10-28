//! What the I/O snapshot reads of a compiled program (specification 44 §13.3; specification 11,
//! "Live Values"): the image's process-image bindings, each with the wire type of its variable and
//! named by the variable's declared name, as the snapshot of a prepared program renders them.

use alloc::vec::Vec;

use smol_str::SmolStr;

use super::image::{type_kind as kind, Image, ImageError, IoBindRecord, SymbolRecord, TypeRecord};
use crate::io_image::{IoEnumBinding, IoValueCodec, PreparedIoBinding};
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
    // The codecs read the type table, which must read.
    image.table::<TypeRecord>()?;
    let records = image.table::<IoBindRecord>()?;
    let mut bindings = Vec::with_capacity(records.len() as usize);
    let mut names = Vec::with_capacity(records.len() as usize);
    for record in records.iter() {
        let Some(address) = image.binding_address(&record) else {
            return Err(ImageError::Table(super::image::SectionId::IoBind));
        };
        let symbol = symbols.get(record.symbol);
        let codec = symbol.map_or_else(IoValueCodec::default, |symbol| {
            type_codec(image, symbol.type_index)
        });
        bindings.push(PreparedIoBinding {
            address,
            symbol: record.symbol,
            codec,
        });
        names.push(
            symbol
                .and_then(|symbol| image.string(symbol.display))
                .map(SmolStr::new),
        );
    }
    Ok((bindings, names))
}

/// The codec of a process-image binding of the variable whose symbol is `symbol`, as
/// [`io_bindings`] gives a binding of the image: its variable's elementary type, the base type of an
/// alias or a subrange, or the integer of an enumeration's size with the enumeration's name and
/// values; raw for any other variable.
#[must_use]
pub fn symbol_codec(image: &Image<'_>, symbol: u32) -> IoValueCodec {
    image
        .table::<SymbolRecord>()
        .ok()
        .and_then(|symbols| symbols.get(symbol))
        .map_or_else(IoValueCodec::default, |symbol| {
            type_codec(image, symbol.type_index)
        })
}

/// The codec of a value of type `type_index`, through its aliases and subranges: its wire type,
/// and for an enumeration its declared name and values, so that a value read from an image is the
/// enumeration's and an undeclared number is refused (specification 32 §6).
fn type_codec(image: &Image<'_>, type_index: u32) -> IoValueCodec {
    let Some(ty) = base_type(image, type_index) else {
        return IoValueCodec::default();
    };
    IoValueCodec {
        wire_type: wire_type(&ty),
        enum_type: if ty.kind == kind::ENUM {
            enum_binding(image, &ty)
        } else {
            None
        },
        string_capacity: None,
    }
}

/// The declared name and values of the enumeration `ty`.
fn enum_binding(image: &Image<'_>, ty: &TypeRecord) -> Option<IoEnumBinding> {
    let types = super::values::Types::of(image).ok()?;
    let mut variants = Vec::new();
    for (name, number) in types.variants(ty).ok()? {
        variants.push((SmolStr::new(types.string(name).ok()?), number));
    }
    Some(IoEnumBinding {
        type_name: SmolStr::new(types.string(ty.name).ok()?),
        variants,
    })
}

/// The type a value of type `type_index` is stored as, through its aliases and subranges.
fn base_type(image: &Image<'_>, type_index: u32) -> Option<TypeRecord> {
    let types = image.table::<TypeRecord>().ok()?;
    let mut ty = types.get(type_index)?;
    // An alias or a subrange names its base type in its detail; the chain ends at an elementary
    // type within as many steps as the table has types.
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
    Some(ty)
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
