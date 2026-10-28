//! The method `controlStudio/monitor` (specification 35, §0.1a; B2, A3.4 and B9.2): it answers the sample the page shows and,
//! when it changed, the target's label. Every rule is trust-control's (`loop_monitor` and `target_label`). This module keeps the
//! observation of the panel's loop and runs the runtime reads.
//!
//! A call never waits for the runtime. It answers at once with the last sample its observation holds, or with a pending sample
//! before the first read has come back. When no read is in flight, the call starts one on a background thread. The read sends the
//! runtime `status` and `control.signals.inspect` (E1.3, E3), and the scan it gets is the sample a later call answers. At most one
//! read is in flight per session, so a dead or slow endpoint holds up only its own observation. A restart (a new debug session,
//! or no runtime) drops the read in flight: its scan never reaches the new observation.

use std::path::Path;
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{SystemTime, UNIX_EPOCH};

use parking_lot::Mutex;
use serde_json::Value;
use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::limits::connection_loss_timeout_ms;
use trust_control::loop_definition::LoopDefinition;
use trust_control::loop_monitor::{
    coherent_readings, pending_sample, unavailable_sample, LoopMonitor, LoopMonitorSample,
    MonitorSource, NOT_PRODUCING_SCANS,
};
use trust_control::messages::{MonitorRequest, MonitorResult};
use trust_control::recording::EvidenceSource;
use trust_control::runtime::{RuntimeConnection, RuntimeStatus};
use trust_control::signals::{
    loop_inspection_bindings, loop_inspection_mode_id, Inspection, InspectionRequest,
};
use trust_control::target_label::target_label;

use crate::commands::{refuse_other_runtime, to_value};
use crate::loops;
use crate::runtime_client::RuntimeClient;

/// The clock the scans are stamped with (A3.4). A read stamps its scan when the scan arrives, so the clock runs on the read's thread.
type Clock = Arc<dyn Fn() -> u64 + Send + Sync>;

/// The observation of the loop the panel shows (B2): the editor session and the loop it belongs to, the debug session it is
/// attached to, the label the page was last given, the observation the reads share, and the read in flight.
#[derive(Debug, Default)]
pub struct MonitorSession {
    /// The editor session and the loop the observation belongs to; another owner starts a new observation.
    owner: Option<(String, String)>,
    /// The debug session the observation belongs to; a new session starts a new observation.
    runtime_session_id: Option<String>,
    /// The target label the page was last given (B9.2).
    reported_label: Option<String>,
    /// What the reads have found, shared with the read in flight.
    observation: Arc<Mutex<Observation>>,
    /// The read in flight, or the last read that ended.
    read: Option<JoinHandle<()>>,
}

/// What the reads found. A read writes its outcome here when it ends, unless a restart came in after it began.
#[derive(Debug, Default)]
struct Observation {
    /// Counts the restarts of the observation; a read that began under an earlier count is dropped when it ends.
    generation: u64,
    /// The state the next read carries on from.
    state: ReadState,
    /// The sample of the last read that ended in this generation.
    sample: Option<LoopMonitorSample>,
    /// The target's label from the last read's status, when the status was known.
    label: Option<String>,
}

/// The state a read carries from one read to the next (B2, A3.4): the program revision the next scan must have, the endpoint it
/// belongs to and the freshness of the scans. A read works on its own copy and hands the copy back when it ends.
#[derive(Debug, Clone, Default)]
struct ReadState {
    program_revision: Option<String>,
    endpoint: Option<String>,
    monitor: LoopMonitor,
}

/// One read the session starts: the loop, the runtime, and the generation and state the read began with.
struct ReadJob {
    observation: Arc<Mutex<Observation>>,
    generation: u64,
    state: ReadState,
    loop_definition: LoopDefinition,
    connection: RuntimeConnection,
    clock: Clock,
}

/// Answers `controlStudio/monitor` from the command's arguments, stamping the scan on the host's clock (A3.4).
///
/// A request that names another runtime (A2.7), or a loop the project cannot give, is answered as an unavailable sample with its
/// reason: the page shows the loop as unavailable, and the target is unknown, so it is physical (A2.4). Only arguments that are
/// not a monitor request are refused as a failure.
///
/// # Errors
/// The arguments are not a monitor request.
pub fn answer_command(
    slot: &Mutex<Option<MonitorSession>>,
    arguments: &[Value],
) -> Result<Value, Failure> {
    let clock: Clock = Arc::new(wall_clock_ms);
    answer_command_with(slot, arguments, &clock)
}

/// `answer_command` with the clock given: `clock` returns the time a scan arrives, in milliseconds.
///
/// # Errors
/// As `answer_command`.
fn answer_command_with(
    slot: &Mutex<Option<MonitorSession>>,
    arguments: &[Value],
    clock: &Clock,
) -> Result<Value, Failure> {
    let value = arguments.first().cloned().unwrap_or(Value::Null);
    let request: MonitorRequest =
        trust_control::json::decode_value(value.clone()).map_err(|error| {
            Failure::new(
                &format!("The Control Studio command has invalid arguments: {error}."),
                Cause::Implementation,
                NextAction::None,
            )
        })?;
    let loop_definition = refuse_other_runtime(&value).and_then(|()| {
        loops::open(
            Path::new(&request.project_root),
            &request.runtime_id,
            &request.loop_id,
        )
    });
    let result = match loop_definition {
        Ok(loop_definition) => {
            let mut slot = slot.lock();
            let owner = (request.editor_session_id.clone(), request.loop_id.clone());
            let session = slot.get_or_insert_with(MonitorSession::default);
            if session.owner.as_ref() != Some(&owner) {
                *session = MonitorSession {
                    owner: Some(owner),
                    ..MonitorSession::default()
                };
            }
            session.observe(&loop_definition, &request, clock)
        }
        Err(failure) => MonitorResult {
            sample: unavailable_sample(MonitorSource::Physical, &request.loop_id, &failure.message),
            target_label: None,
        },
    };
    to_value(&result)
}

/// Why the runtime gave no sample: the failure's sentence and what went wrong, such as an endpoint that is not a runtime endpoint
/// or a refused connection (B2, A3.4). A detail that already holds the sentence is the whole reason.
fn runtime_reason(failure: &Failure) -> String {
    match failure.detail.as_deref().map(str::trim) {
        Some(detail) if detail.starts_with(failure.message.as_str()) => detail.to_owned(),
        Some(detail) if !detail.is_empty() => format!("{} {detail}", failure.message),
        _ => failure.message.clone(),
    }
}

impl MonitorSession {
    /// Answers the loop's sample at once, and starts the next read when none is in flight (B2). The answer is the sample the last
    /// read gave, or a pending sample before the first one: a call never waits for the runtime.
    fn observe(
        &mut self,
        loop_definition: &LoopDefinition,
        request: &MonitorRequest,
        clock: &Clock,
    ) -> MonitorResult {
        if self.runtime_session_id != request.runtime_session_id {
            self.runtime_session_id
                .clone_from(&request.runtime_session_id);
            self.restart();
        }
        let Some(connection) = request.connection.as_ref() else {
            self.restart();
            return MonitorResult {
                sample: unavailable_sample(
                    MonitorSource::Physical,
                    &loop_definition.id,
                    NOT_PRODUCING_SCANS,
                ),
                target_label: None,
            };
        };
        let answer = self.answer(loop_definition);
        if !self.reading() {
            self.start_read(loop_definition, connection, clock);
        }
        answer
    }

    /// The answer of a call: the sample the last read gave for this observation, or a pending sample before one. The target's
    /// label is given when it differs from the label the page was last given (B9.2).
    fn answer(&mut self, loop_definition: &LoopDefinition) -> MonitorResult {
        let (held, label) = {
            let observation = self.observation.lock();
            (observation.sample.clone(), observation.label.clone())
        };
        let mut sample = held.unwrap_or_else(|| pending_sample(&loop_definition.id));
        if let LoopMonitorSample::Available(available) = &mut sample {
            available
                .runtime_session_id
                .clone_from(&self.runtime_session_id);
        }
        let target_label =
            label.filter(|label| self.reported_label.as_deref() != Some(label.as_str()));
        if target_label.is_some() {
            self.reported_label.clone_from(&target_label);
        }
        MonitorResult {
            sample,
            target_label,
        }
    }

    /// Starts a read on a background thread, from a copy of this observation's state (B2).
    fn start_read(
        &mut self,
        loop_definition: &LoopDefinition,
        connection: &RuntimeConnection,
        clock: &Clock,
    ) {
        let (generation, state) = {
            let observation = self.observation.lock();
            (observation.generation, observation.state.clone())
        };
        let job = ReadJob {
            observation: Arc::clone(&self.observation),
            generation,
            state,
            loop_definition: loop_definition.clone(),
            connection: connection.clone(),
            clock: Arc::clone(clock),
        };
        self.read = Some(thread::spawn(move || run_read(job)));
    }

    /// Whether a read is in flight: the read this session started has not ended yet.
    fn reading(&self) -> bool {
        self.read.as_ref().is_some_and(|read| !read.is_finished())
    }

    /// A new debug session, or no runtime, starts a new observation: the sample and the label it held go, the runtime state is
    /// reset, and a read in flight is dropped when it ends. The label the page was given is kept (B9.2).
    fn restart(&mut self) {
        let mut observation = self.observation.lock();
        observation.generation += 1;
        observation.state.program_revision = None;
        observation.state.endpoint = None;
        observation.state.monitor.reset_runtime_observation();
        observation.sample = None;
        observation.label = None;
    }
}

impl ReadState {
    /// The runtime's status, when it answers, and the loop's sample from one coherent scan (B2). The state is updated in place.
    fn read(
        &mut self,
        loop_definition: &LoopDefinition,
        connection: &RuntimeConnection,
        clock: &(dyn Fn() -> u64 + Send + Sync),
    ) -> (Option<RuntimeStatus>, LoopMonitorSample) {
        if self
            .endpoint
            .as_deref()
            .is_some_and(|endpoint| endpoint != connection.endpoint)
        {
            self.program_revision = None;
        }
        let mut client = match RuntimeClient::connect(connection) {
            Ok(client) => client,
            Err(failure) => {
                let sample = unavailable_sample(
                    MonitorSource::Physical,
                    &loop_definition.id,
                    &runtime_reason(&failure),
                );
                return (None, sample);
            }
        };
        // The endpoint belongs to the observation once a connection to it exists.
        self.endpoint = Some(connection.endpoint.clone());
        // A status that cannot be read leaves the target physical and unlabelled, as the target's safe default (A2.4). The
        // scan is still asked for, so a runtime that answers the scan but not the status is still observed.
        let status = client.status().ok();
        let source = status
            .as_ref()
            .map_or(MonitorSource::Physical, |status| source_of(status.source()));
        let sample = self.scan(&mut client, loop_definition, status.as_ref(), source, clock);
        (status, sample)
    }

    /// One coherent scan of the loop as the sample of this observation (B2, A3.4). A pending scan is answered as pending, and the
    /// program revision it gives is kept for the next read.
    fn scan(
        &mut self,
        client: &mut RuntimeClient,
        loop_definition: &LoopDefinition,
        status: Option<&RuntimeStatus>,
        source: MonitorSource,
        clock: &(dyn Fn() -> u64 + Send + Sync),
    ) -> LoopMonitorSample {
        let bindings = loop_inspection_bindings(loop_definition);
        let mode_id = loop_definition
            .controller
            .mode_signal
            .as_deref()
            .and_then(|symbol| loop_inspection_mode_id(loop_definition, &bindings, symbol))
            .map(str::to_string);
        let request = match InspectionRequest::new(
            &loop_definition.revision,
            self.program_revision.as_deref(),
            bindings,
        ) {
            Ok(request) => request,
            Err(failure) => {
                return unavailable_sample(source, &loop_definition.id, &failure.message)
            }
        };
        match client.inspect(&request) {
            Err(failure) => {
                unavailable_sample(source, &loop_definition.id, &runtime_reason(&failure))
            }
            Ok(Inspection::Pending {
                program_revision,
                reason,
            }) => {
                self.program_revision = Some(program_revision);
                unavailable_sample(source, &loop_definition.id, &reason)
            }
            Ok(Inspection::Available(scan)) => {
                // The scan is stamped when it arrives, on the engine's clock; the runtime's own time is never compared with it.
                let now = clock();
                self.program_revision = Some(scan.program_revision.clone());
                let readings = coherent_readings(loop_definition, &scan, mode_id.as_deref());
                // The stale threshold is the connection-loss timeout of the resource cycle the runtime reports (A3.4).
                let stale_after_ms =
                    connection_loss_timeout_ms(status.map(|status| status.cycle_interval_ms));
                self.monitor
                    .observe(loop_definition, source, Some(readings), now, stale_after_ms)
            }
        }
    }
}

/// Runs one read on the session's thread, and writes its outcome into the observation unless a restart came in after it began.
fn run_read(job: ReadJob) {
    let ReadJob {
        observation,
        generation,
        mut state,
        loop_definition,
        connection,
        clock,
    } = job;
    let (status, sample) = state.read(&loop_definition, &connection, &*clock);
    let label = status.as_ref().map(target_label);
    let mut shared = observation.lock();
    if shared.generation == generation {
        shared.state = state;
        shared.sample = Some(sample);
        shared.label = label;
    }
}

fn source_of(source: EvidenceSource) -> MonitorSource {
    match source {
        EvidenceSource::SimulatedPlant => MonitorSource::Simulation,
        EvidenceSource::PhysicalPlant => MonitorSource::Physical,
    }
}

/// The host's clock in milliseconds since the Unix epoch, the clock the page's samples are stamped with (A3.4).
fn wall_clock_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
        })
}

#[cfg(test)]
#[path = "monitor_action_tests.rs"]
mod tests;
