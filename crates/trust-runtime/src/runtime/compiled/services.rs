//! What a computer's services ask of a compiled program between its scans, on the scan thread
//! (specification 44 §8.8, §13.3): the simulated plants, Control Studio's isolated runs and the
//! HMI's method commands reach the program here, through the program access seam, and read whether
//! the target is simulated.

use trust_runtime_services::runner::ResourceCycle;

use super::CompiledRuntime;
use crate::error::RuntimeError;
use crate::io::IoAddress;
use crate::runtime::program_access::{Access, ProgramRead, ProgramWrite, View};
use crate::value::Value;

impl CompiledRuntime {
    /// Runs `f` with the program, which it writes through the runtime's checks: between scans, on
    /// the scan thread, where nothing else reaches the program (specification 44 §13.3).
    pub fn with_program<R>(&mut self, f: impl FnOnce(&mut dyn ProgramWrite) -> R) -> R {
        f(&mut Access(self.cycle.program_mut()))
    }

    /// Runs `f` with the program as the last scan left it, for reading.
    pub fn with_program_view<R>(&self, f: impl FnOnce(&dyn ProgramRead) -> R) -> R {
        f(&View(self.cycle.program()))
    }

    /// Writes every value of `writes` at its process-image address, or none of them: each address is
    /// read first, so an address the image does not hold refuses the batch before anything changes,
    /// and a value its address refuses puts back the values already written (specification 46 §13.2:
    /// a plant publishes its feedback whole).
    ///
    /// # Errors
    ///
    /// The refusal of the first address or value that is refused.
    pub fn write_io_batch(&mut self, writes: &[(IoAddress, Value)]) -> Result<(), RuntimeError> {
        let mut previous = Vec::with_capacity(writes.len());
        for (address, _) in writes {
            previous.push(self.read_io(address)?);
        }
        for (index, (address, value)) in writes.iter().enumerate() {
            if let Err(error) = self.write_io(address, value.clone()) {
                for ((address, _), old) in writes[..index].iter().zip(&previous) {
                    // The value was read from this address a moment ago, so it is taken again.
                    let _ = self.write_io(address, old.clone());
                }
                return Err(error);
            }
        }
        Ok(())
    }

    /// Whether all the program's attached I/O is simulated (specification 35 A2.4): every driver
    /// says it is simulated, and no fieldbus client (ADS, OPC UA) is configured.
    #[must_use]
    pub fn simulation_only_io(&self) -> bool {
        self.cycle.process_io().all_drivers_simulated()
            && self.ads.connection_count() == 0
            && self.opcua_client.connection_count() == 0
    }

    /// Whether the program runs on a simulated target: simulation is on for it (specification 46
    /// §6.1) and all its attached I/O is simulated (specification 35 A2.4).
    #[must_use]
    pub fn simulated_target(&self) -> bool {
        self.simulation_enabled && self.simulation_only_io()
    }

    /// The program's process-image bindings, in the image's order: each address with the symbol of
    /// the variable bound there and the codec of its declared type (specification 44 §5.6).
    #[must_use]
    pub fn io_bindings(&self) -> &[trust_runtime_core::io_image::PreparedIoBinding] {
        &self.bindings
    }

    /// The program's generation: a restart and an online change each install the next
    /// (specification 44 §6.7, §12.2), and a [`Located`](crate::Located) found in one generation names
    /// nothing in another.
    #[must_use]
    pub fn generation(&self) -> crate::memory::Generation {
        self.cycle.generation()
    }

    /// The HMI exchange the program's `Hmi.Display` calls serve, once shared (specification 39
    /// §18).
    #[must_use]
    pub fn hmi_exchange(&self) -> Option<std::sync::Arc<crate::hmi::exchange::HmiExchange>> {
        self.hmi.exchange()
    }

    /// Shares `exchange` with the web server: the program's `Hmi.Display` calls serve it from now
    /// on, and the method commands queued for an earlier program are withdrawn.
    pub fn set_hmi_exchange(
        &mut self,
        exchange: std::sync::Arc<crate::hmi::exchange::HmiExchange>,
    ) {
        self.hmi.set_exchange(exchange);
    }

    /// Adds the `io.toml` driver `driver` named `name`, which runs the link `link` (specification
    /// 42, §2), after the others.
    pub fn add_io_driver_with_link(
        &mut self,
        name: impl Into<smol_str::SmolStr>,
        link: impl Into<smol_str::SmolStr>,
        driver: Box<dyn crate::io::IoDriver>,
    ) {
        self.cycle
            .process_io_mut()
            .add_driver_with_link(name, link, driver);
    }

    /// Notes the link of an `io.toml` driver configured with `enabled = false`, which reads Disabled
    /// (specification 42, §3 item 1).
    pub fn add_disabled_link(&mut self, link: impl Into<smol_str::SmolStr>) {
        self.links.add_disabled(link.into());
    }

    /// Whether the program instance `program` ran in the last scan: a program of a task ran when its
    /// task was due in that scan, and a program outside every task runs in every scan. A periodic
    /// task first runs one interval after the start, so a program's first execution need not be the
    /// first scan (an isolated run holds its candidate through it, specification 35 A7).
    #[must_use]
    pub fn program_ran_in_last_cycle(&self, program: &str) -> bool {
        let mut scheduled = false;
        for task in crate::debug::control_program_tasks(&self.image.image()) {
            if task
                .programs
                .iter()
                .any(|name| name.eq_ignore_ascii_case(program))
            {
                scheduled = true;
                if self
                    .last_task_runs
                    .iter()
                    .any(|(name, _)| name.eq_ignore_ascii_case(task.name.as_str()))
                {
                    return true;
                }
            }
        }
        !scheduled
            && self
                .programs
                .iter()
                .any(|name| name.eq_ignore_ascii_case(program))
    }

    /// Publishes the alarm service's records to the OpenOT ring when it is enabled (specification 40,
    /// §5.2).
    pub fn publish_alarm_records(&mut self, records: &[crate::hmi::alarms::AlarmEvent]) {
        if !records.is_empty() {
            self.openot_telemetry.publish_alarm_records(records);
        }
    }

    /// Whether simulation is on for the program (specification 46 §6.1), which the runtime's
    /// `simulation.toml` or the VS Code simulator's launch sets.
    #[must_use]
    pub fn simulation_enabled(&self) -> bool {
        self.simulation_enabled
    }

    /// Sets whether simulation is on for the program.
    pub fn set_simulation_enabled(&mut self, enabled: bool) {
        self.simulation_enabled = enabled;
    }
}
