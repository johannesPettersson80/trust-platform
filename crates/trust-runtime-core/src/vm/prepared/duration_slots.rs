//! The native duration slots of the overloaded timers (specification 34 §6.2; checklist C1-19d).
//!
//! STBC declares the `PT` and `ET` of an overloaded `TP`, `TON` or `TOF` as `TIME`, yet a call may
//! give them `LTIME`, and `PT` determines the family of `ET` (specification 08 §5). Preparation gives
//! them the native duration slot, the internal primitive `0x0101`, which holds a duration with the
//! family it was written with, so that a store laid out at declared sizes keeps it. The explicit
//! `*_TIME` and `*_LTIME` blocks keep their declared types.

use smol_str::SmolStr;

use super::budget::PreparationBudget;
use crate::bytecode::{StorageLayout, StorageOwner, TypeData, TypeEntry, TypeKind};
use crate::error::RuntimeError;
use crate::stdlib::contracts::{self, FbParameterType, ANY_DURATION_STATE};
use crate::vm::module::VmModule;
use crate::vm::type_policy::resolved_primitive_shape;

/// The STBC primitive id of `TIME` (specification 12 §6.2), which STBC gives an overloaded timer's
/// duration parameters.
const TIME: u16 = 16;

/// Give the `PT` and `ET` of every overloaded timer template the native duration slot: its
/// parameters, which call binding normalizes a value to, and its instance declarations, which the
/// flat store lays out. One `TYPE_TABLE` entry of the slot's primitive is added when a template
/// needs it; a parameter that STBC does not declare `TIME` is left for admission to refuse.
pub(super) fn bind_duration_slots(
    vm: &mut VmModule,
    layout: &mut StorageLayout,
    budget: &mut PreparationBudget,
) -> Result<(), RuntimeError> {
    let mut slot = None;
    // The templates are listed first, since the walk changes the module whose index lists them.
    budget.records::<(u32, u32)>(vm.function_block_ids.iter().len())?;
    let templates: alloc::vec::Vec<(u32, u32)> = vm.function_block_ids.iter().collect();
    for (name_idx, pou) in templates {
        let name = &vm.strings[name_idx as usize];
        budget.charge(name.len(), 1)?;
        let Some(import) = contracts::function_block(&SmolStr::new(name.to_ascii_uppercase()))
        else {
            continue;
        };
        if import.primitive(FbParameterType::Duration) != ANY_DURATION_STATE {
            continue;
        }
        let durations = || {
            import
                .parameters
                .iter()
                .filter(|parameter| parameter.ty == FbParameterType::Duration)
                .map(|parameter| parameter.name)
        };
        let slot = match slot {
            Some(slot) => slot,
            None => {
                let id =
                    u32::try_from(vm.types.entries.len()).map_err(|_| RuntimeError::Overflow)?;
                budget.records::<TypeEntry>(1)?;
                vm.types.entries.push(TypeEntry {
                    kind: TypeKind::Primitive,
                    name_idx: None,
                    data: TypeData::Primitive {
                        prim_id: ANY_DURATION_STATE,
                        max_length: 0,
                    },
                });
                *slot.insert(id)
            }
        };
        let types = &vm.types;
        let is_time = |type_id: u32| {
            resolved_primitive_shape(types, type_id, 0).map(|(prim, _)| prim) == Some(TIME)
        };
        if let Some(params) = vm.pou_params.get_mut(&pou) {
            for param in params.iter_mut() {
                budget.charge(0, 1)?;
                if durations().any(|name| param.name.eq_ignore_ascii_case(name))
                    && is_time(param.type_id)
                {
                    param.type_id = slot;
                }
            }
        }
        for declaration in &mut layout.entries {
            budget.charge(0, 1)?;
            if declaration.owner != StorageOwner::Instance
                || declaration.owner_pou_id != Some(pou)
                || !declaration.owns_storage()
            {
                continue;
            }
            let named = vm
                .strings
                .get(declaration.name_idx as usize)
                .is_some_and(|name| {
                    durations().any(|duration| name.eq_ignore_ascii_case(duration))
                });
            if named && declaration.type_id.is_some_and(is_time) {
                declaration.type_id = Some(slot);
            }
        }
    }
    Ok(())
}
