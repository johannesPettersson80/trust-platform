use super::{
    contracts::Request,
    program::IsolatedProgram,
    record_period::{period_count, period_time_seconds, RecordPeriod},
    snapshot::{digest, Snapshot},
};
use crate::harness::SourceFile;
use crate::value::{Duration, Value};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::PathBuf;
/// What the worker builds the run's program with (specification 44 §4.6): the toolchain the
/// runtime builds with, and the runtime's temporary folder, which it builds in. The worker's
/// environment is cleared (annex E8.1), so the runtime names both; without them the worker uses
/// the toolchain the tools find and its own temporary folder.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BuildSetting {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub toolchain: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temporary_folder: Option<PathBuf>,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TrialInput {
    pub snapshot: Snapshot,
    pub request: Request,
    pub index: usize,
    pub parameters: BTreeMap<String, f64>,
    #[serde(default)]
    pub build: BuildSetting,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RunInput {
    pub snapshot: Snapshot,
    pub run: super::run_contracts::RunRequest,
    #[serde(default)]
    pub build: BuildSetting,
}
#[derive(Serialize)]
#[serde(untagged)]
pub(super) enum WorkerInput {
    Search(TrialInput),
    Run(RunInput),
}
impl WorkerInput {
    /// The plan of the run and what it builds its program with.
    fn plan(self) -> Result<(super::execution_plan::ExecutionPlan, BuildSetting), String> {
        match self {
            Self::Search(input) => super::execution_plan::ExecutionPlan::search(
                input.snapshot,
                &input.request,
                input.index,
                input.parameters,
            )
            .map(|plan| (plan, input.build)),
            Self::Run(input) => {
                super::execution_plan::ExecutionPlan::run(input.snapshot, &input.run)
                    .map(|plan| (plan, input.build))
            }
        }
    }
}
pub(super) const PREFIX: &str = "TRUST_SEARCH_RESULT ";
/// Most bytes one worker may write for one run or trial (specification 35, A7), derived from the
/// largest valid run at the sample and channel limits, plus a 1 MiB reserve. Its sizes are those of
/// `worker_checkpoint.rs`; S = 100,000 samples and L = 1,600,000 channel samples.
///
/// - A streamed run writes its checkpoint, `MAX_CHECKPOINT` = 56,324,288 bytes, then a final result
///   that repeats its header: S × 25 + 512 KiB = 3,024,288 bytes. Together, 59,348,576 bytes.
/// - A search trial writes one result holding its complete run: S × (83 + 25) + L × 27 + 512 KiB =
///   54,524,288 bytes.
///
/// The larger run is 59,348,576 bytes; with the reserve the cap is 60,397,152 bytes (57.6 MiB), so
/// a valid single run or trial always fits. A search job's retained total is also bounded by this
/// value (`service.rs`). A job whose trials together exceed it fails explicitly, without truncation.
pub(super) const MAX_OUTPUT: u64 = {
    let run = super::worker_checkpoint::MAX_CHECKPOINT as u64
        + super::execution_settings::MAX_RUN_SAMPLES * super::worker_checkpoint::TIME_BYTES
        + super::worker_checkpoint::HEADER_BYTES;
    let trial = super::execution_settings::MAX_RUN_SAMPLES
        * (super::worker_checkpoint::SAMPLE_BYTES + super::worker_checkpoint::TIME_BYTES)
        + super::execution_settings::MAX_CHANNEL_SAMPLES * super::worker_checkpoint::VALUE_BYTES
        + super::worker_checkpoint::HEADER_BYTES;
    (if run > trial { run } else { trial }) + super::worker_checkpoint::RESERVE_BYTES
};
pub fn stdio() -> Result<(), String> {
    let memory_limit = super::limits::apply()?;
    let mut text = String::new();
    std::io::stdin()
        .take(8 * 1024 * 1024 + 1)
        .read_to_string(&mut text)
        .map_err(|e| e.to_string())?;
    if text.len() > 8 * 1024 * 1024 {
        return Err("worker snapshot exceeds 8MiB wire limit".into());
    }
    let envelope: JsonValue = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let input = if envelope.get("run").is_some() {
        if envelope.get("request").is_some() {
            return Err("worker purpose is ambiguous".into());
        }
        WorkerInput::Run(serde_json::from_value(envelope).map_err(|e| e.to_string())?)
    } else {
        WorkerInput::Search(serde_json::from_value(envelope).map_err(|e| e.to_string())?)
    };
    let mut output = match input
        .plan()
        .and_then(|(plan, build)| execute(&plan, &build))
    {
        Ok(value) => value,
        Err(reason) => json!({"status":"failed","reason":reason}),
    };
    output["worker_memory_limit_bytes"] = json!(memory_limit);
    output["runtime_version"] = json!(env!("CARGO_PKG_VERSION"));
    println!(
        "{PREFIX}{}",
        serde_json::to_string(&output).map_err(|e| e.to_string())?
    );
    Ok(())
}

/// Keeps the run's parameter values through a scan (protocol annex E8.3). A program may set its own parameter record during
/// its first execution, its startup, which is the first scan in which its task runs: the run's values are then written again
/// at the task boundary after that scan, as Apply writes a complete record at a task boundary of a running program, and they
/// must hold from that boundary to the end of the run. A program that changes one after its first execution would not keep
/// an applied candidate either, so the run fails. `started` holds the programs that have executed in an earlier scan.
fn hold_candidate(
    program: &mut IsolatedProgram,
    parameters: &BTreeMap<String, f64>,
    started: &mut BTreeSet<String>,
) -> Result<(), String> {
    let starting: BTreeSet<String> = program
        .programs()
        .iter()
        .filter(|name| {
            !started.contains(*name) && program.runtime().program_ran_in_last_cycle(name)
        })
        .cloned()
        .collect();
    for (symbol, value) in parameters {
        if program.read(symbol)? == *value {
            continue;
        }
        // A parameter outside every program may be set by the startup of whichever program first executes.
        let in_startup = match program.owning_program(symbol) {
            Some(owner) => starting.contains(owner),
            None => started.is_empty() && !starting.is_empty(),
        };
        if !in_startup {
            return Err(format!(
                "ST changed controller parameter {symbol}; candidate not adopted"
            ));
        }
        program.assign(symbol, *value)?;
        if program.read(symbol)? != *value {
            return Err("candidate must use exact runtime floating-point representation".into());
        }
    }
    started.extend(starting);
    Ok(())
}

/// The elementary IEC type of a parameter's value, which a run's declared `iec_type` must name.
fn parameter_type(value: &Value) -> Option<&'static str> {
    match value {
        Value::Bool(_) => Some("BOOL"),
        Value::Real(_) => Some("REAL"),
        Value::LReal(_) => Some("LREAL"),
        _ => None,
    }
}

/// Checks that each parameter of `input` names a writable, unaddressed REAL or LREAL member, or a
/// fixed configuration BOOL, of the type its declaration gives (protocol annex E8.4, E8.5), in the
/// run's program.
fn check_parameters(
    input: &super::execution_plan::ExecutionPlan,
    program: &mut IsolatedProgram,
) -> Result<(), String> {
    for symbol in input.parameters.keys() {
        if !program.parameter_writable(symbol) {
            return Err(format!("{symbol} is not a writable unaddressed parameter"));
        }
        let value = program.value(symbol)?;
        let fixed_bool = input.evaluation.is_none()
            && input
                .parameter_types
                .get(symbol)
                .is_some_and(|kind| kind == "BOOL")
            && matches!(value, Value::Bool(_));
        if !matches!(value, Value::Real(_) | Value::LReal(_)) && !fixed_bool {
            return Err("parameters require REAL/LREAL values".into());
        }
        if input
            .parameter_types
            .get(symbol)
            .is_some_and(|expected| parameter_type(&value) != Some(expected.as_str()))
        {
            return Err("parameter IEC type does not match actual ST".into());
        }
        if !program.bound_addresses(symbol).is_empty() {
            return Err("parameters cannot target addressed storage".into());
        }
    }
    Ok(())
}

fn execute(
    input: &super::execution_plan::ExecutionPlan,
    build: &BuildSetting,
) -> Result<JsonValue, String> {
    if input.snapshot.sources.is_empty()
        || input.snapshot.sources.len() > 64
        || input
            .snapshot
            .sources
            .iter()
            .map(|s| s.text.len())
            .sum::<usize>()
            > 1_048_576
        || input.snapshot.simulation.len() > 262_144
        || input.simulation.len() > 262_144
    {
        return Err("worker source bounds exceeded".into());
    }
    if input.snapshot.project_revision
        != digest((
            &input.snapshot.project_identity,
            &input.snapshot.sources,
            &input.snapshot.simulation,
        ))
    {
        return Err("worker snapshot revision mismatch".into());
    }
    // A snapshot without live inputs (an older caller's, or a fixture's) needs no revision.
    let carries_inputs = !input.snapshot.process_inputs.is_empty()
        || !input.snapshot.process_inputs_revision.is_empty();
    if carries_inputs
        && input.snapshot.process_inputs_revision != digest(&input.snapshot.process_inputs)
    {
        return Err("worker live input revision mismatch".into());
    }
    let sources: Vec<SourceFile> = input
        .snapshot
        .sources
        .iter()
        .map(|s| SourceFile::with_path(&s.path, &s.text))
        .collect();
    // Compiling installs the admitted executable before any PLC scan deadline is armed. The
    // parent still bounds all preparation by the job budget.
    let mut program =
        IsolatedProgram::compile(&sources, input.snapshot.conversion_rules()?, build)?;
    // The first scan sees every bound input as the live plant had it; the run's plant then
    // writes only its own feedback targets.
    super::process_inputs::apply(program.runtime_mut(), &input.snapshot.process_inputs)?;
    let mut config = crate::simulation::SimulationConfig::from_toml_text(
        &input.simulation,
        "isolated execution simulation",
    )
    .map_err(|e| e.to_string())?;
    if !config.enabled || config.plant_system.plants.is_empty() {
        return Err("enabled simulated plant required".into());
    }
    config.seed = input.settings.seed;
    let mut simulator = crate::simulation::SimulationController::new(config);
    check_parameters(input, &mut program)?;
    for (symbol, value) in &input.settings.initial_values {
        program.assign(symbol, *value)?;
    }
    for (symbol, value) in &input.parameters {
        program.assign(symbol, *value)?;
        if program.read(symbol)? != *value {
            return Err("candidate must use exact runtime floating-point representation".into());
        }
    }
    let n = input.settings.duration_ms / input.settings.step_ms;
    let record_every = input.settings.record_every;
    let sample_times: Vec<f64> = (0..period_count(n + 1, record_every))
        .map(|period| period_time_seconds(period, record_every, n, input.settings.step_ms))
        .collect();
    let mut conditions = json!({"initialState":input.settings.initial_values,"events":input.settings.events.iter().map(|event|json!({"atSeconds":event.at_ms as f64/1000.0,"channel":event.symbol,"value":event.value})).collect::<Vec<_>>(),"durationSeconds":input.settings.duration_ms as f64/1000.0,"sampleTimesSeconds":sample_times,"seed":input.settings.seed,"modelRevision":input.model_revision,"programRevision":digest(&input.snapshot.sources),"parameterSchemaRevision":input.parameter_schema_revision});
    if record_every > 1 {
        conditions["recordEvery"] = json!(record_every);
    }
    if let Some(model_provenance) = &input.model_provenance {
        conditions["modelProvenance"] = json!(model_provenance);
        conditions["simulationConfigRevision"] = json!(input.simulation_config_revision);
    }
    if let Some(relay) = &input.feedback_relay {
        conditions["feedbackRelay"] = json!(relay);
    }
    let mut relay_high = input.feedback_relay.as_ref().is_some_and(|relay| {
        if relay.restore == relay.high {
            false
        } else if relay.restore == relay.low {
            true
        } else {
            relay.positive_gain
        }
    });
    let mut relay_last_switch_ms = input
        .feedback_relay
        .as_ref()
        .map_or(0, |relay| relay.start_ms);
    // A streamed run keeps its samples only in its checkpoint frames; search trials keep them here.
    let mut samples = Vec::with_capacity(if input.evaluation.is_some() {
        period_count(n + 1, record_every) as usize
    } else {
        0
    });
    let mut sample_count = 0_usize;
    let mut period = RecordPeriod::default();
    let mut violations = Vec::new();
    let mut score = 0.0;
    let mut previous_error = None;
    let recorded_run = |samples: &[JsonValue]| json!({"id":format!("{}-{}",if input.evaluation.is_some(){"trial"}else{"run"},input.index),"evidence":"ST-executed","parameters":input.parameters.clone(),"conditions":conditions.clone(),"channels":input.settings.observations.iter().map(|channel|json!({"id":channel.symbol,"unit":channel.unit})).collect::<Vec<_>>(),"samples":samples});
    let mut checkpoint = if input.evaluation.is_none() {
        Some(super::worker_checkpoint::Writer::new(&recorded_run(&[]))?)
    } else {
        None
    };
    // The programs that have executed in an earlier scan of the run: a program's writes during its first execution are its
    // startup (E8.3).
    let mut started_programs: BTreeSet<String> = BTreeSet::new();
    let executed = (|| -> Result<(), String> {
        for sequence in 0..=n {
            let ms = sequence * input.settings.step_ms;
            let now = Duration::from_millis(ms as i64);
            program.runtime_mut().set_current_time(now);
            simulator
                .apply_pre_cycle(now, program.runtime_mut())
                .map_err(|e| e.to_string())?;
            for event in input
                .settings
                .events
                .iter()
                .filter(|event| event.at_ms == ms)
            {
                program.assign(&event.symbol, event.value)?;
            }
            if let Some(relay) = &input.feedback_relay {
                let measured = program.read(&relay.process_variable)?;
                let command =
                    relay.command_at(ms, measured, &mut relay_high, &mut relay_last_switch_ms)?;
                program.assign(&relay.command, command)?;
            }
            program.runtime_mut().set_execution_deadline(Some(
                std::time::Instant::now()
                    + std::time::Duration::from_millis(input.settings.per_trial_wall_ms.min(100)),
            ));
            program
                .runtime_mut()
                .execute_cycle()
                .map_err(|e| e.to_string())?;
            hold_candidate(&mut program, &input.parameters, &mut started_programs)?;
            let mut readings = Vec::with_capacity(input.settings.observations.len());
            for channel in &input.settings.observations {
                readings.push(program.read(&channel.symbol)?);
            }
            if let Some(evaluation) = &input.evaluation {
                let error = program.read(&evaluation.objective.reference_symbol)?
                    - program.read(&evaluation.objective.feedback_symbol)?;
                if let Some(previous) = previous_error {
                    score +=
                        (previous * previous + error * error) * 0.5 * input.settings.step_ms as f64
                            / 1000.0;
                }
                previous_error = Some(error);
                for bound in &evaluation.requirements {
                    if ms >= bound.start_ms && ms <= bound.end_ms {
                        let value = program.read(&bound.symbol)?;
                        if (value < bound.minimum || value > bound.maximum)
                            && !violations.contains(&bound.id)
                        {
                            violations.push(bound.id.clone());
                        }
                    }
                }
            }
            period.push(&readings);
            // A sample closes after its last grid point; the run's last point closes a partial period.
            if (sequence + 1) % record_every == 0 || sequence == n {
                let window = sequence / record_every;
                // A mean that overflows to a non-finite number has no value: it is null and bad.
                let measured: Vec<(Option<f64>, u64)> = period
                    .means()
                    .into_iter()
                    .map(|mean| {
                        if mean.is_finite() {
                            (Some(mean), super::worker_checkpoint::QUALITY_GOOD)
                        } else {
                            (None, super::worker_checkpoint::QUALITY_BAD)
                        }
                    })
                    .collect();
                let sample = super::worker_checkpoint::sample_row(
                    window,
                    period_time_seconds(window, record_every, n, input.settings.step_ms),
                    &measured,
                );
                match &mut checkpoint {
                    Some(checkpoint) => checkpoint.sample(&sample)?,
                    None => samples.push(sample),
                }
                sample_count += 1;
                period = RecordPeriod::default();
            }
            simulator
                .apply_post_cycle(now, program.runtime())
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    })();
    if let Err(reason) = executed {
        if input.evaluation.is_some() || sample_count == 0 {
            return Err(reason);
        }
        // The streamed samples are already in the checkpoint frames; the supervisor assembles them.
        return Ok(
            json!({"status":"failed","reason":reason,"io_provenance":"isolated-simulation-no-physical-drivers","run":recorded_run(&[])}),
        );
    }
    if !score.is_finite() {
        return Err("nonfinite objective".into());
    }
    // A streamed run's final result carries its header alone; the frames hold its samples.
    let run = if input.evaluation.is_some() {
        recorded_run(&samples)
    } else {
        recorded_run(&[])
    };
    Ok(if let Some(evaluation) = &input.evaluation {
        json!({"index":input.index,"parameters":input.parameters,"status":if violations.is_empty(){"qualified"}else{"rejected"},"violations":violations,"score":score,"score_unit":format!("{}^2 s",evaluation.objective.error_unit),"integration":"trapezoidal","io_provenance":"isolated-simulation-no-physical-drivers","run":run})
    } else {
        json!({"status":"executed","io_provenance":"isolated-simulation-no-physical-drivers","run":run})
    })
}
