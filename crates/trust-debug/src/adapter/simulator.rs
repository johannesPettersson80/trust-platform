//! The simulator of a project compiled for this computer (specification 44 §8.9, NX-16): the
//! adapter builds the project's image from the launch's sources, runs it in its own process as
//! trust-runtime runs a compiled program (the resource's scheduler, the `io.toml` drivers and a
//! control server at the launch's endpoint), and debugs it through the control protocol as an
//! attach does.

use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::Arc;

use serde_json::Value;
use smol_str::SmolStr;
use trust_runtime::config::{IoConfig, RuntimeConfig};
use trust_runtime::control::{ControlEndpoint, SourceFile as ControlSourceFile};
use trust_runtime::harness::SourceFile;
use trust_runtime::native::{build, debug_hooks, triple, NativeError, Options, Target, Toolchain};
use trust_runtime::scheduler::{ResourceHandle, ResourceRunner, StartGate, StdClock};
use trust_runtime::value::Duration;
use trust_runtime::{CompiledRuntime, RestartMode};

use crate::protocol::{LaunchArguments, OutputEventBody, ReloadArguments, Request};
use crate::session::SourceOptionsUpdate;

use super::control_bridge::{default_control_endpoint, DebugControlServer, ServedProgram};
use super::launch::{launch_control_auth_token, launch_control_endpoint, launch_runtime_root};
use super::remote::{RemoteEndpoint, RemoteSession};
use super::{DebugAdapter, DispatchOutcome};

/// Why a launch runs its program on the bytecode engine instead (specification 44 NX-16).
pub(super) enum EngineReason {
    /// No toolchain is installed (specification 44 §4.6).
    NoToolchain(String),
    /// The project has a construct the native build does not compile yet (NX-15).
    NotCompiledYet(String),
}

impl EngineReason {
    /// What the debug console says.
    pub(super) fn message(&self) -> String {
        match self {
            Self::NoToolchain(detail) => format!(
                "the simulator runs the program on the bytecode engine: no toolchain to compile it \
                 ({detail})"
            ),
            Self::NotCompiledYet(detail) => {
                format!("the simulator runs the program on the bytecode engine: {detail}")
            }
        }
    }
}

/// The launch's sources as the native build takes them, each labelled by its path in the project
/// folder `root`, as `trust-runtime build` labels them (specification 44 §5.3).
fn labelled(root: &Path, sources: &[(String, String)]) -> Vec<SourceFile> {
    sources
        .iter()
        .map(|(path, text)| {
            let path = Path::new(path);
            let label = path.strip_prefix(root).unwrap_or(path);
            SourceFile::with_path(label.to_string_lossy().replace('\\', "/"), text.clone())
        })
        .collect()
}

/// The project's image built from `sources` as `trust-runtime build` builds it, a debug image
/// unless `[build] debug` says otherwise (specification 44 §13.2), in the project's build folder;
/// or why the launch runs the bytecode engine.
///
/// # Errors
///
/// The error of sources that do not compile, or of a build that fails.
pub(super) fn build_image(
    root: &Path,
    sources: &[(String, String)],
) -> Result<Result<Vec<u8>, EngineReason>, String> {
    let toolchain = match Toolchain::from_env() {
        Ok(toolchain) => toolchain,
        Err(error) => return Ok(Err(EngineReason::NoToolchain(error.to_string()))),
    };
    let target = Target::this_cpu()
        .ok_or_else(|| String::from("this computer's CPU runs no compiled program"))?;
    let hooks = debug_hooks(root).map_err(|error| error.to_string())?;
    let work = root.join("build").join("simulator").join(triple(target));
    match build(
        &labelled(root, sources),
        &Options { target, hooks },
        &toolchain,
        &work,
    ) {
        Ok(built) => Ok(Ok(built.image)),
        Err(error @ NativeError::Unsupported { .. }) => {
            Ok(Err(EngineReason::NotCompiledYet(error.to_string())))
        }
        Err(error) => Err(error.to_string()),
    }
}

/// The compiled program a launch runs: its scheduler, which waits for [`Self::open`], and the
/// control server the adapter and the panels connect to.
pub(super) struct Simulator {
    handle: ResourceHandle<StdClock>,
    gate: Arc<StartGate>,
    _server: DebugControlServer,
    /// The launch's program file and project folder, which a reload builds from again.
    program: String,
    root: PathBuf,
}

impl Simulator {
    /// Starts `image` as trust-runtime starts a compiled program (specification 44 §8.8): the
    /// resource of `runtime.toml` when the project has one, the drivers of `io.toml`, a cold start,
    /// and the control server at `endpoint`.
    ///
    /// # Errors
    ///
    /// The error of an image, a configuration or a driver that does not load, or of a control
    /// server that does not start.
    pub(super) fn start(
        image: Vec<u8>,
        program_file: &str,
        root: &Path,
        sources: Vec<ControlSourceFile>,
        endpoint: ControlEndpoint,
        auth_token: Option<String>,
    ) -> Result<Self, String> {
        let runtime_toml = root.join("runtime.toml");
        let config = runtime_toml
            .is_file()
            .then(|| RuntimeConfig::load(&runtime_toml))
            .transpose()
            .map_err(|error| error.to_string())?;
        let mut program = match &config {
            Some(config) => CompiledRuntime::load_with_statement_budget(
                image,
                config.resource_name.clone(),
                config.max_statements_per_scan,
            ),
            None => CompiledRuntime::load(image, SmolStr::new("RESOURCE")),
        }
        .map_err(|error| error.to_string())?;
        let served = ServedProgram::default();
        program.set_debug_control(served.debug.clone());
        program.set_metrics_sink(served.metrics.clone());
        program.set_io_health_sink(Some(served.io_health.clone()));
        if let Some(config) = &config {
            program.set_watchdog_policy(config.watchdog);
            program.set_fault_policy(config.fault_policy);
        }
        let io_toml = root.join("io.toml");
        if io_toml.is_file() {
            let io = IoConfig::load(&io_toml).map_err(|error| error.to_string())?;
            program
                .add_io_config(&io)
                .map_err(|error| format!("io.toml: {error}"))?;
        }
        program
            .restart(RestartMode::Cold)
            .map_err(|error| error.to_string())?;
        let cycle_interval = config
            .as_ref()
            .map_or(Duration::from_millis(10), |config| config.cycle_interval);
        let gate = Arc::new(StartGate::new());
        let handle = ResourceRunner::new(program, StdClock::new(), cycle_interval)
            .with_start_gate(gate.clone())
            .spawn("trust-simulator")
            .map_err(|error| error.to_string())?;
        let server = DebugControlServer::start_compiled(
            served,
            handle.control(),
            sources,
            endpoint,
            auth_token,
            Some(root.to_path_buf()),
        )
        .map_err(|error| error.to_string())?;
        Ok(Self {
            handle,
            gate,
            _server: server,
            program: program_file.to_string(),
            root: root.to_path_buf(),
        })
    }

    /// Lets the scheduler start its first scan.
    pub(super) fn open(&self) {
        self.gate.open();
    }

    /// Stops the program and its scheduler and closes its control server.
    pub(super) fn stop(mut self) {
        self.handle.stop();
        self.gate.open();
        let _ = self.handle.join();
    }
}

/// The project folder of a launch: its `runtimeRoot` (or `cwd`), else the folder of its program.
fn launch_root(args: &LaunchArguments, program: &str) -> PathBuf {
    launch_runtime_root(args)
        .map(PathBuf::from)
        .or_else(|| Path::new(program).parent().map(Path::to_path_buf))
        .unwrap_or_default()
}

/// The endpoint a client connects to for the control server at `endpoint`.
fn remote_endpoint(endpoint: &ControlEndpoint) -> RemoteEndpoint {
    match endpoint {
        ControlEndpoint::Tcp(address) => RemoteEndpoint::Tcp(*address),
        #[cfg(unix)]
        ControlEndpoint::Unix(path) => RemoteEndpoint::Unix(path.clone()),
    }
}

impl DebugAdapter {
    /// The launch of `program` compiled for this computer (specification 44 NX-16): the image is
    /// built and started with its control server, the adapter connects to it as an attach does,
    /// the breakpoints set while the launch was pending reach it, `stopOnEntry` pauses it, and its
    /// scheduler then starts. `Err` gives why the program runs on the bytecode engine instead, with
    /// the events so far.
    pub(super) fn launch_compiled(
        &mut self,
        request: &Request<Value>,
        args: &LaunchArguments,
        program: &str,
        stop_on_entry: bool,
        mut events: Vec<Value>,
    ) -> Result<DispatchOutcome, (EngineReason, Vec<Value>)> {
        let root = launch_root(args, program);
        let sources = match self.session.launch_sources(program) {
            Ok(sources) => sources,
            Err(error) => return Ok(self.launch_failed(request, events, &error.to_string())),
        };
        let image = match build_image(&root, &sources) {
            Ok(Ok(image)) => image,
            Ok(Err(reason)) => return Err((reason, events)),
            Err(error) => {
                return Ok(self.launch_failed(request, events, &format!("the build: {error}")));
            }
        };
        let endpoint = match launch_control_endpoint(args) {
            Some(text) => match ControlEndpoint::parse(&text) {
                Ok(endpoint) => endpoint,
                Err(error) => {
                    let message = format!("control endpoint error: {error}");
                    return Ok(self.launch_failed(request, events, &message));
                }
            },
            None => default_control_endpoint(),
        };
        let token = launch_control_auth_token(args);
        let simulator = match Simulator::start(
            image,
            program,
            &root,
            self.session.control_sources(),
            endpoint.clone(),
            token.clone(),
        ) {
            Ok(simulator) => simulator,
            Err(error) => return Ok(self.launch_failed(request, events, &error)),
        };
        let remote = match RemoteSession::connect(remote_endpoint(&endpoint), token) {
            Ok(remote) => remote,
            Err(error) => {
                simulator.stop();
                let message = format!("connecting to the simulator: {error}");
                return Ok(self.launch_failed(request, events, &message));
            }
        };
        self.remote_session = Some(remote);
        self.simulator = Some(simulator);
        events.push(self.debug_output_message(
            "[trust-debug] launch: the program runs compiled for this computer",
        ));
        events.extend(self.apply_attach_breakpoints());
        if stop_on_entry {
            self.pause_expected.store(true, Ordering::SeqCst);
            if let Some(remote) = self.remote_session.as_mut() {
                let _ = remote.pause();
            }
        }
        if let Some(simulator) = &self.simulator {
            simulator.open();
        }
        self.start_remote_polling();
        Ok(DispatchOutcome {
            responses: vec![self.ok_response::<Value>(request, None)],
            events,
            ..DispatchOutcome::default()
        })
    }

    /// The failed answer of a launch, which keeps none of the breakpoints that waited for it.
    fn launch_failed(
        &mut self,
        request: &Request<Value>,
        events: Vec<Value>,
        message: &str,
    ) -> DispatchOutcome {
        self.attach_breakpoints.clear();
        DispatchOutcome {
            responses: vec![self.error_response(request, message)],
            events,
            ..DispatchOutcome::default()
        }
    }

    /// `text` in the debug console.
    pub(super) fn console_message(&self, text: String) -> Value {
        self.event(
            "output",
            Some(OutputEventBody {
                output: format!("{text}\n"),
                category: Some("console".to_string()),
                source: None,
                line: None,
                column: None,
            }),
        )
    }

    /// `stReload` of a compiled launch (specification 44 NX-16): the project is built again from
    /// the sources the reload names and the runtime changes its running program online with the
    /// new image (§12), keeping a pause; a build or a change that fails leaves the running program
    /// as it was.
    ///
    /// # Errors
    ///
    /// Why the program was not changed.
    pub(super) fn reload_compiled(&mut self, args: ReloadArguments) -> Result<(), String> {
        self.session.update_source_options(SourceOptionsUpdate {
            root: args.runtime_root,
            include_globs: args.runtime_include_globs,
            exclude_globs: args.runtime_exclude_globs,
            ignore_pragmas: args.runtime_ignore_pragmas,
        });
        let (program, root) = match &self.simulator {
            Some(simulator) => (
                args.program.unwrap_or_else(|| simulator.program.clone()),
                simulator.root.clone(),
            ),
            None => return Err(String::from("no compiled program runs")),
        };
        let sources = self
            .session
            .launch_sources(&program)
            .map_err(|error| error.to_string())?;
        let image = match build_image(&root, &sources)? {
            Ok(image) => image,
            Err(reason) => return Err(reason.message()),
        };
        let remote = self
            .remote_session
            .as_mut()
            .ok_or_else(|| String::from("not connected to the simulator"))?;
        remote.reload_program(&image)?;
        self.watch_cache.clear();
        self.variable_handles.clear();
        Ok(())
    }

    /// Stops a compiled launch's program and closes its control server.
    pub(super) fn stop_simulator(&mut self) {
        if let Some(simulator) = self.simulator.take() {
            simulator.stop();
        }
    }
}
