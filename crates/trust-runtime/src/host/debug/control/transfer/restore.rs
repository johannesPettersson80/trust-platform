//! Reverts a committed transfer to the values that it held at its task boundary (A6.3, E6.7).
use super::*;

/// Writes back every member that the commit wrote, and every output at its task-boundary value, in the same scan. An
/// output keeps its IEC type, so a BOOL output is restored as BOOL. Returns whether every write succeeded.
pub(super) fn restore(pending: &Pending, storage: &mut VariableStorage) -> bool {
    let mut restored = true;
    for (reference, previous) in &pending.restore_points {
        restored &= storage.write_by_ref_ref(reference, previous.clone());
    }
    for member in std::iter::once(&pending.request.binding.output)
        .chain(pending.request.binding.additional_outputs.iter())
    {
        let Some(&value) = pending.boundary_outputs.get(&member.symbol) else {
            continue;
        };
        restored &= match parameters::resolve_path(storage, &member.symbol) {
            Ok(resolved) => storage.write_by_ref(resolved.reference, output_value(member, value)),
            Err(_) => false,
        };
    }
    restored
}

fn output_value(member: &NumberMember, value: f64) -> Value {
    match member.iec_type.as_str() {
        "REAL" => Value::Real(value as f32),
        "BOOL" => Value::Bool(value != 0.0),
        _ => Value::LReal(value),
    }
}
