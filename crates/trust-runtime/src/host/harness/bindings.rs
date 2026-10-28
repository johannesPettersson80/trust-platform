//! A runtime whose program only declares located variables: the process image that host
//! services such as a simulated plant or an I/O driver are exercised against.

use trust_hir::TypeId;

use super::{CompileError, TestHarness};
use crate::io::{IoAddress, IoSize};
use crate::CompiledRuntime;

/// Builds a runtime whose only program declares a located variable of each `(address, type)`,
/// so that its process image binds those addresses with those types. The program is built for
/// this computer and started as the test harness starts one, with the toolchain of specification
/// 44 §4.6 at `TRUST_TOOLCHAIN`.
pub fn runtime_with_bindings(
    bindings: &[(IoAddress, TypeId)],
) -> Result<CompiledRuntime, CompileError> {
    let mut vars = String::new();
    for (index, (address, type_id)) in bindings.iter().enumerate() {
        let text = address_text(address)?;
        let type_name = elementary_type_name(*type_id)
            .ok_or_else(|| CompileError::new(format!("binding {text} needs an elementary type")))?;
        vars.push_str(&format!("    Bound{index} AT {text} : {type_name};\n"));
    }
    TestHarness::from_source(&format!("PROGRAM Main\nVAR\n{vars}END_VAR\nEND_PROGRAM\n"))
        .map(TestHarness::into_runtime)
}

/// A flat elementary address as ST writes it: `%IX0.1`, `%QL8`.
fn address_text(address: &IoAddress) -> Result<String, CompileError> {
    if address.path.len() > 1 || address.wildcard || matches!(address.size, IoSize::Bytes(_)) {
        return Err(CompileError::new(
            "a test binding needs an elementary flat address",
        ));
    }
    Ok(address.to_string())
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
