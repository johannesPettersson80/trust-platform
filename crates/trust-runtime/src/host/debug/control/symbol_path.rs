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
