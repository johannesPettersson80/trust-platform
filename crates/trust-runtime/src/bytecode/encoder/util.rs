use smol_str::SmolStr;

use crate::io::{IoAddress, IoSize};
use crate::memory::IoArea;

use super::BytecodeError;

pub(super) fn normalize_name(name: &SmolStr) -> SmolStr {
    SmolStr::new(name.to_ascii_uppercase())
}

pub(super) fn count_for_loops(stmts: &[crate::program_model::Stmt]) -> usize {
    let mut count: usize = 0;
    for stmt in stmts {
        match stmt {
            crate::program_model::Stmt::For { body, .. } => {
                count = count.saturating_add(1);
                count = count.saturating_add(count_for_loops(body));
            }
            crate::program_model::Stmt::If {
                then_block,
                else_if,
                else_block,
                ..
            } => {
                count = count.saturating_add(count_for_loops(then_block));
                for (_, block) in else_if {
                    count = count.saturating_add(count_for_loops(block));
                }
                count = count.saturating_add(count_for_loops(else_block));
            }
            crate::program_model::Stmt::Case {
                branches,
                else_block,
                ..
            } => {
                for (_, block) in branches {
                    count = count.saturating_add(count_for_loops(block));
                }
                count = count.saturating_add(count_for_loops(else_block));
            }
            crate::program_model::Stmt::While { body, .. }
            | crate::program_model::Stmt::Repeat { body, .. } => {
                count = count.saturating_add(count_for_loops(body));
            }
            crate::program_model::Stmt::Label { stmt, .. } => {
                if let Some(stmt) = stmt.as_deref() {
                    count = count.saturating_add(count_for_loops(std::slice::from_ref(stmt)));
                }
            }
            _ => {}
        }
    }
    count
}

pub(super) fn to_u32(value: usize, context: &str) -> Result<u32, BytecodeError> {
    u32::try_from(value)
        .map_err(|_| BytecodeError::InvalidSection(format!("{context} overflow").into()))
}

pub(super) fn format_io_address(address: &IoAddress) -> SmolStr {
    let area = match address.area {
        IoArea::Input => 'I',
        IoArea::Output => 'Q',
        IoArea::Memory => 'M',
    };
    let size = match address.size {
        IoSize::Bit => 'X',
        IoSize::Byte => 'B',
        IoSize::Word => 'W',
        IoSize::DWord => 'D',
        IoSize::LWord => 'L',
    };
    if address.wildcard {
        return SmolStr::new(format!("%{area}{size}*"));
    }
    let mut parts: Vec<String> = if address.path.is_empty() {
        vec![address.byte.to_string()]
    } else {
        address.path.iter().map(|part| part.to_string()).collect()
    };
    if matches!(address.size, IoSize::Bit) {
        parts.push(address.bit.to_string());
    }
    let joined = parts.join(".");
    SmolStr::new(format!("%{area}{size}{joined}"))
}
