use super::{
    contracts::Request,
    snapshot::{digest, Snapshot},
};
use crate::{
    value::{Duration, Value},
    Runtime,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};
use std::collections::BTreeMap;
use std::io::Read;
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TrialInput {
    pub snapshot: Snapshot,
    pub request: Request,
    pub index: usize,
    pub parameters: BTreeMap<String, f64>,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RunInput {
    pub snapshot: Snapshot,
    pub run: super::run_contracts::RunRequest,
}
#[derive(Serialize)]
#[serde(untagged)]
pub(super) enum WorkerInput {
    Search(TrialInput),
    Run(RunInput),
}
impl WorkerInput {
    fn plan(self) -> Result<super::execution_plan::ExecutionPlan, String> {
        match self {
            Self::Search(input) => super::execution_plan::ExecutionPlan::search(
                input.snapshot,
                &input.request,
                input.index,
                input.parameters,
            ),
            Self::Run(input) => {
                super::execution_plan::ExecutionPlan::run(input.snapshot, &input.run)
            }
        }
    }
}
pub(super) const PREFIX: &str = "TRUST_SEARCH_RESULT ";
pub(super) const MAX_OUTPUT: u64 = 32 * 1024 * 1024;
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
    let mut output = match input.plan().and_then(|plan| execute(&plan)) {
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
fn numeric(value: Value) -> Result<f64, String> {
    crate::debug::numeric_control_value(&value)
        .ok_or_else(|| "signal is not an exactly representable numeric/BOOL value".into())
}
fn read(runtime: &Runtime, symbol: &str) -> Result<f64, String> {
    let value =
        numeric(crate::debug::resolve_control_parameter_path(runtime.storage(), symbol)?.value)?;
    if !value.is_finite() {
        return Err(format!("nonfinite signal {symbol}"));
    }
    Ok(value)
}
fn assign(runtime: &mut Runtime, symbol: &str, number: f64) -> Result<(), String> {
    let resolved = crate::debug::resolve_control_parameter_path(runtime.storage(), symbol)?;
    let value = match resolved.value {
        Value::LReal(_) => Value::LReal(number),
        Value::Real(_) => Value::Real(number as f32),
        Value::Bool(_) if number == 0.0 || number == 1.0 => Value::Bool(number == 1.0),
        other => {
            if number.fract() != 0.0 || number.abs() > 9_007_199_254_740_991.0 {
                return Err(
                    "integer initial/event value must be exactly integral and representable".into(),
                );
            }
            let kind = match other {
                Value::SInt(_) => trust_hir::TypeId::SINT,
                Value::Int(_) => trust_hir::TypeId::INT,
                Value::DInt(_) => trust_hir::TypeId::DINT,
                Value::LInt(_) => trust_hir::TypeId::LINT,
                Value::USInt(_) => trust_hir::TypeId::USINT,
                Value::UInt(_) => trust_hir::TypeId::UINT,
                Value::UDInt(_) => trust_hir::TypeId::UDINT,
                Value::ULInt(_) => trust_hir::TypeId::ULINT,
                _ => return Err("unsupported initial/event/parameter type".into()),
            };
            crate::harness::coerce_value_to_type(Value::LInt(number as i64), kind)
                .map_err(|error| error.to_string())?
        }
    };
    if !numeric(value.clone())?.is_finite() {
        return Err("value exceeds runtime type range".into());
    }
    let reference = resolved.reference;
    let addresses: Vec<_> = runtime
        .io()
        .bindings()
        .iter()
        .filter(|binding| match &binding.target {
            crate::io::IoTarget::Name(name) => name.eq_ignore_ascii_case(symbol),
            crate::io::IoTarget::Reference(value) => value == &reference,
        })
        .map(|binding| (binding.address.clone(), binding.value_type))
        .collect();
    for (address, kind) in addresses {
        let io_value = match kind {
            Some(kind) => crate::io::coerce_to_io(value.clone(), kind, address.size)
                .map_err(|error| error.to_string())?,
            None => value.clone(),
        };
        runtime
            .io_mut()
            .write(&address, io_value)
            .map_err(|e| e.to_string())?;
    }
    if !runtime.storage_mut().write_by_ref(reference, value) {
        return Err(format!("cannot assign scalar binding {symbol}"));
    }
    Ok(())
}
fn execute(input: &super::execution_plan::ExecutionPlan) -> Result<JsonValue, String> {
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
    let sources = input
        .snapshot
        .sources
        .iter()
        .map(|s| crate::harness::SourceFile::with_path(&s.path, &s.text))
        .collect();
    let mut runtime = crate::harness::CompileSession::from_sources(sources)
        .build_runtime()
        .map_err(|e| e.to_string())?;
    // Compile and validate the executable before arming a PLC scan deadline.
    // Lazy VM loading inside execute_cycle would charge source compilation to
    // the first scan. The parent still bounds all preparation by the job budget.
    let source_texts = input
        .snapshot
        .sources
        .iter()
        .map(|source| source.text.as_str())
        .collect::<Vec<_>>();
    let module = crate::bytecode::build_module_from_runtime_with_sources(&runtime, &source_texts)
        .map_err(|error| error.to_string())?;
    runtime
        .apply_bytecode_module(&module, None)
        .map_err(|error| error.to_string())?;
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
    let metadata = runtime.metadata_snapshot();
    for symbol in input.parameters.keys() {
        if !metadata.control_parameter_writable(symbol) {
            return Err(format!("{symbol} is not a writable unaddressed parameter"));
        }
        let resolved = crate::debug::resolve_control_parameter_path(runtime.storage(), symbol)?;
        let value = resolved.value;
        let fixed_bool = input.evaluation.is_none()
            && input
                .parameter_types
                .get(symbol)
                .is_some_and(|kind| kind == "BOOL")
            && matches!(value, Value::Bool(_));
        if !matches!(value, Value::Real(_) | Value::LReal(_)) && !fixed_bool {
            return Err("parameters require REAL/LREAL values".into());
        }
        if input.parameter_types.get(symbol).is_some_and(|expected| {
            crate::debug::dap::value_type_name(&value).as_ref() != Some(expected)
        }) {
            return Err("parameter IEC type does not match actual ST".into());
        }
        let reference = resolved.reference;
        if runtime
            .io()
            .bindings()
            .iter()
            .any(|binding| match &binding.target {
                crate::io::IoTarget::Name(name) => name.eq_ignore_ascii_case(symbol),
                crate::io::IoTarget::Reference(value) => value == &reference,
            })
        {
            return Err("parameters cannot target addressed storage".into());
        }
    }
    for (symbol, value) in &input.settings.initial_values {
        assign(&mut runtime, symbol, *value)?;
    }
    for (symbol, value) in &input.parameters {
        assign(&mut runtime, symbol, *value)?;
        if read(&runtime, symbol)? != *value {
            return Err("candidate must use exact runtime floating-point representation".into());
        }
    }
    let n = input.settings.duration_ms / input.settings.step_ms;
    let sample_times: Vec<f64> = (0..=n)
        .map(|i| (i * input.settings.step_ms) as f64 / 1000.0)
        .collect();
    let mut conditions = json!({"initialState":input.settings.initial_values,"events":input.settings.events.iter().map(|event|json!({"atSeconds":event.at_ms as f64/1000.0,"channel":event.symbol,"value":event.value})).collect::<Vec<_>>(),"durationSeconds":input.settings.duration_ms as f64/1000.0,"sampleTimesSeconds":sample_times,"seed":input.settings.seed,"modelRevision":input.model_revision,"programRevision":digest(&input.snapshot.sources),"parameterSchemaRevision":input.parameter_schema_revision});
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
    let mut samples = Vec::with_capacity((n + 1) as usize);
    let mut violations = Vec::new();
    let mut score = 0.0;
    let mut previous_error = None;
    let recorded_run = |samples: &[JsonValue]| json!({"id":format!("{}-{}",if input.evaluation.is_some(){"trial"}else{"run"},input.index),"evidence":"ST-executed","parameters":input.parameters.clone(),"conditions":conditions.clone(),"channels":input.settings.observations.iter().map(|channel|json!({"id":channel.symbol,"unit":channel.unit})).collect::<Vec<_>>(),"samples":samples});
    let mut checkpoint = if input.evaluation.is_none() {
        Some(super::worker_checkpoint::Writer::new(&recorded_run(&[]))?)
    } else {
        None
    };
    let executed = (|| -> Result<(), String> {
        for sequence in 0..=n {
            let ms = sequence * input.settings.step_ms;
            let now = Duration::from_millis(ms as i64);
            runtime.set_current_time(now);
            simulator
                .apply_pre_cycle(now, &mut runtime)
                .map_err(|e| e.to_string())?;
            for event in input
                .settings
                .events
                .iter()
                .filter(|event| event.at_ms == ms)
            {
                assign(&mut runtime, &event.symbol, event.value)?;
            }
            if let Some(relay) = &input.feedback_relay {
                let measured = read(&runtime, &relay.process_variable)?;
                let command =
                    relay.command_at(ms, measured, &mut relay_high, &mut relay_last_switch_ms)?;
                assign(&mut runtime, &relay.command, command)?;
            }
            runtime.set_execution_deadline(Some(
                std::time::Instant::now()
                    + std::time::Duration::from_millis(input.settings.per_trial_wall_ms.min(100)),
            ));
            runtime.execute_cycle().map_err(|e| e.to_string())?;
            for (symbol, value) in &input.parameters {
                if read(&runtime, symbol)? != *value {
                    return Err(format!(
                        "ST changed controller parameter {symbol}; candidate not adopted"
                    ));
                }
            }
            let mut values = serde_json::Map::new();
            for channel in &input.settings.observations {
                values.insert(
                    channel.symbol.clone(),
                    json!({"value":read(&runtime,&channel.symbol)?,"quality":"good"}),
                );
            }
            if let Some(evaluation) = &input.evaluation {
                let error = read(&runtime, &evaluation.objective.reference_symbol)?
                    - read(&runtime, &evaluation.objective.feedback_symbol)?;
                if let Some(previous) = previous_error {
                    score +=
                        (previous * previous + error * error) * 0.5 * input.settings.step_ms as f64
                            / 1000.0;
                }
                previous_error = Some(error);
                for bound in &evaluation.requirements {
                    if ms >= bound.start_ms && ms <= bound.end_ms {
                        let value = read(&runtime, &bound.symbol)?;
                        if (value < bound.minimum || value > bound.maximum)
                            && !violations.contains(&bound.id)
                        {
                            violations.push(bound.id.clone());
                        }
                    }
                }
            }
            let sample =
                json!({"timeSeconds":ms as f64/1000.0,"sequence":sequence,"values":values});
            if let Some(checkpoint) = &mut checkpoint {
                checkpoint.sample(&sample)?;
            }
            samples.push(sample);
            simulator
                .apply_post_cycle(now, &runtime)
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    })();
    if let Err(reason) = executed {
        if input.evaluation.is_some() || samples.is_empty() {
            return Err(reason);
        }
        return Ok(
            json!({"status":"failed","reason":reason,"io_provenance":"isolated-simulation-no-physical-drivers","run":recorded_run(&samples)}),
        );
    }
    if !score.is_finite() {
        return Err("nonfinite objective".into());
    }
    let run = recorded_run(&samples);
    Ok(if let Some(evaluation) = &input.evaluation {
        json!({"index":input.index,"parameters":input.parameters,"status":if violations.is_empty(){"qualified"}else{"rejected"},"violations":violations,"score":score,"score_unit":format!("{}^2 s",evaluation.objective.error_unit),"integration":"trapezoidal","io_provenance":"isolated-simulation-no-physical-drivers","run":run})
    } else {
        json!({"status":"executed","io_provenance":"isolated-simulation-no-physical-drivers","run":run})
    })
}
