//! Prepared native-call plans (spec 34 MEM-09, TIME-08).
//!
//! Every `CALL_NATIVE` site resolves its argument binding once, at preparation: the formal
//! index of each actual (named and positional), the explicit-presence pattern, the output
//! descriptors and the callee. A scan reads the plan and resolves no argument names, builds no
//! uppercase type key and allocates none of it. Every charge, result and refusal of a call is
//! unchanged (TIME-08).
use super::budget::PreparationBudget;
use crate::bytecode::{
    InitializerIndex, StorageLayout, StorageOwner, StorageRole, TypeData, TypeKind,
    NATIVE_CALL_KIND_FUNCTION, NATIVE_CALL_KIND_FUNCTION_BLOCK, NATIVE_CALL_KIND_METHOD,
};
use crate::error::RuntimeError;
use crate::stdlib::fbs::{self, BuiltinFbKind};
use crate::vm::module::{VmModule, VmNativeArgSpec, VmNativeSymbolSpec, VmParamMeta};
use alloc::{sync::Arc, vec, vec::Vec};

/// The callee a prepared call site resolves to (spec 34 MEM-08).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CallCallee {
    /// A user function, by its prepared POU identity.
    Function { pou: u32 },
    /// A function block, by its prepared POU and built-in kind when it is a built-in.
    FunctionBlock {
        pou: u32,
        builtin: Option<BuiltinFbKind>,
    },
    /// A method of the receiver's class-like template POU.
    Method { owner: u32, pou: u32 },
}

/// The argument binding of one prepared call site.
#[derive(Debug)]
pub(crate) struct CallArguments {
    /// Payload position of every formal, `u32::MAX` when the call omits it.
    formal_args: Vec<u32>,
    /// Explicit presence of every formal slot; a supplied NULL is present.
    present: Vec<bool>,
    /// Output formals with their direction, in formal order.
    outputs: Vec<(u32, u8)>,
}

impl CallArguments {
    /// The payload position of `formal`, when the call supplies it.
    pub(crate) fn actual(&self, formal: usize) -> Option<usize> {
        self.formal_args
            .get(formal)
            .copied()
            .filter(|position| *position != u32::MAX)
            .and_then(|position| usize::try_from(position).ok())
    }

    /// The explicit-presence bit of every formal slot.
    pub(crate) fn present(&self) -> &[bool] {
        &self.present
    }

    /// The payload positions of the outputs and in-outs the call supplies, whose destinations its
    /// copy-back writes (C1-19c4).
    pub(crate) fn output_positions(&self) -> impl Iterator<Item = usize> + '_ {
        self.outputs
            .iter()
            .filter_map(|&(formal, _)| self.actual(formal as usize))
    }

    /// The direction of output formal `formal`, when the call supplies it. Read by the
    /// call-plan tests (spec 34 MEM-09).
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn output_direction(&self, formal: usize) -> Option<u8> {
        let formal = u32::try_from(formal).ok()?;
        self.outputs
            .iter()
            .find(|(index, _)| *index == formal)
            .map(|(_, direction)| *direction)
    }
}

/// One prepared native-call site.
#[derive(Debug)]
pub struct CallPlan {
    /// Encoded call kind (`NATIVE_CALL_KIND_*`).
    pub(crate) kind: u8,
    /// Index of the call's symbol in the module's native-symbol table.
    pub(crate) symbol_idx: u32,
    /// The resolved callee.
    pub(crate) callee: CallCallee,
    /// The resolved argument binding.
    pub(crate) arguments: CallArguments,
}

/// One decoded `CALL_NATIVE` instruction.
struct CallSite {
    offset: u32,
    kind: u32,
    symbol_idx: u32,
    owner: Option<u32>,
}

/// Resolve every planned `CALL_NATIVE` site once, before RUN (spec 34 MEM-09).
pub(super) fn prepare_call_plans(
    vm: &VmModule,
    layout: &StorageLayout,
    initializers: &InitializerIndex,
    budget: &mut PreparationBudget,
) -> Result<Vec<(u32, Arc<CallPlan>)>, RuntimeError> {
    let mut sites = Vec::new();
    for (id, pou) in vm.pou_by_id.iter() {
        decode_calls(
            &vm.code,
            pou.code_start,
            pou.code_end,
            Some(*id),
            &mut sites,
        );
    }
    for entry in &initializers.entries {
        let start = entry.code_offset as usize;
        let end = start.saturating_add(entry.code_length as usize);
        decode_calls(&vm.code, start, end, entry.owner_pou_id, &mut sites);
    }
    sites.sort_by_key(|site| site.offset);
    budget.records::<(u32, Arc<CallPlan>)>(sites.len())?;
    let mut plans = Vec::new();
    plans
        .try_reserve_exact(sites.len())
        .map_err(|_| RuntimeError::PreparationLimit)?;
    for site in sites {
        let Some(plan) = build_plan(vm, layout, &site) else {
            continue;
        };
        budget.records::<u32>(plan.arguments.formal_args.len())?;
        budget.records::<bool>(plan.arguments.present.len())?;
        budget.records::<(u32, u8)>(plan.arguments.outputs.len())?;
        plans.push((site.offset, Arc::new(plan)));
    }
    Ok(plans)
}

/// Whether a scan reads prepared call plans (spec 34 MEM-09; test support).
pub(crate) fn plans_enabled() -> bool {
    #[cfg(all(feature = "std", feature = "test-support"))]
    {
        PLANS_ENABLED.with(core::cell::Cell::get)
    }
    #[cfg(not(all(feature = "std", feature = "test-support")))]
    {
        true
    }
}

/// Resolve every call at the call site instead of through its plan (test support).
#[cfg(all(feature = "std", feature = "test-support"))]
pub(crate) fn set_plans_enabled(enabled: bool) {
    PLANS_ENABLED.with(|cell| cell.set(enabled));
}

#[cfg(all(feature = "std", feature = "test-support"))]
thread_local! {
    static PLANS_ENABLED: core::cell::Cell<bool> = const { core::cell::Cell::new(true) };
}

/// Decode every `CALL_NATIVE` instruction of one admitted body range.
fn decode_calls(
    code: &[u8],
    start: usize,
    end: usize,
    owner: Option<u32>,
    out: &mut Vec<CallSite>,
) {
    let Some(body) = code.get(start..end) else {
        return;
    };
    let mut pc = 0usize;
    while pc < body.len() {
        let opcode = body[pc];
        let Some(width) = crate::vm::opcode_operand_len(opcode) else {
            return;
        };
        let next = pc + 1 + width;
        if next > body.len() {
            return;
        }
        if opcode == 0x09 {
            let kind = read_u32(body, pc + 1);
            let symbol_idx = read_u32(body, pc + 5);
            out.push(CallSite {
                offset: (start + pc) as u32,
                kind,
                symbol_idx,
                owner,
            });
        }
        pc = next;
    }
}

fn read_u32(body: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([body[at], body[at + 1], body[at + 2], body[at + 3]])
}

/// Resolve one call site, or nothing when it cannot be planned at preparation.
fn build_plan(vm: &VmModule, layout: &StorageLayout, site: &CallSite) -> Option<CallPlan> {
    let spec = vm.native_symbol_specs.get(site.symbol_idx)?;
    let VmNativeSymbolSpec::Parsed {
        target_name,
        normalized_target_name,
        resolved_function_pou_id,
        arg_specs,
        ..
    } = spec
    else {
        return None;
    };
    let kind = u8::try_from(site.kind).ok()?;
    let (callee, params) = match site.kind {
        NATIVE_CALL_KIND_FUNCTION => {
            let pou = (*resolved_function_pou_id)?;
            (CallCallee::Function { pou }, vm.pou_params(pou)?)
        }
        NATIVE_CALL_KIND_FUNCTION_BLOCK => {
            let owner = site.owner?;
            let type_id = declared_member_type(vm, layout, owner, target_name.as_str())?;
            let pou = function_block_pou(vm, type_id)?;
            let builtin = vm.pou_name(pou).and_then(fbs::builtin_kind);
            (
                CallCallee::FunctionBlock { pou, builtin },
                vm.pou_params(pou)?,
            )
        }
        NATIVE_CALL_KIND_METHOD => {
            let (owner, pou) = sole_method_owner(vm, normalized_target_name.as_str())?;
            (CallCallee::Method { owner, pou }, vm.pou_params(pou)?)
        }
        _ => return None,
    };
    let arguments = build_arguments(params, arg_specs)?;
    Some(CallPlan {
        kind,
        symbol_idx: site.symbol_idx,
        callee,
        arguments,
    })
}

/// Resolve each formal to its payload position, exactly as the call-time resolver would.
/// Any form the call-time resolver rejects leaves the site unplanned.
fn build_arguments(params: &[VmParamMeta], specs: &[VmNativeArgSpec]) -> Option<CallArguments> {
    let positional_count = specs.iter().take_while(|spec| spec.name.is_none()).count();
    if positional_count > params.len() {
        return None;
    }
    if specs
        .iter()
        .skip(positional_count)
        .any(|spec| spec.name.is_none())
    {
        return None;
    }
    let mut formal_args = vec![u32::MAX; params.len()];
    for (index, slot) in formal_args.iter_mut().enumerate().take(positional_count) {
        *slot = u32::try_from(index).ok()?;
    }
    for (arg_index, spec) in specs.iter().enumerate().skip(positional_count) {
        let name = spec.name.as_deref()?;
        let param_index = params
            .iter()
            .position(|param| param.name.eq_ignore_ascii_case(name))?;
        if formal_args[param_index] != u32::MAX {
            return None;
        }
        formal_args[param_index] = u32::try_from(arg_index).ok()?;
    }
    let mut present = Vec::with_capacity(params.len());
    let mut outputs = Vec::new();
    for (index, param) in params.iter().enumerate() {
        if param.direction > 2 {
            return None;
        }
        let actual = formal_args[index] != u32::MAX;
        present.push(param.default_const_idx.is_some() || (param.direction != 1 && actual));
        if matches!(param.direction, 1 | 2) && actual {
            outputs.push((u32::try_from(index).ok()?, param.direction));
        }
    }
    Some(CallArguments {
        formal_args,
        present,
        outputs,
    })
}

/// The function-block POU a declared receiver type resolves to, through aliases.
fn function_block_pou(vm: &VmModule, mut type_id: u32) -> Option<u32> {
    for _ in 0..=crate::bytecode::BYTECODE_MAX_TYPE_NESTING {
        let entry = vm.types.entries.get(type_id as usize)?;
        match &entry.data {
            TypeData::Alias { target_type_id } => type_id = *target_type_id,
            TypeData::Pou { pou_id } if entry.kind == TypeKind::FunctionBlock => {
                return Some(*pou_id)
            }
            _ => return None,
        }
    }
    None
}

/// The declared type of a member of `owner` (or an ancestor), from the storage layout.
fn declared_member_type(
    vm: &VmModule,
    layout: &StorageLayout,
    owner: u32,
    name: &str,
) -> Option<u32> {
    let mut current = Some(owner);
    for _ in 0..=vm.pou_by_id.len() {
        let pou = current?;
        for declaration in &layout.entries {
            if declaration.owner == StorageOwner::Instance
                && declaration.role == StorageRole::Variable
                && declaration.owner_pou_id == Some(pou)
                && vm
                    .strings
                    .get(declaration.name_idx as usize)
                    .is_some_and(|declared| declared.eq_ignore_ascii_case(name))
            {
                return declaration.type_id;
            }
        }
        current = vm.parent_pou_ids.get(&pou).copied();
    }
    None
}

/// The single class-like owner declaring `method_upper`, when exactly one does.
fn sole_method_owner(vm: &VmModule, method_upper: &str) -> Option<(u32, u32)> {
    let mut found = None;
    for (owner, name, pou) in vm.method_table_by_owner.iter() {
        if vm.strings[name as usize].eq_ignore_ascii_case(method_upper) {
            if found.is_some() {
                return None;
            }
            found = Some((owner, pou));
        }
    }
    found
}

/// The function-block POU whose prepared name matches `name`, without a type-name key.
pub(crate) fn function_block_pou_by_name(module: &VmModule, name: &str) -> Option<u32> {
    module.function_block_ids.get(&module.strings, name)
}

/// The class POU whose prepared name matches `name`, without a type-name key.
pub(crate) fn class_pou_by_name(module: &VmModule, name: &str) -> Option<u32> {
    module.class_ids.get(&module.strings, name)
}

/// The module's POU of `kind` whose name matches `name`, case-insensitively (test support).
#[cfg(test)]
fn pou_by_name(module: &VmModule, kind: crate::bytecode::PouKind, name: &str) -> Option<u32> {
    module.pou_by_id.iter().find_map(|(id, pou)| {
        (pou.kind == kind && pou.name.eq_ignore_ascii_case(name)).then_some(*id)
    })
}

impl crate::memory::figure::HeapBytes for CallPlan {
    fn add_heap(&self, count: &mut crate::memory::figure::HeapCount) {
        count.of(&self.arguments.formal_args);
        count.of(&self.arguments.present);
        count.of(&self.arguments.outputs);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bytecode::PouKind;
    use crate::vm::{PreparationLimits, PreparedModule};
    use smol_str::SmolStr;

    fn prepared(bytes: &[u8]) -> PreparedModule {
        PreparedModule::from_bytes(bytes, PreparationLimits::default()).expect("admit the fixture")
    }

    fn blocks() -> PreparedModule {
        prepared(include_bytes!(
            "../../../../trust-runtime/tests/fixtures/bounded-c/blocks/program.stbc"
        ))
    }

    fn methods() -> PreparedModule {
        prepared(include_bytes!(
            "../../../../trust-runtime/tests/fixtures/bounded-c/methods/program.stbc"
        ))
    }

    #[test]
    fn named_and_positional_actuals_resolve_to_the_same_formal_indexes() {
        let params = vec![
            VmParamMeta {
                name: SmolStr::new("value"),
                type_id: 0,
                direction: 0,
                default_const_idx: None,
            },
            VmParamMeta {
                name: SmolStr::new("factor"),
                type_id: 0,
                direction: 0,
                default_const_idx: None,
            },
        ];
        // `F(value)` and `F(value := value)` both supply the first formal and omit the second.
        let positional = vec![VmNativeArgSpec {
            name: None,
            is_target: false,
        }];
        let named = vec![VmNativeArgSpec {
            name: Some(SmolStr::new("value")),
            is_target: false,
        }];
        let positional = build_arguments(&params, &positional).expect("positional call plans");
        let named = build_arguments(&params, &named).expect("named call plans");
        assert_eq!(positional.actual(0), Some(0));
        assert_eq!(named.actual(0), Some(0));
        assert_eq!(positional.actual(1), None);
        assert_eq!(named.actual(1), None);
        assert_eq!(positional.present(), [true, false].as_slice());
        assert_eq!(named.present(), [true, false].as_slice());
    }

    #[test]
    fn an_fb_call_dispatches_by_its_instances_template_pou() {
        let prepared = blocks();
        let pulse = pou_by_name(&prepared.vm, PouKind::FunctionBlock, "Pulse")
            .expect("the fixture declares the Pulse function block");
        let plan = prepared
            .call_plans
            .iter()
            .find_map(|(_, plan)| match plan.callee {
                CallCallee::FunctionBlock { pou, builtin: None } if pou == pulse => Some(plan),
                _ => None,
            })
            .expect("the pulse call has a prepared plan");
        // Pulse declares (trigger, increment, count, active); the call names all four in order.
        // Outputs carry no input value, so only the two inputs are present.
        assert_eq!(plan.arguments.actual(0), Some(0));
        assert_eq!(plan.arguments.actual(1), Some(1));
        assert_eq!(plan.arguments.actual(2), Some(2));
        assert_eq!(plan.arguments.actual(3), Some(3));
        assert_eq!(
            plan.arguments.present(),
            [true, true, false, false].as_slice()
        );
        assert_eq!(plan.arguments.output_direction(0), None);
        assert_eq!(plan.arguments.output_direction(2), Some(1));
        assert_eq!(plan.arguments.output_direction(3), Some(1));
    }

    #[test]
    fn a_method_call_resolves_through_the_prepared_method_table() {
        let prepared = methods();
        let raise = pou_by_name(&prepared.vm, PouKind::Method, "Raise")
            .expect("the fixture declares the Raise method");
        let (owner, plan) = prepared
            .call_plans
            .iter()
            .find_map(|(_, plan)| match plan.callee {
                CallCallee::Method { owner, pou } if pou == raise => Some((owner, plan)),
                _ => None,
            })
            .expect("the Raise call has a prepared plan");
        assert_eq!(
            prepared.vm.resolve_method_pou_id(owner, "RAISE"),
            Some(raise)
        );
        // Raise declares one input, delta.
        assert_eq!(plan.arguments.actual(0), Some(0));
        assert_eq!(plan.arguments.present(), [true].as_slice());
    }
}
