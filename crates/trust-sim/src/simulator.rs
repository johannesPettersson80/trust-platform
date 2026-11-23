//! A simulator of a project's program and its simulation, in a process of the test's own
//! (specification 48 RU-58; specification 11 §6.8.1, "Simulator for machine tests").

use std::io::{self, Read, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use serde_json::{json, Value as Json};
use trust_runtime_client::{ByteStream, ControlClient, ControlError};

use crate::{Error, Inputs, Result, Returned, Value};

/// How much of the simulator's standard error an error quotes.
const STDERR_TAIL: usize = 2_000;

/// A simulator of a project's program and its simulation: `trust-runtime simulate`, a process of
/// its own, whose logical time moves only when the test asks. Dropping it ends the process.
#[derive(Debug)]
pub struct Simulator {
    child: Child,
    client: Option<ControlClient>,
    stderr: Arc<Mutex<String>>,
    now: Duration,
}

/// How a restart starts the program again: warm keeps its retained values, cold starts every
/// variable at its initial value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Restart {
    /// The retained values stay; the others start at their initial values.
    Warm,
    /// Every variable starts at its initial value.
    Cold,
}

impl Simulator {
    pub(crate) fn start(runtime: &Path, root: &Path) -> Result<Self> {
        let mut child = Command::new(runtime)
            .arg("simulate")
            .arg("--project")
            .arg(root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| Error::Simulator(format!("trust-runtime simulate: {error}")))?;
        let stderr = Arc::new(Mutex::new(String::new()));
        if let Some(mut pipe) = child.stderr.take() {
            let stderr = stderr.clone();
            std::thread::spawn(move || {
                let mut text = String::new();
                let _ = pipe.read_to_string(&mut text);
                stderr
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .push_str(&text);
            });
        }
        let (Some(input), Some(output)) = (child.stdin.take(), child.stdout.take()) else {
            return Err(Error::Simulator(
                "the simulator's standard input or output".into(),
            ));
        };
        let client = ControlClient::from_stream(Pipes { input, output }, None);
        Ok(Self {
            child,
            client: Some(client),
            stderr,
            now: Duration::ZERO,
        })
    }

    /// The logical time: 0 before the first cycle, then the time of the last cycle or the end of the
    /// last [`Simulator::run_for`].
    #[must_use]
    pub fn now(&self) -> Duration {
        self.now
    }

    /// Runs `cycles` cycles of the resource, one every cycle interval of logical time.
    ///
    /// # Errors
    ///
    /// [`Error::Fault`] when a cycle faults, which ends the run there.
    pub fn run_cycles(&mut self, cycles: u64) -> Result<()> {
        self.advance(json!({ "cycles": cycles }))
    }

    /// Runs the cycles due within `duration` of logical time and ends at its end.
    ///
    /// # Errors
    ///
    /// [`Error::Fault`] when a cycle faults, which ends the run there.
    pub fn run_for(&mut self, duration: Duration) -> Result<()> {
        let nanos = u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX);
        self.advance(json!({ "duration_ns": nanos }))
    }

    /// Writes `value` into the variable `path` before the next cycle, as an input: the program may
    /// change it.
    ///
    /// # Errors
    ///
    /// [`Error::Refused`] for a path that names no variable, or a value its type does not take.
    pub fn write<V: Value>(&mut self, path: &str, value: V) -> Result<()> {
        self.request(
            "set",
            json!({ "target": format!("global:{path}"), "value": value.literal() }),
        )
        .map(drop)
    }

    /// Holds the variable `path` at `value` until [`Simulator::release`], whatever the program
    /// writes.
    ///
    /// # Errors
    ///
    /// [`Error::Refused`] for a path that names no variable, or a value its type does not take.
    pub fn force<V: Value>(&mut self, path: &str, value: V) -> Result<()> {
        self.request(
            "var.force",
            json!({ "target": format!("global:{path}"), "value": value.literal() }),
        )
        .map(drop)
    }

    /// Releases the force on the variable `path`.
    ///
    /// # Errors
    ///
    /// [`Error::Refused`] for a path that names no variable.
    pub fn release(&mut self, path: &str) -> Result<()> {
        self.request("var.unforce", json!({ "target": format!("global:{path}") }))
            .map(drop)
    }

    /// Reads the variable `path` as the last cycle left it.
    ///
    /// # Errors
    ///
    /// [`Error::Refused`] for a path that names no variable or before the first cycle, and
    /// [`Error::Kind`] for a value of another kind than `V`.
    pub fn read<V: Value>(&mut self, path: &str) -> Result<V> {
        let read = self.request("variables.read", json!({ "paths": [path] }))?;
        let value = &read["values"][0]["value"];
        V::from_json(value).ok_or_else(|| Error::Kind {
            path: path.to_string(),
            value: value.to_string(),
            kind: V::KIND,
        })
    }

    /// Calls the method `method` of the interface `interface` of the instance `instance` between two
    /// cycles, with `inputs`, as a screen's command calls it (`method.call`), and returns the
    /// method's result: `()` for a method without one. An input not given takes its declared
    /// initial value. The call runs at the logical time of the last cycle, and the next cycle sees
    /// what the method changed.
    ///
    /// ```no_run
    /// # fn main() -> trust_sim::Result<()> {
    /// # let mut plant = trust_sim::Project::open("..")?.simulator()?;
    /// let started: bool = plant.call(
    ///     "Main.Heating",
    ///     "Plant.ISequence",
    ///     "Start",
    ///     trust_sim::Inputs::new().with("Setpoint", 21.5),
    /// )?;
    /// assert!(started);
    /// plant.call::<()>("Main.Heating", "Plant.ISequence", "Reset", trust_sim::Inputs::new())?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// [`Error::Refused`] for an instance, an interface, a method or an input the program does not
    /// have, a value an input's type does not take, and a method that faulted, which faults the
    /// resource as a faulted cycle does; [`Error::Kind`] for a result of another kind than `R`.
    pub fn call<R: Returned>(
        &mut self,
        instance: &str,
        interface: &str,
        method: &str,
        inputs: Inputs,
    ) -> Result<R> {
        let answer = self.request(
            "method.call",
            json!({
                "instance": instance,
                "interface": interface,
                "method": method,
                "inputs": inputs.into_json(),
            }),
        )?;
        let value = &answer["value"];
        R::from_result(value).ok_or_else(|| Error::Kind {
            path: format!("{instance}.{method}"),
            value: value.to_string(),
            kind: R::RESULT_KIND,
        })
    }

    /// Restarts the program, warm or cold, as `trust-runtime ctl restart` does.
    ///
    /// # Errors
    ///
    /// [`Error::Refused`] when the runtime refuses the restart.
    pub fn restart(&mut self, mode: Restart) -> Result<()> {
        let mode = match mode {
            Restart::Warm => "warm",
            Restart::Cold => "cold",
        };
        self.request("restart", json!({ "mode": mode })).map(drop)
    }

    fn advance(&mut self, params: Json) -> Result<()> {
        let advanced = self.request("simulator.advance", params)?;
        if let Some(nanos) = advanced["time_ns"].as_u64() {
            self.now = Duration::from_nanos(nanos);
        }
        match advanced.get("fault") {
            Some(fault) => Err(Error::Fault {
                time: self.now,
                fault: fault.to_string(),
            }),
            None => Ok(()),
        }
    }

    fn request(&mut self, kind: &str, params: Json) -> Result<Json> {
        let client = self
            .client
            .as_mut()
            .ok_or_else(|| Error::Simulator("the simulator has ended".into()))?;
        client
            .request(kind, Some(params))
            .map_err(|error| match error {
                ControlError::Refused { error, .. } => Error::Refused {
                    request: kind.to_string(),
                    message: error,
                },
                other => Error::Simulator(format!("{kind}: {other}{}", self.stderr_tail())),
            })
    }

    fn stderr_tail(&self) -> String {
        let text = self.stderr.lock().unwrap_or_else(PoisonError::into_inner);
        if text.is_empty() {
            return String::new();
        }
        let start = text.len().saturating_sub(STDERR_TAIL);
        let start = (start..text.len())
            .find(|index| text.is_char_boundary(*index))
            .unwrap_or(text.len());
        format!("\nthe simulator's standard error:\n{}", &text[start..])
    }
}

impl Drop for Simulator {
    fn drop(&mut self) {
        // Closing its standard input ends the simulator (specification 11 §6.8.1).
        drop(self.client.take());
        if self.child.wait().is_err() {
            let _ = self.child.kill();
        }
    }
}

/// The simulator's standard input and output, the byte stream of its control link.
#[derive(Debug)]
struct Pipes {
    input: ChildStdin,
    output: ChildStdout,
}

impl Read for Pipes {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.output.read(buffer)
    }
}

impl Write for Pipes {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.input.write(buffer)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.input.flush()
    }
}

impl ByteStream for Pipes {
    /// A pipe has no timeout of its own: the simulator bounds each cycle itself and answers when it
    /// ran.
    fn set_timeout(&self, _timeout: Duration) -> io::Result<()> {
        Ok(())
    }
}
