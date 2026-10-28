//! The program of an isolated run (specification 35 A2.6; protocol annex E8.3): the snapshot's
//! Structured Text, freshly compiled for this computer with the project's conversion rules
//! (specification 07 §2.10) and started at simulation time zero as a runtime starts the program it
//! loads, and its signals, read and written by the symbols Control Studio names them by, through
//! the program access seam (specification 44 §13.3).

use std::collections::BTreeMap;

use trust_runtime_core::native::image::{SymbolRecord, NONE, SYMBOL_PROGRAM};

use super::worker::BuildSetting;

use crate::harness::SourceFile;
use crate::io::IoAddress;
use crate::native::{build_with_conversions, Options, Target, Toolchain};
use crate::runtime::program_access::{Located, ProgramRead};
use crate::stdlib::conversions::ConversionProfile;
use crate::value::{Duration, Value};
use crate::{CompiledRuntime, RestartMode};

/// The resource name of a program built without a `CONFIGURATION` (specification 10 §11.1.3).
const RESOURCE_NAME: &str = "RESOURCE";

/// The compiled program of one run, with the places of the signals the run has named.
pub(super) struct IsolatedProgram {
    runtime: CompiledRuntime,
    /// The program instances, by their declared names.
    programs: Vec<String>,
    /// Where each signal the run named lies: found once, since a run never changes its program.
    places: BTreeMap<String, Located>,
}

impl IsolatedProgram {
    /// The program of `sources`, built with the conversion rules `profile` and with what `build`
    /// names: the toolchain in its folder, or without one the toolchain the tools find
    /// (specification 44 §4.6), in a folder of this process in its temporary folder, or without
    /// one in this process's temporary folder, which the build removes. The image is a debug image
    /// like the one a project builds by default (§13.2). It starts as trust-runtime starts a
    /// program it loads: a cold restart gives every variable its starting value, and its tasks
    /// count their activations from time zero.
    ///
    /// # Errors
    ///
    /// No toolchain, sources that do not compile, or an image this computer refuses.
    pub(super) fn compile(
        sources: &[SourceFile],
        profile: ConversionProfile,
        build: &BuildSetting,
    ) -> Result<Self, String> {
        let toolchain = match &build.toolchain {
            Some(folder) => Toolchain::at(folder),
            None => Toolchain::find(),
        }
        .map_err(|error| error.to_string())?;
        let target = Target::this_cpu()
            .ok_or_else(|| String::from("this computer's CPU runs no program"))?;
        let work = build
            .temporary_folder
            .clone()
            .unwrap_or_else(std::env::temp_dir)
            .join(format!("trust-isolated-run-{}", std::process::id()));
        let built = build_with_conversions(
            sources,
            &[],
            &Options {
                target,
                hooks: true,
            },
            profile,
            &toolchain,
            &work,
        );
        // A folder that cannot be removed stays in the temporary folder.
        let _ = std::fs::remove_dir_all(&work);
        let built = built.map_err(|error| error.to_string())?;
        let resource = built.resource.unwrap_or_else(|| RESOURCE_NAME.into());
        let mut runtime =
            CompiledRuntime::load(built.image, resource).map_err(|error| error.to_string())?;
        runtime
            .restart(RestartMode::Cold)
            .map_err(|error| error.to_string())?;
        runtime.reset_task_timing(Duration::ZERO);
        let programs = runtime.with_program_view(program_instances);
        Ok(Self {
            runtime,
            programs,
            places: BTreeMap::new(),
        })
    }

    /// The program's runtime, which the run's plant reads after each scan.
    pub(super) fn runtime(&self) -> &CompiledRuntime {
        &self.runtime
    }

    /// The program's runtime, which the run's plant writes and whose scans the run executes.
    pub(super) fn runtime_mut(&mut self) -> &mut CompiledRuntime {
        &mut self.runtime
    }

    /// The program instances, by their declared names.
    pub(super) fn programs(&self) -> &[String] {
        &self.programs
    }

    /// The program instance `symbol` belongs to: its first name when that names a program
    /// instance, as `P1` in `P1.Parameters.IntegralGain`; `None` for a signal outside every program.
    pub(super) fn owning_program(&self, symbol: &str) -> Option<&str> {
        let first = symbol.split('.').next()?;
        self.programs
            .iter()
            .find(|name| name.eq_ignore_ascii_case(first))
            .map(String::as_str)
    }

    /// Where `symbol` lies, as Control Studio resolves a symbol path
    /// ([`crate::debug::resolve_control_parameter_path`]): a member of a program instance or of a
    /// global, at any depth (`Main.Kp`, `P1.Parameters.IntegralGain`), names compared without case.
    fn place(&mut self, symbol: &str) -> Result<Located, String> {
        if let Some(place) = self.places.get(symbol) {
            return Ok(*place);
        }
        let place = self
            .runtime
            .with_program_view(|program| {
                crate::debug::resolve_control_parameter_path(program, symbol)
            })?
            .at;
        self.places.insert(symbol.to_string(), place);
        Ok(place)
    }

    /// Whether `symbol` names a member a run may write as a parameter: declared, and neither
    /// constant, in-out nor located along its path ([`crate::debug::control_parameter_writable`]).
    pub(super) fn parameter_writable(&self, symbol: &str) -> bool {
        self.runtime.with_program_view(|program| {
            crate::debug::control_parameter_writable(&program.program_image(), symbol)
        })
    }

    /// The value of `symbol` as the last scan left it.
    ///
    /// # Errors
    ///
    /// A symbol that names nothing, or a place that holds no value (an instance).
    pub(super) fn value(&mut self, symbol: &str) -> Result<Value, String> {
        let place = self.place(symbol)?;
        self.runtime
            .with_program_view(|program| program.read_at(&place))
            .map_err(|error| format!("{symbol}: {error}"))
    }

    /// The finite number `symbol` holds (a BOOL reads as 0 or 1).
    ///
    /// # Errors
    ///
    /// As [`Self::value`], and a value that is no exactly representable number or not finite.
    pub(super) fn read(&mut self, symbol: &str) -> Result<f64, String> {
        let value = crate::debug::numeric_control_value(&self.value(symbol)?).ok_or_else(|| {
            String::from("signal is not an exactly representable numeric/BOOL value")
        })?;
        if !value.is_finite() {
            return Err(format!("nonfinite signal {symbol}"));
        }
        Ok(value)
    }

    /// Writes `number` into `symbol` as a value of the variable's own type
    /// ([`crate::debug::control_number_as`]). A variable bound to process-image addresses takes
    /// the value at each of them too, in the image's representation, since its scan samples it
    /// from there.
    ///
    /// # Errors
    ///
    /// A symbol that names nothing, a number the variable's type does not take, or a write the
    /// program refuses.
    pub(super) fn assign(&mut self, symbol: &str, number: f64) -> Result<(), String> {
        let place = self.place(symbol)?;
        let value = crate::debug::control_number_as(&self.value(symbol)?, number)?;
        if let Ok(index) = self.runtime.find_variable(symbol) {
            let codec = self.runtime.io_codec(index);
            for address in self.runtime.bound_addresses(index) {
                let image = codec
                    .encode(value.clone(), address.size)
                    .map_err(|error| error.to_string())?;
                self.runtime
                    .write_io(&address, image)
                    .map_err(|error| error.to_string())?;
            }
        }
        self.runtime
            .with_program(|program| program.write_at(&place, &value))
            .map_err(|_| format!("cannot assign scalar binding {symbol}"))
    }

    /// The process-image addresses the program binds `symbol` to; none for a variable without an
    /// address, or a member that no symbol of the program names.
    pub(super) fn bound_addresses(&self, symbol: &str) -> Vec<IoAddress> {
        self.runtime
            .find_variable(symbol)
            .map(|index| self.runtime.bound_addresses(index))
            .unwrap_or_default()
    }
}

/// The program instances of the program `program` reads, by their declared names: the root
/// symbols that the image marks as program instances (specification 44 §5.7).
fn program_instances(program: &dyn ProgramRead) -> Vec<String> {
    let image = program.program_image();
    let Ok(symbols) = image.table::<SymbolRecord>() else {
        return Vec::new();
    };
    symbols
        .iter()
        .filter(|symbol| symbol.flags & SYMBOL_PROGRAM != 0 && symbol.parent == NONE)
        .filter_map(|symbol| image.string(symbol.display).map(str::to_string))
        .collect()
}
