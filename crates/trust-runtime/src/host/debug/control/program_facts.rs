//! What Control Studio's services read of the program a resource runs, from the program's image
//! rather than its sources (specification 44 §5.2, §5.6, §5.7; specification 35 A2.2, A2.3, A6.8):
//! a runtime may run a deployed image without its sources, so the program's revision, its tasks and
//! whether a member may take a live write come from the image's header, task table, symbol table and
//! process-image bindings.

use std::time::Duration;

use smol_str::SmolStr;
use trust_runtime_core::native::image::{
    EntryKind, EntryRecord, Image, IoBindRecord, SymbolRecord, TaskRecord, TypeRecord,
    BACKGROUND_PRIORITY, BIND_OFFSET_SHIFT, NONE, SYMBOL_CONSTANT, SYMBOL_DIRECT, SYMBOL_IN_OUT,
    SYMBOL_PROGRAM,
};
use trust_runtime_core::native::memory::Place;

use crate::io::IoAddress;

/// The revision of the program's sources: the SHA-256 of the project's sources and settings that
/// the image's header carries (specification 44 §5.2). Two images of other sources differ in it, and
/// a rebuild of the same sources keeps it.
pub(crate) fn source_revision(image: &Image<'_>) -> String {
    format!("sha256:{}", hex::encode(image.header().source_hash))
}

/// One task of the program, as its image's task table declares it (specification 44 §5.6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProgramTask {
    /// The task's declared name.
    pub(crate) name: SmolStr,
    /// The task's interval; zero for an event task.
    pub(crate) interval: Duration,
    /// The program instances the task runs, by their declared names, in the task's order.
    pub(crate) programs: Vec<SmolStr>,
}

/// The program's tasks in the image's order, the programs outside every task left out; none for an
/// image whose tables do not read.
pub(crate) fn program_tasks(image: &Image<'_>) -> Vec<ProgramTask> {
    let (Ok(tasks), Ok(entries), Ok(symbols)) = (
        image.table::<TaskRecord>(),
        image.table::<EntryRecord>(),
        image.table::<SymbolRecord>(),
    ) else {
        return Vec::new();
    };
    tasks
        .iter()
        .filter(|task| task.priority != BACKGROUND_PRIORITY)
        .map(|task| ProgramTask {
            name: SmolStr::new(image.string(task.name).unwrap_or_default()),
            interval: Duration::from_nanos(task.interval_ns),
            programs: image
                .task_programs(&task)
                .filter_map(|entry| {
                    let entry = entries.get(entry)?;
                    if entry.kind != EntryKind::ProgramBody as u32 {
                        return None;
                    }
                    symbols
                        .iter()
                        .find(|symbol| {
                            symbol.flags & SYMBOL_PROGRAM != 0
                                && symbol.parent == NONE
                                && symbol.pou == entry.index
                        })
                        .and_then(|symbol| image.string(symbol.display))
                        .map(SmolStr::new)
                })
                .collect(),
        })
        .collect()
}

/// Whether `name` names a program instance of the image, compared without case.
pub(crate) fn is_program_instance(image: &Image<'_>, name: &str) -> bool {
    image.find_program_instance(name).is_some()
}

/// Whether `name` names a global variable or a program instance of the image, compared without
/// case: the first name of a Control Studio symbol path (specification 35, control signal reads).
pub(crate) fn is_global_or_program(image: &Image<'_>, name: &str) -> bool {
    image
        .find_symbol(&name.to_ascii_uppercase())
        .is_some_and(|(_, symbol)| {
            symbol.parent == NONE
                && (symbol.flags & SYMBOL_PROGRAM != 0
                    || (symbol.pou == NONE && (1..=6).contains(&symbol.area)))
        })
}

/// What the declarations along a member path of a program instance say of the member a Control
/// Studio binding names (specification 35 A6.8; protocol annex E6.2, E7.1).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct BindingLeaf {
    /// A declaration along the path is `CONSTANT`.
    pub(crate) constant: bool,
    /// A declaration along the path is `VAR_IN_OUT`.
    pub(crate) in_out: bool,
    /// A declaration along the path is located (`AT %…`).
    pub(crate) direct: bool,
    /// The process-image address of the outermost located declaration along the path, when the
    /// image binds it.
    pub(crate) address: Option<IoAddress>,
}

/// The member `path` names, `Program.member.field`: a declared member of a program instance, or a
/// member or field below one, each part a declaration with a symbol of its own; `None` for a path
/// that names no such member, a `VAR_EXTERNAL` included, whose program holds no variable of its
/// own (it names the global).
pub(crate) fn binding_leaf(image: &Image<'_>, path: &str) -> Option<BindingLeaf> {
    let parts: Vec<&str> = path.split('.').collect();
    if parts.len() < 2 || parts.iter().any(|part| part.is_empty()) {
        return None;
    }
    let mut canonical = parts[0].to_ascii_uppercase();
    let (_, root) = image.find_symbol(&canonical)?;
    if root.flags & SYMBOL_PROGRAM == 0 || root.parent != NONE {
        return None;
    }
    let bindings = image.table::<IoBindRecord>().ok()?;
    let mut leaf = BindingLeaf::default();
    for part in &parts[1..] {
        canonical.push('.');
        canonical.push_str(&part.to_ascii_uppercase());
        let (index, symbol) = image.find_symbol(&canonical)?;
        leaf.constant |= symbol.flags & SYMBOL_CONSTANT != 0;
        leaf.in_out |= symbol.flags & SYMBOL_IN_OUT != 0;
        let bound = bindings.iter().find(|binding| binding.symbol == index);
        leaf.direct |= symbol.flags & SYMBOL_DIRECT != 0 || bound.is_some();
        if leaf.address.is_none() {
            leaf.address = bound.and_then(|binding| image.binding_address(&binding));
        }
    }
    Some(leaf)
}

/// Whether `path` names a member a parameter set or an experiment may write: declared, and neither
/// constant, in-out nor located along its path (specification 35 A6.8; protocol annex E7.1).
pub(crate) fn parameter_writable(image: &Image<'_>, path: &str) -> bool {
    binding_leaf(image, path).is_some_and(|leaf| !leaf.constant && !leaf.in_out && !leaf.direct)
}

/// Every process-image binding of the image (specification 44 §5.6): the place of the variable,
/// member or element it binds, and its address.
pub(crate) fn bound_places(image: &Image<'_>) -> Vec<(Place, IoAddress)> {
    let (Ok(symbols), Ok(bindings)) =
        (image.table::<SymbolRecord>(), image.table::<IoBindRecord>())
    else {
        return Vec::new();
    };
    bindings
        .iter()
        .filter_map(|binding| {
            let symbol = symbols.get(binding.symbol)?;
            let address = image.binding_address(&binding)?;
            let place = Place::Area {
                area: symbol.area,
                offset: symbol
                    .offset
                    .saturating_add(binding.flags >> BIND_OFFSET_SHIFT),
            };
            Some((place, address))
        })
        .collect()
}

/// The bytes a value of type `type_index` takes; `None` for a type the type table does not hold, a
/// program instance's.
pub(crate) fn type_size(image: &Image<'_>, type_index: u32) -> Option<u32> {
    image
        .table::<TypeRecord>()
        .ok()?
        .get(type_index)
        .map(|record| record.size)
}

#[cfg(test)]
#[path = "program_facts/tests.rs"]
mod tests;
