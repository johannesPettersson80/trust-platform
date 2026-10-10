//! Tests of the supplied model's section (`generalModelPersistence.ts`; numerical annex N4). The supplied model, the grid
//! (1 to 10 rad/s, three points) and the response settings are those of `control-studio-general-model-session.test.ts`;
//! the unanalysed supplied model is that of `control-studio-engineer-study-persistence.test.ts` ("persists an unanalysed
//! supplied model").

use serde_json::{json, Value};

use super::*;
use crate::limits::STUDY_STORAGE_LIMIT_BYTES;
use crate::model::{
    analyze_general_model_time, calculate_general_state_structure, decode_general_model,
    ModelIdentity, ModelValidation,
};
use crate::study::{canonical_json, Study};

/// One channel of a fixture model, in the JSON words of the model file.
fn channel(id: &str, unit: &str, maximum: f64) -> Value {
    json!({ "id": id, "unit": unit, "operatingRange": { "minimum": 0.0, "maximum": maximum } })
}

/// The supplied model of the session test: `2 / (s + 2)` from a drive in V to a speed in rpm.
fn supplied_model() -> GeneralModel {
    decode_general_model(&json!({
        "kind": "transfer-function", "revision": "m", "source": "provided", "validation": "unvalidated",
        "evidence": [], "delaySeconds": 0.0,
        "inputs": [channel("drive", "V", 10.0)], "outputs": [channel("speed", "rpm", 100.0)],
        "numerator": [2.0], "denominator": [1.0, 2.0]
    }))
    .expect("the supplied model")
}

/// The supplied model of the persistence test: `1 / (s + 1)` from a command in V to a position in m, provided and
/// unvalidated, with no evidence.
fn provided_model() -> GeneralModel {
    let range = json!({ "minimum": -10.0, "maximum": 10.0 });
    decode_general_model(&json!({
        "kind": "transfer-function", "revision": "model:provided:1", "source": "provided",
        "validation": "unvalidated", "evidence": [], "delaySeconds": 0.0,
        "inputs": [{ "id": "command", "unit": "V", "operatingRange": range }],
        "outputs": [{ "id": "position", "unit": "m", "operatingRange": range }],
        "numerator": [1.0], "denominator": [1.0, 1.0]
    }))
    .expect("the provided model")
}

/// A sampled one-state model `x⁺ = 0.9 x + 0.1 u`, `y = x`, at a sample time of 0.1 s.
fn sampled_model() -> GeneralModel {
    decode_general_model(&json!({
        "kind": "discrete-state-space", "revision": "sampled", "source": "provided", "validation": "unvalidated",
        "evidence": [], "delaySeconds": 0.0, "sampleTimeSeconds": 0.1,
        "inputs": [channel("u", "V", 10.0)], "outputs": [channel("y", "m", 10.0)],
        "states": [channel("x", "m", 10.0)],
        "a": [[0.9]], "b": [[0.1]], "c": [[1.0]], "d": [[0.0]]
    }))
    .expect("the sampled model")
}

/// The model with its identity changed.
fn edited(mut model: GeneralModel, change: impl FnOnce(&mut ModelIdentity)) -> GeneralModel {
    match &mut model {
        GeneralModel::TransferFunction { identity, .. }
        | GeneralModel::ContinuousStateSpace { identity, .. }
        | GeneralModel::DiscreteStateSpace { identity, .. } => change(identity),
    }
    model
}

fn grid(maximum_frequency: f64) -> ModelFrequencyGrid {
    ModelFrequencyGrid {
        minimum_frequency: 1.0,
        maximum_frequency,
        points: 3,
    }
}

/// A point of a response with its magnitude and phase.
fn point(frequency: f64, real: f64, imaginary: f64) -> FrequencyPoint {
    FrequencyPoint {
        frequency_rad_per_second: frequency,
        real,
        imaginary,
        magnitude: real.hypot(imaginary),
        phase_radians: imaginary.atan2(real),
    }
}

fn capabilities() -> FrequencyCapabilities {
    FrequencyCapabilities {
        relative_gain: Capability {
            available: true,
            reason: None,
        },
        step_response: Capability {
            available: false,
            reason: Some(
                "Select a qualified time-response execution adapter and initial conditions.".into(),
            ),
        },
        transfer_conversion: Capability {
            available: true,
            reason: None,
        },
    }
}

/// The analysis of `2 / (s + 2)` on the grid: `G(jω) = 2 / (2 + jω)`, one stable pole, a relative gain of one.
fn transfer_analysis(model: &GeneralModel, grid: ModelFrequencyGrid) -> SuppliedModelAnalysis {
    let identity = model.identity();
    let points = (0..grid.points)
        .map(|index| {
            let omega = grid.frequency_at(index);
            let scale = 4.0 + omega * omega;
            point(omega, 4.0 / scale, -2.0 * omega / scale)
        })
        .collect::<Vec<_>>();
    SuppliedModelAnalysis {
        grid,
        result: GeneralModelFrequencyResult {
            model: model.clone(),
            channels: vec![ChannelFrequencyResponse {
                input: identity.inputs[0].clone(),
                output: identity.outputs[0].clone(),
                points,
            }],
            poles: vec![ModelPole {
                real: -2.0,
                imaginary: 0.0,
            }],
            stable: true,
            pole_scope: "Model poles only; pure delay is not represented in the pole list. This is not closed-loop stability.".into(),
            relative_gain_matrix: Some(vec![vec![1.0]]),
            state_structure: None,
            capabilities: capabilities(),
        },
        response: None,
    }
}

/// The analysis of the sampled model on the grid: `G(z) = 0.1 / (z − 0.9)` at `z = exp(jω · 0.1)`, with its structure.
fn sampled_analysis(model: &GeneralModel, grid: ModelFrequencyGrid) -> SuppliedModelAnalysis {
    let identity = model.identity();
    let points = (0..grid.points)
        .map(|index| {
            let omega = grid.frequency_at(index);
            let angle = omega * 0.1;
            let (cosine, sine) = (angle.cos(), angle.sin());
            let scale = (cosine - 0.9).powi(2) + sine * sine;
            point(omega, 0.1 * (cosine - 0.9) / scale, -0.1 * sine / scale)
        })
        .collect::<Vec<_>>();
    let GeneralModel::DiscreteStateSpace { system, .. } = model else {
        panic!("a sampled model was expected");
    };
    SuppliedModelAnalysis {
        grid,
        result: GeneralModelFrequencyResult {
            model: model.clone(),
            channels: vec![ChannelFrequencyResponse {
                input: identity.inputs[0].clone(),
                output: identity.outputs[0].clone(),
                points,
            }],
            poles: vec![ModelPole {
                real: 0.9,
                imaginary: 0.0,
            }],
            stable: true,
            pole_scope: "Model poles only.".into(),
            relative_gain_matrix: Some(vec![vec![1.0]]),
            state_structure: Some(calculate_general_state_structure(system)),
            capabilities: capabilities(),
        },
        response: None,
    }
}

fn section_with(model: &GeneralModel, analysis: SuppliedModelAnalysis) -> SuppliedModelSection {
    SuppliedModelSection::new(model.clone(), Some(analysis))
}

fn refusal(section: &SuppliedModelSection) -> String {
    section.encode().expect_err("refused").message
}

#[test]
fn an_unanalysed_supplied_model_is_kept_and_read_back_whole() {
    // The model keeps its provided source, its unvalidated state and its empty evidence (the TypeScript test).
    let section = SuppliedModelSection::new(provided_model(), None);
    let text = section.encode().expect("encoded");
    let decoded = SuppliedModelSection::decode(&text).expect("decoded");
    assert_eq!(decoded, section);
    assert_eq!(decoded.model.identity().revision, "model:provided:1");
    assert_eq!(decoded.model.identity().source, ModelSource::Provided);
    assert_eq!(
        decoded.model.identity().validation,
        ModelValidation::Unvalidated
    );
    assert_eq!(decoded.model.identity().evidence, Vec::<String>::new());
    assert_eq!(decoded.encode().expect("again"), text);

    let mut study = Study::new("loop:tank", "sha256:loop-1");
    study.set_section(&section).expect("kept");
    assert_eq!(
        study.recordings,
        Vec::<String>::new(),
        "a supplied model comes from no recording"
    );
    let reopened = Study::decode(&study.encode().expect("encoded"), "loop:tank").expect("decoded");
    assert_eq!(
        reopened.section::<SuppliedModelSection>().expect("read"),
        Some(section)
    );
}

#[test]
fn a_saved_analysis_round_trips_with_the_words_of_the_typescript_result() {
    let model = supplied_model();
    let section = section_with(&model, transfer_analysis(&model, grid(10.0)));
    let text = section.encode().expect("encoded");
    let decoded = SuppliedModelSection::decode(&text).expect("decoded");
    assert_eq!(decoded, section);
    assert_eq!(
        decoded.encode().expect("again"),
        text,
        "the same section gives the same canonical JSON"
    );

    let value = serde_json::to_value(section).expect("json");
    assert_eq!(
        value["analysis"]["grid"],
        json!({ "minimumFrequency": 1.0, "maximumFrequency": 10.0, "points": 3 })
    );
    let result = &value["analysis"]["result"];
    for key in [
        "model",
        "channels",
        "poles",
        "stable",
        "poleScope",
        "relativeGainMatrix",
        "capabilities",
    ] {
        assert!(result.get(key).is_some(), "{key} is part of the result");
    }
    assert_eq!(
        result.get("stateStructure"),
        None,
        "a transfer function has none"
    );
    assert_eq!(
        result["channels"][0]["points"][1]["frequencyRadPerSecond"],
        json!(grid(10.0).frequency_at(1))
    );
    assert_eq!(
        result["capabilities"]["relativeGain"],
        json!({ "available": true })
    );
}

#[test]
fn the_grid_points_are_spaced_evenly_on_a_logarithmic_scale_and_end_at_the_maximum() {
    let grid = grid(10.0);
    assert!((grid.frequency_at(0) - 1.0).abs() <= 1e-15);
    assert!((grid.frequency_at(1) - 10.0_f64.sqrt()).abs() <= 1e-12);
    assert!((grid.frequency_at(2) - 10.0).abs() <= 1e-15);
}

#[test]
fn a_grid_has_2_to_512_points_a_positive_increasing_range_and_16384_channel_frequency_points() {
    let grid = |minimum: f64, maximum: f64, points: u32| ModelFrequencyGrid {
        minimum_frequency: minimum,
        maximum_frequency: maximum,
        points,
    };
    assert!(grid(1.0, 10.0, 3).is_valid(1));
    assert!(grid(1.0, 10.0, 2).is_valid(1));
    assert!(
        grid(0.1, 100.0, 512).is_valid(32),
        "32 channels of 512 points"
    );
    assert!(
        !grid(0.1, 100.0, 512).is_valid(33),
        "33 channels of 512 points are over 16384 in all"
    );
    assert!(
        !grid(1.0, 10.0, 1).is_valid(1),
        "a grid has two points at least"
    );
    assert!(!grid(1.0, 10.0, 513).is_valid(1));
    assert!(!grid(0.0, 10.0, 3).is_valid(1), "frequencies are positive");
    assert!(!grid(-1.0, 10.0, 3).is_valid(1));
    assert!(!grid(10.0, 10.0, 3).is_valid(1), "the range increases");
    assert!(!grid(10.0, 1.0, 3).is_valid(1));
    assert!(!grid(1.0, f64::INFINITY, 3).is_valid(1));
    assert!(!grid(f64::NAN, 10.0, 3).is_valid(1));
    assert!(
        !grid(1.0, 10.0, 3).is_valid(usize::MAX),
        "a count that overflows is refused"
    );
}

#[test]
fn a_forged_result_is_refused_whole() {
    type Edit = fn(&mut SuppliedModelAnalysis);
    let model = supplied_model();
    let honest = section_with(&model, transfer_analysis(&model, grid(10.0)));
    honest.encode().expect("the honest analysis");

    // control-studio-general-model-session.test.ts: a relative gain array that is not a matrix.
    let mut value = serde_json::to_value(honest).expect("json");
    value["analysis"]["result"]["relativeGainMatrix"] = json!("broken");
    assert!(SuppliedModelSection::decode(&canonical_json(&value)).is_err());

    let cases: [(Edit, &str); 14] = [
        (
            |analysis| analysis.result.relative_gain_matrix = Some(vec![vec![1.0, 2.0]]),
            "relative gain array",
        ),
        (
            |analysis| analysis.result.relative_gain_matrix = Some(vec![vec![f64::NAN]]),
            "relative gain array",
        ),
        (
            |analysis| analysis.result.poles[0].real = f64::NAN,
            "not finite",
        ),
        (|analysis| analysis.result.stable = false, "stability flag"),
        (
            |analysis| analysis.result.poles[0].real = 1.0,
            "stability flag",
        ),
        (
            |analysis| {
                analysis.result.channels[0].points.pop();
            },
            "one point per grid frequency",
        ),
        (
            |analysis| analysis.result.channels[0].points[1].frequency_rad_per_second = 4.0,
            "grid frequency",
        ),
        (
            |analysis| analysis.result.channels[0].points[0].real = f64::NAN,
            "not finite",
        ),
        (
            |analysis| analysis.result.channels[0].input.unit = "A".into(),
            "input-output pair",
        ),
        (
            |analysis| analysis.result.channels.clear(),
            "one response per input-output pair",
        ),
        (
            |analysis| {
                analysis.result.model = edited(supplied_model(), |identity| {
                    identity.revision = "other".into();
                });
            },
            "another model",
        ),
        (
            |analysis| analysis.grid.points = 513,
            "frequency grid needs",
        ),
        (
            |analysis| analysis.grid.minimum_frequency = 0.0,
            "frequency grid needs",
        ),
        (
            |analysis| {
                analysis.result.state_structure = Some(GeneralStateStructure::Unavailable {
                    reason: "not available".into(),
                });
            },
            "state structure",
        ),
    ];
    for (change, expected) in cases {
        let mut analysis = transfer_analysis(&model, grid(10.0));
        change(&mut analysis);
        let message = refusal(&section_with(&model, analysis));
        assert!(message.contains(expected), "{expected}: {message}");
    }
}

#[test]
fn the_analysis_of_a_sampled_model_stays_below_its_nyquist_frequency() {
    let model = sampled_model();
    let honest = section_with(&model, sampled_analysis(&model, grid(10.0)));
    honest
        .encode()
        .expect("a grid below the Nyquist frequency of 31.4 rad/s");
    let text = honest.encode().expect("encoded");
    assert_eq!(
        SuppliedModelSection::decode(&text).expect("decoded"),
        honest
    );

    let aliased = section_with(&model, sampled_analysis(&model, grid(40.0)));
    let message = refusal(&aliased);
    assert!(message.contains("Nyquist"), "{message}");

    let mut broken = sampled_analysis(&model, grid(10.0));
    match &mut broken.result.state_structure {
        Some(GeneralStateStructure::Qualified {
            controllability, ..
        }) => {
            controllability.matrix.clear();
        }
        other => panic!("a qualified structure was expected, got {other:?}"),
    }
    let message = refusal(&section_with(&model, broken));
    assert!(message.contains("state structure"), "{message}");
}

#[test]
fn a_time_response_is_kept_only_for_the_model_it_was_computed_for() {
    let model = supplied_model();
    let settings = json!({
        "stepSeconds": 0.25, "durationSeconds": 1.0, "initialState": [2.0], "input": [1.0],
        "connection": "open-loop"
    });
    let response = |model: &GeneralModel| {
        analyze_general_model_time(&serde_json::to_value(model).expect("json"), &settings)
            .expect("a response")
    };
    let mut analysis = transfer_analysis(&model, grid(10.0));
    analysis.response = Some(response(&model));
    let section = section_with(&model, analysis.clone());
    let text = section.encode().expect("encoded");
    assert_eq!(
        SuppliedModelSection::decode(&text).expect("decoded"),
        section
    );

    let other = edited(model.clone(), |identity| {
        identity.revision = "replacement".into();
    });
    let mut foreign = analysis.clone();
    foreign.response = Some(response(&other));
    assert!(refusal(&section_with(&model, foreign)).contains("time response"));

    let mut truncated = analysis;
    truncated
        .response
        .as_mut()
        .expect("a response")
        .points
        .pop();
    assert!(refusal(&section_with(&model, truncated)).contains("time response"));
}

#[test]
fn a_fitted_model_or_a_model_that_breaks_the_contract_is_not_a_supplied_model() {
    let fitted = edited(supplied_model(), |identity| {
        identity.source = ModelSource::Fitted;
    });
    assert!(
        refusal(&SuppliedModelSection::new(fitted, None)).contains("identification that produced")
    );

    let blank = edited(supplied_model(), |identity| identity.revision = " ".into());
    assert!(refusal(&SuppliedModelSection::new(blank, None)).contains("model contract"));

    let validated_without_evidence = edited(supplied_model(), |identity| {
        identity.validation = ModelValidation::Validated;
    });
    assert!(
        refusal(&SuppliedModelSection::new(validated_without_evidence, None))
            .contains("model contract"),
        "a validated model names its evidence"
    );

    let simulated = edited(supplied_model(), |identity| {
        identity.source = ModelSource::Simulation;
    });
    SuppliedModelSection::new(simulated, None)
        .encode()
        .expect("the exact model of a simulated plant is supplied by the simulation");
}

#[test]
fn the_parked_parts_of_the_typescript_analysis_are_refused_not_dropped() {
    let model = supplied_model();
    let section = section_with(&model, transfer_analysis(&model, grid(10.0)));
    for key in ["poleView", "feedback", "periodic"] {
        let mut value = serde_json::to_value(&section).expect("json");
        value["analysis"][key] = json!({});
        assert!(
            SuppliedModelSection::decode(&canonical_json(&value)).is_err(),
            "{key} belongs to a feature that is parked with stage V7"
        );
    }
    let mut extra = serde_json::to_value(section).expect("json");
    extra["approved"] = json!(true);
    assert!(SuppliedModelSection::decode(&canonical_json(&extra)).is_err());
}

#[test]
fn a_document_over_the_storage_limit_is_refused() {
    // Zero bytes are valid text, and the allocation is not touched until it is read.
    let limit = usize::try_from(STUDY_STORAGE_LIMIT_BYTES).expect("addressable");
    let oversize = String::from_utf8(vec![0_u8; limit + 1]).expect("NUL is UTF-8");
    let failure = SuppliedModelSection::decode(&oversize).expect_err("over the limit");
    assert!(failure.message.contains("257 MiB"), "{}", failure.message);
}

#[test]
fn a_supplied_analysis_whose_model_differs_in_content_is_refused_though_its_revision_is_kept() {
    // control-studio-saved-model-authority.test.ts: a dependent model that differs in content is refused even when its
    // revision is kept. Here the dependent copy is the model inside the analysis of the supplied-model section.
    let model = supplied_model();
    let mut changed = model.clone();
    if let GeneralModel::TransferFunction { transfer, .. } = &mut changed {
        transfer.numerator[0] += 1.0;
    }
    assert_eq!(
        changed.identity().revision,
        model.identity().revision,
        "the revision is kept"
    );

    let mut analysis = transfer_analysis(&model, grid(10.0));
    analysis.result.model = changed.clone();
    assert!(refusal(&section_with(&model, analysis)).contains("another model"));

    let changed_section =
        SuppliedModelSection::new(changed, Some(transfer_analysis(&model, grid(10.0))));
    assert!(
        refusal(&changed_section).contains("another model"),
        "the model and its analysis must agree in content"
    );
}
