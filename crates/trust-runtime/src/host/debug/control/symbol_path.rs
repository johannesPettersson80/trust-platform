//! Control Studio's symbol paths (specification 35, control signal reads and experiments by variable
//! name): a member of a program instance or of a global instance, or a field below one, found through
//! the program image's symbol and type tables (specification 44 §13.3), and the forces that hold it.

use trust_runtime_core::memory::Generation;
use trust_runtime_core::native::memory::{member_symbol, retained_place, symbol_place, Place};

use super::program_facts;
use super::ForcedVarTarget;
use crate::runtime::{Located, ProgramRead};
use crate::value::Value;

/// A variable a symbol path names: where it lies in the program generation it was resolved in, and
/// its value there.
#[derive(Clone, Debug)]
pub(crate) struct ResolvedSymbol {
    pub(crate) at: Located,
    pub(crate) value: Value,
}

/// The variable `symbol` names in `program`: a path of two or more names whose first is a global or
/// a program instance and whose later names are members, a derived block's inherited members
/// included, or fields below them.
pub(crate) fn resolve(program: &dyn ProgramRead, symbol: &str) -> Result<ResolvedSymbol, String> {
    let mut parts = symbol.split('.');
    let root = parts.next().unwrap_or_default();
    if root.is_empty() {
        return Err("empty symbol".into());
    }
    if !program_facts::is_global_or_program(&program.program_image(), root) {
        return Err("program instance not found".into());
    }
    let mut saw_member = false;
    for part in parts {
        if part.is_empty() {
            return Err(format!("symbol {symbol} not found"));
        }
        saw_member = true;
    }
    if !saw_member {
        return Err("symbol must name an instance member".into());
    }
    let not_found = || format!("symbol {symbol} not found");
    let at = program.locate(symbol).map_err(|_| not_found())?;
    let value = program.read_at(&at).map_err(|_| not_found())?;
    Ok(ResolvedSymbol { at, value })
}

/// `number` as a value of a variable's own type, `existing` being its current value: REAL and LREAL
/// take it as it is, BOOL takes 0 or 1, an integer type an exactly integral number in its range.
/// Control Studio's variable writes (initial values, events, experiments) share this rule.
pub(crate) fn number_as(existing: &Value, number: f64) -> Result<Value, String> {
    let value = match existing {
        Value::LReal(_) => Value::LReal(number),
        Value::Real(_) => Value::Real(number as f32),
        Value::Bool(_) if number == 0.0 || number == 1.0 => Value::Bool(number == 1.0),
        other => {
            if number.fract() != 0.0 || number.abs() > 9_007_199_254_740_991.0 {
                return Err(
                    "integer initial/event value must be exactly integral and representable".into(),
                );
            }
            let kind = match other {
                Value::SInt(_) => trust_hir::TypeId::SINT,
                Value::Int(_) => trust_hir::TypeId::INT,
                Value::DInt(_) => trust_hir::TypeId::DINT,
                Value::LInt(_) => trust_hir::TypeId::LINT,
                Value::USInt(_) => trust_hir::TypeId::USINT,
                Value::UInt(_) => trust_hir::TypeId::UINT,
                Value::UDInt(_) => trust_hir::TypeId::UDINT,
                Value::ULInt(_) => trust_hir::TypeId::ULINT,
                _ => return Err("unsupported initial/event/parameter type".into()),
            };
            crate::harness::coerce_value_to_type(Value::LInt(number as i64), kind)
                .map_err(|error| error.to_string())?
        }
    };
    if !super::recording::numeric(&value).is_some_and(f64::is_finite) {
        return Err("value exceeds runtime type range".into());
    }
    Ok(value)
}

/// Whether the force on `force` holds the variable at `at`, judged by place in `program`: the
/// forced variable's bytes and the variable's bytes overlap, so a force of a structure holds each of
/// its fields. A force of an instance member resolved in another program generation than
/// `generation` holds nothing, as the scan boundary refuses it.
pub(crate) fn forced_holds(
    program: &dyn ProgramRead,
    generation: Generation,
    force: &ForcedVarTarget,
    at: &Located,
) -> bool {
    let image = program.program_image();
    let forced = match force {
        ForcedVarTarget::Global(name) => image
            .find_variable(name)
            .ok()
            .and_then(|index| symbol_place(&image, index).ok()),
        ForcedVarTarget::Retain(label) => retained_place(&image, label).ok(),
        ForcedVarTarget::Instance(forced_in, instance, member) => (*forced_in == generation)
            .then(|| member_symbol(&image, instance.0, member).ok())
            .flatten()
            .and_then(|index| symbol_place(&image, index).ok()),
    };
    let Some((place, type_index)) = forced else {
        return false;
    };
    let extent = |place: Place, type_index: u32| match place {
        Place::Area { area, offset } => Some((
            area,
            offset,
            offset.saturating_add(program_facts::type_size(&image, type_index)?.max(1)),
        )),
        Place::Frame { .. } => None,
    };
    match (extent(place, type_index), extent(at.place, at.type_index)) {
        (Some((area, start, end)), Some((other_area, other_start, other_end))) => {
            area == other_area && start < other_end && other_start < end
        }
        // A type the table does not size is compared by its start.
        _ => place == at.place,
    }
}
