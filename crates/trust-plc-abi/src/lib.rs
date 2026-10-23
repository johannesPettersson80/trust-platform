//! The kernel interface of truST's compiled programs (specification 44 §8.2).
//!
//! A program is compiled on the engineering computer into a native module, which the kernel of
//! every device runs. This crate is what the two share: the context the kernel fills before it calls
//! an entry point ([`Cx`]), the memory areas a program reads and writes ([`Area`]), the faults a
//! program ends a scan with ([`FaultCode`]), the layout of a reference ([`RefValue`]) and of a
//! string ([`Str`], [`WStr`]), and the functions a scan may call in the kernel ([`KernelApi`]).
//!
//! The crate is compiled into every module (the [`module`] side) and into every kernel (the
//! [`kernel`] side). It is the only crate of a module with `unsafe` code: the casts between the
//! kernel's raw areas and a program's typed views, the read of the breakpoint bits and the call of a
//! kernel function. A program's own code is safe Rust (specification 44 §6.1).
//!
//! A program's variables lie in its areas as cells ([`core::cell::Cell`]): every variable is read
//! with `get` and written with `set`, through shared references, so that a program may hold a
//! reference to a variable while it writes it through another path, as Structured Text allows
//! (`VAR_IN_OUT`, `REF_TO`), and still compiles to one load or store at a fixed offset.

#![cfg_attr(not(feature = "host"), no_std)]
#![warn(missing_docs)]

mod fault;
pub mod kernel;
pub mod module;
mod plain;
mod reference;
mod strings;

pub use fault::{Fault, FaultCode, FaultRecord};
pub use plain::Plain;
pub use reference::RefValue;
pub use strings::{Str, StrView, WStr, WStrView};

/// The version of this interface. A module carries the version it was built against in its image's
/// header, and a kernel runs only a module of its own version (specification 44 §8.2, §10.2).
pub const INTERFACE_VERSION: u32 = 2;

/// The id of the input area, `%I` (specification 44 §7.1).
pub const AREA_INPUTS: u8 = 1;
/// The id of the output area, `%Q`.
pub const AREA_OUTPUTS: u8 = 2;
/// The id of the marker area, `%M`.
pub const AREA_MARKERS: u8 = 3;
/// The id of the area of the non-retained globals.
pub const AREA_GLOBALS: u8 = 4;
/// The id of the area of the retained variables, globals and program variables.
pub const AREA_RETAIN: u8 = 5;
/// The id of the area of the program instances.
pub const AREA_INSTANCES: u8 = 6;
/// The id of the task stack, which holds the frames of functions and methods.
pub const AREA_STACK: u8 = 7;
/// The number of areas.
pub const AREA_COUNT: usize = 7;

/// One area of a program's memory: its first byte and its length (specification 44 §7.1).
///
/// The kernel owns the memory. A base is never null and is aligned to 8 bytes, also for an area of
/// no bytes, so that a typed view of an empty area is valid.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Area {
    /// The first byte of the area.
    pub base: *mut u8,
    /// The bytes of the area.
    pub len: u32,
}

/// The calling convention of an entry point and of a kernel function, `extern "sysv64"` on x86-64
/// and `extern "C"` elsewhere: the module of an x86-64 computer is one freestanding module for
/// Linux, Windows and macOS alike (specification 44 §4.4), so its convention is named, not the
/// convention of the computer's operating system.
#[cfg(target_arch = "x86_64")]
macro_rules! abi_fn {
    ($($arg:ty),* => $ret:ty) => { extern "sysv64" fn($($arg),*) -> $ret };
}
#[cfg(not(target_arch = "x86_64"))]
macro_rules! abi_fn {
    ($($arg:ty),* => $ret:ty) => { extern "C" fn($($arg),*) -> $ret };
}

/// An entry point of a module: a task body, the cold-start or the warm-start initialization
/// (specification 44 §5.4). It returns 0, or the code of the fault that ended it.
pub type EntryFn = abi_fn!(*mut Cx => u32);

/// The kernel's function that a statement calls when its breakpoint bit is set: a breakpoint, a
/// pause or a step (specification 44 §13.2). It returns 0 to continue, else the code of a fault that
/// ends the scan.
pub type StopFn = abi_fn!(*mut Cx, u32 => u32);

/// The kernel's function that applies the queued writes of Live Values to one variable, named by its
/// symbol index, at a safe point (specification 44 §13.3). It returns 0, else a fault code.
pub type DebugWriteFn = abi_fn!(*mut Cx, u32 => u32);

/// The kernel's function that reads the real a text spells, as the runtime reads it (specification
/// 44 §8.2; specification 07 §2.8): the text is `len` bytes at `text`, white space trimmed and
/// separators taken out, and the real goes to `out`. It returns 0, else the code of a fault:
/// `TypeMismatch` for a text that spells no real.
pub type ReadRealFn = abi_fn!(*mut Cx, *const u8, u32, *mut f64 => u32);

/// The functions a scan may call in the kernel (specification 44 §8.2). Everything else a program
/// needs, its time, inputs, outputs and variables, is memory that the kernel prepared before the
/// call.
#[repr(C)]
pub struct KernelApi {
    /// A statement whose breakpoint bit is set stops here.
    pub stop: StopFn,
    /// Applies queued writes to one variable.
    pub debug_write: DebugWriteFn,
    /// Reads the real a text spells.
    pub read_real: ReadRealFn,
}

/// What the kernel gives an entry point: the areas, the task's logical time, the statement budget,
/// the breakpoint bits and the kernel's functions; and what the module gives back: the statement it
/// is executing and, when it ends with a fault, the fault's record (specification 44 §8.2, §8.3).
#[repr(C)]
pub struct Cx {
    /// The interface version the kernel implements, [`INTERFACE_VERSION`].
    pub version: u32,
    /// The index of the running task in the image's task table, or `u32::MAX` for a program that
    /// runs in no task and for an initialization.
    pub task: u32,
    /// The areas, in the order of their ids: `areas[0]` is [`AREA_INPUTS`].
    pub areas: [Area; AREA_COUNT],
    /// The task's logical time in nanoseconds, `TIME()` (specification 11 §5.2.1).
    pub now_ns: i64,
    /// The statements this activation may still execute (specification 44 §8.4).
    pub budget: u32,
    /// The id of the statement being executed (specification 44 §13).
    pub stmt: u32,
    /// Not 0 while the kernel wants statements to look at their breakpoint bits: a breakpoint is
    /// set, or a pause or a step is pending.
    pub armed: u32,
    /// The bytes of the breakpoint bits.
    pub breakpoint_bytes: u32,
    /// The breakpoint bits, one per statement id.
    pub breakpoints: *const u8,
    /// The kernel's functions.
    pub api: *const KernelApi,
    /// The kernel's own pointer, which only its functions read.
    pub kernel: *mut core::ffi::c_void,
    /// The fault that ended the call, which the module writes before it returns the fault's code.
    pub fault: FaultRecord,
}

#[cfg(test)]
mod tests;
