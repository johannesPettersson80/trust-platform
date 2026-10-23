//! What a module's generated code calls: the start of an entry point, the typed views of its areas,
//! the statement hook and the faults (specification 44 §6.5, §8.3, §8.4, §13.2).

use core::marker::PhantomData;

use crate::{
    Area, Cx, Fault, FaultCode, FaultRecord, Plain, RefValue, AREA_COUNT, INTERFACE_VERSION,
};

/// The generation of every reference a compiled program takes: 1, until an online change gives an
/// installation a generation of its own (specification 44 §7.3, §12, milestone M3).
pub const GENERATION: u32 = 1;

/// The areas of one entry call. Their views live as long as the call: the kernel keeps the areas
/// in place and writes none of them until the call returns.
pub struct Areas<'s> {
    areas: [Area; AREA_COUNT],
    scope: PhantomData<&'s ()>,
}

impl<'s> Areas<'s> {
    /// The view of area `id` as `T`, the area type the generated code declares for it.
    ///
    /// # Errors
    ///
    /// [`FaultCode::ModuleInternal`] when no area has the id, or the area is shorter than `T` or
    /// not aligned for it, which a kernel that admitted the image never gives (specification 44
    /// §10.2).
    #[allow(unsafe_code)]
    pub fn view<T: Plain>(&self, cx: &mut Cx, id: u8) -> Result<&'s T, Fault> {
        let Some(area) = usize::from(id)
            .checked_sub(1)
            .and_then(|index| self.areas.get(index))
        else {
            return Err(fault(cx, FaultCode::ModuleInternal, i64::from(id), 0, 0));
        };
        let size = core::mem::size_of::<T>();
        let fits = usize::try_from(area.len).is_ok_and(|len| len >= size);
        if !fits || area.base.is_null() || area.base.align_offset(core::mem::align_of::<T>()) != 0 {
            return Err(fault(
                cx,
                FaultCode::ModuleInternal,
                i64::from(id),
                i64::try_from(size).unwrap_or(i64::MAX),
                i64::from(area.len),
            ));
        }
        // SAFETY: the area's `len` bytes from `base` are memory the kernel owns and keeps in place
        // for the whole entry call (`'s` cannot outlive the call: `run` makes it), `T` fits them
        // and `base` is aligned for it, and `T: Plain` makes every bit pattern a value of `T` that
        // changes only through cells, so a shared view is sound while the kernel writes nothing.
        Ok(unsafe { &*area.base.cast::<T>() })
    }

    /// A reference of `kind` to `target`, a variable that lies in one of the areas: the target's
    /// area, its offset there and its extent, in this program's generation (specification 44
    /// §7.3).
    ///
    /// # Errors
    ///
    /// [`FaultCode::ReferenceLifetime`] when `target` lies in no area, as a frame variable does, to
    /// which IEC 61131-3 allows no reference (§6.4.4.10.2).
    pub fn refer<T: Plain>(&self, cx: &mut Cx, target: &T, kind: u8) -> Result<RefValue, Fault> {
        let address = core::ptr::from_ref(target).addr();
        let extent = core::mem::size_of::<T>();
        for (index, area) in self.areas.iter().enumerate() {
            let base = area.base.addr();
            let end = usize::try_from(area.len)
                .ok()
                .and_then(|len| base.checked_add(len));
            let inside = address >= base
                && address
                    .checked_add(extent)
                    .zip(end)
                    .is_some_and(|(target_end, end)| target_end <= end);
            if inside {
                return Ok(RefValue {
                    area: u8::try_from(index + 1).unwrap_or(u8::MAX),
                    kind,
                    char_index: 0,
                    offset: u32::try_from(address - base).unwrap_or(u32::MAX),
                    extent: u32::try_from(extent).unwrap_or(u32::MAX),
                    generation: GENERATION,
                });
            }
        }
        Err(fault(cx, FaultCode::ReferenceLifetime, 0, 0, 0))
    }

    /// The variable `reference` names, viewed as `T` (specification 44 §7.3): the reference is not
    /// `NULL`, it was taken in this program's generation, it spans exactly a `T`, and its area holds
    /// it, aligned for `T`.
    ///
    /// # Errors
    ///
    /// [`FaultCode::NullReference`] for `NULL`, [`FaultCode::ReferenceLifetime`] for a reference of
    /// another generation or outside its area, and [`FaultCode::TypeMismatch`] for a target that is
    /// no `T`.
    pub fn deref<T: Plain>(&self, cx: &mut Cx, reference: &RefValue) -> Result<&'s T, Fault> {
        current(cx, reference)?;
        let size = core::mem::size_of::<T>();
        if usize::try_from(reference.extent).ok() != Some(size) {
            return Err(fault(
                cx,
                FaultCode::TypeMismatch,
                i64::from(reference.extent),
                i64::try_from(size).unwrap_or(i64::MAX),
                0,
            ));
        }
        self.at(cx, reference)
    }

    /// The class of the instance `reference` names (specification 44 §6.4): the class id the
    /// instance holds in its first four bytes, which every instance of a class that implements an
    /// interface or takes part in inheritance holds.
    ///
    /// # Errors
    ///
    /// [`FaultCode::NullReference`] for `NULL`, [`FaultCode::ReferenceLifetime`] for a reference of
    /// another generation or outside its area, and [`FaultCode::TypeMismatch`] for a reference that
    /// names no instance.
    pub fn class_of(&self, cx: &mut Cx, reference: &RefValue) -> Result<u32, Fault> {
        current(cx, reference)?;
        if reference.kind != RefValue::INSTANCE {
            return Err(fault(
                cx,
                FaultCode::TypeMismatch,
                i64::from(reference.kind),
                i64::from(RefValue::INSTANCE),
                0,
            ));
        }
        let class: &core::cell::Cell<u32> = self.at(cx, reference)?;
        Ok(class.get())
    }

    /// The instance `reference` names, viewed as `T`, the type of class `class` (specification 44
    /// §6.4): the instance holds the class id `class`, and its area holds a `T` at the reference's
    /// offset, aligned for it. A reference to an instance spans the type it was taken as, which may
    /// be a base of the instance's own class; the class id, not the extent, says what lies there.
    ///
    /// # Errors
    ///
    /// As [`Areas::class_of`], and [`FaultCode::TypeMismatch`] for an instance of another class.
    pub fn instance<T: Plain>(
        &self,
        cx: &mut Cx,
        reference: &RefValue,
        class: u32,
    ) -> Result<&'s T, Fault> {
        let held = self.class_of(cx, reference)?;
        if held != class {
            return Err(fault(
                cx,
                FaultCode::TypeMismatch,
                i64::from(held),
                i64::from(class),
                0,
            ));
        }
        self.at(cx, reference)
    }

    /// The `T` at the area and offset of `reference`, which [`current`] has checked: inside the
    /// area and aligned for `T`.
    #[allow(unsafe_code)]
    fn at<T: Plain>(&self, cx: &mut Cx, reference: &RefValue) -> Result<&'s T, Fault> {
        let size = core::mem::size_of::<T>();
        let area = usize::from(reference.area)
            .checked_sub(1)
            .and_then(|index| self.areas.get(index));
        let offset = usize::try_from(reference.offset).ok();
        let target = area.zip(offset).and_then(|(area, offset)| {
            let len = usize::try_from(area.len).ok()?;
            (!area.base.is_null() && offset.checked_add(size)? <= len)
                .then(|| area.base.wrapping_add(offset))
        });
        let Some(target) = target else {
            return Err(fault(
                cx,
                FaultCode::ReferenceLifetime,
                i64::from(reference.area),
                i64::from(reference.offset),
                i64::try_from(size).unwrap_or(i64::MAX),
            ));
        };
        if target.align_offset(core::mem::align_of::<T>()) != 0 {
            return Err(fault(
                cx,
                FaultCode::TypeMismatch,
                i64::from(reference.offset),
                0,
                0,
            ));
        }
        // SAFETY: `target` and the `size_of::<T>()` bytes after it lie inside an area the kernel
        // owns and keeps in place for the whole entry call (`'s` cannot outlive it), the pointer is
        // aligned for `T`, and `T: Plain` makes every bit pattern a value of `T` that changes only
        // through cells, as for `view`.
        Ok(unsafe { &*target.cast::<T>() })
    }
}

/// Checks that `reference` names something of this program's generation: it is not `NULL`, and it
/// was taken in [`GENERATION`] (specification 44 §7.3).
fn current(cx: &mut Cx, reference: &RefValue) -> Result<(), Fault> {
    if reference.kind == RefValue::NULL {
        return Err(fault(cx, FaultCode::NullReference, 0, 0, 0));
    }
    if reference.generation != GENERATION {
        return Err(fault(
            cx,
            FaultCode::ReferenceLifetime,
            i64::from(reference.generation),
            i64::from(GENERATION),
            0,
        ));
    }
    Ok(())
}

/// Runs the body of an entry point: checks the kernel's interface version, hands the body the
/// context and the areas, and returns 0 or the code of the fault the body ended with
/// (specification 44 §8.3).
///
/// The context is the kernel's raw pointer, as the entry point receives it: the generated entry is
/// the only caller, and making this function `unsafe` would put `unsafe` into generated code.
#[allow(unsafe_code, clippy::not_unsafe_ptr_arg_deref)]
pub fn run(
    cx: *mut Cx,
    body: impl for<'s> FnOnce(&'s mut Cx, Areas<'s>) -> Result<(), Fault>,
) -> u32 {
    if cx.is_null() {
        return FaultCode::ModuleInternal as u32;
    }
    // SAFETY: the kernel calls an entry point with its own context, valid and used by nothing else
    // until the call returns (specification 44 §8.3).
    let cx = unsafe { &mut *cx };
    if cx.version != INTERFACE_VERSION {
        return fault(
            cx,
            FaultCode::ModuleInternal,
            i64::from(cx.version),
            i64::from(INTERFACE_VERSION),
            0,
        )
        .raw();
    }
    let areas = Areas {
        areas: cx.areas,
        scope: PhantomData,
    };
    match body(cx, areas) {
        Ok(()) => 0,
        Err(fault) => fault.raw(),
    }
}

/// Starts statement `id`: records it, counts it against the budget and stops at its breakpoint
/// (specification 44 §8.4, §13.2). Two loads and two branches when no debugger is armed.
///
/// # Errors
///
/// [`FaultCode::InfiniteLoop`] when the budget is used up, or the fault the kernel's stop returns.
#[inline]
pub fn stmt(cx: &mut Cx, id: u32) -> Result<(), Fault> {
    cx.stmt = id;
    if cx.budget == 0 {
        return Err(budget_used(cx));
    }
    cx.budget -= 1;
    if cx.armed != 0 {
        return stop_at(cx, id);
    }
    Ok(())
}

/// Counts one iteration of a loop against the budget, so that an empty loop body is bounded too
/// (specification 44 §8.4).
///
/// # Errors
///
/// [`FaultCode::InfiniteLoop`] when the budget is used up.
#[inline]
pub fn tick(cx: &mut Cx) -> Result<(), Fault> {
    if cx.budget == 0 {
        return Err(budget_used(cx));
    }
    cx.budget -= 1;
    Ok(())
}

#[cold]
#[inline(never)]
fn budget_used(cx: &mut Cx) -> Fault {
    fault(cx, FaultCode::InfiniteLoop, 0, 0, 0)
}

/// Takes one from the call depth before a recursive call (specification 44 §6.4); [`leave`] gives it
/// back when the call returns.
///
/// # Errors
///
/// [`FaultCode::CallDepth`] when no depth is left.
#[inline]
pub fn enter(cx: &mut Cx) -> Result<(), Fault> {
    if cx.depth == 0 {
        return Err(depth_used(cx));
    }
    cx.depth -= 1;
    Ok(())
}

/// Gives back the call depth that [`enter`] took, when the recursive call returns, with a fault too.
#[inline]
pub fn leave(cx: &mut Cx) {
    cx.depth = cx.depth.saturating_add(1);
}

#[cold]
#[inline(never)]
fn depth_used(cx: &mut Cx) -> Fault {
    fault(cx, FaultCode::CallDepth, 0, 0, 0)
}

#[cold]
#[inline(never)]
#[allow(unsafe_code)]
fn stop_at(cx: &mut Cx, id: u32) -> Result<(), Fault> {
    let byte = id / 8;
    if byte >= cx.breakpoint_bytes || cx.breakpoints.is_null() || cx.api.is_null() {
        return Ok(());
    }
    let Ok(index) = usize::try_from(byte) else {
        return Ok(());
    };
    // SAFETY: the kernel keeps `breakpoint_bytes` readable bytes at `breakpoints` for the call, and
    // `index` is below that count.
    let bits = unsafe { cx.breakpoints.add(index).read() };
    if bits & (1 << (id % 8)) == 0 {
        return Ok(());
    }
    // SAFETY: the kernel's function table lives as long as the kernel, which outlives the call.
    let api = unsafe { &*cx.api };
    match Fault::from_raw((api.stop)(cx, id)) {
        None => Ok(()),
        Some(fault) => Err(fault),
    }
}

/// The real the text `text` spells, read by the kernel as the runtime reads it (specification 44
/// §8.2): `text` is trimmed and holds no separators.
///
/// # Errors
///
/// The fault the kernel's reader gives, `TypeMismatch` for a text that spells no real.
#[allow(unsafe_code)]
pub fn read_real(cx: &mut Cx, text: &[u8]) -> Result<f64, Fault> {
    if cx.api.is_null() {
        return Err(fault(cx, FaultCode::ModuleInternal, 0, 0, 0));
    }
    let length = u32::try_from(text.len()).unwrap_or(u32::MAX);
    // SAFETY: the kernel's function table lives as long as the kernel, which outlives the call.
    let api = unsafe { &*cx.api };
    let mut value = 0.0;
    let code = (api.read_real)(cx, text.as_ptr(), length, &raw mut value);
    match FaultCode::from_raw(code) {
        None if code == 0 => Ok(value),
        Some(code) => Err(fault(cx, code, 0, 0, 0)),
        None => Err(fault(cx, FaultCode::ModuleInternal, i64::from(code), 0, 0)),
    }
}

/// Records fault `code` with its numbers as the fault of the call, and returns it to be returned
/// up to the entry point (specification 44 §8.6).
#[cold]
#[inline(never)]
pub fn fault(cx: &mut Cx, code: FaultCode, a: i64, b: i64, c: i64) -> Fault {
    cx.fault = FaultRecord {
        code: code as u32,
        stmt: cx.stmt,
        detail: [a, b, c],
    };
    Fault::new(code)
}

/// Makes the recursive call `call` (specification 44 §6.4): takes one from the call depth before it
/// runs and gives it back when it returns, with a fault too. It is a function of its own that the
/// module never inlines, nor a body that makes such a call, so every turn of a recursion passes
/// through two functions and the link keeps a relocation for each call of the cycle, from which the
/// build finds the cycle's frames (§8.5); a call of a function to itself would keep none.
///
/// # Errors
///
/// [`FaultCode::CallDepth`] when no depth is left, and the fault of `call`.
#[inline(never)]
pub fn recurse<T>(cx: &mut Cx, call: impl FnOnce(&mut Cx) -> Result<T, Fault>) -> Result<T, Fault> {
    enter(cx)?;
    let result = call(cx);
    leave(cx);
    result
}
