//! A stepped scan of a program on the shared engine (`cycle.step`, specification 11 §6.8.1): the
//! names resolved as the test harness resolves them, the restart, the inputs, one scan without the
//! drivers, the observers or the capture of the retained values, and the watches.

use alloc::vec::Vec;

use smol_str::SmolStr;
use trust_runtime_core::bytecode::StorageRole;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::memory::InstanceId;
use trust_runtime_core::ports::{MonotonicClock, MonotonicInstant};
use trust_runtime_core::value::{Duration, RefSegment, Value};
use trust_runtime_core::vm::RuntimeState;

use super::{EngineCycle, ProcessIo};
use crate::cycle_step::{StepOutcome, StepRefusal, StepRequest};

/// A variable a step names: a resource global by its name, or a member of a program instance by
/// the program's name and the member's path.
enum Variable<'n> {
    Global(&'n str),
    Member {
        program: SmolStr,
        instance: InstanceId,
        path: &'n str,
    },
}

impl<C: MonotonicClock, I: ProcessIo, M> EngineCycle<'_, C, I, M> {
    /// The step of [`ResourceCycle::step`](super::ResourceCycle::step) on the engine. A restart
    /// restarts the program at `restart_at` as `RuntimeState::restart` does in place; the scan is
    /// the engine's cycle without hooks, whose fault the step clears after reading it.
    pub(super) fn run_step(
        &mut self,
        request: &StepRequest,
        restart_at: Duration,
        deadline: Option<MonotonicInstant>,
    ) -> Result<StepOutcome, StepRefusal> {
        let programs = self.program_names();
        let find = |name: &SmolStr| {
            find(&self.state, &programs, name)
                .map_err(|reason| StepRefusal::name(name, reason))
                .map(|_| ())
        };
        for (name, _) in &request.inputs {
            find(name)?;
        }
        for name in &request.watch {
            find(name)?;
        }
        if let Some(mode) = request.restart {
            self.state.set_current_time(restart_at);
            self.state.restart(mode).map_err(StepRefusal::Restart)?;
        }
        for (name, value) in &request.inputs {
            self.write(&programs, name, value.clone())
                .map_err(|error| StepRefusal::write(name, &error))?;
        }
        for (text, address, value) in &request.direct_inputs {
            self.state
                .write_image(address, value.clone())
                .map_err(|error| StepRefusal::write(text, &error))?;
        }
        self.state.set_current_time(request.time);
        self.services.arm(deadline);
        let error = self.state.execute_cycle(request.time).err();
        self.services.arm(None);
        if error.is_some() {
            // A stepped scan's fault is reported, not latched (specification 11 §6.8.1).
            self.state.clear_fault();
        }
        let mut values = Vec::with_capacity(request.watch.len());
        for name in &request.watch {
            let value = self
                .read(&programs, name)
                .map_err(|error| StepRefusal::read(name, &error))?;
            values.push((name.clone(), value));
        }
        let mut direct = Vec::with_capacity(request.watch_direct.len());
        for (text, address) in &request.watch_direct {
            let value = self
                .state
                .read_image(address)
                .map_err(|error| StepRefusal::read(text, &error))?;
            direct.push((text.clone(), value));
        }
        Ok(StepOutcome {
            values,
            direct,
            error,
        })
    }

    /// The names of the program instances of the installed module, in declaration order.
    fn program_names(&self) -> Vec<SmolStr> {
        let strings = self.prepared.strings();
        self.prepared
            .storage_declarations()
            .filter(|declaration| declaration.role == StorageRole::ProgramRoot)
            .filter_map(|declaration| {
                usize::try_from(declaration.name_idx)
                    .ok()
                    .and_then(|index| strings.get(index))
                    .map(SmolStr::new)
            })
            .collect()
    }

    /// Writes `value` into the variable `name` names.
    fn write(
        &mut self,
        programs: &[SmolStr],
        name: &str,
        value: Value,
    ) -> Result<(), RuntimeError> {
        match find(&self.state, programs, name).map_err(|_| RuntimeError::NullReference)? {
            Variable::Global(name) => self.state.write_global(name, value),
            Variable::Member { program, path, .. } => {
                let segments = path
                    .split('.')
                    .map(|field| RefSegment::Field(SmolStr::new(field)))
                    .collect::<Vec<_>>();
                self.state.write_named_path(&program, &segments, value)
            }
        }
    }

    /// The value of the variable `name` names.
    fn read(&self, programs: &[SmolStr], name: &str) -> Result<Value, RuntimeError> {
        let storage = self.state.storage();
        let value =
            match find(&self.state, programs, name).map_err(|_| RuntimeError::NullReference)? {
                Variable::Global(name) => storage.get_global(name),
                Variable::Member { instance, path, .. } => {
                    storage.get_instance_var_recursive(instance, path)
                }
            };
        value.cloned().ok_or(RuntimeError::NullReference)
    }
}

/// The variable `name` names, as the test harness resolves a name: a global by its name, a
/// qualified path `Program.member` by its full name, and a program variable by its name alone when
/// exactly one program declares it.
fn find<'n>(
    state: &RuntimeState<'_>,
    programs: &[SmolStr],
    name: &'n str,
) -> Result<Variable<'n>, &'static str> {
    let program = |candidate: &str| {
        programs
            .iter()
            .find(|program| program.eq_ignore_ascii_case(candidate))
    };
    let instance = |program: &SmolStr| state.read_global_instance(program).ok().flatten();
    let storage = state.storage();
    if program(name).is_none() && storage.get_global(name).is_some() {
        return Ok(Variable::Global(name));
    }
    if let Some((root, path)) = name.split_once('.')
        && let Some(program) = program(root)
        && let Some(instance) = instance(program)
        && storage.get_instance_var_recursive(instance, path).is_some()
    {
        return Ok(Variable::Member {
            program: program.clone(),
            instance,
            path,
        });
    }
    let mut found = None;
    for program in programs {
        if let Some(instance) = instance(program)
            && storage.get_instance_var_recursive(instance, name).is_some()
            && found
                .replace(Variable::Member {
                    program: program.clone(),
                    instance,
                    path: name,
                })
                .is_some()
        {
            return Err("names a variable of several programs");
        }
    }
    found.ok_or("names no variable of the program")
}
