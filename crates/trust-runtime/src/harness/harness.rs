//! Test harness for driving runtime cycles.

#![allow(missing_docs)]

use crate::error::RuntimeError;
use crate::io::IoAddress;
use crate::memory::InstanceId;
use crate::value::{Duration, Value};
use crate::Runtime;

use super::types::{CompileError, CycleResult};
use super::{CompileSession, SourceFile};

const DEFAULT_MAX_CYCLES: u64 = 10_000;

/// Test harness for PLC code unit testing.
pub struct TestHarness {
    runtime: Runtime,
    cycle_count: u64,
}

impl TestHarness {
    /// Access the underlying runtime.
    #[must_use]
    pub fn runtime(&self) -> &Runtime {
        &self.runtime
    }

    /// Mutate the underlying runtime.
    pub fn runtime_mut(&mut self) -> &mut Runtime {
        &mut self.runtime
    }

    /// Consume the harness and return the runtime.
    #[must_use]
    pub fn into_runtime(self) -> Runtime {
        self.runtime
    }

    /// Creates a new test harness from source code.
    pub fn from_source(source: &str) -> Result<Self, CompileError> {
        let runtime = CompileSession::from_source(source).build_runtime()?;
        Ok(Self {
            runtime,
            cycle_count: 0,
        })
    }

    /// Creates a new test harness from multiple source files.
    pub fn from_sources(sources: &[&str]) -> Result<Self, CompileError> {
        let source_files = sources.iter().copied().map(SourceFile::new).collect();
        let runtime = CompileSession::from_sources(source_files).build_runtime()?;
        Ok(Self {
            runtime,
            cycle_count: 0,
        })
    }

    /// Sets an input value.
    pub fn set_input(&mut self, name: &str, value: impl Into<Value>) {
        let value = value.into();
        if self.runtime.storage().get_global(name).is_some() {
            self.runtime.storage_mut().set_global(name, value);
            return;
        }
        if let Some(instance_id) = self.find_program_var_instance(name) {
            self.runtime
                .storage_mut()
                .set_instance_var(instance_id, name, value);
        } else {
            self.runtime.storage_mut().set_global(name, value);
        }
    }

    /// Gets an output value.
    #[must_use]
    pub fn get_output(&self, name: &str) -> Option<Value> {
        if let Some(value) = self.runtime.storage().get_global(name) {
            return Some(value.clone());
        }
        self.read_program_var(name)
    }

    /// Gets a VAR_ACCESS value.
    #[must_use]
    pub fn get_access(&self, name: &str) -> Option<Value> {
        self.runtime.read_access(name)
    }

    /// Sets a VAR_ACCESS value.
    pub fn set_access(&mut self, name: &str, value: impl Into<Value>) -> Result<(), RuntimeError> {
        self.runtime.write_access(name, value.into())
    }

    /// Sets a direct input address.
    pub fn set_direct_input(
        &mut self,
        address: &str,
        value: impl Into<Value>,
    ) -> Result<(), RuntimeError> {
        let addr = IoAddress::parse(address)?;
        self.runtime.io_mut().write(&addr, value.into())
    }

    /// Gets a direct output address.
    pub fn get_direct_output(&self, address: &str) -> Result<Value, RuntimeError> {
        let addr = IoAddress::parse(address)?;
        self.runtime.io().read(&addr)
    }

    /// Binds a variable name to a direct address.
    pub fn bind_direct(&mut self, name: &str, address: &str) -> Result<(), RuntimeError> {
        let addr = IoAddress::parse(address)?;
        if self.runtime.storage().get_global(name).is_some() {
            self.runtime.io_mut().bind(name, addr);
            return Ok(());
        }
        if let Some(instance_id) = self.find_program_var_instance(name) {
            if let Some(reference) = self
                .runtime
                .storage()
                .ref_for_instance_recursive(instance_id, name)
            {
                self.runtime.io_mut().bind_ref(reference, addr);
                return Ok(());
            }
        }
        self.runtime.io_mut().bind(name, addr);
        Ok(())
    }

    /// Runs one cycle.
    pub fn cycle(&mut self) -> CycleResult {
        let result = self.runtime.execute_cycle();
        self.cycle_count += 1;
        CycleResult {
            cycle_number: self.cycle_count,
            elapsed_time: self.runtime.current_time(),
            errors: result.err().into_iter().collect(),
        }
    }

    /// Runs multiple cycles.
    pub fn run_cycles(&mut self, count: u32) -> Vec<CycleResult> {
        (0..count).map(|_| self.cycle()).collect()
    }

    /// Runs until a condition is met.
    pub fn run_until<F>(&mut self, condition: F) -> Vec<CycleResult>
    where
        F: Fn(&Runtime) -> bool,
    {
        self.run_until_max(condition, DEFAULT_MAX_CYCLES)
    }

    /// Runs until a condition is met, with a maximum cycle guard.
    pub fn run_until_max<F>(&mut self, condition: F, max_cycles: u64) -> Vec<CycleResult>
    where
        F: Fn(&Runtime) -> bool,
    {
        let mut results = Vec::new();
        while !condition(&self.runtime) {
            if results.len() as u64 >= max_cycles {
                panic!("run_until exceeded {max_cycles} cycles without condition becoming true");
            }
            results.push(self.cycle());
        }
        results
    }

    /// Advances simulation time.
    pub fn advance_time(&mut self, duration: Duration) {
        self.runtime.advance_time(duration);
    }

    /// Restarts the runtime (cold or warm).
    pub fn restart(&mut self, mode: crate::RestartMode) -> Result<(), RuntimeError> {
        self.runtime.restart(mode)
    }

    /// Restart the runtime and reload the retain store.
    pub fn restart_with_retain(&mut self, mode: crate::RestartMode) -> Result<(), RuntimeError> {
        self.runtime.restart(mode)?;
        self.runtime.load_retain_store()?;
        Ok(())
    }

    /// Reload a single source file, preserving retained variables when possible.
    pub fn reload_source(&mut self, source: &str) -> Result<(), CompileError> {
        let retained = self.runtime.retain_snapshot();
        let debug = self.runtime.debug_control();
        let current_time = self.runtime.current_time();
        let cycle_count = self.cycle_count;

        let mut rebuilt = TestHarness::from_source(source)?;
        if let Some(control) = debug {
            rebuilt.runtime.set_debug_control(control);
        }
        rebuilt.runtime.apply_retain_snapshot(&retained);
        rebuilt.runtime.set_current_time(current_time);
        rebuilt.cycle_count = cycle_count;

        *self = rebuilt;
        Ok(())
    }

    /// Reload multiple source files, preserving retained variables when possible.
    pub fn reload_sources(&mut self, sources: &[&str]) -> Result<(), CompileError> {
        let retained = self.runtime.retain_snapshot();
        let debug = self.runtime.debug_control();
        let current_time = self.runtime.current_time();
        let cycle_count = self.cycle_count;

        let mut rebuilt = TestHarness::from_sources(sources)?;
        if let Some(control) = debug {
            rebuilt.runtime.set_debug_control(control);
        }
        rebuilt.runtime.apply_retain_snapshot(&retained);
        rebuilt.runtime.set_current_time(current_time);
        rebuilt.cycle_count = cycle_count;

        *self = rebuilt;
        Ok(())
    }

    /// Gets the current simulation time.
    #[must_use]
    pub fn current_time(&self) -> Duration {
        self.runtime.current_time()
    }

    /// Gets the cycle count.
    #[must_use]
    pub fn cycle_count(&self) -> u64 {
        self.cycle_count
    }

    /// Asserts that a variable has a specific value.
    pub fn assert_eq(&self, name: &str, expected: impl Into<Value>) {
        let value = self
            .get_output(name)
            .unwrap_or_else(|| panic!("missing variable '{name}'"));
        assert_eq!(value, expected.into());
    }
}

impl TestHarness {
    fn find_program_var_instance(&self, name: &str) -> Option<InstanceId> {
        let storage = self.runtime.storage();
        let mut match_id = None;
        for program in self.runtime.programs().values() {
            let Some(Value::Instance(id)) = storage.get_global(program.name.as_ref()) else {
                continue;
            };
            if storage.get_instance_var(*id, name).is_some() {
                if match_id.is_some() {
                    return None;
                }
                match_id = Some(*id);
            }
        }
        match_id
    }

    fn read_program_var(&self, name: &str) -> Option<Value> {
        let storage = self.runtime.storage();
        let mut match_value = None;
        for program in self.runtime.programs().values() {
            let Some(Value::Instance(id)) = storage.get_global(program.name.as_ref()) else {
                continue;
            };
            if let Some(value) = storage.get_instance_var(*id, name) {
                if match_value.is_some() {
                    return None;
                }
                match_value = Some(value.clone());
            }
        }
        match_value
    }
}
