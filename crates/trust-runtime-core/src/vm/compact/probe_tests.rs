//! Translation probe (measurement only): prints the compact code of every body of a compiled
//! program and, for each call the compact code gives the general call, its prepared plan, so a
//! bench program's translation can be read. Ignored in normal runs:
//! `TRUST_COMPACT_PROBE=<program.stbc> <test binary> compact_probe --ignored --nocapture`.

extern crate std;

use super::*;
use crate::vm::prepared::PreparationLimits;

#[test]
#[ignore = "a measurement probe"]
fn compact_probe() {
    let path = std::env::var("TRUST_COMPACT_PROBE").expect("TRUST_COMPACT_PROBE names a program");
    let artifact = std::fs::read(path).expect("the program reads");
    let prepared = PreparedModule::from_bytes(&artifact, PreparationLimits::default())
        .expect("the program prepares");
    let bodies = CompactCode::of_module(&prepared);
    for &pou in prepared.vm.pou_by_id.keys() {
        let Some((_, code)) = bodies.iter().find(|(id, _)| *id == pou) else {
            std::println!("POU {pou}: general path");
            continue;
        };
        std::println!(
            "POU {pou}: {} ops, {} slots, {} block calls",
            code.ops.len(),
            code.slots.len(),
            code.blocks.len()
        );
        for op in &code.ops {
            std::println!("  {op:?}");
            if !matches!(op.kind, OpKind::Call | OpKind::Convert) {
                continue;
            }
            let Some(plan) = prepared.call_plan(op.at) else {
                std::println!("    no plan");
                continue;
            };
            std::println!("    plan {plan:?}");
            if let CallCallee::FunctionBlock { pou, .. } = plan.callee {
                let mut scratch = Scratch::default();
                let translation = Translation {
                    prepared: &prepared,
                    pou,
                    s: &mut scratch,
                    writes: false,
                    count: 0,
                };
                let declarations = prepared.declarations(StorageOwner::Instance, Some(pou));
                for (index, &id) in declarations.iter().enumerate() {
                    let member = prepared
                        .declaration_offset(id)
                        .and_then(|offset| translation.scalar(id, offset, index as u32, None));
                    std::println!(
                        "    member {index} (declaration {id}): offset {:?}, {member:?}",
                        prepared.declaration_offset(id)
                    );
                }
                let type_ids: Vec<u32> = prepared
                    .vm
                    .types
                    .entries
                    .iter()
                    .enumerate()
                    .filter(|(_, entry)| {
                        matches!(entry.data, TypeData::Pou { pou_id } if pou_id == pou)
                    })
                    .map(|(id, _)| id as u32)
                    .collect();
                for type_id in type_ids {
                    std::println!(
                        "    type {type_id}: block_pou {:?}, loads_a_value {}",
                        translation.block_pou(type_id),
                        translation.loads_a_value(type_id)
                    );
                }
                std::println!("    formals {:?}", prepared.builtin_formals(pou));
                std::println!(
                    "    params {:?}",
                    prepared.vm.pou_params(pou).map(|params| params
                        .iter()
                        .map(|param| (param.name.clone(), param.direction))
                        .collect::<Vec<_>>())
                );
                std::println!(
                    "    declarations {:?}",
                    prepared.declarations(StorageOwner::Instance, Some(pou))
                );
            }
            std::println!(
                "    spec {:?}",
                prepared.vm.native_symbol_spec(plan.symbol_idx)
            );
        }
        for block in &code.blocks {
            std::println!("  {block:?}");
        }
    }
}

/// Services that keep the compact loop: no statement observer, no deadline, no wall clock.
struct Quiet;

impl crate::vm::ExecutionServices for Quiet {
    fn debug_active(&self) -> bool {
        false
    }
    fn observes_statements(&self) -> bool {
        false
    }
    fn deadline_exceeded(&self) -> bool {
        false
    }
    fn has_wall_clock(&self) -> bool {
        false
    }
    fn current_dt(&self) -> Result<crate::value::DateTimeValue, crate::error::RuntimeError> {
        Err(crate::error::RuntimeError::UndefinedFunction(
            "CURRENT_DT".into(),
        ))
    }
}

/// Runs the program of `TRUST_COMPACT_PROBE` for three cycles of 100 ms, so a diagnostic build can
/// print what its scans do: `<test binary> compact_run_probe --ignored --nocapture`.
#[test]
#[ignore = "a measurement probe"]
fn compact_run_probe() {
    let path = std::env::var("TRUST_COMPACT_PROBE").expect("TRUST_COMPACT_PROBE names a program");
    let artifact = std::fs::read(path).expect("the program reads");
    let prepared = PreparedModule::from_bytes(&artifact, PreparationLimits::default())
        .expect("the program prepares");
    let mut state = prepared
        .instantiate_with_services(0, &Quiet)
        .expect("the program constructs");
    for cycle in 1..=3 {
        let result = state.execute_cycle(crate::value::Duration::from_millis(100 * cycle));
        std::println!("cycle {cycle}: {result:?}");
    }
}
