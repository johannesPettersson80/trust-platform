//! Machine tests of a truST project in `cargo test` (specification 48 RU-58): the project's program
//! and its simulated plant (`simulation.toml`) run cycle by cycle, and the test writes and forces
//! values, runs cycles or logical time, reads any variable by its path, calls a method as a screen's
//! command does, and sees faults.
//!
//! The installed trust-runtime runs the program, in a simulator process of the test's own
//! (`trust-runtime simulate`): the same runtime as the VS Code extension's, on a clock that only the
//! test moves, so that a test gives the same result on every run and an hour of plant time takes
//! only the time its scans take. trust-sim finds the runtime through `TRUST_RUNTIME`, else on the
//! `PATH`, else in the newest truST VS Code extension, and refuses one of another version.
//!
//! # Example
//!
//! A tank's level alarm: above 90 the program raises it, and a forced level holds it down. The
//! example needs a project and an installed trust-runtime, so its test compiles it and does not run
//! it:
//!
//! ```no_run
//! #[test]
//! fn the_alarm_follows_the_level() -> trust_sim::Result<()> {
//!     let mut plant = trust_sim::Project::open("..")?.simulator()?;
//!     plant.write("Tank.level", 95.0)?;
//!     plant.run_cycles(3)?;
//!     assert!(plant.read::<bool>("Tank.alarm")?);
//!
//!     // A forced level holds whatever the program or the plant writes.
//!     plant.force("Tank.level", 40.0)?;
//!     plant.run_for(std::time::Duration::from_secs(60))?;
//!     assert!(!plant.read::<bool>("Tank.alarm")?);
//!     plant.release("Tank.level")?;
//!
//!     // The operator's Start of the filling sequence, a method of its interface.
//!     let started: bool = plant.call(
//!         "Tank.Filling",
//!         "Plant.ISequence",
//!         "Start",
//!         trust_sim::Inputs::new().with("Setpoint", 80.0),
//!     )?;
//!     assert!(started);
//!     Ok(())
//! }
//! # fn main() {}
//! ```
//!
//! A cycle that faults ends the run with [`Error::Fault`], and the project's fault policy then acts
//! as in a run: a test that expects the fault matches the error, reads the values and restarts the
//! program with [`Simulator::restart`].
//!
//! # Replaying a trace
//!
//! A runtime records the cycles of the commissioned machine as a cycle trace (`trust-runtime ctl
//! record`, specification 48 RU-59). [`Replay`] runs that trace in a simulator of the project as it
//! is now: the simulator takes the recorded state, applies each recorded change from outside, runs
//! each recorded cycle on the recorded inputs and clock readings, and compares its outputs with the
//! trace's (RU-60). A change of the program that is not meant to change the machine's behaviour
//! keeps the test green; one that changes an output fails it with a report of the first cycle that
//! differs:
//!
//! ```no_run
//! #[test]
//! fn the_commissioning_trace_still_matches() -> trust_sim::Result<()> {
//!     let project = trust_sim::Project::open("..")?;
//!     trust_sim::Replay::open("../traces/commissioning.trusttrace")?.run(&project)
//! }
//! # fn main() {}
//! ```

#![forbid(unsafe_code)]

mod project;
mod replay;
mod runtime;
mod simulator;
mod value;

use std::path::PathBuf;
use std::time::Duration;

pub use project::Project;
pub use replay::Replay;
pub use simulator::{Restart, Simulator};
/// `trust_plc`'s times and dates, which a machine test reads and writes as a Rust POU holds them.
pub use trust_plc::{Date, Dt, LDate, LTime, LTod, Ldt, Time, Tod};
/// The answer of a replay ([`Simulator::replay`]), whose `Display` is its report.
pub use trust_runtime_client::replay::{
    Difference, FirstDifference, OutputDifference, ReplayAnswer, ReplayKind,
};
pub use value::{Inputs, Returned, Value};

/// The result of a machine test's step.
pub type Result<T> = std::result::Result<T, Error>;

/// Why a step of a machine test failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// No trust-runtime was found.
    #[error(
        "no trust-runtime found; looked in {looked}. Install truST, or name the runtime in \
         TRUST_RUNTIME"
    )]
    RuntimeNotFound {
        /// The places looked in.
        looked: String,
    },
    /// The trust-runtime found is not trust-sim's version.
    #[error(
        "trust-runtime {runtime} at {path} is not trust-sim's version {sim}: use trust-sim \
         {runtime}, or trust-runtime {sim}",
        path = .path.display()
    )]
    RuntimeVersion {
        /// The runtime's path.
        path: PathBuf,
        /// The runtime's version.
        runtime: String,
        /// trust-sim's version.
        sim: String,
    },
    /// `trust-runtime build` refused the project.
    #[error("trust-runtime build failed for {}:\n{messages}", .project.display())]
    Build {
        /// The project's folder.
        project: PathBuf,
        /// The build's messages.
        messages: String,
    },
    /// A cycle faulted.
    #[error("the cycle at {time:?} faulted: {fault}")]
    Fault {
        /// The logical time of the cycle.
        time: Duration,
        /// The fault, as the runtime's `faults` lists it.
        fault: String,
    },
    /// The simulator refused a request: a path that names no variable, a value its type does not
    /// take, or a read before the first cycle.
    #[error("{request} refused: {message}")]
    Refused {
        /// The request.
        request: String,
        /// The runtime's reason.
        message: String,
    },
    /// A value read is of another kind than the test asked for.
    #[error("{path} holds {value}, which is not {kind}")]
    Kind {
        /// The variable's path.
        path: String,
        /// The value read.
        value: String,
        /// The kind the test asked for.
        kind: &'static str,
    },
    /// The simulator process could not start, or its link failed.
    #[error("the simulator: {0}")]
    Simulator(String),
    /// A trace's file could not be read.
    #[error("the trace {} cannot be read: {message}", .path.display())]
    Trace {
        /// The trace's file.
        path: PathBuf,
        /// Why it cannot be read.
        message: String,
    },
    /// A replayed trace differs from the build (specification 48 RU-60): the report gives the
    /// first cycle whose outputs or fault differ, each output that differs with the trace's value
    /// and the build's, and how many later cycles differ.
    #[error("the replay of {} differs from the trace:\n{report}", .trace.display())]
    Replay {
        /// The trace's file.
        trace: PathBuf,
        /// The report, as `trust-dev replay` prints it.
        report: String,
    },
}
