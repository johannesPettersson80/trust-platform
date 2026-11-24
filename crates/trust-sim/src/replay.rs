//! Replaying a cycle trace in a simulator of a project (specification 48 RU-60, "In `cargo
//! test`"): a trace that a runtime recorded at commissioning becomes a test of the next change.

use std::path::{Path, PathBuf};

use crate::{Error, Project, Result};

/// A cycle trace, the `.trusttrace` file a runtime recorded (specification 48 RU-59), to replay in
/// a simulator of a project: the simulator takes the trace's start state and forces, applies each
/// recorded cycle's changes from outside, runs the cycle with the recorded tasks, inputs and clock
/// readings, and compares its outputs with the trace's (RU-60). A changed project takes the
/// recorded values as an online change carries them.
#[derive(Debug, Clone)]
pub struct Replay {
    path: PathBuf,
    trace: Vec<u8>,
}

impl Replay {
    /// The trace in the file at `path`.
    ///
    /// # Errors
    ///
    /// [`Error::Trace`] for a file that cannot be read.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let trace = std::fs::read(&path).map_err(|error| Error::Trace {
            path: path.clone(),
            message: error.to_string(),
        })?;
        Ok(Self { path, trace })
    }

    /// Replays the trace in a new simulator of `project`: `Ok(())` when every cycle's outputs, and
    /// a fault that ended the trace, match the trace's.
    ///
    /// # Errors
    ///
    /// [`Error::Replay`] with the report when a cycle's outputs or its fault differ;
    /// [`Error::Refused`] for a trace the simulator refuses, such as a damaged one
    /// (`trace_invalid`); and the errors of [`Project::simulator`].
    pub fn run(&self, project: &Project) -> Result<()> {
        let answer = project.simulator()?.replay(&self.trace)?;
        if answer.matched {
            Ok(())
        } else {
            Err(Error::Replay {
                trace: self.path.clone(),
                report: answer.to_string(),
            })
        }
    }
}
