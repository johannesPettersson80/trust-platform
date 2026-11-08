//! The simulation a resource runs, shared between its scan thread and its control (specification
//! 46 §6.1; specification 47): the scan thread locks it only for a cycle's plant steps, before and
//! after the program runs, so that a control request reads and clears the plants while the
//! debugger holds the program between them (specification 13, "Pause").

use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use super::SimulationSpeed;
use crate::simulation::{
    PlantTraceClearError, PlantTraceClearResult, SimulationController, SimulationRuntimeSnapshot,
};

/// The resource's simulation, when it runs one, and whether all of its program's attached I/O is
/// simulated, which its snapshot reports (specification 35, A2.4).
#[derive(Clone, Default)]
pub(crate) struct LiveSimulation(Arc<Mutex<Option<Attached>>>);

struct Attached {
    controller: SimulationController,
    simulation_only_io: bool,
}

impl fmt::Debug for LiveSimulation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LiveSimulation")
            .field("attached", &self.lock().is_some())
            .finish()
    }
}

impl LiveSimulation {
    fn lock(&self) -> MutexGuard<'_, Option<Attached>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Attaches `controller` for a program whose attached I/O is all simulated when
    /// `simulation_only_io`, or, with none, detaches the simulation.
    pub(crate) fn set(&self, controller: Option<SimulationController>, simulation_only_io: bool) {
        *self.lock() = controller.map(|controller| Attached {
            controller,
            simulation_only_io,
        });
    }

    /// Whether the resource runs a simulation.
    pub(crate) fn is_attached(&self) -> bool {
        self.lock().is_some()
    }

    /// `step` on the attached simulation, under the lock; `None` without one.
    pub(crate) fn with<R>(&self, step: impl FnOnce(&mut SimulationController) -> R) -> Option<R> {
        self.lock()
            .as_mut()
            .map(|attached| step(&mut attached.controller))
    }

    /// The plants' status and trace as the simulation holds them now (specification 47,
    /// `plant.status`, `plant.trace.read`), the status's time scale the resource's live `speed`
    /// where it has one; `None` without a simulation.
    #[must_use]
    pub(crate) fn snapshot(
        &self,
        speed: Option<&SimulationSpeed>,
    ) -> Option<SimulationRuntimeSnapshot> {
        let attached = self.lock();
        let attached = attached.as_ref()?;
        let mut snapshot = attached.controller.runtime_snapshot();
        snapshot.simulation_only_io = Some(attached.simulation_only_io);
        if let Some(speed) = speed {
            snapshot.status.time_scale = speed.get();
        }
        Some(snapshot)
    }

    /// Clears the trace of the plant `plant_id` when the active configuration is the revision
    /// `revision` (specification 47, `plant.trace.clear`).
    ///
    /// # Errors
    ///
    /// [`PlantTraceClearError::StatusUnavailable`] without a simulation, and the simulation's
    /// refusal.
    pub(crate) fn clear_plant_trace(
        &self,
        plant_id: &str,
        revision: &str,
    ) -> Result<PlantTraceClearResult, PlantTraceClearError> {
        self.with(|simulation| simulation.clear_plant_trace(plant_id, revision))
            .unwrap_or(Err(PlantTraceClearError::StatusUnavailable))
    }
}
