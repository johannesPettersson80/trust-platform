//! What a kernel with a debugger does when a statement of a debug image stops (specification 44
//! §8.2, §13.2): it hands the stop to its sink, walks the call frames the module linked into the
//! context, and reads and writes the program's memory while the module waits in the stop.
//!
//! The kernel itself has no `unsafe` code. The pointers it needs at a stop (the context's sink, the
//! frames on the module's stack, the areas the module views as cells) are read here, behind safe
//! functions whose contract is the stop's: they are called while the module waits inside the
//! kernel's `stop`, when every frame of the chain is live and nothing else touches the memory.

use core::cell::Cell;
use core::ffi::c_void;
use core::marker::PhantomData;

use super::Entry;
use crate::{Cx, Frame, AREA_COUNT};

/// What a kernel does when a statement of a debug image stops: its debugger decides whether the
/// scan holds there, before the statement runs.
pub trait StopSink {
    /// Statement `stmt` stops, with the module's context `cx`; returns 0 for the statement to run,
    /// else the code of the fault the module ends its call with.
    fn stop(&mut self, cx: &mut Cx, stmt: u32) -> u32;
}

/// Calls `entry` with `sink` behind `cx.kernel` for the length of the call, where the kernel
/// function [`stop_to_sink`] finds it, and returns 0 or the code of the fault the call ended with.
pub fn call_with_sink(entry: Entry, cx: &mut Cx, sink: &mut dyn StopSink) -> u32 {
    let mut sink: &mut dyn StopSink = sink;
    cx.kernel = (&raw mut sink).cast::<c_void>();
    let code = entry.call(cx);
    cx.kernel = core::ptr::null_mut();
    code
}

/// The kernel function `stop` of a kernel with a debugger: the stop goes to the sink that
/// [`call_with_sink`] put behind `cx.kernel`; without one the statement runs.
#[cfg(target_arch = "x86_64")]
pub extern "sysv64" fn stop_to_sink(cx: *mut Cx, stmt: u32) -> u32 {
    stop_with(cx, stmt)
}

/// The kernel function `stop` of a kernel with a debugger: the stop goes to the sink that
/// [`call_with_sink`] put behind `cx.kernel`; without one the statement runs.
#[cfg(not(target_arch = "x86_64"))]
pub extern "C" fn stop_to_sink(cx: *mut Cx, stmt: u32) -> u32 {
    stop_with(cx, stmt)
}

#[allow(unsafe_code)]
fn stop_with(cx: *mut Cx, stmt: u32) -> u32 {
    // SAFETY: a module calls `stop` with the context the kernel gave its entry point, which is
    // valid for the call and which the module does not touch until `stop` returns.
    let Some(cx) = (unsafe { cx.as_mut() }) else {
        return 0;
    };
    let sink = cx.kernel.cast::<&mut dyn StopSink>();
    if sink.is_null() {
        return 0;
    }
    // SAFETY: `call_with_sink` put a pointer to its own `&mut dyn StopSink` behind `cx.kernel` for
    // the length of the entry's call, which this stop is inside, and nothing else reads it.
    let sink = unsafe { &mut *sink };
    sink.stop(cx, stmt)
}

/// The most frames a walk reads, so that a damaged chain cannot hold the kernel in a loop.
pub const MAX_FRAMES: usize = 4096;

/// One frame of a held scan's call stack, as the module linked it into the context (specification
/// 44 §8.2). It is valid while the stop it was read in lasts, which its lifetime keeps it to.
#[derive(Debug, Clone, Copy)]
pub struct FrameView<'cx> {
    /// The POU's id (specification 44 §5.7).
    pub pou: u32,
    /// The caller's statement that made the call.
    pub call_stmt: u32,
    locals: *const u8,
    instance: *const u8,
    stop: PhantomData<&'cx Cx>,
}

impl FrameView<'_> {
    /// Whether the frame has frame variables.
    #[must_use]
    pub fn has_locals(&self) -> bool {
        !self.locals.is_null()
    }

    /// The address of the instance the POU runs on, or `None` for a function. A kernel finds its
    /// area and offset by the areas' bases (`area_address`).
    #[must_use]
    pub fn instance_address(&self) -> Option<usize> {
        (!self.instance.is_null()).then_some(self.instance as usize)
    }

    /// Copies `out.len()` bytes from `offset` of the frame's variables, whose frame holds `size`
    /// bytes (its POU symbol's size); `false` outside the frame or for a frame without variables.
    #[must_use]
    pub fn read(&self, size: usize, offset: usize, out: &mut [u8]) -> bool {
        let Some(cells) = self.cells(size) else {
            return false;
        };
        copy_out(cells, offset, out)
    }

    /// Writes `data` at `offset` of the frame's variables, whose frame holds `size` bytes; `false`
    /// outside the frame or for a frame without variables.
    #[must_use]
    pub fn write(&self, size: usize, offset: usize, data: &[u8]) -> bool {
        let Some(cells) = self.cells(size) else {
            return false;
        };
        copy_in(cells, offset, data)
    }

    #[allow(unsafe_code)]
    fn cells(&self, size: usize) -> Option<&[Cell<u8>]> {
        if self.locals.is_null() {
            return None;
        }
        // SAFETY: the module linked this frame for the length of its call with `locals` at its
        // frame structure of `size` bytes (its POU symbol's size), whose fields are cells, and the
        // call waits in the stop this view was read in, which outlives the returned slice.
        Some(unsafe { core::slice::from_raw_parts(self.locals.cast::<Cell<u8>>(), size) })
    }
}

/// The frames `cx` links, innermost first: the frame of the POU that holds the stopped statement,
/// then its caller's, up to the body the kernel called; at most [`MAX_FRAMES`].
#[must_use]
pub fn frames(cx: &Cx) -> Frames<'_> {
    Frames {
        next: cx.frame,
        left: MAX_FRAMES,
        stop: PhantomData,
    }
}

/// The frames of a held scan, innermost first.
pub struct Frames<'cx> {
    next: *const Frame,
    left: usize,
    stop: PhantomData<&'cx Cx>,
}

impl<'cx> Iterator for Frames<'cx> {
    type Item = FrameView<'cx>;

    #[allow(unsafe_code)]
    fn next(&mut self) -> Option<FrameView<'cx>> {
        if self.next.is_null() || self.left == 0 {
            return None;
        }
        self.left -= 1;
        // SAFETY: every frame of the chain was linked by a POU whose call has not returned, as a
        // POU unlinks its frame before it returns, so each one is live while the module waits in
        // the stop that `frames` was called in.
        let frame = unsafe { self.next.read() };
        self.next = frame.caller;
        Some(FrameView {
            pou: frame.pou,
            call_stmt: frame.call_stmt,
            locals: frame.locals,
            instance: frame.instance,
            stop: PhantomData,
        })
    }
}

/// The address and the length of area `id` of `cx`, or `None` for an id outside 1 to 7.
#[must_use]
pub fn area_address(cx: &Cx, id: u8) -> Option<(usize, usize)> {
    let area = cx
        .areas
        .get(usize::from(id).checked_sub(1)?)
        .filter(|_| usize::from(id) <= AREA_COUNT)?;
    Some((area.base as usize, area.len as usize))
}

/// Copies `out.len()` bytes from `offset` of area `id` of `cx`; `false` outside the area.
#[must_use]
pub fn read_area(cx: &Cx, id: u8, offset: usize, out: &mut [u8]) -> bool {
    area_cells(cx, id).is_some_and(|cells| copy_out(cells, offset, out))
}

/// Writes `data` at `offset` of area `id` of `cx`; `false` outside the area.
#[must_use]
pub fn write_area(cx: &Cx, id: u8, offset: usize, data: &[u8]) -> bool {
    area_cells(cx, id).is_some_and(|cells| copy_in(cells, offset, data))
}

#[allow(unsafe_code)]
fn area_cells(cx: &Cx, id: u8) -> Option<&[Cell<u8>]> {
    let area = cx.areas.get(usize::from(id).checked_sub(1)?)?;
    let len = usize::try_from(area.len).ok()?;
    // SAFETY: the kernel gave the area `len` bytes at `base`, never null, for the length of the
    // call the context is lent to; the module views them as cells, and while it waits in a stop
    // nothing else reads or writes them.
    Some(unsafe { core::slice::from_raw_parts(area.base.cast::<Cell<u8>>(), len) })
}

fn copy_out(cells: &[Cell<u8>], offset: usize, out: &mut [u8]) -> bool {
    let Some(source) = offset
        .checked_add(out.len())
        .and_then(|end| cells.get(offset..end))
    else {
        return false;
    };
    for (byte, cell) in out.iter_mut().zip(source) {
        *byte = cell.get();
    }
    true
}

fn copy_in(cells: &[Cell<u8>], offset: usize, data: &[u8]) -> bool {
    let Some(target) = offset
        .checked_add(data.len())
        .and_then(|end| cells.get(offset..end))
    else {
        return false;
    };
    for (cell, byte) in target.iter().zip(data) {
        cell.set(*byte);
    }
    true
}

/// The functions of a kernel with a debugger that reads reals with `read_real`: a stop goes to the
/// sink of the call ([`call_with_sink`]), a call without one runs on, and no write is queued.
#[must_use]
pub const fn with_debugger(read_real: crate::ReadRealFn) -> crate::KernelApi {
    crate::KernelApi {
        stop: stop_to_sink,
        debug_write: super::no_queued_write,
        read_real,
        wall_clock: super::wall_clock::<super::NoWallClock>,
    }
}
