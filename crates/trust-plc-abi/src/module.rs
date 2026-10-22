//! What a module's generated code calls: the start of an entry point, the typed views of its areas,
//! the statement hook and the faults (specification 44 §6.5, §8.3, §8.4, §13.2).

use core::marker::PhantomData;

use crate::{Area, Cx, Fault, FaultCode, FaultRecord, Plain, AREA_COUNT, INTERFACE_VERSION};

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
