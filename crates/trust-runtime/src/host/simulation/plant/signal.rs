//! Plant command sources and feedback targets (Specification 33 §13.1–§13.3): a process-image address, or the
//! symbolic path of a compiled program variable that has no address.

use crate::debug::resolve_control_parameter_path;
use crate::error::RuntimeError;
use crate::io::IoAddress;
use crate::memory::IoArea;
use crate::value::{Value, ValueRef};
use crate::Runtime;

#[derive(Clone, Debug, PartialEq)]
pub(super) enum PlantSignal {
    Address(IoAddress),
    Symbol(String),
}

impl PlantSignal {
    /// An address always starts with `%`; any other text names a program variable by its dotted path.
    pub(super) fn parse(text: &str, areas: &[IoArea], role: &str) -> Result<Self, RuntimeError> {
        if text.trim_start().starts_with('%') {
            let mut parsed = super::validation::parse_addresses(&[text.to_string()], areas, role)?;
            return Ok(Self::Address(parsed.remove(0)));
        }
        let identifier = |part: &str| {
            let mut chars = part.chars();
            chars
                .next()
                .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
                && chars.all(|next| next.is_ascii_alphanumeric() || next == '_')
        };
        if text.len() > 256 || !text.split('.').all(identifier) {
            return super::validation::invalid(format!(
                "plant {role} is neither an address nor a program variable: {text}"
            ));
        }
        Ok(Self::Symbol(text.to_string()))
    }

    pub(super) fn address(&self) -> Option<&IoAddress> {
        match self {
            Self::Address(address) => Some(address),
            Self::Symbol(_) => None,
        }
    }

    /// Two targets that would write the same process-image bits or the same program variable.
    pub(super) fn overlaps(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Address(left), Self::Address(right)) => {
                super::super::addresses_overlap(left, right)
            }
            (Self::Symbol(left), Self::Symbol(right)) => left.eq_ignore_ascii_case(right),
            _ => false,
        }
    }

    pub(super) fn read(&self, runtime: &Runtime) -> Result<f64, RuntimeError> {
        match self {
            Self::Address(address) => super::numeric_io::read_numeric(runtime.io(), address),
            Self::Symbol(symbol) => super::numeric_io::decode_storage(&resolve(runtime, symbol)?.1),
        }
    }
}

/// The program variable's storage reference and current value.
pub(super) fn resolve(runtime: &Runtime, symbol: &str) -> Result<(ValueRef, Value), RuntimeError> {
    let resolved = resolve_control_parameter_path(runtime.storage(), symbol).map_err(|reason| {
        RuntimeError::SimulationFault(format!("plant signal {symbol}: {reason}").into())
    })?;
    Ok((resolved.reference, resolved.value))
}
