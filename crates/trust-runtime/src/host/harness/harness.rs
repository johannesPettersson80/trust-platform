//! Test harness for driving the cycles of a compiled program (specification 44 §14.2): the sources
//! are built for this computer and the kernel runs the program, as `trust-runtime` runs it.

#![allow(missing_docs)]

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::boundary::BoundaryError;
use crate::error::RuntimeError;
use crate::io::IoAddress;
use crate::memory::IoArea;
use crate::native::{build_with_conversions, Options, Target, Toolchain};
use crate::stdlib::conversions::ConversionProfile;
use crate::value::{read_partial_access, write_partial_access, Duration, Value};
use crate::CompiledRuntime;

use super::places::{self, Place};
use super::types::{CompileError, CycleResult};
use super::{CompileSession, NamedAccess, SourceFile};

const DEFAULT_MAX_CYCLES: u64 = 10_000;

/// The resource name of a program built without a `CONFIGURATION` (specification 10 §11.1.3).
const RESOURCE_NAME: &str = "RESOURCE";

/// Test harness for PLC code unit testing.
pub struct TestHarness {
    runtime: CompiledRuntime,
    cycle_count: u64,
    /// The conversion rules the program is built with, which a reload keeps (specification 07
    /// §2.10).
    conversions: ConversionProfile,
    /// The program's `VAR_ACCESS` declarations (specification 10 §4.1.2).
    access: Vec<NamedAccess>,
    /// The variables bound to process-image addresses with [`Self::bind_direct`].
    direct: Vec<DirectBinding>,
}

/// A variable bound to a process-image address with [`TestHarness::bind_direct`]: an input copies
/// the address into the variable before each cycle, an output the variable into the address after
/// it, and a marker both.
#[derive(Debug, Clone)]
struct DirectBinding {
    name: String,
    address: IoAddress,
}

impl TestHarness {
    /// Access the underlying runtime.
    #[must_use]
    pub fn runtime(&self) -> &CompiledRuntime {
        &self.runtime
    }

    /// Mutate the underlying runtime.
    pub fn runtime_mut(&mut self) -> &mut CompiledRuntime {
        &mut self.runtime
    }

    /// Consume the harness and return the runtime.
    #[must_use]
    pub fn into_runtime(self) -> CompiledRuntime {
        self.runtime
    }

    /// Creates a new test harness from source code.
    pub fn from_source(source: &str) -> Result<Self, CompileError> {
        Self::from_source_files(&[SourceFile::new(source)])
    }

    /// Creates a new test harness from multiple source files.
    pub fn from_sources(sources: &[&str]) -> Result<Self, CompileError> {
        let files = sources
            .iter()
            .copied()
            .map(SourceFile::new)
            .collect::<Vec<_>>();
        Self::from_source_files(&files)
    }

    /// Creates a new test harness from a compile session: its sources, its program instances
    /// registered at build time and its conversion rules (docs/specs/07-standard-functions.md
    /// 2.10), which a reload keeps.
    pub fn from_session(session: CompileSession) -> Result<Self, CompileError> {
        let (sources, tests, conversions) = session.build_inputs()?;
        Self::built(sources, tests, conversions)
    }

    /// The program of `sources`, built with the IEC conversion rules.
    fn from_source_files(sources: &[SourceFile]) -> Result<Self, CompileError> {
        Self::built(sources, &[], ConversionProfile::Iec)
    }

    /// The program of `sources`, with the program instances `tests` and the conversion rules
    /// `conversions`, started as `trust-runtime` starts a program it loads: a cold restart gives
    /// every variable its starting value (specification 44 §6.7), and the tasks count their
    /// activations from time zero.
    fn built(
        sources: &[SourceFile],
        tests: &[smol_str::SmolStr],
        conversions: ConversionProfile,
    ) -> Result<Self, CompileError> {
        let built = build_image(sources, tests, conversions)?;
        // A program's `RESOURCE` is the runtime's identity (specification 10 §11.1.3).
        let resource = built.resource.unwrap_or_else(|| RESOURCE_NAME.into());
        let mut runtime = CompiledRuntime::load(built.image, resource)
            .map_err(|error| CompileError::new(error.to_string()))?;
        runtime
            .restart(crate::RestartMode::Cold)
            .map_err(|error| CompileError::new(error.to_string()))?;
        runtime.reset_task_timing(Duration::ZERO);
        Ok(Self {
            runtime,
            cycle_count: 0,
            conversions,
            access: access_declarations(sources)?,
            direct: Vec::new(),
        })
    }

    /// Sets an input value.
    pub fn set_input(&mut self, name: &str, value: impl Into<Value>) {
        self.try_set_input(name, value)
            .unwrap_or_else(|error| panic!("{error}"));
    }

    /// Sets an input value and reports missing or ambiguous boundary names.
    pub fn try_set_input(
        &mut self,
        name: &str,
        value: impl Into<Value>,
    ) -> Result<(), BoundaryError> {
        let place = places::resolve(&self.runtime, name)?;
        places::write(&mut self.runtime, &place, value.into())
            .map_err(|error| BoundaryError::from_runtime(name, error))
    }

    /// Gets an output value.
    #[must_use]
    pub fn get_output(&self, name: &str) -> Option<Value> {
        self.try_get_output(name).ok()
    }

    /// Gets an output value and reports missing or ambiguous boundary names.
    pub fn try_get_output(&self, name: &str) -> Result<Value, BoundaryError> {
        let place = places::resolve(&self.runtime, name)?;
        places::read(&self.runtime, &place)
            .map_err(|error| BoundaryError::from_runtime(name, error))
    }

    /// Reads a `VAR_ACCESS` value (specification 10 §4.1.2): the value at the place its path names,
    /// with its partial selection; `None` for a name that no declaration has.
    #[must_use]
    pub fn get_access(&self, name: &str) -> Option<Value> {
        let access = self.access.iter().find(|access| access.name == name)?;
        let value = places::read(&self.runtime, &access_place(access).ok()?).ok()?;
        match access.partial {
            Some(partial) => read_partial_access(&value, partial).ok(),
            None => Some(value),
        }
    }

    /// Writes a `VAR_ACCESS` value (specification 10 §4.1.2): at the place its path names, only the
    /// selected bits of a partial selection.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::UndefinedVariable`] for a name that no declaration has,
    /// [`RuntimeError::ReadOnlyAlias`] for a declaration that takes no writes, and the refusal of a
    /// value the place does not take; nothing is written then.
    pub fn set_access(&mut self, name: &str, value: impl Into<Value>) -> Result<(), RuntimeError> {
        let access = self
            .access
            .iter()
            .find(|access| access.name == name)
            .ok_or_else(|| RuntimeError::UndefinedVariable(name.into()))?;
        if !access.writable {
            return Err(RuntimeError::ReadOnlyAlias(name.into()));
        }
        let place = access_place(access)?;
        let value = match access.partial {
            Some(partial) => {
                let current = places::read(&self.runtime, &place)?;
                write_partial_access(current, partial, value.into())?
            }
            None => value.into(),
        };
        places::write(&mut self.runtime, &place, value)
    }

    /// Binds the variable `name` names to the process-image `address`: an input address is copied
    /// into the variable before each cycle, the variable into an output address after it, and a
    /// marker both ways. A later binding of the same name replaces the earlier one.
    ///
    /// # Errors
    ///
    /// [`BoundaryError::UndeclaredBinding`] for a name that names no variable, and the boundary
    /// error of an address that does not parse.
    pub fn bind_direct(&mut self, name: &str, address: &str) -> Result<(), BoundaryError> {
        let address = IoAddress::parse(address)
            .map_err(|error| BoundaryError::from_runtime(address, error))?;
        match places::resolve(&self.runtime, name) {
            Ok(_) => {}
            Err(BoundaryError::UnresolvedName { .. }) => {
                return Err(BoundaryError::UndeclaredBinding { path: name.into() });
            }
            Err(error) => return Err(error),
        }
        self.direct.retain(|binding| binding.name != name);
        self.direct.push(DirectBinding {
            name: name.to_string(),
            address,
        });
        Ok(())
    }

    /// Copies each bound input and marker address into its variable.
    fn sample_direct(&mut self) -> Result<(), RuntimeError> {
        for binding in &self.direct {
            if binding.address.area == IoArea::Output {
                continue;
            }
            let value = self.runtime.read_io(&binding.address)?;
            let place = bound_place(&self.runtime, binding)?;
            places::write(&mut self.runtime, &place, value)?;
        }
        Ok(())
    }

    /// Copies each variable bound to an output or marker address into the address.
    fn publish_direct(&mut self) -> Result<(), RuntimeError> {
        for binding in &self.direct {
            if binding.address.area == IoArea::Input {
                continue;
            }
            let place = bound_place(&self.runtime, binding)?;
            let value = places::read(&self.runtime, &place)?;
            self.runtime.write_io(&binding.address, value)?;
        }
        Ok(())
    }

    /// Sets a direct input address.
    pub fn set_direct_input(
        &mut self,
        address: &str,
        value: impl Into<Value>,
    ) -> Result<(), RuntimeError> {
        let addr = IoAddress::parse(address)?;
        self.runtime.write_io(&addr, value.into())
    }

    /// Gets a direct output address.
    pub fn get_direct_output(&self, address: &str) -> Result<Value, RuntimeError> {
        let addr = IoAddress::parse(address)?;
        self.runtime.read_io(&addr)
    }

    /// Runs one cycle, with the bound addresses copied into their variables before it and the
    /// bound variables into their addresses after it.
    pub fn cycle(&mut self) -> CycleResult {
        let result = self
            .sample_direct()
            .and_then(|()| self.runtime.execute_cycle())
            .and_then(|()| self.publish_direct());
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
        F: Fn(&CompiledRuntime) -> bool,
    {
        self.run_until_max(condition, DEFAULT_MAX_CYCLES)
    }

    /// Runs until a condition is met, with a maximum cycle guard.
    pub fn run_until_max<F>(&mut self, condition: F, max_cycles: u64) -> Vec<CycleResult>
    where
        F: Fn(&CompiledRuntime) -> bool,
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
        let now = self.runtime.current_time();
        self.runtime.set_current_time(Duration::from_nanos(
            now.as_nanos().saturating_add(duration.as_nanos()),
        ));
    }

    /// Restarts the program (cold or warm).
    pub fn restart(&mut self, mode: crate::RestartMode) -> Result<(), RuntimeError> {
        self.runtime.restart(mode)
    }

    /// Restarts the program and reloads the retain store.
    pub fn restart_with_retain(&mut self, mode: crate::RestartMode) -> Result<(), RuntimeError> {
        self.runtime.restart_with_retain_store(mode)
    }

    /// Changes the running program online to the program of `source` (specification 44 §12): the
    /// new program takes the running program's values, its task schedule and its logical time, and
    /// is built with the running program's conversion rules (specification 07 §2.10).
    pub fn reload_source(&mut self, source: &str) -> Result<(), CompileError> {
        self.reload_source_files(&[SourceFile::new(source)])
    }

    /// Changes the running program online to the program of `sources`, as
    /// [`Self::reload_source`] does.
    pub fn reload_sources(&mut self, sources: &[&str]) -> Result<(), CompileError> {
        let files = sources
            .iter()
            .copied()
            .map(SourceFile::new)
            .collect::<Vec<_>>();
        self.reload_source_files(&files)
    }

    fn reload_source_files(&mut self, sources: &[SourceFile]) -> Result<(), CompileError> {
        let image = build_image(sources, &[], self.conversions)?.image;
        let access = access_declarations(sources)?;
        self.runtime
            .apply_online_change_bytes(&image)
            .map_err(|error| CompileError::new(error.to_string()))?;
        self.access = access;
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

/// The `VAR_ACCESS` declarations of `sources`, by the names of the program's symbols.
fn access_declarations(sources: &[SourceFile]) -> Result<Vec<NamedAccess>, CompileError> {
    // Only a program that declares access paths has any to resolve.
    if !sources
        .iter()
        .any(|source| source.text.to_ascii_uppercase().contains("VAR_ACCESS"))
    {
        return Ok(Vec::new());
    }
    Ok(CompileSession::from_sources(sources.to_vec())
        .declarations()?
        .access_paths()
        .to_vec())
}

/// The place a `VAR_ACCESS` declaration names.
///
/// # Errors
///
/// [`RuntimeError::NullReference`] for a place within a function block instance that an array or a
/// structure holds, which no symbol names.
fn access_place(access: &NamedAccess) -> Result<Place, RuntimeError> {
    let (variable, path) = access.symbol.clone().ok_or(RuntimeError::NullReference)?;
    Ok(Place { variable, path })
}

/// The place of a bound variable, resolved again for each cycle, so that a reload keeps the binding.
fn bound_place(program: &CompiledRuntime, binding: &DirectBinding) -> Result<Place, RuntimeError> {
    places::resolve(program, &binding.name)
        .map_err(|_| RuntimeError::UndefinedVariable(binding.name.as_str().into()))
}

/// The program image of `sources` with the program instances `tests` and the conversion rules
/// `conversions`, built for this computer with statement hooks, with the toolchain of
/// specification 44 §4.6.
fn build_image(
    sources: &[SourceFile],
    tests: &[smol_str::SmolStr],
    conversions: ConversionProfile,
) -> Result<crate::native::Build, CompileError> {
    static BUILDS: AtomicU64 = AtomicU64::new(0);
    let toolchain = Toolchain::find().map_err(|error| CompileError::new(error.to_string()))?;
    let target = Target::this_cpu()
        .ok_or_else(|| CompileError::new("this computer's CPU runs no program image"))?;
    let work = std::env::temp_dir().join(format!(
        "trust-harness-{}-{}",
        std::process::id(),
        BUILDS.fetch_add(1, Ordering::Relaxed)
    ));
    let built = build_with_conversions(
        sources,
        tests,
        &Options {
            target,
            hooks: true,
        },
        conversions,
        &toolchain,
        &work,
    );
    remove_work(work);
    built.map_err(|error| CompileError::new(error.to_string()))
}

/// Removes the build's work folder; a folder that cannot be removed stays in the temporary folder.
fn remove_work(work: PathBuf) {
    let _ = std::fs::remove_dir_all(work);
}

#[cfg(test)]
#[path = "harness_contract_tests.rs"]
mod contract_tests;
