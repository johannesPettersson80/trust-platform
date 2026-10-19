//! The memory a loaded program lives in (specification 34 STATE-08 and Appendix D.2).
//!
//! A program's engine state borrows its prepared module and its execution services, and the debugger's
//! statement hook keeps the module for as long as the services live, so both must be `'static` while
//! the program runs. A chip that loads a newer program in place cannot keep them for the rest of the
//! boot, as a program leaked once would: every load would leak a module. Instead the cycle owns them.
//! [`ProgramMemory::start`] puts the module and the services on the heap, builds the program's cycle
//! over them and hands the cycle back with the memory as its last field, so the memory is freed when
//! the cycle is dropped, after the engine state, the process I/O and the checkpoint store, which
//! borrow or hold what the memory holds.
//!
//! This is the one place where the chip frees what it extended to `'static`: two registered unsafe
//! sites, the borrow and the free. The borrows end with the cycle, because the memory is the cycle's
//! own field and no method of the cycle hands out a reference of its lifetime.

use alloc::boxed::Box;
use core::ptr::NonNull;

use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::ports::MonotonicClock;
use trust_runtime_core::vm::PreparedModule;
use trust_runtime_services::runner::engine::{
    CycleSettings, DeviceServices, EngineCycle, ProcessIo,
};
use trust_runtime_services::runner::RetainConfig;

/// A program's prepared module and execution services on the heap, which the cycle of the program
/// owns and frees when it is dropped (specification 34 Appendix D.2).
pub struct ProgramMemory<C> {
    module: NonNull<PreparedModule>,
    services: NonNull<DeviceServices<C>>,
}

/// The cycle of a program that owns its memory: it frees the module and the services when it drops.
pub type ProgramCycle<C, I> = EngineCycle<'static, C, I, ProgramMemory<C>>;

impl<C: MonotonicClock + 'static> ProgramMemory<C> {
    /// Starts the program of `module` on the engine with `services` and the process I/O `io`, as
    /// power-on starts it (a cold start), with the checkpoint store of `retain` when there is one,
    /// and lets the debugger hold its scans at its statement starts (specification 13, "Safe
    /// Points"). The returned cycle owns the module and the services.
    ///
    /// # Errors
    ///
    /// The [`RuntimeError`] of a program the device cannot start, or of a checkpoint log that cannot
    /// be opened. The module and the services are freed.
    pub fn start<I: ProcessIo>(
        module: PreparedModule,
        services: DeviceServices<C>,
        io: I,
        settings: CycleSettings,
        retain: Option<RetainConfig>,
    ) -> Result<ProgramCycle<C, I>, RuntimeError> {
        let memory = Self {
            module: NonNull::from(Box::leak(Box::new(module))),
            services: NonNull::from(Box::leak(Box::new(services))),
        };
        // SAFETY: both pointers came from `Box::leak` just above, so they are valid and nothing else
        // reaches the boxes. The borrows are extended to `'static`, and every one of them is stored in
        // the cycle built below, or dropped in this function. The cycle takes `memory` as its last
        // field, so the borrows in its other fields end before `ProgramMemory::drop` frees the boxes,
        // and no method of the cycle returns a reference of its lifetime. If the cycle is not built,
        // `memory` drops here, after the last use of the borrows.
        let (module, services) = unsafe { (memory.module.as_ref(), memory.services.as_ref()) };
        let cycle = match retain {
            Some(retain) => EngineCycle::with_retain(module, services, io, settings, retain)?,
            None => EngineCycle::start(module, services, io, settings)?,
        };
        Ok(cycle.with_memory(memory).with_debugger())
    }
}

impl<C> Drop for ProgramMemory<C> {
    fn drop(&mut self) {
        // SAFETY: both pointers came from `Box::leak` in `start` and are freed here and nowhere else,
        // once, because `ProgramMemory` is neither `Clone` nor `Copy`. The cycle that owns this
        // memory drops its engine state, its process I/O and its checkpoint store before it drops the
        // memory (the memory is its last field), so no borrow of either box is alive. The services
        // go first: they hold a borrow of the module for the debugger's statement hook.
        unsafe {
            drop(Box::from_raw(self.services.as_ptr()));
            drop(Box::from_raw(self.module.as_ptr()));
        }
    }
}
