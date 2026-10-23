//! What Control Studio's real-runtime tests of stages V4.6 to V4.8 share beyond the runtime helpers of `common`: a copy of a project
//! that `trust-runtime` runs in simulation at a chosen speed, the command line of `trust-control-studio` run as a child process with
//! its output in files, the failed checks that a long test collects, and the reads that its checks make on the live controller.
//! `control_studio_validate.rs` (stage V3) keeps its own copies of these items.

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

use serde_json::Value;
use trust_control::loop_definition::{LoopDefinition, ModeWhenTrue};
use trust_control::recording::EvidenceSource;
use trust_control::runtime::RuntimeConnection;
use trust_control::signals::{loop_inspection_bindings, InspectionSample};
use trust_control_studio::journal::Journal;
use trust_control_studio::runtime_client::RuntimeClient;
use trust_control_studio::{loops, storage};

use super::{
    copy_dir, inspect_until_available, prepare_runtime_toml, run_trust_runtime, start_runtime,
    wait_for_status, RemoveDir, Runtime, TOKEN,
};

/// The runtime that the engine's verbs name for a project's loops; every example's loop is bound to it.
pub const RUNTIME_ID: &str = "simulator";
/// How often a test looks at a running program or at a file that the program writes.
const POLL: Duration = Duration::from_millis(25);

/// The failed checks of a test. A check that does not block the next step is recorded here and the test goes on, so that one run
/// names every failure that it reaches.
#[derive(Default)]
pub struct Findings {
    failed: Vec<String>,
}

impl Findings {
    /// Records `finding` unless `holds`. The test goes on.
    pub fn check(&mut self, holds: bool, finding: impl Into<String>) {
        if !holds {
            self.failed.push(finding.into());
        }
    }

    /// Records `finding` unless `holds`, and ends the test at once when it fails: the step cannot go on without it. The failures
    /// recorded before it are reported with it.
    pub fn require(&mut self, holds: bool, finding: impl Into<String>) {
        if !holds {
            self.failed.push(finding.into());
            self.conclude();
        }
    }

    /// Whether every check so far held.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.failed.is_empty()
    }

    /// The findings recorded so far, in the order they were recorded.
    #[must_use]
    pub fn failed(&self) -> &[String] {
        &self.failed
    }

    /// Fails the test with every finding recorded so far.
    pub fn conclude(&self) {
        assert!(
            self.failed.is_empty(),
            "{} check(s) failed:\n- {}",
            self.failed.len(),
            self.failed.join("\n- ")
        );
    }
}

/// A run of the command line, with its output in files so that nothing blocks on a full pipe. Dropping it ends the process if it
/// still runs, so a failed test leaves none behind.
pub struct Program {
    child: Child,
    out: PathBuf,
    err: PathBuf,
}

impl Program {
    /// Starts `trust-control-studio` with `arguments`, its output in `folder/name.out` and `folder/name.err`. With `with_token` the
    /// control token is in the environment (§0.1b: never an argument); otherwise the variable is removed, so a developer's value
    /// never gets in.
    pub fn start(arguments: &[String], with_token: bool, folder: &Path, name: &str) -> Self {
        let out = folder.join(format!("{name}.out"));
        let err = folder.join(format!("{name}.err"));
        let mut command = Command::new(
            std::env::var_os("CARGO_BIN_EXE_trust-control-studio").expect(
                "Cargo must provide trust-control-studio binary while executing integration tests",
            ),
        );
        command
            .args(arguments)
            .stdin(Stdio::null())
            .stdout(File::create(&out).expect("the program's output file"))
            .stderr(File::create(&err).expect("the program's error file"));
        if with_token {
            command.env("TRUST_CONTROL_TOKEN", TOKEN);
        } else {
            command.env_remove("TRUST_CONTROL_TOKEN");
        }
        let child = command.spawn().expect("start the program");
        Self { child, out, err }
    }

    /// Waits for the program to end, for `limit` at most.
    pub fn wait(&mut self, limit: Duration) -> ExitStatus {
        let begun = Instant::now();
        loop {
            if let Some(status) = self.child.try_wait().expect("poll the program") {
                return status;
            }
            assert!(
                begun.elapsed() < limit,
                "the program did not end within {} s; its standard error:\n{}",
                limit.as_secs(),
                read_text(&self.err)
            );
            sleep(POLL);
        }
    }

    /// Waits until the standard error holds `text`, for `limit` at most. Returns whether it did.
    pub fn wait_for_error_text(&self, text: &str, limit: Duration) -> bool {
        let begun = Instant::now();
        while begun.elapsed() < limit {
            if read_text(&self.err).contains(text) {
                return true;
            }
            sleep(POLL);
        }
        false
    }

    /// Sends the interrupt that Ctrl-C sends (`kill -INT`): the first one cancels the operation the program runs (§0.1b).
    pub fn interrupt(&self) {
        let status = Command::new("kill")
            .arg("-INT")
            .arg(self.child.id().to_string())
            .status()
            .expect("the kill command runs");
        assert!(status.success(), "the interrupt reached the program");
    }

    /// The answer on standard output, as `--json` writes it: `{"result": …}` or `{"failure": …}`. `Value::Null` when the output is
    /// not JSON, which the checks then report.
    pub fn answer(&self) -> Value {
        serde_json::from_str(&read_text(&self.out)).unwrap_or_default()
    }

    /// The readable text on standard output, as a verb without `--json` writes it.
    pub fn stdout_text(&self) -> String {
        read_text(&self.out)
    }

    /// What a failed check shows of the program that ended with `status`: its standard output (start) and its standard error.
    pub fn report(&self, status: ExitStatus) -> String {
        let output: String = read_text(&self.out).chars().take(2_000).collect();
        format!(
            "{status}\nstandard output (start):\n{output}\nstandard error:\n{}",
            read_text(&self.err)
        )
    }
}

impl Drop for Program {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// The arguments of `verb` on the loop `loop_id` of `project`: the project, the loop and the journal folder, then `extra`.
pub fn verb_arguments(
    verb: &str,
    project: &Path,
    loop_id: &str,
    journal: &Path,
    extra: &[&str],
) -> Vec<String> {
    let mut arguments = vec![
        verb.to_string(),
        "--project".to_string(),
        project.display().to_string(),
        "--loop".to_string(),
        loop_id.to_string(),
        "--journal-dir".to_string(),
        journal.display().to_string(),
    ];
    arguments.extend(extra.iter().map(|argument| (*argument).to_string()));
    arguments
}

/// A copy of an example project, built, with its runtime running at `time_scale` and its first scan read, and what the checks read
/// from its loop. The copy and its journal folder are removed when the lab drops.
pub struct Lab {
    /// The runtime process. It is declared first, so it ends before the folder goes.
    _runtime: Runtime,
    /// The folder of the test: the project copy, the journal folder and the output files of the program.
    pub folder: PathBuf,
    /// The copied project.
    pub project: PathBuf,
    /// The folder of the operation journals, passed as `--journal-dir`.
    pub journal: PathBuf,
    /// Where the runtime listens, and the test token.
    pub connection: RuntimeConnection,
    /// The loop the test works on, as the engine opens it.
    pub loop_definition: LoopDefinition,
    /// The program revision that the runtime reported at its first scan.
    pub program_revision: String,
    _remove: RemoveDir,
}

impl Lab {
    /// Copies `example` into `folder/project`, sets the simulation's speed to `time_scale`, builds the project, starts its runtime
    /// and waits for the first complete scan, which makes the target a simulated plant (E1.3). `loop_file` is the loop's file,
    /// relative to the project. The copy drops any `TEST_PROGRAM` source, which an ordinary runtime build rejects.
    pub fn start(name: &str, example: &str, loop_file: &str, time_scale: u32) -> Self {
        let folder = std::env::temp_dir().join(format!("trust-cs-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&folder);
        let remove = RemoveDir(folder.clone());
        let project = folder.join("project");
        copy_dir(Path::new(example), &project);
        remove_test_programs(&project);
        let socket = project.join(".control.sock");
        assert!(
            socket.as_os_str().len() <= 100,
            "the socket path {} is too long; use a shorter TMPDIR",
            socket.display()
        );
        prepare_runtime_toml(&project, &socket);
        set_time_scale(&project, time_scale);
        run_trust_runtime(&["build", "--project", &project.display().to_string()]);
        let declared = LoopDefinition::decode(&read_text(&project.join(loop_file)))
            .expect("the copy's loop decodes");
        let loop_definition = loops::open(&project, RUNTIME_ID, &declared.id)
            .expect("Control Studio opens the example's loop");

        let log = project.join("runtime.log");
        let runtime = start_runtime(&project, &log);
        let connection = RuntimeConnection {
            endpoint: format!("unix://{}", socket.display()),
            token: Some(TOKEN.into()),
            runtime_id: None,
        };
        let first = wait_for_status(&connection, &log);
        assert_eq!(
            first.simulation_time_scale, time_scale,
            "the runtime runs at the speed of simulation.toml"
        );
        let mut client = RuntimeClient::connect(&connection).expect("connected");
        let bindings = loop_inspection_bindings(&loop_definition);
        inspect_until_available(
            &mut client,
            &loop_definition,
            &bindings,
            &first.program_revision,
        );
        let status = client.status().expect("status");
        assert_eq!(
            status.source(),
            EvidenceSource::SimulatedPlant,
            "a simulated runtime with simulated I/O is a simulated plant: {status:?}"
        );

        let journal = folder.join("journal");
        Self {
            _runtime: runtime,
            folder,
            project,
            journal,
            connection,
            loop_definition,
            program_revision: first.program_revision,
            _remove: remove,
        }
    }

    /// Runs `verb` on the lab's loop to its end, with `extra` arguments. With `with_token` the control token is in the environment.
    /// The program's output goes to files named `name`.
    pub fn run(
        &self,
        name: &str,
        verb: &str,
        extra: &[&str],
        with_token: bool,
        limit: Duration,
    ) -> (Program, ExitStatus) {
        let arguments = verb_arguments(
            verb,
            &self.project,
            &self.loop_definition.id,
            &self.journal,
            extra,
        );
        let mut program = Program::start(&arguments, with_token, &self.folder, name);
        let status = program.wait(limit);
        (program, status)
    }

    /// A client of the lab's runtime.
    pub fn client(&self) -> RuntimeClient {
        RuntimeClient::connect(&self.connection).expect("the runtime answers")
    }

    /// One coherent sample of the loop's own bindings (`loop_inspection_bindings`), read from the live runtime (E3).
    pub fn sample(&self) -> InspectionSample {
        let bindings = loop_inspection_bindings(&self.loop_definition);
        inspect_until_available(
            &mut self.client(),
            &self.loop_definition,
            &bindings,
            &self.program_revision,
        )
    }

    /// The parameter record of the loop as the runtime holds it (`control.parameters.status`, E7.2).
    pub fn parameter_values(&self) -> BTreeMap<String, f64> {
        self.client()
            .parameter_values(&self.loop_definition)
            .expect("the runtime reports the parameter values")
    }

    /// Whether the loop is in Auto now, or `None` when its mode value is missing or is not a mode.
    pub fn is_auto(&self) -> Option<bool> {
        is_auto(
            &self.loop_definition,
            value_of(&self.sample(), "controllerMode"),
        )
    }
}

/// The value that `sample` holds for the binding `id`; `loop_inspection_bindings` names them (`processVariable`, `controllerMode`,
/// `setpoint`, ...). `None` when the sample has no such binding or no value for it.
pub fn value_of(sample: &InspectionSample, id: &str) -> Option<f64> {
    let index = sample
        .bindings
        .iter()
        .position(|binding| binding.id == id)?;
    sample.values.get(index).copied().flatten()
}

/// Whether the loop is in Auto, read from its mode value (TRUE is Auto or Manual as `modeWhenTrue` says; the default is Auto).
/// `None` when the value is missing or is neither 0 nor 1.
pub fn is_auto(loop_definition: &LoopDefinition, value: Option<f64>) -> Option<bool> {
    let true_is_auto = loop_definition
        .controller
        .mode_when_true
        .unwrap_or(ModeWhenTrue::Auto)
        == ModeWhenTrue::Auto;
    match value? {
        mode if mode > 0.5 => Some(true_is_auto),
        mode if mode < 0.5 => Some(!true_is_auto),
        _ => None,
    }
}

/// Whether `actual` is `expected` to the eight significant digits the engine writes gains with (a difference of one part in a
/// million is the same value).
pub fn close(actual: f64, expected: f64) -> bool {
    (actual - expected).abs() <= 1e-6 * expected.abs().max(1.0)
}

/// Whether two parameter records hold the same values, each one `close`.
pub fn same_values(actual: &BTreeMap<String, f64>, expected: &BTreeMap<String, f64>) -> bool {
    actual.len() == expected.len()
        && expected.iter().all(|(symbol, value)| {
            actual
                .get(symbol)
                .is_some_and(|measured| close(*measured, *value))
        })
}

/// The text of a file, or an empty text when it cannot be read.
pub fn read_text(path: &Path) -> String {
    String::from_utf8_lossy(&fs::read(path).unwrap_or_default()).into_owned()
}

/// The JSON of a file, or `Value::Null` when it is missing or is not JSON.
pub fn json_file(path: &Path) -> Value {
    serde_json::from_str(&read_text(path)).unwrap_or_default()
}

/// The loop's working study as `study.json` holds it, or `Value::Null` when there is none.
pub fn study_of(project: &Path, loop_id: &str) -> Value {
    json_file(&storage::study_directory(project, loop_id).join("study.json"))
}

/// The operation journal of the loop, as the engine reads it (A8.3, A8.5).
pub fn journal_of(journal: &Path, loop_definition: &LoopDefinition) -> Journal {
    Journal::open(journal, &loop_definition.project_id, &loop_definition.id)
        .expect("the journal reads")
}

/// Every file of `root` with its bytes, so that a refused verb can be shown to have written nothing. The runtime's own log, which
/// grows while it runs, and its control socket are left out.
pub fn snapshot(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn collect(root: &Path, folder: &Path, files: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(folder).expect("a folder of the copy") {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                collect(root, &path, files);
            } else if path.file_name().is_some_and(|name| name != "runtime.log")
                && let Ok(bytes) = fs::read(&path)
            {
                let relative = path.strip_prefix(root).expect("inside the copy");
                files.insert(relative.to_path_buf(), bytes);
            }
        }
    }
    let mut files = BTreeMap::new();
    collect(root, root, &mut files);
    files
}

/// Removes the copied sources that declare a `TEST_PROGRAM`: an ordinary runtime build rejects an unconfigured test program
/// (specification 22).
fn remove_test_programs(project: &Path) {
    let Ok(entries) = fs::read_dir(project.join("src")) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let is_structured_text = path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("st"));
        let declares_test_program = is_structured_text
            && fs::read_to_string(&path).is_ok_and(|text| {
                text.lines()
                    .any(|line| line.trim_start().starts_with("TEST_PROGRAM"))
            });
        if declares_test_program {
            fs::remove_file(&path).expect("remove the copied test program");
        }
    }
}

/// Sets `[simulation] time_scale` of the copy; the copy must have exactly one such line.
fn set_time_scale(project: &Path, scale: u32) {
    let path = project.join("simulation.toml");
    let text = fs::read_to_string(&path).expect("simulation.toml");
    let mut replaced = 0_u32;
    let lines: Vec<String> = text
        .lines()
        .map(|line| {
            if line.split('=').next().map(str::trim) == Some("time_scale") {
                replaced += 1;
                format!("time_scale = {scale}")
            } else {
                line.to_string()
            }
        })
        .collect();
    assert_eq!(replaced, 1, "simulation.toml has one time_scale line");
    fs::write(&path, format!("{}\n", lines.join("\n"))).expect("write simulation.toml");
}
