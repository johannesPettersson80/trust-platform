//! Loading the installed program in place (specification 34 STATE-08 and §11.2).
//!
//! A `restart` of the Stopped resource starts the installed program again. A computer loads the
//! bundle that its project folder names. A chip loads the newest committed container of its program
//! store when that container is newer than the one its program came from, without a reset, so that
//! its control link stays up: a reset would drop the USB connection, which a computer's reload never
//! does. The loop owns the order and the device supplies the one step that is its own, building a
//! program from an admitted container ([`ProgramLoader`]), so that the boot of a device and its
//! reload take the same steps.
//!
//! The order is the one that keeps the peak memory at one program and one container. The program that
//! ended is dropped first, with its I/O lines, its checkpoint handle and its memory, and the Live
//! Values of the platform are emptied. Only then is the newest container read, checked and admitted
//! ([`select::boot`](crate::loading::select::boot)), and handed to the loader, which prepares it,
//! claims its lines and starts it in the requested mode. The loop feeds the hardware watchdog between
//! these steps (TIME-04). A program that cannot start leaves no program: the resource is Faulted with
//! `no installed program: <reason>`, and the next `restart` loads the newest container again.
//!
//! The same loader builds the candidate of an online change ([`ProgramLoader::prepare_change`]), whose
//! order is in `online_change.rs`: there the running program stays installed until the candidate is
//! ready, so the heap holds both.

use alloc::boxed::Box;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use smol_str::SmolStr;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::ports::{ControlLink, MonotonicClock, RunnerWait};
use trust_runtime_core::retain::RestartMode;
use trust_runtime_core::vm::RetainApplyEvent;

use super::online_change::Candidate;
use super::program::{no_program, Program};
use super::{EngineLiveValues, ResourceCycle, RetainSource, Runner, RunnerPlatform};
use crate::loading::select::{Booted, ScanError};

/// What the loop shows of a program that is being loaded: it is not a reason a restart would be
/// refused with, because the program is replaced or the load fails with its own reason.
const LOADING: &str = "the installed program is being loaded";

/// A program that a device built from an admitted container, started in the requested mode.
pub struct Loaded<Y> {
    /// The program's cycle, started cold or warm as the request asked.
    pub cycle: Y,
    /// The retained values that the start dropped or migrated, for the loop to record at the
    /// program's time zero (specification 11 §6.7).
    pub events: Vec<RetainApplyEvent>,
    /// The newest events the program's event store keeps: the capacity of its record, or the
    /// device's default when the record sets none (specification 34 §5.1.1).
    pub event_capacity: usize,
}

/// What a device supplies so that its loop can load the installed program in place (specification 34
/// STATE-08). `P` is the platform the loop runs over and `Y` the program's cycle.
///
/// The loop calls [`load`](Self::load) after it has dropped the program that ended, so the device's
/// memory and lines are free for the new program, and it never calls it while a program runs.
pub trait ProgramLoader<P, Y> {
    /// The bytes of memory the device has for a container now. The loop reads no container longer than
    /// this (specification 34 §5.1).
    fn room(&self) -> usize;

    /// Builds the program of `booted`, the newest committed container, admitted: prepares it with the
    /// limits the device allows, claims the lines of its record, sets the values of its record on
    /// `platform`, constructs it and starts it as `mode` asks (specification 34 STATE-08). The device
    /// feeds the hardware watchdog of `platform` between the steps that can take long.
    ///
    /// # Errors
    ///
    /// The reason the program cannot start, without the `no installed program: ` that the loop puts
    /// before it. Whatever the call acquired is released when it fails.
    fn load(
        &mut self,
        platform: &mut P,
        booted: Booted,
        mode: RestartMode,
    ) -> Result<Loaded<Y>, String>;

    /// Prepares and checks the candidate of an online change: the program of `program`, the upload of a
    /// `bytecode.reload` request, built over the record of the program that runs, `running`, while that
    /// program stays installed and keeps its lines (specification 34 §11.3; UPDATE-01, UPDATE-06).
    ///
    /// The device, in this order:
    ///
    /// 1. prepares the module with the limits that the heap allows now, its room less a reserve it
    ///    keeps ([`ChangeMemory`](crate::runner::ChangeMemory)), and frees `program` as soon as the
    ///    module is prepared;
    /// 2. checks the candidate's bindings against the lines of the running program's record
    ///    ([`unwired_binding`](crate::runner::unwired_binding));
    /// 3. builds the candidate from the running program's state with the engine's replacement
    ///    ([`replacement`](crate::runner::engine::EngineCycle::replacement)), over the running
    ///    program's settings and an empty process I/O, so that it claims no line and takes no
    ///    checkpoint store. The candidate continues the running program's installation generation,
    ///    keeps its logical time, its process images and its tasks' schedule, and keeps the retained
    ///    values the running program holds: the retained function-block instances and references
    ///    stay live when it declares the same retained storage, and the retained values migrate by
    ///    name otherwise. No checkpoint is read (decisions OC-1 and OC-2);
    /// 4. checks that the running program's safe state is valid in the candidate
    ///    ([`check_safe_outputs`](crate::runner::engine::EngineCycle::check_safe_outputs)), and gives
    ///    the candidate's Live Values the candidate's generation
    ///    ([`set_generation`](crate::runner::EngineLiveValues::set_generation)).
    ///
    /// The call changes nothing of the running program, of `platform` or of the lines: it does not
    /// write an output, claim or release a pin, take the checkpoint flash, or set the values of the
    /// platform. The device feeds the hardware watchdog of `platform` between the steps that can take
    /// long.
    ///
    /// # Errors
    ///
    /// The reply to the request: the `insufficient memory` refusal with its numbers, a binding that
    /// has no line, or the error of the preparation, the construction or the values, as a computer
    /// replies with the error of its transaction. Whatever the call built is dropped.
    fn prepare_change(
        &mut self,
        platform: &mut P,
        running: &mut Y,
        program: Vec<u8>,
    ) -> Result<Candidate<Y>, String>;

    /// The commit of an online change, in the one step that cannot fail: hands the lines and the
    /// checkpoint store of `running` to the candidate without releasing or claiming a line
    /// ([`hand_over_to`](crate::runner::engine::EngineCycle::hand_over_to)), and gives `platform` the
    /// values of the candidate, its Live Values (specification 34 §11.3). The loop makes the candidate
    /// its program and drops `running` afterwards.
    fn commit_change(&mut self, platform: &mut P, running: &mut Y, candidate: &mut Candidate<Y>);
}

/// A program built by the loader, with what the loop takes over from the platform and the install
/// sequence of its container.
struct Built<Y> {
    loaded: Loaded<Y>,
    live: EngineLiveValues,
    sequence: u64,
}

/// The failure of a store whose containers cannot be ordered, as a restart reports it.
fn scan_failure(error: ScanError) -> RuntimeError {
    no_program(&error.to_string())
}

impl<L, W, Y, P, C> Runner<L, W, Y, P, C>
where
    L: ControlLink,
    W: RunnerWait,
    Y: ResourceCycle,
    P: RunnerPlatform,
    C: MonotonicClock + Clone,
{
    /// Starts the installed program again for a `restart` of a resource whose program has ended: it
    /// loads the newest committed container in place when the store holds one newer than the container
    /// the program came from, and restarts the program in place otherwise (specification 34 STATE-08).
    /// Returns the retained values the start dropped or migrated.
    ///
    /// # Errors
    ///
    /// The failure of the restart, or of the load: `no installed program: <reason>`.
    pub(super) fn load_or_restart(
        &mut self,
        mode: RestartMode,
    ) -> Result<Vec<RetainApplyEvent>, RuntimeError> {
        if self.loader.is_some() && self.shell.install.newer_container().map_err(scan_failure)? {
            return self.reload(mode);
        }
        self.cycle.restart(mode, RetainSource::Checkpoint)
    }

    /// Drops the program that ended, loads the newest committed container in its place and starts it
    /// in `mode`. A program that cannot start leaves the loop without one, with the reason.
    fn reload(&mut self, mode: RestartMode) -> Result<Vec<RetainApplyEvent>, RuntimeError> {
        self.unload();
        self.shell.feed();
        match self.build(mode) {
            Ok(Built {
                loaded,
                live,
                sequence,
            }) => {
                let Loaded {
                    mut cycle, events, ..
                } = loaded;
                cycle.attach_hold(self.hold.clone());
                self.tasks = cycle.tasks();
                self.live = live;
                self.cycle = Program::Installed(cycle);
                self.shell.install.set_loaded(Some(sequence));
                Ok(events)
            }
            Err(reason) => {
                self.cycle = Program::Missing(SmolStr::new(&reason));
                Err(no_program(&reason))
            }
        }
    }

    /// The program that ended gives way before anything of the new one is read: its cycle is dropped,
    /// with the memory, the lines and the checkpoint handle it owns, and so are its Live Values, the
    /// writes and forces queued for it and its task list.
    fn unload(&mut self) {
        self.cycle = Program::Missing(SmolStr::new_static(LOADING));
        self.live.clear();
        self.live = EngineLiveValues::empty();
        self.tasks = Vec::new();
        let _ = self.shell.with_platform(RunnerPlatform::unload_program);
        self.shell.install.set_loaded(None);
    }

    /// Reads and admits the newest committed container and builds its program, in the order of the
    /// module documentation.
    fn build(&mut self, mode: RestartMode) -> Result<Built<Y>, String> {
        let in_use = || String::from("the loop's platform or program store is in use");
        let provider = self
            .loader
            .as_mut()
            .ok_or_else(|| String::from("the loop has no program loader"))?;
        let device = self.shell.device_descriptor().ok_or_else(in_use)?;
        let booted = self
            .shell
            .install
            .boot(&device, provider.room())
            .ok_or_else(in_use)?
            .map_err(|error| error.reason().to_string())?;
        let sequence = booted.sequence;
        self.shell.feed();
        let (program, live) = self
            .shell
            .with_platform(
                |platform| -> Result<(Loaded<Y>, EngineLiveValues), String> {
                    let program = provider.load(platform, booted, mode)?;
                    Ok((program, platform.live_values().clone()))
                },
            )
            .ok_or_else(in_use)??;
        self.shell.feed();
        if !self.shell.records.resize(program.event_capacity) {
            return Err(format!(
                "no room for an event store of {} events",
                program.event_capacity
            ));
        }
        Ok(Built {
            loaded: program,
            live,
            sequence,
        })
    }
}

impl<L, W, Y, P, C> Runner<L, W, Y, P, C> {
    /// Lets the loop load the installed program in place (specification 34 STATE-08): a `restart` of
    /// the Stopped resource that finds a committed container newer than the one the program came from
    /// drops the program and has `loader` build the newest container's, without a reset. `came_from`
    /// is the install sequence of the container that the loop's program came from, or that the boot
    /// refused; `None` when the program came from no container, or the store held none. The loop
    /// needs a program store ([`Runner::with_program_store`]) to find the container.
    ///
    /// The loader also lets the loop change its program online: it then takes `bytecode.reload`
    /// (specification 34 §11.3), builds the candidate with [`ProgramLoader::prepare_change`] and
    /// commits it with [`ProgramLoader::commit_change`] at the program's next boundary. A loop
    /// without a loader answers `bytecode.reload` with `unsupported request`.
    #[must_use]
    pub fn with_program_loader(
        mut self,
        loader: impl ProgramLoader<P, Y> + 'static,
        came_from: Option<u64>,
    ) -> Self {
        self.loader = Some(Box::new(loader));
        self.shell.install.set_loaded(came_from);
        // A loader that builds candidates also lets the loop change its program online.
        self.shell.change.enable();
        self
    }
}
