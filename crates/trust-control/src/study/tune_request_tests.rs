//! Tests of a Tune run's request and result (specification 35, B4.2, B4.5, C0, C2, N5.R): the method words, the pins of the family that the
//! request or the study gives, the manual settings and the preview, the relay rule's candidate, the settings that the result names by their
//! symbols, and a program's own law. They were part of `tune_tests.rs`, which keeps the Tune section.

use std::collections::BTreeMap;

use serde_json::json;

use super::test_support::record;
use super::*;
use crate::experiment::ControlExperimentKind;
use crate::identification::identify_result::identify_signals;
use crate::loop_definition::{ControllerLaw, ControllerProject, Parameter, RealType};
use crate::messages::{TuneMethod, TuneRole};
use crate::stages::stage_availability;
use crate::tuning::candidate::CandidateSource;
use crate::tuning::relay_rule::RelayCycleEvidence;
use crate::tuning::settings_pins::{ControllerSettingsPins, PinnedSettings};
use crate::tuning::structure::Structure;
use crate::tuning::texts;

/// The record symbols of the commissioning example's proportional and integral gains.
const PROPORTIONAL: &str = "P1.Parameters.ProportionalGain";

const INTEGRAL: &str = "P1.Parameters.IntegralGain";

/// The measurement filter time that [`example_loop_with_filter`] declares, a REAL parameter in seconds from zero to 10.
const FILTER: &str = "P1.Parameters.MeasurementFilterTime";

/// The loop of the commissioning example: a direct-acting PID loop whose parameters the engineer may enter, with the bindings
/// its PID law names (`examples/control-pid-commissioning`).
fn example_loop() -> LoopDefinition {
    LoopDefinition::decode(include_str!(
        "../../../../examples/control-pid-commissioning/control-loop.json"
    ))
    .expect("the commissioning example's loop")
}

/// The example loop with a measurement filter that its PID law declares (C0).
fn example_loop_with_filter() -> LoopDefinition {
    let mut loop_definition = example_loop();
    if let Some(ControllerLaw::Pid(pid)) = &mut loop_definition.controller.law {
        pid.measurement_filter_time_symbol = Some(FILTER.to_owned());
    }
    if let Some(binding) = &mut loop_definition.parameter_binding {
        binding.parameters.push(Parameter {
            symbol: FILTER.to_owned(),
            iec_type: RealType::Real,
            unit: "s".to_owned(),
            convention: "Control.PID MeasurementFilterTime".to_owned(),
            minimum: 0.0,
            maximum: 10.0,
        });
    }
    loop_definition
}

/// The study of the example loop: the first-order model of the model tests, on the loop's own signals, so that the study is
/// current for the loop (A8.4).
fn example_identify(loop_definition: &LoopDefinition) -> IdentifySection {
    let base = crate::study::tune::model::tests::identify_section(None);
    IdentifySection::new(
        base.method,
        base.source,
        identify_signals(loop_definition, ControlExperimentKind::SetpointStep),
        &base.estimation_recording_id,
        None,
        base.selected_model.clone(),
        base.presentation
            .clone()
            .expect("a fit keeps its presentation"),
    )
}

/// A request for the example loop with the method given and nothing else.
fn example_request(method: Option<TuneMethod>) -> TuneRequest {
    TuneRequest {
        project_root: "/projects/control-pid-commissioning".to_owned(),
        runtime_id: "simulator".to_owned(),
        loop_id: "control-pid-commissioning".to_owned(),
        storage_folder: "/state/trust".to_owned(),
        method,
        choice: None,
        structure: None,
        pins: None,
        settings: None,
        connection: None,
    }
}

#[test]
fn the_tune_method_words_are_the_stored_ones_and_only_the_formulas_have_a_formula() {
    for (method, word, formula) in [
        (TuneMethod::Simc, "SIMC", Some(TuningMethod::Simc)),
        (TuneMethod::Amigo, "AMIGO", Some(TuningMethod::Amigo)),
        (TuneMethod::Relay, "relay", None),
        (TuneMethod::Manual, "manual", None),
    ] {
        assert_eq!(serde_json::to_value(method).expect("a word"), json!(word));
        assert_eq!(method.formula(), formula, "{word}");
    }
    assert_eq!(
        TuneMethod::Simc.title(),
        "Simple internal model control (SIMC)"
    );
    assert_eq!(TuneMethod::Relay.title(), "Relay rule (Ziegler–Nichols PI)");
    assert_eq!(TuneMethod::Manual.title(), "Manual");
}

#[test]
fn pins_take_the_family_of_the_request_then_the_candidate_then_the_recorded_pins() {
    // The page names pins without a family; the study gives it (B4.2).
    let pins = PinnedSettings {
        loop_revision: "sha256:rev".to_owned(),
        values: BTreeMap::from([("Main.Kp".to_owned(), 2.0)]),
    };
    let recorded = ControllerSettingsPins {
        loop_revision: "sha256:rev".to_owned(),
        family: Structure::PID,
        values: BTreeMap::new(),
    };
    let named = resolve_pins(
        &pins,
        Some(Structure::PI),
        Some(Structure::PD),
        Some(&recorded),
    )
    .expect("the structure the request names gives the family");
    assert_eq!(named.family, Structure::PI);
    assert_eq!(named.values, pins.values);
    let candidate = resolve_pins(&pins, None, Some(Structure::PD), Some(&recorded))
        .expect("the candidate gives the family");
    assert_eq!(candidate.family, Structure::PD);
    let recorded_family =
        resolve_pins(&pins, None, None, Some(&recorded)).expect("the recorded pins give it");
    assert_eq!(recorded_family.family, Structure::PID);
    assert_eq!(recorded_family.loop_revision, "sha256:rev");
    let refused = resolve_pins(&pins, None, None, None).expect_err("no family applies");
    assert_eq!(
        (refused.message.as_str(), refused.field.as_deref()),
        (texts::PINS_NEED_FAMILY, Some("family"))
    );
}

#[test]
fn a_pin_outside_the_family_the_study_gives_is_refused_with_its_symbol() {
    // The family the pins were set for names the settings they hold: a derivative setting is no setting of a PI (B4.2, C0).
    let loop_definition = example_loop();
    let identify = CurrentModel::identified(example_identify(&loop_definition));
    let pins = ControllerSettingsPins {
        loop_revision: loop_definition.revision.clone(),
        family: Structure::PI,
        values: BTreeMap::from([("P1.Parameters.DerivativeGain".to_owned(), 0.1)]),
    };
    let refused = check_before_start(&TuneInputs {
        loop_definition: &loop_definition,
        model: Some(&identify),
        previous: None,
        request: &example_request(Some(TuneMethod::Simc)),
        relay: None,
        pins: Some(&pins),
    })
    .expect_err("a derivative setting is refused for a PI");
    assert_eq!(refused.cause, Cause::Settings, "{refused:?}");
    assert!(
        refused.message.contains("P1.Parameters.DerivativeGain"),
        "the refusal names the symbol: {refused:?}"
    );
}

#[test]
fn a_preview_needs_its_settings_as_numbers_by_their_symbols() {
    let entered = json!({ "Main.Kp": 1.2 });
    assert_eq!(
        preview_settings(Some(&entered)).expect("numbers by symbol"),
        BTreeMap::from([("Main.Kp".to_owned(), 1.2)])
    );
    // The page sends each entry as typed (B4.5): text that reads as a number is that number.
    let typed = json!({ "Main.Kp": "1.2" });
    assert_eq!(
        preview_settings(Some(&typed)).expect("typed numbers by symbol"),
        BTreeMap::from([("Main.Kp".to_owned(), 1.2)])
    );
    let missing_value = json!(null);
    let unreadable = json!({ "Main.Kp": "fast" });
    for settings in [None, Some(&missing_value), Some(&unreadable)] {
        assert_eq!(
            preview_settings(settings)
                .expect_err("the settings are not numbers")
                .message,
            texts::PREVIEW_NEEDS_SETTINGS
        );
    }
}

#[test]
fn the_manual_settings_the_pins_and_the_relay_record_are_refused_at_once_with_their_sentences() {
    let loop_definition = example_loop();
    let identify = CurrentModel::identified(example_identify(&loop_definition));
    let refused_by = |request: &TuneRequest, relay: Option<&RelayCycleEvidence>| {
        check_before_start(&TuneInputs {
            loop_definition: &loop_definition,
            model: Some(&identify),
            previous: None,
            request,
            relay,
            pins: None,
        })
        .expect_err("the request is refused")
    };

    // The manual method takes its settings, and the other methods refuse them (B4.2).
    let missing = refused_by(&example_request(Some(TuneMethod::Manual)), None);
    assert_eq!(
        (missing.message.as_str(), missing.cause, missing.next_action),
        (
            texts::MANUAL_NEEDS_SETTINGS,
            Cause::Settings,
            NextAction::None
        )
    );
    let mut with_settings = example_request(Some(TuneMethod::Simc));
    with_settings.settings = Some(BTreeMap::from([(PROPORTIONAL.to_owned(), 2.0)]));
    assert_eq!(
        refused_by(&with_settings, None).message,
        texts::SETTINGS_NEED_MANUAL
    );

    // The entered settings are the loop's, and within their bounds (B4.5).
    let mut out_of_range = example_request(Some(TuneMethod::Manual));
    out_of_range.settings = Some(BTreeMap::from([(PROPORTIONAL.to_owned(), 150.0)]));
    assert_eq!(
        refused_by(&out_of_range, None).message,
        "P1.Parameters.ProportionalGain must be between 0 and 100 %/°C; 150 is outside that range."
    );
    let mut unknown = example_request(Some(TuneMethod::Manual));
    unknown.settings = Some(BTreeMap::from([("P1.Other".to_owned(), 1.0)]));
    assert_eq!(
        refused_by(&unknown, None).message,
        "P1.Other is not a setting of this controller."
    );

    // The pins belong to this loop revision and to the structure that the request fixes (B4.2). The design takes them resolved, with
    // their family (`TuneInputs::pins`), and the request names them without one.
    let refused_with_pins = |request: &TuneRequest, pins: &ControllerSettingsPins| {
        check_before_start(&TuneInputs {
            loop_definition: &loop_definition,
            model: Some(&identify),
            previous: None,
            request,
            relay: None,
            pins: Some(pins),
        })
        .expect_err("the request is refused")
    };
    let mut other_revision = example_request(Some(TuneMethod::Simc));
    other_revision.pins = Some(PinnedSettings {
        loop_revision: "sha256:another-revision".to_owned(),
        values: BTreeMap::new(),
    });
    let resolved_other_revision = ControllerSettingsPins {
        loop_revision: "sha256:another-revision".to_owned(),
        family: Structure::PI,
        values: BTreeMap::new(),
    };
    assert_eq!(
        refused_with_pins(&other_revision, &resolved_other_revision).message,
        texts::PINS_LOOP_REVISION
    );
    let mut other_family = example_request(Some(TuneMethod::Simc));
    other_family.structure = Some(Structure::PD);
    other_family.pins = Some(PinnedSettings {
        loop_revision: loop_definition.revision.clone(),
        values: BTreeMap::new(),
    });
    let resolved_other_family = ControllerSettingsPins {
        loop_revision: loop_definition.revision.clone(),
        family: Structure::PI,
        values: BTreeMap::new(),
    };
    assert_eq!(
        refused_with_pins(&other_family, &resolved_other_family).message,
        texts::PINS_FAMILY
    );

    // The relay rule needs a qualified relay record of the study (N5.R, N2.9).
    let relay = example_request(Some(TuneMethod::Relay));
    let no_record = refused_by(&relay, None);
    assert_eq!(
        (
            no_record.message.as_str(),
            no_record.cause,
            no_record.next_action
        ),
        (
            texts::RELAY_NOT_IDENTIFIED,
            Cause::ModelOrData,
            NextAction::ChooseMethod
        )
    );
    let reason = "At least three complete feedback relay cycles are required.";
    let not_qualified = RelayCycleEvidence::NotQualified {
        reason: reason.to_owned(),
    };
    assert_eq!(refused_by(&relay, Some(&not_qualified)).message, reason);
}

#[test]
fn manual_settings_without_a_structure_are_refused_by_tune_and_by_a_preview_with_the_page_sentence()
{
    // B4.2: the entered settings take the structure the request fixes. Without one, Tune and a preview refuse them.
    let loop_definition = example_loop();
    let identify = CurrentModel::identified(example_identify(&loop_definition));
    let mut request = example_request(Some(TuneMethod::Manual));
    request.settings = Some(BTreeMap::from([
        (PROPORTIONAL.to_owned(), 2.0),
        (INTEGRAL.to_owned(), 0.5),
    ]));
    let inputs = TuneInputs {
        loop_definition: &loop_definition,
        model: Some(&identify),
        previous: None,
        request: &request,
        relay: None,
        pins: None,
    };
    let refused =
        check_before_start(&inputs).expect_err("manual settings need a controller configuration");
    assert_eq!(
        (refused.message.as_str(), refused.cause, refused.next_action),
        (
            texts::MANUAL_NEEDS_CONTROLLER,
            Cause::Settings,
            NextAction::None
        )
    );
    assert_eq!(
        tune(&inputs).expect_err("Tune refuses them too").message,
        texts::MANUAL_NEEDS_CONTROLLER
    );
    assert_eq!(
        preview(&inputs, None)
            .expect_err("a preview refuses them too")
            .message,
        texts::MANUAL_NEEDS_CONTROLLER
    );

    // A fixed structure takes the same settings.
    request.structure = Some(Structure::PI);
    let fixed = TuneInputs {
        loop_definition: &loop_definition,
        model: Some(&identify),
        previous: None,
        request: &request,
        relay: None,
        pins: None,
    };
    check_before_start(&fixed).expect("a fixed structure takes the entered settings");
}

#[test]
fn a_manual_entry_is_the_same_candidate_through_tune_and_through_preview_and_preview_keeps_nothing()
{
    let loop_definition = example_loop();
    let identify = CurrentModel::identified(example_identify(&loop_definition));
    let settings = BTreeMap::from([(PROPORTIONAL.to_owned(), 2.0), (INTEGRAL.to_owned(), 0.5)]);
    let mut request = example_request(Some(TuneMethod::Manual));
    request.settings = Some(settings.clone());
    request.structure = Some(Structure::PI);
    let inputs = TuneInputs {
        loop_definition: &loop_definition,
        model: Some(&identify),
        previous: None,
        request: &request,
        relay: None,
        pins: None,
    };
    let outcome = tune(&inputs).expect("the entered PI settings are a candidate");
    assert_eq!(outcome.section.method, Some(TuneMethod::Manual));
    assert_eq!(outcome.result.method, Some(TuneMethod::Manual));
    assert_eq!(outcome.result.method_title, "Manual");
    assert_eq!(
        record(outcome.result.candidate.as_ref()).source,
        CandidateSource::Manual
    );
    assert_eq!(
        record(outcome.result.candidate.as_ref()).structure,
        Structure::PI
    );
    assert!(
        !record(outcome.result.candidate.as_ref()).applied,
        "Tune never applies"
    );
    assert_eq!(outcome.result.reason, texts::MANUAL_READY);
    // The answer carries the evidence its stages are computed from: the model and the candidate it designed (B9.4).
    assert!(outcome.result.stage_evidence.has_model && outcome.result.stage_evidence.has_candidate);
    assert_eq!(
        outcome.result.stages,
        stage_availability(&outcome.result.stage_evidence)
    );

    // The preview answers the same settings with the same candidate and prediction. Its result is a `TuneResult`, which has
    // no study section, so nothing is stored.
    // A preview evaluates the manual method with these settings, as the engine builds its request (`preview_action`).
    let preview_request = TuneRequest {
        project_root: request.project_root.clone(),
        runtime_id: request.runtime_id.clone(),
        loop_id: request.loop_id.clone(),
        storage_folder: String::new(),
        method: Some(TuneMethod::Manual),
        choice: None,
        structure: Some(Structure::PI),
        pins: None,
        settings: Some(settings),
        connection: None,
    };
    let previewed = preview(
        &TuneInputs {
            request: &preview_request,
            ..inputs
        },
        None,
    )
    .expect("the preview answers the entered settings");
    assert_eq!(previewed.candidate, outcome.result.candidate);
    assert_eq!(previewed.outcome, outcome.result.outcome);
    assert_eq!(previewed.response, outcome.result.response);
    // A preview keeps nothing, so its evidence is the study's own: the model, and no candidate, because the study holds no earlier
    // Tune section (B4.5). Its stages are computed from that evidence.
    assert!(previewed.stage_evidence.has_model && !previewed.stage_evidence.has_candidate);
    assert_eq!(
        previewed.stages,
        stage_availability(&previewed.stage_evidence)
    );

    // Without a model, the preview is refused at once, with the next action Identify (B4.7).
    let without_model = preview(
        &TuneInputs {
            model: None,
            ..inputs
        },
        None,
    )
    .expect_err("a preview needs a model");
    assert_eq!(without_model.next_action, NextAction::Identify);
}

/// B4.5: the running controller's settings, when the engine read them, add the two current traces to the prediction on the
/// candidate's time base, and the candidate keeps its own settling time, overshoot, outcome and traces. Without them the response
/// holds the candidate's three traces.
#[test]
fn the_running_settings_add_the_current_traces_and_leave_the_candidate_alone() {
    let loop_definition = example_loop();
    let identify = CurrentModel::identified(example_identify(&loop_definition));
    let mut request = example_request(Some(TuneMethod::Manual));
    request.settings = Some(BTreeMap::from([
        (PROPORTIONAL.to_owned(), 2.0),
        (INTEGRAL.to_owned(), 0.5),
    ]));
    request.structure = Some(Structure::PI);
    let inputs = TuneInputs {
        loop_definition: &loop_definition,
        model: Some(&identify),
        previous: None,
        request: &request,
        relay: None,
        pins: None,
    };
    let alone = tune(&inputs).expect("the entered PI settings are a candidate");
    let alone_traces = &alone
        .result
        .response
        .as_ref()
        .expect("a predicted response")
        .traces;
    assert_eq!(alone_traces.len(), 3, "the candidate's three traces");

    let running = BTreeMap::from([(PROPORTIONAL.to_owned(), 1.0), (INTEGRAL.to_owned(), 0.1)]);
    let compared = tune_held(&inputs, None, Some(&running)).expect("the comparison is predicted");
    let response = compared
        .result
        .response
        .as_ref()
        .expect("a predicted response");
    let roles: Vec<TuneRole> = response.traces.iter().map(|trace| trace.role).collect();
    assert_eq!(
        roles,
        [
            TuneRole::Setpoint,
            TuneRole::Measurement,
            TuneRole::Command,
            TuneRole::CurrentMeasurement,
            TuneRole::CurrentCommand,
        ]
    );
    assert_eq!(response.traces[..3], alone_traces[..]);
    assert_eq!(compared.result.candidate, alone.result.candidate);
    assert_eq!(compared.result.outcome, alone.result.outcome);
    assert_eq!(
        compared.result.settling_time_seconds,
        alone.result.settling_time_seconds
    );
    assert_eq!(
        compared.result.overshoot_percent,
        alone.result.overshoot_percent
    );

    // The preview with the same running settings predicts the same comparison, and keeps nothing (B4.5).
    let previewed = preview(&inputs, Some(&running)).expect("the preview answers the settings");
    assert_eq!(previewed.response, compared.result.response);
}

#[test]
fn a_qualified_relay_record_gives_the_relay_rule_candidate_with_its_ku_and_pu() {
    let loop_definition = example_loop();
    let identify = CurrentModel::identified(example_identify(&loop_definition));
    let relay = RelayCycleEvidence::Qualified {
        cycles: 4,
        output_amplitude: 1.0,
        response_amplitude: 1.0,
        period_seconds: 4.0,
        ultimate_gain: 2.0,
        proportional_gain: 0.9,
        integral_time_seconds: 4.0 / 1.2,
        integral_gain: 0.27,
    };
    let request = example_request(Some(TuneMethod::Relay));
    let outcome = tune(&TuneInputs {
        loop_definition: &loop_definition,
        model: Some(&identify),
        previous: None,
        request: &request,
        relay: Some(&relay),
        pins: None,
    })
    .expect("a qualified relay record designs the relay rule");
    assert_eq!(outcome.result.method, Some(TuneMethod::Relay));
    assert_eq!(
        outcome.result.method_title,
        "Relay rule (Ziegler–Nichols PI)"
    );
    assert_eq!(
        record(outcome.result.candidate.as_ref()).source,
        CandidateSource::RelayRule
    );
    assert_eq!(
        record(outcome.result.candidate.as_ref()).structure,
        Structure::PI
    );
    assert_eq!(
        outcome.result.reason,
        "Relay rule from Ku = 2 and Pu = 4 s: Kp = 0.45·Ku, Ti = Pu/1.2. Validate before applying."
    );
    assert!(outcome.result.method_description.contains("Ku"));
    assert!(outcome.result.method_description.contains("Pu"));
    // The example is direct-acting, so Kp and Ki are positive (N6.4); Ki = Kp/Ti with Ti = 4/1.2 s.
    assert!(
        (record(outcome.result.candidate.as_ref())
            .parameters
            .proportional_gain
            - 0.9)
            .abs()
            <= 1e-9
    );
    assert!(
        (record(outcome.result.candidate.as_ref())
            .parameters
            .integral_gain
            - 0.27)
            .abs()
            <= 1e-9
    );
}

#[test]
fn a_law_that_declares_a_filter_writes_its_filter_time_and_predicts_with_it() {
    let loop_definition = example_loop_with_filter();
    let identify = CurrentModel::identified(example_identify(&loop_definition));
    let calculated_request = example_request(Some(TuneMethod::Simc));
    let calculated = tune(&TuneInputs {
        loop_definition: &loop_definition,
        model: Some(&identify),
        previous: None,
        request: &calculated_request,
        relay: None,
        pins: None,
    })
    .expect("SIMC designs a PI with the filter declared");
    assert_eq!(
        record(calculated.result.candidate.as_ref()).measurement_filter_seconds,
        Some(0.0),
        "a calculated candidate writes Tf = 0 unless Tf is pinned (C0)"
    );

    let pinned_request = example_request(Some(TuneMethod::Simc));
    let pinned_settings = ControllerSettingsPins {
        loop_revision: loop_definition.revision.clone(),
        family: Structure::PI,
        values: BTreeMap::from([(FILTER.to_owned(), 0.5)]),
    };
    let pinned = tune(&TuneInputs {
        loop_definition: &loop_definition,
        model: Some(&identify),
        previous: None,
        request: &pinned_request,
        relay: None,
        pins: Some(&pinned_settings),
    })
    .expect("the pinned filter time keeps its value");
    assert_eq!(
        record(pinned.result.candidate.as_ref()).measurement_filter_seconds,
        Some(0.5)
    );
    assert_ne!(
        pinned.result.response, calculated.result.response,
        "the filter lag is part of the predicted response (C0)"
    );

    let plain_loop = example_loop();
    let plain_identify = CurrentModel::identified(example_identify(&plain_loop));
    let plain = tune(&TuneInputs {
        loop_definition: &plain_loop,
        model: Some(&plain_identify),
        previous: None,
        request: &calculated_request,
        relay: None,
        pins: None,
    })
    .expect("SIMC designs a PI without a filter");
    assert_eq!(
        record(plain.result.candidate.as_ref()).measurement_filter_seconds,
        None
    );
}

#[test]
fn the_result_names_each_setting_of_the_candidate_by_its_symbol() {
    // A manual PI entry is the candidate, and the result gives its record under the loop's own symbols (B4.2, C0). The
    // measurement filter time that the loop declares is named with the value the engineer entered.
    let loop_definition = example_loop();
    let identify = CurrentModel::identified(example_identify(&loop_definition));
    let mut request = example_request(Some(TuneMethod::Manual));
    request.settings = Some(BTreeMap::from([
        (PROPORTIONAL.to_owned(), 2.0),
        (INTEGRAL.to_owned(), 0.5),
    ]));
    request.structure = Some(Structure::PI);
    let outcome = tune(&TuneInputs {
        loop_definition: &loop_definition,
        model: Some(&identify),
        previous: None,
        request: &request,
        relay: None,
        pins: None,
    })
    .expect("the entered PI settings are a candidate");
    assert_eq!(outcome.result.settings.get(PROPORTIONAL), Some(&2.0));
    assert_eq!(outcome.result.settings.get(INTEGRAL), Some(&0.5));
    let bound = loop_definition
        .parameter_binding
        .as_ref()
        .expect("the example binds its parameters");
    assert!(
        outcome.result.settings.keys().all(|symbol| bound
            .parameters
            .iter()
            .any(|parameter| &parameter.symbol == symbol)),
        "every named setting is a parameter the loop binds"
    );

    let filtered_loop = example_loop_with_filter();
    let filtered_identify = CurrentModel::identified(example_identify(&filtered_loop));
    let mut filtered_request = example_request(Some(TuneMethod::Manual));
    filtered_request.settings = Some(BTreeMap::from([
        (PROPORTIONAL.to_owned(), 2.0),
        (INTEGRAL.to_owned(), 0.5),
        (FILTER.to_owned(), 0.25),
    ]));
    filtered_request.structure = Some(Structure::PI);
    let filtered = tune(&TuneInputs {
        loop_definition: &filtered_loop,
        model: Some(&filtered_identify),
        previous: None,
        request: &filtered_request,
        relay: None,
        pins: None,
    })
    .expect("the entered settings with the filter time are a candidate");
    assert_eq!(filtered.result.settings.get(FILTER), Some(&0.25));
}

/// The commissioning example as a program's own PI law (C2): its controller law and its PID initialization are replaced by a
/// served `parallel-pid` seed that names the example's proportional and integral parameters.
fn example_program_pi_loop() -> LoopDefinition {
    let mut loop_definition = example_loop();
    loop_definition.controller.law = None;
    loop_definition.controller.pid_initialization = None;
    loop_definition.controller.project = Some(
        ControllerProject::from_value(&json!({
            "convention": "project-owned-native-controller",
            "family": "PI",
            "initialization": { "kind": "declared-state-v1", "states": [] },
            "seed": { "kind": "parallel-pid", "family": "PI",
                "proportionalSymbol": PROPORTIONAL, "integralSymbol": INTEGRAL },
            "axes": [],
            "objective": { "requirementId": "target-error", "direction": "minimize" },
            "maximumCandidates": 4, "maximumCycles": 50000, "wallBudgetMs": 60000
        }))
        .expect("a served PI seed is a complete project block"),
    );
    loop_definition
}

#[test]
fn a_program_pi_law_gives_a_project_candidate_whose_settings_name_its_variables() {
    // C2 Tune: the answer carries the program's candidate, with the values of the variables of its structure, and no Control.PID record.
    let loop_definition = example_program_pi_loop();
    let identify = CurrentModel::identified(example_identify(&loop_definition));
    let mut request = example_request(Some(TuneMethod::Simc));
    request.structure = Some(Structure::PI);
    let outcome = tune(&TuneInputs {
        loop_definition: &loop_definition,
        model: Some(&identify),
        previous: None,
        request: &request,
        relay: None,
        pins: None,
    })
    .expect("the program's PI law is designed on the model");
    assert!(
        outcome.result.candidate.is_none(),
        "a program's law keeps no record"
    );
    assert!(outcome.section.candidate.is_none());
    let candidate = outcome
        .result
        .project_candidate
        .as_ref()
        .expect("the program's candidate");
    assert_eq!(candidate.structure, Structure::PI);
    assert!(!candidate.applied, "Tune never applies");
    assert_eq!(
        candidate.values,
        BTreeMap::from([
            (PROPORTIONAL.to_owned(), candidate.gains.proportional_gain),
            (INTEGRAL.to_owned(), candidate.gains.integral_gain),
        ]),
        "a PI candidate holds its proportional and integral variables only"
    );
    assert_eq!(outcome.result.settings, candidate.values);
    assert_eq!(outcome.section.project_candidate.as_ref(), Some(candidate));

    // A proportional design of the same law holds its proportional variable only (C2, the family's own variables).
    let mut proportional = example_request(Some(TuneMethod::Simc));
    proportional.structure = Some(Structure::P);
    let answer = tune(&TuneInputs {
        loop_definition: &loop_definition,
        model: Some(&identify),
        previous: None,
        request: &proportional,
        relay: None,
        pins: None,
    })
    .expect("the program's P is designed on the model");
    let candidate = answer
        .result
        .project_candidate
        .as_ref()
        .expect("the program's candidate");
    assert_eq!(candidate.structure, Structure::P);
    assert_eq!(
        candidate.values,
        BTreeMap::from([(PROPORTIONAL.to_owned(), candidate.gains.proportional_gain)])
    );
    assert!(
        candidate.gains.integral_gain.abs() <= 1e-12,
        "P has no integral term"
    );
}
