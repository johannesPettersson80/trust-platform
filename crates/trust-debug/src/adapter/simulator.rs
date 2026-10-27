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
use trust_runtime::control::ControlEndpoint;
use trust_runtime::harness::SourceFile;
use trust_runtime::native::{
    build_with_conversions, debug_hooks, triple, Options, Target, Toolchain,
};
use trust_runtime::scheduler::{
    ResourceHandle, ResourceRunner, SimulationSpeed, StartGate, StdClock,
};
use trust_runtime::simulation::SimulationController;
use trust_runtime::stdlib::conversions::ConversionProfile;
use trust_runtime::value::Duration;
use trust_runtime::{CompiledRuntime, RestartMode};

use crate::protocol::{LaunchArguments, ReloadArguments, Request};
use crate::runtime::DebugRuntime;
use crate::session::{
    attaches_physical_io, load_project_simulation, SimulationFacts, SourceOptionsUpdate,
};

use super::control_bridge::{
    default_control_endpoint, DebugControlServer, ServedProgram, SimulatorRun,
};
use super::launch::{launch_control_auth_token, launch_control_endpoint, launch_runtime_root};
use super::remote::{RemoteEndpoint, RemoteSession};
use super::{DebugAdapter, DispatchOutcome};

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
/// unless `[build] debug` says otherwise (specification 44 §13.2), with the conversion rules
/// `conversions` of the project's `vendor_profile`, which the session resolved with the sources
/// (docs/specs/07-standard-functions.md 2.10), in the project's build folder, with the toolchain
/// of §4.6.
///
/// # Errors
///
/// No toolchain, sources that do not compile, or a build that fails.
pub(super) fn build_image(
    root: &Path,
    sources: &[(String, String)],
    conversions: ConversionProfile,
) -> Result<Vec<u8>, String> {
    let toolchain = Toolchain::find().map_err(|error| error.to_string())?;
    let target = Target::this_cpu()
        .ok_or_else(|| String::from("this computer's CPU runs no compiled program"))?;
    let hooks = debug_hooks(root).map_err(|error| error.to_string())?;
    let work = root.join("build").join("simulator").join(triple(target));
    build_with_conversions(
        &labelled(root, sources),
        &[],
        &Options { target, hooks },
        conversions,
        &toolchain,
        &work,
    )
    .map(|built| built.image)
    .map_err(|error| error.to_string())
}

/// The compiled program a launch runs: its scheduler, which waits for [`Self::open`], and the
/// control server the adapter and the panels connect to.
pub(super) struct Simulator {
    handle: ResourceHandle<StdClock>,
    gate: Arc<StartGate>,
    server: DebugControlServer,
    /// The launch's program file and project folder, which a reload builds from again.
    program: String,
    root: PathBuf,
}

impl Simulator {
    /// Starts `image` as trust-runtime starts a compiled program (specification 44 §8.8): the
    /// resource and its cycle of `runtime.toml` when the project has one, else the 10 ms cycle
    /// (specification 13), the drivers of `io.toml`, the simulation of `simulation.toml`
    /// (specification 46), a cold start, and the control server at `endpoint`, which reports the
    /// tasks and serves the sources of `session` and whose `restart` requests the scheduler takes.
    ///
    /// # Errors
    ///
    /// The error of an image, a configuration or a driver that does not load, or of a control
    /// server that does not start.
    pub(super) fn start(
        image: Vec<u8>,
        program_file: &str,
        root: &Path,
        session: &dyn DebugRuntime,
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
        let mut physical_io = false;
        if io_toml.is_file() {
            let io = IoConfig::load(&io_toml).map_err(|error| error.to_string())?;
            physical_io = attaches_physical_io(&io);
            program
                .add_io_config(&io)
                .map_err(|error| format!("io.toml: {error}"))?;
        }
        // The project's `simulation.toml`: its speed, and its plants when they are enabled, which
        // the scheduler runs around each scan with the revision of the file, for which
        // `plant.status` answers (specification 46, §6.1 and §13; specification 47). The target is
        // simulated unless a physical driver is attached (specification 35, A2.4).
        let simulation_file = load_project_simulation(root).map_err(|error| error.to_string())?;
        let facts = SimulationFacts::for_io(
            physical_io,
            simulation_file
                .as_ref()
                .map_or(1, |(config, _)| config.time_scale),
        );
        let simulation =
            simulation_file
                .filter(|(config, _)| config.enabled)
                .map(|(config, revision)| {
                    SimulationController::new(config).with_active_configuration_revision(revision)
                });
        // The runtime reports the simulated target from its first scan (specification 35, A2.4;
        // specification 46, §6.1).
        program.set_simulation_enabled(facts.enabled);
        program
            .restart(RestartMode::Cold)
            .map_err(|error| error.to_string())?;
        let cycle_interval = config
            .as_ref()
            .map_or(Duration::from_millis(10), |config| config.cycle_interval);
        let run = SimulatorRun {
            cycle_interval,
            watchdog: program.watchdog_policy(),
            fault_policy: program.fault_policy(),
            simulation: facts,
        };
        let gate = Arc::new(StartGate::new());
        // The scheduler takes the `restart` requests the control server queues (specification 11
        // §6.7), as trust-runtime's does. A debug session publishes no runtime mesh catalogue, so
        // its runner answers a `MeshCatalogue` request with an explicit unavailable result
        // (specification 43, "Debug-session boundary").
        let mut runner = ResourceRunner::new(program, StdClock::new(), cycle_interval)
            .with_start_gate(gate.clone())
            .with_restart_signal(served.pending_restart.clone())
            .refusing_mesh_catalogue();
        if facts.enabled {
            // A simulated target runs on simulation time at the project's speed, which `config.set`
            // changes, and reports the speed it achieves (specification 46, §6.1 items 1 to 6).
            runner = runner.with_simulation_speed(
                SimulationSpeed::new(facts.time_scale),
                Some(served.metrics.clone()),
            );
        }
        if let Some(simulation) = simulation {
            runner = runner.with_simulation(simulation);
        }
        let handle = runner
            .spawn("trust-simulator")
            .map_err(|error| error.to_string())?;
        let server = DebugControlServer::start_compiled(
            served,
            run,
            handle.control(),
            session,
            endpoint,
            auth_token,
            Some(root.to_path_buf()),
        )
        .map_err(|error| error.to_string())?;
        Ok(Self {
            handle,
            gate,
            server,
            program: program_file.to_string(),
            root: root.to_path_buf(),
        })
    }

    /// Lets the scheduler start its first scan.
    pub(super) fn open(&self) {
        self.gate.open();
    }

    /// The control server the adapter and the panels connect to.
    pub(super) fn control_server(&self) -> &DebugControlServer {
        &self.server
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
    /// The folder the file labels of the debugged program resolve against (specification 13, "Stack
    /// Trace and Navigation"): the folder a launch built the program from, else the project root an
    /// attach names.
    pub(super) fn label_root(&self) -> Option<PathBuf> {
        self.simulator
            .as_ref()
            .map(|simulator| simulator.root.clone())
            .or_else(|| self.session.source_root().map(PathBuf::from))
    }

    /// The launch of `program` compiled for this computer (specification 44 NX-16): the image is
    /// built and started with its control server, the adapter connects to it as an attach does,
    /// the breakpoints set while the launch was pending reach it, `stopOnEntry` pauses it, and its
    /// scheduler then starts.
    pub(super) fn launch_compiled(
        &mut self,
        request: &Request<Value>,
        args: &LaunchArguments,
        program: &str,
        stop_on_entry: bool,
        mut events: Vec<Value>,
    ) -> DispatchOutcome {
        let root = launch_root(args, program);
        let sources = match self.session.launch_sources(program) {
            Ok(sources) => sources,
            Err(error) => return self.launch_failed(request, events, &error.to_string()),
        };
        let image = match build_image(&root, &sources, self.session.conversion_profile()) {
            Ok(image) => image,
            Err(error) => {
                return self.launch_failed(request, events, &format!("the build: {error}"));
            }
        };
        let endpoint = match launch_control_endpoint(args) {
            Some(text) => match ControlEndpoint::parse(&text) {
                Ok(endpoint) => endpoint,
                Err(error) => {
                    let message = format!("control endpoint error: {error}");
                    return self.launch_failed(request, events, &message);
                }
            },
            None => default_control_endpoint(),
        };
        let token = launch_control_auth_token(args);
        let simulator = match Simulator::start(
            image,
            program,
            &root,
            self.session.as_ref(),
            endpoint.clone(),
            token.clone(),
        ) {
            Ok(simulator) => simulator,
            Err(error) => return self.launch_failed(request, events, &error),
        };
        let remote = match RemoteSession::connect(remote_endpoint(&endpoint), token) {
            Ok(remote) => remote,
            Err(error) => {
                simulator.stop();
                let message = format!("connecting to the simulator: {error}");
                return self.launch_failed(request, events, &message);
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
        DispatchOutcome {
            responses: vec![self.ok_response::<Value>(request, None)],
            events,
            ..DispatchOutcome::default()
        }
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
        // The simulator reads the project's speed again at a reload (specification 46, §6.1 item
        // 5); a `simulation.toml` that does not read leaves the running program as it was.
        let time_scale = load_project_simulation(&root)
            .map_err(|error| error.to_string())?
            .map_or(1, |(config, _)| config.time_scale);
        let image = build_image(&root, &sources, self.session.conversion_profile())?;
        let remote = self
            .remote_session
            .as_mut()
            .ok_or_else(|| String::from("not connected to the simulator"))?;
        remote.reload_program(&image)?;
        // The control server reports the program that now runs (specification 35, A2.3).
        self.sync_control_program(time_scale);
        Ok(())
    }

    /// Stops a compiled launch's program and closes its control server.
    pub(super) fn stop_simulator(&mut self) {
        if let Some(simulator) = self.simulator.take() {
            simulator.stop();
        }
    }
}
