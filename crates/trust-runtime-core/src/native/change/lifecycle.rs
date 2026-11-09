//! CODESYS's calls at an online change (specification 44 §12.3, step 4; specification 10 §4.8): the
//! change entries of the running and of the new program that run, from what the change does with
//! each top-level variable they name. A variable both programs declare is copied when its type's
//! layout differs, compared as the carry compares types (§12.2), and left alone otherwise; one that
//! only one program declares is dropped or added.

use alloc::vec::Vec;

use super::shape::same;
use super::Tables;
use crate::error::RuntimeError;
use crate::native::image::{EntryKind, EntryRecord};
use crate::native::values::damaged;

/// The positions in the entry tables of the change entries that run, in the order they run.
#[derive(Default)]
pub(super) struct Calls {
    /// The running program's change-exit entries.
    pub(super) exits: Vec<usize>,
    /// The new program's change-start entry; `None` when its image has no change entries, so that
    /// its cold-init entry starts it.
    pub(super) start: Option<usize>,
    /// The new program's change-init entries.
    pub(super) inits: Vec<usize>,
    /// The new program's change-reinit entries.
    pub(super) reinits: Vec<usize>,
}

/// What the change does with one top-level variable.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Fate {
    /// Both programs declare it with a type of the same layout.
    Kept,
    /// Both programs declare it, with types whose layouts differ.
    Copied,
    /// Only one program declares it: dropped from the running one, or added by the new one.
    Alone,
}

/// The change entries of `running` and `changed` that run at the change from `running` to
/// `changed`. A change-exit or change-init entry serves a copied variable when its index is odd and
/// a dropped or added one when it is even (§5.4).
///
/// # Errors
///
/// The error of an entry, symbol or type table of either image that does not read.
pub(super) fn calls(running: &Tables<'_>, changed: &Tables<'_>) -> Result<Calls, RuntimeError> {
    let mut calls = Calls::default();
    let old = running
        .image
        .table::<EntryRecord>()
        .map_err(|_| damaged("entry table"))?;
    for (position, entry) in old.iter().enumerate() {
        if entry.kind == EntryKind::ChangeExit as u32
            && serves(fate(running, entry.index / 2, changed, true)?, entry.index)
        {
            calls.exits.push(position);
        }
    }
    let new = changed
        .image
        .table::<EntryRecord>()
        .map_err(|_| damaged("entry table"))?;
    for (position, entry) in new.iter().enumerate() {
        if entry.kind == EntryKind::ChangeStart as u32 {
            calls.start = Some(position);
        } else if entry.kind == EntryKind::ChangeInit as u32
            && serves(fate(changed, entry.index / 2, running, false)?, entry.index)
        {
            calls.inits.push(position);
        } else if entry.kind == EntryKind::ChangeReinit as u32
            && fate(changed, entry.index, running, false)? == Fate::Copied
        {
            calls.reinits.push(position);
        }
    }
    Ok(calls)
}

/// Whether the change-exit or change-init entry of index `index` runs for a variable whose fate
/// is `fate`.
fn serves(fate: Fate, index: u32) -> bool {
    let copied = index % 2 == 1;
    matches!((fate, copied), (Fate::Copied, true) | (Fate::Alone, false))
}

/// What the change does with the variable of symbol `index` of `own`, which is the running
/// program's when `own_runs`, looked up by its canonical name in `other`.
fn fate(
    own: &Tables<'_>,
    index: u32,
    other: &Tables<'_>,
    own_runs: bool,
) -> Result<Fate, RuntimeError> {
    let symbol = own
        .symbols
        .get(index)
        .ok_or_else(|| damaged("symbol table"))?;
    let canonical = own
        .image
        .string(symbol.canonical)
        .ok_or_else(|| damaged("string pool"))?;
    let Some((_, theirs)) = other.image.find_symbol(canonical) else {
        return Ok(Fate::Alone);
    };
    let alike = if own_runs {
        same(
            &own.types,
            symbol.type_index,
            &other.types,
            theirs.type_index,
        )?
    } else {
        same(
            &other.types,
            theirs.type_index,
            &own.types,
            symbol.type_index,
        )?
    };
    Ok(if alike { Fate::Kept } else { Fate::Copied })
}
