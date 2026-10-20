//! What an online change takes from the running program and gives to its candidate (specification 34
//! §11.3; specification 11 §6.7.1; UPDATE-01 to UPDATE-07).
//!
//! The candidate is a program built over the same record, while the running program stays installed. It
//! is built from the running program's own state by the engine's replacement
//! ([`replacement`](crate::runner::engine::EngineCycle::replacement)), the body that a computer's
//! online change runs too: the candidate continues the running program's installation generation,
//! keeps its logical time and its process images, keeps the retained function-block instances and
//! references live when it declares the same retained storage and migrates the retained values by
//! name otherwise, and takes the newest saved snapshot over them. It is constructed with the running
//! program's settings and an empty process I/O, and is checked against the running program's safe
//! state ([`check_safe_outputs`](crate::runner::engine::EngineCycle::check_safe_outputs)). The commit
//! then hands over the two things that belong to the machine and not to the program, its lines and its
//! checkpoint store ([`hand_over_to`](crate::runner::engine::EngineCycle::hand_over_to)), without
//! releasing or claiming a line and without writing an output.

use alloc::vec::Vec;

use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::memory::{Generation, VariableStorage};
use trust_runtime_core::ports::MonotonicClock;
use trust_runtime_core::retain::RetainSnapshot;
use trust_runtime_core::value::{Value, ValueRef};
use trust_runtime_core::vm::{PreparationLimits, PreparedModule, RetainApplyEvent};

use super::{CycleSettings, DeviceServices, EngineCycle, ProcessIo, RESOURCE_INDEX};

impl<C, I, M> EngineCycle<'_, C, I, M> {
    /// The settings the program runs with: the cycle interval, the watchdog and fault policies and the
    /// safe value of each output, which an online change gives its candidate unchanged because the
    /// change brings no record (specification 34 §11.3).
    #[must_use]
    pub fn settings(&self) -> &CycleSettings {
        &self.settings
    }

    /// The preparation limits the program's module was prepared with.
    #[must_use]
    pub fn limits(&self) -> PreparationLimits {
        self.prepared.limits()
    }

    /// The installation generation the program's state is in (specification 34 §11.3, MEM-07). Every
    /// restart and every online change installs the next one, so a reference taken in an earlier
    /// generation is refused as stale. It is read from the store, not from the storage view, whose
    /// projection of every value a chip would build and keep for this one number (C1-19f).
    #[must_use]
    pub fn generation(&self) -> Generation {
        trust_runtime_core::memory::view::StoreAccess::generation(&self.state)
    }

    /// The program's variables, for inspection: a read-only view, as `RuntimeState::storage` gives it.
    #[must_use]
    pub fn storage(&self) -> &VariableStorage {
        self.state.storage()
    }

    /// Reads the variable `reference` names, through the engine's checked read: a reference of another
    /// installation generation is refused with [`RuntimeError::ReferenceLifetime`] (specification 34
    /// UPDATE-04).
    ///
    /// # Errors
    ///
    /// The [`RuntimeError`] of a reference that is stale or names no variable.
    pub fn read_reference(&mut self, reference: &ValueRef) -> Result<Value, RuntimeError> {
        self.state.read_reference(reference)
    }

    /// Checks that each safe output of the settings is a valid write of this program's process image,
    /// as a computer's `validate_replacement_policy` checks the I/O policy of the running
    /// configuration against a candidate. A stop or a fault writes these values into the image, so a
    /// candidate that cannot take them would fail the safe state of the machine.
    ///
    /// # Errors
    ///
    /// The [`RuntimeError`] of the first safe output that the image refuses.
    pub fn check_safe_outputs(&mut self) -> Result<(), RuntimeError> {
        for (address, value) in &self.settings.safe_outputs {
            self.state.validate_image_write(address, value.clone())?;
        }
        Ok(())
    }

    /// The commit of an online change takes the machine's two shared parts from this program and
    /// gives them to `successor`: the process I/O, which owns the claimed lines with the levels their
    /// outputs hold, and the checkpoint store, which owns the checkpoint flash and the log. The two
    /// programs exchange them, so the successor's empty process I/O, which claimed no line, goes with
    /// this program and is dropped with it. Nothing is claimed, released or written, and the call
    /// cannot fail.
    pub fn hand_over_to(&mut self, successor: &mut Self) {
        core::mem::swap(&mut self.io, &mut successor.io);
        core::mem::swap(&mut self.retain, &mut successor.retain);
    }
}

impl<C: MonotonicClock, I, M> EngineCycle<'_, C, I, M> {
    /// Builds the cycle of the program `prepared` that replaces this one at a boundary, over the
    /// process I/O `io` and the settings `settings`, and returns it with the retained values that
    /// its construction dropped or migrated (specification 34 §11.3; specification 11 §6.7.1). It is the
    /// engine's replacement of a computer's online change on the state this device borrows
    /// (`RuntimeState::prepare_replacement`):
    ///
    /// - the candidate continues this program's installation generation with the next one, so a
    ///   reference or an instance identity taken before the change is refused as stale after it
    ///   (UPDATE-04);
    /// - it keeps the logical time of this program and carries its process images;
    /// - a candidate that declares the same retained storage keeps the retained function-block
    ///   instances and references live, and a candidate that does not migrates a snapshot of this
    ///   program's retained values by name, and is refused when a retained live object cannot be
    ///   serialized into one;
    /// - `durable`, the newest saved snapshot of the device's retain store, is applied over all of it
    ///   by name.
    ///
    /// The candidate holds the module and the services given here and no borrow of this program, so
    /// whatever memory this program lives in can be freed once it is dropped. It has no checkpoint
    /// store: the commit gives it this program's ([`hand_over_to`](Self::hand_over_to)). This program
    /// is unchanged, whether the candidate is built or refused.
    ///
    /// # Errors
    ///
    /// The [`RuntimeError`] of a candidate that cannot be built: a retained value that its declared
    /// type refuses, a retained live object that cannot be migrated, an allocation that the device
    /// refuses, or an allowance that is exceeded.
    pub fn replacement<'b, J: ProcessIo>(
        &mut self,
        prepared: &'b PreparedModule,
        services: &'b DeviceServices<C>,
        io: J,
        settings: CycleSettings,
        durable: Option<&RetainSnapshot>,
    ) -> Result<(EngineCycle<'b, C, J>, Vec<RetainApplyEvent>), RuntimeError> {
        let (state, events) =
            self.state
                .prepare_replacement(prepared, RESOURCE_INDEX, services, durable)?;
        Ok((
            EngineCycle {
                state,
                prepared,
                services,
                io,
                settings,
                retain: None,
                memory: (),
            },
            events,
        ))
    }
}
