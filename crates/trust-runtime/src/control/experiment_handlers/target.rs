//! What an experiment writes (Specification 35, experiments by variable name): a process-image
//! address, or a program variable named by its path and written between scans as the debugger's
//! writes are. The trajectory, the guards and the restoration are the same for both.

use super::values::{numeric_value, snapshot_type, snapshot_value};
use crate::debug::DebugControl;
use crate::io::{validate_process_image_address, IoAddress, IoSnapshot};
use crate::memory::VariableStorage;
use crate::runtime::RuntimeMetadata;
use crate::value::{Value, ValueRef};
use serde_json::{json, Value as JsonValue};
use trust_hir::TypeId;

type Refusal = (String, &'static str);

#[derive(Clone, Debug)]
pub(super) enum ExperimentTarget {
    Address { address: IoAddress, text: String },
    Symbol(SymbolTarget),
}

#[derive(Clone, Debug)]
pub(super) struct SymbolTarget {
    path: String,
    reference: ValueRef,
    /// The variable's value when the experiment started; every write takes its type.
    current: Value,
}

impl ExperimentTarget {
    /// The target a start request names: exactly one of `address` and `symbol`.
    pub(super) fn decode(
        address: Option<&str>,
        symbol: Option<&str>,
        debug: &DebugControl,
        metadata: Option<&RuntimeMetadata>,
    ) -> Result<Self, Refusal> {
        match (address, symbol) {
            (Some(text), None) => {
                let address = IoAddress::parse(text)
                    .map_err(|error| (error.to_string(), "invalid_address"))?;
                validate_process_image_address(&address)
                    .map_err(|error| (error.to_string(), "invalid_address"))?;
                Ok(Self::Address {
                    address,
                    text: text.to_string(),
                })
            }
            (None, Some(path)) => SymbolTarget::decode(path, debug, metadata).map(Self::Symbol),
            _ => Err((
                "an experiment writes one target: give either `address` or `symbol`".into(),
                "invalid_target",
            )),
        }
    }

    /// Whether the experiment writes a variable, whose reference names the program that resolved it.
    pub(super) fn is_symbol(&self) -> bool {
        matches!(self, Self::Symbol(_))
    }

    /// The address, for the parts of an experiment only an address has (a project abort binding).
    pub(super) fn address(&self) -> Option<&IoAddress> {
        match self {
            Self::Address { address, .. } => Some(address),
            Self::Symbol(_) => None,
        }
    }

    /// The target as the request named it, `("address", "%MW0")` or `("symbol", "Main.Setpoint")`.
    pub(super) fn echo(&self) -> (&'static str, JsonValue) {
        match self {
            Self::Address { text, .. } => ("address", json!(text)),
            Self::Symbol(symbol) => ("symbol", json!(symbol.path)),
        }
    }

    /// The type the runtime holds the target in: the address's binding, or the variable's own.
    pub(super) fn runtime_type(&self, snapshot: &IoSnapshot) -> Option<TypeId> {
        match self {
            Self::Address { address, .. } => snapshot_type(snapshot, address),
            Self::Symbol(symbol) => type_of(&symbol.current),
        }
    }

    /// The value written for a requested one: for an address converted to its declared binding
    /// type when one is declared, for a variable to the variable's own type.
    pub(super) fn coerce(&self, raw: Value, declared: Option<TypeId>) -> Result<Value, String> {
        match self {
            Self::Address { address, .. } => match declared {
                Some(kind) => crate::io::coerce_to_io(raw, kind, address.size)
                    .map_err(|error| error.to_string()),
                None => Ok(raw),
            },
            Self::Symbol(symbol) => {
                let number = numeric_value(&raw)
                    .filter(|number| number.is_finite())
                    .ok_or("experiment value must be finite and numeric")?;
                crate::debug::control_number_as(&symbol.current, number)
            }
        }
    }

    /// The target's value in the latest completed scan, `None` when it cannot be read: an address
    /// from the I/O snapshot, a variable from the debug snapshot through the reference it
    /// resolved to at the start (a reloaded program that moved it reads as missing).
    pub(super) fn current(&self, debug: &DebugControl, snapshot: &IoSnapshot) -> Option<Value> {
        match self {
            Self::Address { address, .. } => snapshot_value(snapshot, address).cloned(),
            Self::Symbol(symbol) => debug
                .with_snapshot(|captured| {
                    crate::debug::resolve_control_parameter_path(&captured.storage, &symbol.path)
                        .ok()
                        .filter(|resolved| resolved.reference == symbol.reference)
                        .map(|resolved| resolved.value)
                })
                .flatten(),
        }
    }

    /// The target's value in the scan an observer was given: an address from that scan's I/O snapshot, a variable from its
    /// storage through the reference it resolved to at the start (a reloaded program that moved it reads as missing).
    pub(super) fn observed(&self, io: &IoSnapshot, storage: &VariableStorage) -> Option<Value> {
        match self {
            Self::Address { address, .. } => snapshot_value(io, address).cloned(),
            Self::Symbol(symbol) => {
                crate::debug::resolve_control_parameter_path(storage, &symbol.path)
                    .ok()
                    .filter(|resolved| resolved.reference == symbol.reference)
                    .map(|resolved| resolved.value)
            }
        }
    }

    /// Whether the target is forced: the address, or a forced variable holding the variable.
    pub(super) fn forced(&self, debug: &DebugControl, snapshot: &IoSnapshot) -> bool {
        match self {
            Self::Address { address, .. } => snapshot.forced.iter().any(|forced| forced == address),
            Self::Symbol(symbol) => debug.forces_reference(&symbol.reference),
        }
    }

    /// Queues a write for the next scan boundary. A variable's write carries the runtime epoch its reference was
    /// resolved in, so a write that outlives that epoch is dropped instead of reaching the program that replaced it.
    pub(super) fn write(&self, debug: &DebugControl, value: Value, epoch: u64) {
        match self {
            Self::Address { address, .. } => debug.enqueue_io_write(address.clone(), value),
            Self::Symbol(symbol) => {
                debug.enqueue_reference_write(symbol.reference.clone(), value, epoch);
            }
        }
    }
}

impl SymbolTarget {
    /// A variable a Control Studio experiment may write: a REAL, LREAL or integer variable of a
    /// compiled program, not constant, external, IN_OUT or addressed (an addressed one is written
    /// by its address), read from the latest completed scan.
    fn decode(
        path: &str,
        debug: &DebugControl,
        metadata: Option<&RuntimeMetadata>,
    ) -> Result<Self, Refusal> {
        let refuse = |reason: &str| {
            (
                format!("experiment variable {path}: {reason}"),
                "invalid_target",
            )
        };
        if path.trim().is_empty() || path.len() > 256 {
            return Err(refuse("give a variable path of 1 to 256 characters"));
        }
        let metadata = metadata.ok_or_else(|| refuse("the program's metadata is unavailable"))?;
        // Until a scan of a reloaded program completes, the debug snapshot still holds the program before the reload.
        if debug.snapshot_is_stale() {
            return Err((
                format!("experiment variable {path}: the runtime has not completed a scan of its current program"),
                "stale_scan",
            ));
        }
        if !metadata.control_parameter_writable(path) {
            return Err(refuse(
                "name a variable of a compiled program that is not constant, external, IN_OUT \
                 or addressed",
            ));
        }
        let resolved = debug
            .with_snapshot(|captured| {
                crate::debug::resolve_control_parameter_path(&captured.storage, path)
            })
            .ok_or_else(|| refuse("no runtime scan has been captured yet"))?
            .map_err(|reason| refuse(reason.as_str()))?;
        if !matches!(
            resolved.value,
            Value::Real(_)
                | Value::LReal(_)
                | Value::SInt(_)
                | Value::Int(_)
                | Value::DInt(_)
                | Value::LInt(_)
                | Value::USInt(_)
                | Value::UInt(_)
                | Value::UDInt(_)
                | Value::ULInt(_)
        ) {
            return Err(refuse(
                "only REAL, LREAL and integer variables take experiment values",
            ));
        }
        Ok(Self {
            path: path.to_string(),
            reference: resolved.reference,
            current: resolved.value,
        })
    }
}

/// The elementary type of a numeric value.
fn type_of(value: &Value) -> Option<TypeId> {
    Some(match value {
        Value::Bool(_) => TypeId::BOOL,
        Value::SInt(_) => TypeId::SINT,
        Value::Int(_) => TypeId::INT,
        Value::DInt(_) => TypeId::DINT,
        Value::LInt(_) => TypeId::LINT,
        Value::USInt(_) => TypeId::USINT,
        Value::UInt(_) => TypeId::UINT,
        Value::UDInt(_) => TypeId::UDINT,
        Value::ULInt(_) => TypeId::ULINT,
        Value::Real(_) => TypeId::REAL,
        Value::LReal(_) => TypeId::LREAL,
        _ => return None,
    })
}
