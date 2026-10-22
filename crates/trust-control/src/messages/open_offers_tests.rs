//! Tests of the offers of the open answer: the families and their labels, the family the page shows by default, the rows of the manual
//! editor, the setpoint choices and the scenario template.

#![allow(clippy::float_cmp)] // The template's window and bounds are the values the test gives and the loop declares, exactly.

use std::collections::BTreeMap;

use super::{
    controller_offer, default_family, family_label, scenario_row_signal, scenario_template,
    setpoint_choices,
};
use crate::loop_definition::LoopDefinition;
use crate::tuning::settings_pins::ControllerSettingsPins;
use crate::tuning::structure::Structure;
use crate::validation::requirement_record::RunCheck;
use crate::validation::scenario_form::scenarios_of_form;

/// The gains of the commissioning example, which its parameter binding names in the order Kp, Ki and Kd.
const KP: &str = "P1.Parameters.ProportionalGain";
const KI: &str = "P1.Parameters.IntegralGain";
const KD: &str = "P1.Parameters.DerivativeGain";

/// The shipped `Control.PID` loop of the commissioning example.
fn pid_loop() -> LoopDefinition {
    LoopDefinition::decode(include_str!(
        "../../../../examples/control-pid-commissioning/control-loop.json"
    ))
    .expect("the commissioning example's loop")
}

#[test]
fn a_family_is_labelled_by_its_words_and_an_acronym_keeps_its_name() {
    assert_eq!(family_label("state-feedback"), "State feedback");
    assert_eq!(family_label("Smith"), "Smith predictor");
    assert_eq!(family_label("lead-lag"), "Lead/lag");
    assert_eq!(family_label("PI"), "PI");
    assert_eq!(family_label("LQG"), "LQG");
}

#[test]
fn a_pid_loop_offers_its_families_its_default_and_the_rows_of_its_gains() {
    let offer = controller_offer(&pid_loop(), Some("PID".to_owned()))
        .expect("the example's controller is offered");
    assert!(offer
        .families
        .iter()
        .any(|family| family.family == "PID" && family.label == "PID"));
    assert_eq!(offer.default_family.as_deref(), Some("PID"));
    let symbols: Vec<&str> = offer
        .setting_rows
        .iter()
        .map(|row| row.symbol.as_str())
        .collect();
    assert_eq!(
        symbols.iter().take(3).copied().collect::<Vec<_>>(),
        [KP, KI, KD]
    );
    let labels: Vec<&str> = offer
        .setting_rows
        .iter()
        .take(3)
        .map(|row| row.label.as_str())
        .collect();
    assert_eq!(
        labels,
        [
            "Proportional gain (Kp)",
            "Integral gain (Ki)",
            "Derivative gain (Kd)"
        ]
    );
    let pid = offer
        .family_settings
        .get("PID")
        .expect("the PID family takes settings");
    assert!(
        pid.contains(&KP.to_owned())
            && pid.contains(&KI.to_owned())
            && pid.contains(&KD.to_owned())
    );
    // The theme journey's PI candidate shows the rows of its proportional and integral gains (control-studio-three-page-themes).
    let pi = offer
        .family_settings
        .get("PI")
        .expect("the PI family takes settings");
    assert!(
        pi.contains(&KP.to_owned()),
        "a PI family takes its proportional gain"
    );
    assert!(
        pi.contains(&KI.to_owned()),
        "a PI family takes its integral gain"
    );
    assert!(
        !pi.contains(&KD.to_owned()),
        "a PI family takes no derivative gain"
    );
}

#[test]
fn the_default_family_is_the_study_s_candidate_then_its_pins_then_automatic() {
    let pins = ControllerSettingsPins {
        loop_revision: "rev-1".to_owned(),
        family: Structure::PI,
        values: BTreeMap::new(),
    };
    assert_eq!(default_family(None, None), None);
    assert_eq!(default_family(None, Some(&pins)).as_deref(), Some("PI"));
}

#[test]
fn a_loop_that_declares_its_setpoint_offers_no_setpoint_choice() {
    assert!(setpoint_choices(&pid_loop()).is_none());
}

#[test]
fn the_scenario_template_bounds_the_process_variable_over_the_validation_run() {
    let loop_definition = pid_loop();
    let process = loop_definition.signals.process_variable.clone();
    let template = scenario_template(&loop_definition, 2.0);
    assert_eq!(
        template.id, "",
        "the template has no name: the engine names the scenario when the list changes"
    );
    assert!(template.initial_values.is_empty() && template.events.is_empty());
    let [requirement] = template.requirements.as_slice() else {
        panic!("the template holds one requirement");
    };
    assert_eq!(requirement.channel, process.symbol);
    assert_eq!(requirement.window.start, 0.0);
    assert_eq!(requirement.window.end, 2.0);
    assert_eq!(
        requirement.check,
        RunCheck::Bounds {
            minimum: process.range.minimum,
            maximum: process.range.maximum,
        }
    );
}

#[test]
fn a_new_row_of_a_scenario_starts_at_the_loops_setpoint() {
    let loop_definition = pid_loop();
    let setpoint = loop_definition
        .signals
        .setpoint
        .as_ref()
        .expect("the example declares its setpoint");
    assert_eq!(
        scenario_row_signal(&loop_definition).as_deref(),
        Some(setpoint.symbol.as_str())
    );
}

#[test]
fn a_loop_without_a_setpoint_leaves_the_signal_of_a_new_row_blank() {
    let mut open_loop = pid_loop();
    open_loop.signals.setpoint = None;
    assert_eq!(scenario_row_signal(&open_loop), None);
}

#[test]
fn the_template_as_the_page_sends_it_back_is_named_by_the_engine() {
    // The page adds the template as it is (B4.3) and sends the list: the engine reads it and gives the scenario without a name its identity.
    let template = scenario_template(&pid_loop(), 2.0);
    let sent = serde_json::json!([serde_json::to_value(&template).expect("the template is JSON")]);
    let read = scenarios_of_form(&sent).expect("the template is a scenario the engine reads");
    assert_eq!(read.len(), 1);
    assert_eq!(read[0].id, "scenario-1");
    assert_eq!(read[0].requirements, template.requirements);
}
