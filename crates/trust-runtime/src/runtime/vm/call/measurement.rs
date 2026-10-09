//! A simulated plant's measurement for a Control.PID (Specification 33 §13.3, `pid:` feedback
//! targets): when a measurement is published for the instance a method is called on and the
//! method is its `Update`, the bound `Input` argument carries the measurement as `ProcessValue`.
//! Only that field of that call changes; the caller's variables keep what the program wrote.

use crate::memory::InstanceId;
use crate::value::Value;

use super::super::VmModule;

pub(super) fn substitute(
    runtime: &super::super::super::core::Runtime,
    module: &VmModule,
    pou_id: u32,
    instance: InstanceId,
    locals: &mut [Value],
) {
    let Some(measurement) = runtime.controller_measurement(instance) else {
        return;
    };
    let is_update = module.pou_name(pou_id).is_some_and(|name| {
        name.rsplit('.')
            .next()
            .is_some_and(|method| method.eq_ignore_ascii_case("Update"))
    });
    if !is_update {
        return;
    }
    let Some(index) = module.pou_params(pou_id).and_then(|params| {
        params
            .iter()
            .position(|param| param.direction == 0 && param.name.eq_ignore_ascii_case("Input"))
    }) else {
        return;
    };
    let slot = usize::from(module.pou_has_return_slot(pou_id)) + index;
    let Some(Value::Struct(input)) = locals.get_mut(slot) else {
        return;
    };
    let input = std::sync::Arc::make_mut(input);
    let Some(field) = input
        .fields()
        .keys()
        .find(|name| name.eq_ignore_ascii_case("ProcessValue"))
        .cloned()
    else {
        return;
    };
    input.set_existing_field(field, Value::LReal(measurement));
}
