//! A project that machine tests run (specification 48 RU-58, "Finding the runtime" and "The
//! build").

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock, PoisonError};

use crate::simulator::Simulator;
use crate::{runtime, Error, Result};

/// The projects built in this process: a test process builds each project once, however many of
/// its tests open it.
static BUILT: OnceLock<Mutex<HashSet<PathBuf>>> = OnceLock::new();

/// A truST project, built by the installed trust-runtime, whose program a [`Simulator`] runs.
#[derive(Debug, Clone)]
pub struct Project {
    root: PathBuf,
    runtime: PathBuf,
}

impl Project {
    /// Opens the project in the folder `path`: finds the installed trust-runtime, which must be
    /// trust-sim's own version, and builds the project with `trust-runtime build`, the build the
    /// VS Code extension runs, once in this process.
    ///
    /// # Errors
    ///
    /// [`Error::RuntimeNotFound`], [`Error::RuntimeVersion`], and [`Error::Build`] with the build's
    /// messages.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Self::open_with_runtime(path, runtime::installed()?)
    }

    /// Opens the project in the folder `path` with the trust-runtime at `runtime` instead of the
    /// installed one, as a CI job that builds trust-runtime itself does. The runtime must be
    /// trust-sim's own version.
    ///
    /// # Errors
    ///
    /// [`Error::RuntimeVersion`], and [`Error::Build`] with the build's messages.
    pub fn open_with_runtime(path: impl AsRef<Path>, runtime: impl Into<PathBuf>) -> Result<Self> {
        let runtime = runtime.into();
        runtime::check_version(&runtime)?;
        let root = path
            .as_ref()
            .canonicalize()
            .map_err(|error| Error::Simulator(format!("{}: {error}", path.as_ref().display())))?;
        let mut built = BUILT
            .get_or_init(|| Mutex::new(HashSet::new()))
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if !built.contains(&root) {
            let output = Command::new(&runtime)
                .arg("build")
                .arg("--project")
                .arg(&root)
                .output()
                .map_err(|error| Error::Simulator(format!("trust-runtime build: {error}")))?;
            if !output.status.success() {
                return Err(Error::Build {
                    project: root,
                    messages: String::from_utf8_lossy(&output.stderr).into_owned(),
                });
            }
            built.insert(root.clone());
        }
        Ok(Self { root, runtime })
    }

    /// The project's folder.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Starts a simulator of the project's program and its simulation: a process of its own, which
    /// ends when the simulator is dropped, so that tests that run in parallel never share one.
    ///
    /// # Errors
    ///
    /// [`Error::Simulator`] when the process cannot start.
    pub fn simulator(&self) -> Result<Simulator> {
        Simulator::start(&self.runtime, &self.root)
    }
}
