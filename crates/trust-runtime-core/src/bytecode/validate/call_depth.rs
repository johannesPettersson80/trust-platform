//! The module's static call depth, which sizes the engine's per-call reservations.
//!
//! Construction reserves the stack executor's operand, frame and pending-call storage, the
//! initializer stacks and the engine's call-frame and instance pre-reservation from the
//! program's real call depth rather than the admitted ceiling, so a chip does not need the
//! ceiling's memory (spec 34 MEM-02, MEM-09, D-C1-4). The walk follows every call target from
//! every entry and, conservatively, of every POU in the module, so a target reached only by
//! method or interface dispatch still counts: a function, function block or method call, a
//! method or interface call over every POU that can implement it, and a nested instance
//! declaration as the initialization it runs. A cycle, which IEC 61131-3 forbids but the engine
//! still executes, makes the depth unbounded, so the caller reserves for `max_call_depth`.

use super::*;

use crate::bytecode::{
    CallDepth, StorageLayout, StorageRole, BYTECODE_MAX_TYPE_NESTING, NATIVE_CALL_KIND_METHOD,
};

/// Call and construction edges between the module's POUs and construction-only roots.
struct CallGraph {
    edges: Vec<Vec<usize>>,
}

impl CallGraph {
    /// One node per POU index entry, filled lazily by the walks below.
    fn with_pous(pou_count: usize, budget: &mut ValidationBudget) -> Result<Self, BytecodeError> {
        let mut edges = Vec::new();
        budget.reserve(&mut edges, pou_count)?;
        edges.resize_with(pou_count, Vec::new);
        Ok(Self { edges })
    }

    /// A construction-only node for a global or resource initializer with no owning POU.
    fn add_root(&mut self, budget: &mut ValidationBudget) -> Result<usize, BytecodeError> {
        budget.push(&mut self.edges, Vec::new())?;
        Ok(self.edges.len() - 1)
    }

    fn add_position(
        &mut self,
        from: usize,
        to: usize,
        budget: &mut ValidationBudget,
    ) -> Result<(), BytecodeError> {
        budget.push(&mut self.edges[from], to)
    }

    fn add_pou_edge(
        &mut self,
        from: usize,
        pou_id: u32,
        tables: &ValidationContext<'_>,
        budget: &mut ValidationBudget,
    ) -> Result<(), BytecodeError> {
        if let Some(position) = tables.pou_position(pou_id, budget)? {
            self.add_position(from, position, budget)?;
        }
        Ok(())
    }

    /// Every POU of one kind, the conservative target of a call whose callee cannot be
    /// resolved statically; a cycle this creates falls back to the ceiling.
    fn add_kind_edges(
        &mut self,
        from: usize,
        kind: PouKind,
        tables: &ValidationContext<'_>,
        budget: &mut ValidationBudget,
    ) -> Result<(), BytecodeError> {
        for entry in &tables.index.entries {
            budget.work(1)?;
            if entry.kind == kind {
                self.add_pou_edge(from, entry.id, tables, budget)?;
            }
        }
        Ok(())
    }
}

/// Compute the module's static call depth for the caller's reservations (spec 34 MEM-09).
pub(super) fn module_call_depth(
    tables: &ValidationContext<'_>,
    layout: Option<&StorageLayout>,
    bodies: &[u8],
    budget: &mut ValidationBudget,
) -> Result<CallDepth, BytecodeError> {
    let mut graph = CallGraph::with_pous(tables.index.entries.len(), budget)?;
    collect_pou_body_edges(tables, bodies, &mut graph, budget)?;
    collect_initializer_edges(tables, bodies, &mut graph, budget)?;
    if let Some(layout) = layout {
        collect_construction_edges(tables, layout, &mut graph, budget)?;
    }
    collect_inheritance_edges(tables, &mut graph, budget)?;
    depth_of(&graph, budget)
}

/// Every POU body's `CALL_NATIVE` targets.
fn collect_pou_body_edges(
    tables: &ValidationContext<'_>,
    bodies: &[u8],
    graph: &mut CallGraph,
    budget: &mut ValidationBudget,
) -> Result<(), BytecodeError> {
    // The framing and resource-limit checks ran in the ordinary POU pass; this second walk
    // only reads the admitted instructions, so a fresh counter cannot reject a valid module.
    let mut decoded = 0usize;
    for entry in &tables.index.entries {
        budget.work(1)?;
        let Some(position) = tables.pou_position(entry.id, budget)? else {
            continue;
        };
        let instructions = decode_pou_code(
            bodies,
            entry.code_offset,
            entry.code_length,
            &mut decoded,
            budget,
        )?;
        collect_call_edges(tables, Some(entry), position, &instructions, graph, budget)?;
    }
    Ok(())
}

/// Calls an initializer body makes count as the calls of its owner (spec 34 MEM-09).
fn collect_initializer_edges(
    tables: &ValidationContext<'_>,
    bodies: &[u8],
    graph: &mut CallGraph,
    budget: &mut ValidationBudget,
) -> Result<(), BytecodeError> {
    let Some(initializers) = tables.initializers else {
        return Ok(());
    };
    let mut decoded = 0usize;
    for entry in &initializers.entries {
        budget.work(1)?;
        let owner = match entry.owner_pou_id {
            Some(id) => tables.pou(id, budget)?,
            None => None,
        };
        let from = match owner {
            Some(pou) => match tables.pou_position(pou.id, budget)? {
                Some(position) => position,
                None => graph.add_root(budget)?,
            },
            None => graph.add_root(budget)?,
        };
        let instructions = decode_pou_code(
            bodies,
            entry.code_offset,
            entry.code_length,
            &mut decoded,
            budget,
        )?;
        collect_call_edges(tables, owner, from, &instructions, graph, budget)?;
    }
    Ok(())
}

/// A declaration of a function-block or class type nests one construction level beneath its
/// owner; the layout's transitive type structure gives the constructed POUs.
fn collect_construction_edges(
    tables: &ValidationContext<'_>,
    layout: &StorageLayout,
    graph: &mut CallGraph,
    budget: &mut ValidationBudget,
) -> Result<(), BytecodeError> {
    for declaration in &layout.entries {
        budget.work(1)?;
        if declaration.role == StorageRole::External {
            continue;
        }
        let Some(type_id) = declaration.type_id else {
            continue;
        };
        let from = match declaration.owner_pou_id {
            Some(id) => match template_position(tables, id, budget)? {
                Some(position) => position,
                None => graph.add_root(budget)?,
            },
            None => graph.add_root(budget)?,
        };
        let mut targets = Vec::new();
        collect_type_pous(tables, type_id, 0, 0, &mut targets, budget)?;
        for (target, levels) in targets {
            // A composite declaration nests the constructed instance one level deeper beneath
            // its owner, so the depth chain carries that many edges (spec 34 MEM-09).
            let mut current = from;
            for _ in 1..levels {
                current = graph.add_root(budget)?;
            }
            graph.add_pou_edge(current, target, tables, budget)?;
        }
    }
    Ok(())
}

/// A class-like POU constructs its parent's declarations as well.
fn collect_inheritance_edges(
    tables: &ValidationContext<'_>,
    graph: &mut CallGraph,
    budget: &mut ValidationBudget,
) -> Result<(), BytecodeError> {
    for entry in &tables.index.entries {
        budget.work(1)?;
        let Some(parent) = entry
            .class_meta
            .as_ref()
            .and_then(|meta| meta.parent_pou_id)
        else {
            continue;
        };
        let Some(position) = tables.pou_position(entry.id, budget)? else {
            continue;
        };
        graph.add_pou_edge(position, parent, tables, budget)?;
    }
    Ok(())
}

/// The template-owning position of a declaration owner: a method's declarations belong to
/// its class-like POU, as construction treats them.
fn template_position(
    tables: &ValidationContext<'_>,
    owner: u32,
    budget: &mut ValidationBudget,
) -> Result<Option<usize>, BytecodeError> {
    let Some(pou) = tables.pou(owner, budget)? else {
        return Ok(None);
    };
    let template = if pou.kind == PouKind::Method {
        pou.owner_pou_id.unwrap_or(owner)
    } else {
        owner
    };
    tables.pou_position(template, budget)
}

/// The function-block or class POUs a declaration's type can construct, through arrays,
/// structures, unions and aliases, each with the construction levels it nests. An alias is
/// transparent and adds no level; an array, structure or union nests one. A reference or
/// interface owns no constructible storage.
fn collect_type_pous(
    tables: &ValidationContext<'_>,
    type_id: u32,
    depth: usize,
    level: usize,
    out: &mut Vec<(u32, usize)>,
    budget: &mut ValidationBudget,
) -> Result<(), BytecodeError> {
    if depth > BYTECODE_MAX_TYPE_NESTING {
        return Err(RejectionReason::ConstructionDemandOverflow.into());
    }
    budget.work(1)?;
    let Some(entry) = tables.types.entries.get(type_id as usize) else {
        return Err(BytecodeError::InvalidIndex {
            kind: "type".into(),
            index: type_id,
        });
    };
    match &entry.data {
        TypeData::Alias { target_type_id } => {
            collect_type_pous(tables, *target_type_id, depth + 1, level, out, budget)
        }
        TypeData::Subrange {
            base_type_id: target_type_id,
            ..
        }
        | TypeData::Enum {
            base_type_id: target_type_id,
            ..
        } => collect_type_pous(tables, *target_type_id, depth + 1, level, out, budget),
        TypeData::Array { elem_type_id, .. } => {
            collect_type_pous(tables, *elem_type_id, depth + 1, level + 1, out, budget)
        }
        TypeData::Struct { fields } | TypeData::Union { fields } => {
            for field in fields {
                budget.work(1)?;
                collect_type_pous(tables, field.type_id, depth + 1, level + 1, out, budget)?;
            }
            Ok(())
        }
        TypeData::Pou { pou_id }
            if matches!(entry.kind, TypeKind::FunctionBlock | TypeKind::Class) =>
        {
            budget.push(out, (*pou_id, level + 1))
        }
        _ => Ok(()),
    }
}

/// The `CALL_NATIVE` target of every instruction in one decoded body.
fn collect_call_edges(
    tables: &ValidationContext<'_>,
    owner: Option<&PouEntry>,
    from: usize,
    instructions: &[Instruction],
    graph: &mut CallGraph,
    budget: &mut ValidationBudget,
) -> Result<(), BytecodeError> {
    for instruction in instructions {
        budget.work(1)?;
        if instruction.opcode != 0x09 {
            continue;
        }
        let kind = instruction.operand(0);
        let symbol_idx = instruction.operand(1);
        let Some(symbol) = tables.strings.entries.get(symbol_idx as usize) else {
            continue;
        };
        // A native-call symbol is `target|E...|T...`; the call graph needs only the target.
        let target_name = symbol.as_str().split('|').next().unwrap_or_default();
        if target_name.is_empty() {
            continue;
        }
        budget.work(target_name.len())?;
        match kind {
            NATIVE_CALL_KIND_FUNCTION => {
                if let Some(id) = tables.named_pou(PouKind::Function, target_name, budget)? {
                    graph.add_pou_edge(from, id, tables, budget)?;
                } else {
                    graph.add_kind_edges(from, PouKind::Function, tables, budget)?;
                }
            }
            NATIVE_CALL_KIND_FUNCTION_BLOCK => {
                add_function_block_targets(tables, owner, from, target_name, graph, budget)?;
            }
            NATIVE_CALL_KIND_METHOD => {
                add_method_targets(tables, from, target_name, graph, budget)?;
            }
            _ => {}
        }
    }
    Ok(())
}

/// A function-block call names the receiver's declared type when the validator can resolve
/// it, so the template POU is known; otherwise every function block is a possible callee.
fn add_function_block_targets(
    tables: &ValidationContext<'_>,
    owner: Option<&PouEntry>,
    from: usize,
    target_name: &str,
    graph: &mut CallGraph,
    budget: &mut ValidationBudget,
) -> Result<(), BytecodeError> {
    if let Some(owner) = owner
        && let Some(type_id) = tables.declared_call_type(owner, target_name, budget)?
        && let Some(pou_id) = tables.function_block_type(type_id, budget)?
    {
        return graph.add_pou_edge(from, pou_id, tables, budget);
    }
    if let Some(pou_id) = tables.named_pou(PouKind::FunctionBlock, target_name, budget)? {
        return graph.add_pou_edge(from, pou_id, tables, budget);
    }
    graph.add_kind_edges(from, PouKind::FunctionBlock, tables, budget)
}

/// A method or interface call reaches every POU that declares a method of that name; a name
/// that matches none, or a class-like receiver resolved only at run time, falls back to every
/// function block and class, whose cycle the depth walk then turns into the ceiling.
fn add_method_targets(
    tables: &ValidationContext<'_>,
    from: usize,
    target_name: &str,
    graph: &mut CallGraph,
    budget: &mut ValidationBudget,
) -> Result<(), BytecodeError> {
    let mut found = false;
    for entry in &tables.index.entries {
        budget.work(1)?;
        let Some(meta) = &entry.class_meta else {
            continue;
        };
        for method in &meta.methods {
            budget.work(1)?;
            let Some(name) = tables.strings.entries.get(method.name_idx as usize) else {
                continue;
            };
            if budget.compare_names(name.as_str(), target_name)?.is_eq() {
                graph.add_pou_edge(from, method.pou_id, tables, budget)?;
                found = true;
            }
        }
    }
    if !found {
        graph.add_kind_edges(from, PouKind::FunctionBlock, tables, budget)?;
        graph.add_kind_edges(from, PouKind::Class, tables, budget)?;
    }
    Ok(())
}

/// Decode one admitted body's instructions from the shared POU-bodies section.
fn decode_pou_code(
    bodies: &[u8],
    code_offset: u32,
    code_length: u32,
    decoded: &mut usize,
    budget: &mut ValidationBudget,
) -> Result<Vec<Instruction>, BytecodeError> {
    let start = code_offset as usize;
    let end = start
        .checked_add(code_length as usize)
        .ok_or_else(|| BytecodeError::from(RejectionReason::CodePositionOverflow))?;
    let code = bodies
        .get(start..end)
        .ok_or_else(|| BytecodeError::from(RejectionReason::PouCodeOutOfBounds))?;
    decode_instructions(code, decoded, budget, |_, _| Ok(()))
}

/// Each node's depth is one plus the deepest of its callees'; a cycle has no depth.
fn depth_of(graph: &CallGraph, budget: &mut ValidationBudget) -> Result<CallDepth, BytecodeError> {
    const WHITE: u8 = 0;
    const GRAY: u8 = 1;
    const BLACK: u8 = 2;
    let node_count = graph.edges.len();
    let mut state = Vec::new();
    budget.reserve(&mut state, node_count)?;
    state.resize(node_count, WHITE);
    let mut depth = Vec::new();
    budget.reserve(&mut depth, node_count)?;
    depth.resize(node_count, 0usize);
    let mut stack: Vec<(usize, usize)> = Vec::new();
    let mut deepest = 1usize;
    for start in 0..node_count {
        if state[start] != WHITE {
            continue;
        }
        budget.work(1)?;
        state[start] = GRAY;
        stack.push((start, 0));
        while let Some(&(node, cursor)) = stack.last() {
            if cursor < graph.edges[node].len() {
                let callee = graph.edges[node][cursor];
                if let Some(top) = stack.last_mut() {
                    top.1 += 1;
                }
                budget.work(1)?;
                match state[callee] {
                    WHITE => {
                        state[callee] = GRAY;
                        stack.push((callee, 0));
                    }
                    GRAY => return Ok(CallDepth::Recursive),
                    _ => depth[node] = depth[node].max(depth[callee].saturating_add(1)),
                }
            } else {
                stack.pop();
                state[node] = BLACK;
                if depth[node] == 0 {
                    depth[node] = 1;
                }
                deepest = deepest.max(depth[node]);
                if let Some(parent) = stack.last_mut() {
                    depth[parent.0] = depth[parent.0].max(depth[node].saturating_add(1));
                }
            }
        }
    }
    Ok(CallDepth::Static(deepest))
}
