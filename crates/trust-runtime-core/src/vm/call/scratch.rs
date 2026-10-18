//! Reusable per-depth call-binding scratch (spec 34 MEM-09; census C1, C3, C4, C5, C7).
//!
//! Construction reserves one vector of each call-binding kind per call depth: the evaluated
//! argument list, the parameter flags, the function and function-block output bindings, the
//! prepared output targets, the prepared copy-back outputs and the standard-library values. A
//! binding takes the vector of its kind and call depth and returns it on every path, so a scan
//! allocates none of them. Each kind
//! tracks its own outstanding count, so a nested call of the same kind takes the next depth's
//! vector; a depth beyond the reservation falls back to a fresh vector, exactly as a binding
//! without the reservation allocates today.
use alloc::vec::Vec;

use crate::error::RuntimeError;
use crate::value::Value;

use super::super::VmTrap;
use super::bindings::{VmFbOutBinding, VmNativeArg, VmOutBinding, VmWriteTarget};

/// The argument-list kind index.
#[cfg(test)]
pub(crate) const KIND_ARGS: usize = 0;
/// The parameter-flag kind index: built-in consumption bits and user presence bits.
#[cfg(test)]
pub(crate) const KIND_FLAGS: usize = 1;
/// The function output-binding kind index.
#[cfg(test)]
pub(crate) const KIND_FUNCTION_OUTPUTS: usize = 2;
/// The standard-library values kind index.
#[cfg(test)]
pub(crate) const KIND_STDLIB_VALUES: usize = 6;
/// The number of call-binding vector kinds.
#[cfg(test)]
pub(crate) const KIND_COUNT: usize = 7;

/// The widest vector of each kind the reserved call scratch holds (spec 34 MEM-09).
#[derive(Debug, Clone, Copy)]
pub struct CallScratchSizes {
    /// The most actual arguments of any call site.
    pub args: usize,
    /// The most parameters of any POU.
    pub params: usize,
    /// The most local slots of any POU: the presence bits' width.
    pub slots: usize,
}

impl CallScratchSizes {
    /// The presence-bit width: the local slots, but at least the arguments, because a built-in
    /// call's consumption bits share the same flag vectors.
    pub(crate) fn flags(self) -> usize {
        self.slots.max(self.args)
    }
}

/// One kind's reserved vectors, one per call depth, and its outstanding count (spec 34 MEM-09).
#[derive(Debug, Default)]
struct KindScratch<T> {
    /// The reserved vectors; a depth out on an active call is `None`.
    slots: Vec<Option<T>>,
    /// The number of vectors of this kind out on active calls; the next take uses this depth.
    out: usize,
    /// The most vectors of this kind ever out at once (test support).
    #[cfg(test)]
    peak: usize,
}

impl<T: Default> KindScratch<T> {
    /// Take the vector of the next depth, empty. A depth beyond the reservation is fresh.
    fn take(&mut self) -> T {
        let depth = self.out;
        self.out = depth.saturating_add(1);
        #[cfg(test)]
        {
            self.peak = self.peak.max(self.out);
        }
        self.slots
            .get_mut(depth)
            .and_then(Option::take)
            .unwrap_or_default()
    }

    /// Return a taken vector at its depth, emptied by the caller. An extra return is ignored.
    fn recycle(&mut self, value: T) {
        if self.out == 0 {
            return;
        }
        self.out -= 1;
        if let Some(slot) = self.slots.get_mut(self.out) {
            *slot = Some(value);
        }
    }

    /// The reserved vectors still held, empty between calls (test support).
    #[cfg(test)]
    fn held(&self) -> usize {
        self.slots.iter().filter(|slot| slot.is_some()).count()
    }
}

/// One call's preflighted output targets: a caller destination and its declared type (spec 34
/// MEM-09; census C4).
pub(crate) type PreparedTargets = Vec<Option<(VmWriteTarget, Option<u32>)>>;

/// One reusable call-binding vector of each kind per call depth, reserved at construction
/// (spec 34 MEM-09).
#[derive(Debug, Default)]
pub struct CallScratch {
    args: KindScratch<Vec<VmNativeArg>>,
    flags: KindScratch<Vec<bool>>,
    function_outputs: KindScratch<Vec<VmOutBinding>>,
    fb_outputs: KindScratch<Vec<VmFbOutBinding>>,
    prepared_targets: KindScratch<PreparedTargets>,
    prepared_outputs: KindScratch<Vec<(VmWriteTarget, Value)>>,
    stdlib_values: KindScratch<Vec<Value>>,
}

impl CallScratch {
    /// Reserve one vector of each kind per call depth, sized by `sizes` (spec 34 MEM-09). Every
    /// take at a reserved depth then makes no allocation.
    pub(crate) fn reserve(
        &mut self,
        levels: usize,
        sizes: CallScratchSizes,
    ) -> Result<(), RuntimeError> {
        let flags = sizes.flags();
        reserve_kind(&mut self.args.slots, levels, || reserve_vec(sizes.args))?;
        reserve_kind(&mut self.flags.slots, levels, || reserve_vec(flags))?;
        reserve_kind(&mut self.function_outputs.slots, levels, || {
            reserve_vec(sizes.params)
        })?;
        reserve_kind(&mut self.fb_outputs.slots, levels, || {
            reserve_vec(sizes.params)
        })?;
        reserve_kind(&mut self.prepared_targets.slots, levels, || {
            reserve_vec(sizes.params)
        })?;
        reserve_kind(&mut self.prepared_outputs.slots, levels, || {
            reserve_vec(sizes.params)
        })?;
        reserve_kind(&mut self.stdlib_values.slots, levels, || {
            reserve_vec(sizes.args)
        })?;
        Ok(())
    }

    /// Take the evaluated argument list of a call being bound, empty (spec 34 MEM-09; census C1).
    pub(crate) fn take_args(&mut self) -> Vec<VmNativeArg> {
        self.args.take()
    }

    /// Return the argument list of [`CallScratch::take_args`], emptied.
    pub(crate) fn recycle_args(&mut self, mut args: Vec<VmNativeArg>) {
        args.clear();
        self.args.recycle(args);
    }

    /// Take `count` false parameter flags — the built-in consumption bits or the user presence
    /// bits (spec 34 MEM-09; census C3, C5).
    pub(crate) fn take_flags(&mut self, count: usize) -> Vec<bool> {
        let mut flags = self.flags.take();
        flags.clear();
        flags.resize(count, false);
        flags
    }

    /// Return the flags of [`CallScratch::take_flags`], emptied.
    pub(crate) fn recycle_flags(&mut self, mut flags: Vec<bool>) {
        flags.clear();
        self.flags.recycle(flags);
    }

    /// Take the output-binding list of a user function or method call, empty (spec 34 MEM-09;
    /// census C5).
    pub(crate) fn take_function_outputs(&mut self) -> Vec<VmOutBinding> {
        self.function_outputs.take()
    }

    /// Return the output bindings of [`CallScratch::take_function_outputs`], emptied.
    pub(crate) fn recycle_function_outputs(&mut self, mut outputs: Vec<VmOutBinding>) {
        outputs.clear();
        self.function_outputs.recycle(outputs);
    }

    /// Take the output-binding list of a function-block call, empty (spec 34 MEM-09; census C3,
    /// C4).
    pub(crate) fn take_fb_outputs(&mut self) -> Vec<VmFbOutBinding> {
        self.fb_outputs.take()
    }

    /// Return the output bindings of [`CallScratch::take_fb_outputs`], emptied.
    pub(crate) fn recycle_fb_outputs(&mut self, mut outputs: Vec<VmFbOutBinding>) {
        outputs.clear();
        self.fb_outputs.recycle(outputs);
    }

    /// Take the preflighted output-target list of a user function-block call, empty (spec 34
    /// MEM-09; census C4).
    pub(crate) fn take_prepared_targets(&mut self) -> PreparedTargets {
        self.prepared_targets.take()
    }

    /// Return the prepared targets of [`CallScratch::take_prepared_targets`], emptied.
    pub(crate) fn recycle_prepared_targets(&mut self, mut targets: PreparedTargets) {
        targets.clear();
        self.prepared_targets.recycle(targets);
    }

    /// Take the preflighted output-value list of a call's copy-back group, empty (spec 34
    /// MEM-09; census C7).
    pub(crate) fn take_prepared_outputs(&mut self) -> Vec<(VmWriteTarget, Value)> {
        self.prepared_outputs.take()
    }

    /// Return the prepared outputs of [`CallScratch::take_prepared_outputs`], emptied.
    pub(crate) fn recycle_prepared_outputs(&mut self, mut outputs: Vec<(VmWriteTarget, Value)>) {
        outputs.clear();
        self.prepared_outputs.recycle(outputs);
    }

    /// Take the evaluated argument values of a standard-library call, empty (spec 34 MEM-09;
    /// census C9). The engine binds the call's arguments into it and returns it on every path.
    pub(crate) fn take_stdlib_values(&mut self) -> Vec<Value> {
        self.stdlib_values.take()
    }

    /// Return the standard-library values of [`CallScratch::take_stdlib_values`], emptied.
    pub(crate) fn recycle_stdlib_values(&mut self, mut values: Vec<Value>) {
        values.clear();
        self.stdlib_values.recycle(values);
    }

    /// The reserved vectors still held per kind (test support).
    #[cfg(test)]
    pub(crate) fn held_counts(&self) -> [usize; KIND_COUNT] {
        [
            self.args.held(),
            self.flags.held(),
            self.function_outputs.held(),
            self.fb_outputs.held(),
            self.prepared_targets.held(),
            self.prepared_outputs.held(),
            self.stdlib_values.held(),
        ]
    }

    /// The most vectors of each kind ever out at once (test support).
    #[cfg(test)]
    pub(crate) fn most_outstanding(&self) -> [usize; KIND_COUNT] {
        [
            self.args.peak,
            self.flags.peak,
            self.function_outputs.peak,
            self.fb_outputs.peak,
            self.prepared_targets.peak,
            self.prepared_outputs.peak,
            self.stdlib_values.peak,
        ]
    }

    /// Bytes that [`CallScratch::reserve`] takes for `levels` depths: each kind's depth table and
    /// every reserved vector.
    #[must_use]
    pub(crate) fn reservation_bytes(levels: usize, sizes: CallScratchSizes) -> Option<usize> {
        let flags = sizes.flags();
        let kinds = [
            (
                core::mem::size_of::<Option<Vec<VmNativeArg>>>(),
                sizes.args,
                core::mem::size_of::<VmNativeArg>(),
            ),
            (
                core::mem::size_of::<Option<Vec<bool>>>(),
                flags,
                core::mem::size_of::<bool>(),
            ),
            (
                core::mem::size_of::<Option<Vec<VmOutBinding>>>(),
                sizes.params,
                core::mem::size_of::<VmOutBinding>(),
            ),
            (
                core::mem::size_of::<Option<Vec<VmFbOutBinding>>>(),
                sizes.params,
                core::mem::size_of::<VmFbOutBinding>(),
            ),
            (
                core::mem::size_of::<Option<Vec<Option<(VmWriteTarget, Option<u32>)>>>>(),
                sizes.params,
                core::mem::size_of::<Option<(VmWriteTarget, Option<u32>)>>(),
            ),
            (
                core::mem::size_of::<Option<Vec<(VmWriteTarget, Value)>>>(),
                sizes.params,
                core::mem::size_of::<(VmWriteTarget, Value)>(),
            ),
            (
                core::mem::size_of::<Option<Vec<Value>>>(),
                sizes.args,
                core::mem::size_of::<Value>(),
            ),
        ];
        let mut total = 0usize;
        for (table, width, element) in kinds {
            let per_level = table.checked_add(width.checked_mul(element)?)?;
            total = total.checked_add(levels.checked_mul(per_level)?)?;
        }
        Some(total)
    }
}

/// Reserve `levels` slots, each built by `build`.
fn reserve_kind<T>(
    slots: &mut Vec<Option<T>>,
    levels: usize,
    mut build: impl FnMut() -> Result<T, RuntimeError>,
) -> Result<(), RuntimeError> {
    slots
        .try_reserve_exact(levels)
        .map_err(|_| VmTrap::BudgetExceeded.into_runtime_error())?;
    for _ in 0..levels {
        slots.push(Some(build()?));
    }
    Ok(())
}

/// A fresh vector with room for `width` entries.
fn reserve_vec<T>(width: usize) -> Result<Vec<T>, RuntimeError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(width)
        .map_err(|_| VmTrap::BudgetExceeded.into_runtime_error())?;
    Ok(values)
}

#[cfg(test)]
#[path = "scratch/tests.rs"]
mod tests;
