use crate::memory::VariableStorage;
use crate::value::{RefSegment, Value, ValueRef};

#[derive(Clone, Debug)]
pub(crate) struct ResolvedSymbol {
    pub(crate) reference: ValueRef,
    pub(crate) value: Value,
}

pub(crate) fn resolve(storage: &VariableStorage, symbol: &str) -> Result<ResolvedSymbol, String> {
    let mut parts = symbol.split('.');
    let root = parts.next().ok_or("empty symbol")?;
    if root.is_empty() {
        return Err("empty symbol".into());
    }
    let mut reference = storage
        .ref_for_global(root)
        .ok_or("program instance not found")?;
    let mut value = storage
        .read_by_ref(reference.clone())
        .ok_or("program instance not found")?
        .clone();
    let mut saw_member = false;
    for part in parts {
        if part.is_empty() {
            return Err(format!("symbol {symbol} not found"));
        }
        saw_member = true;
        if let Value::Instance(id) = value {
            reference = storage
                .resolved_instance_field_ref(id, part)
                .ok_or_else(|| format!("symbol {symbol} not found"))?;
        } else {
            reference.path.push(RefSegment::Field(part.into()));
        }
        value = storage
            .read_by_ref(reference.clone())
            .ok_or_else(|| format!("symbol {symbol} not found"))?
            .clone();
    }
    if !saw_member {
        return Err("symbol must name an instance member".into());
    }
    Ok(ResolvedSymbol { reference, value })
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

/// Whether a forced variable holds the variable (or field) a reference names: the forced member
/// itself, or the member a structure field below it belongs to.
pub(crate) fn forced_holds(
    storage: &VariableStorage,
    force: &super::ForcedVarTarget,
    reference: &ValueRef,
) -> bool {
    let forced = match force {
        super::ForcedVarTarget::Instance(id, member) => {
            storage.resolved_instance_field_ref(*id, member)
        }
        super::ForcedVarTarget::Global(name) => storage.ref_for_global(name),
        super::ForcedVarTarget::Retain(_) => None,
    };
    forced.is_some_and(|forced| {
        forced.location == reference.location && forced.offset == reference.offset
    })
}
