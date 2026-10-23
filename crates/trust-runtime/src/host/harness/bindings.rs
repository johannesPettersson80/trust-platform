//! A runtime whose program only declares located variables: the process image that host
//! services such as a simulated plant or an I/O driver are exercised against.

use trust_hir::TypeId;

use super::{CompileError, CompileSession};
use crate::io::{IoAddress, IoSize};
use crate::memory::IoArea;
use crate::Runtime;

/// Builds a runtime whose only program declares a located variable of each `(address, type)`,
/// so that its process image binds those addresses with those types.
pub fn runtime_with_bindings(bindings: &[(IoAddress, TypeId)]) -> Result<Runtime, CompileError> {
    let mut vars = String::new();
    for (index, (address, type_id)) in bindings.iter().enumerate() {
        let text = address_text(address)?;
        let type_name = elementary_type_name(*type_id)
            .ok_or_else(|| CompileError::new(format!("binding {text} needs an elementary type")))?;
        vars.push_str(&format!("    Bound{index} AT {text} : {type_name};\n"));
    }
    CompileSession::from_source(format!("PROGRAM Main\nVAR\n{vars}END_VAR\nEND_PROGRAM\n"))
        .build_runtime()
}

/// A flat address as ST writes it: `%IX0.1`, `%QL8`.
fn address_text(address: &IoAddress) -> Result<String, CompileError> {
    let area = match address.area {
        IoArea::Input => 'I',
        IoArea::Output => 'Q',
        IoArea::Memory => 'M',
    };
    if address.path.len() > 1 || address.wildcard {
        return Err(CompileError::new(
            "a test binding needs an elementary flat address",
        ));
    }
    let byte = address.byte;
    Ok(match address.size {
        IoSize::Bit => format!("%{area}X{byte}.{}", address.bit),
        IoSize::Byte => format!("%{area}B{byte}"),
        IoSize::Word => format!("%{area}W{byte}"),
        IoSize::DWord => format!("%{area}D{byte}"),
        IoSize::LWord => format!("%{area}L{byte}"),
        IoSize::Bytes(_) => {
            return Err(CompileError::new(
                "a test binding needs an elementary flat address",
            ))
        }
    })
}

fn elementary_type_name(type_id: TypeId) -> Option<&'static str> {
    Some(match type_id {
        TypeId::BOOL => "BOOL",
        TypeId::SINT => "SINT",
        TypeId::INT => "INT",
        TypeId::DINT => "DINT",
        TypeId::LINT => "LINT",
        TypeId::USINT => "USINT",
        TypeId::UINT => "UINT",
        TypeId::UDINT => "UDINT",
        TypeId::ULINT => "ULINT",
        TypeId::BYTE => "BYTE",
        TypeId::WORD => "WORD",
        TypeId::DWORD => "DWORD",
        TypeId::LWORD => "LWORD",
        TypeId::REAL => "REAL",
        TypeId::LREAL => "LREAL",
        _ => return None,
    })
}
