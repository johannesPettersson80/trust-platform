//! The channel step of the step-by-step loop setup (specification 35, B1.4 and C0 "Missing adapters"; `configureLoopChannels.ts`).
//!
//! Channels are parked for checklist V7, so only the rules that need no channel model are ported here: a single channel's
//! rate limits (`Limits`) and its applied-action feedback (`Signals.actuator`). Multiple channels and the ON/OFF actuator,
//! with its dwell times, need the channel model and are refused by name. The `channels` object that the extension writes
//! is not written here: the loop keeps it unset until V7 ports the channel model.

use crate::failure::{Cause, Failure, NextAction};
use crate::loop_definition::{Direction, LoopDefinition, LoopSignal};
use crate::setup::prompts::{
    claimed_symbols, is_applied_feedback_candidate, js_number, js_trim, ActuatorChoice,
    ChannelMode, FeedbackChoice, RateAnswer, SetupStepAnswers,
};
use crate::setup::texts;

/// The answer to a rate question (`actuatorSetup`, `configureLoopChannels.ts`): blank declares no limit, and any other
/// answer must be a finite number of zero or more.
///
/// # Errors
/// The answer is neither blank nor a nonnegative finite number.
pub fn accept_rate(answer: &str) -> Result<RateAnswer, String> {
    if js_trim(answer).is_empty() {
        return Ok(RateAnswer::Unlimited);
    }
    js_number(answer)
        .filter(|rate| *rate >= 0.0)
        .map(RateAnswer::PerSecond)
        .ok_or_else(|| texts::prompts::nonnegative_value().to_owned())
}

/// Sets up the single channel of a loop (`configureLoopChannels.ts`, single channel): the continuous actuator's rate
/// limits, the applied-action feedback, and an empty list of further signals (a single channel adds none).
///
/// # Errors
/// The channel is multiple, or its actuator is ON/OFF (both parked for V7); an answer is missing; or the chosen feedback
/// cannot report the applied action.
pub fn apply_channels(
    loop_definition: &mut LoopDefinition,
    answers: &SetupStepAnswers,
) -> Result<(), Failure> {
    match answers.channel_mode {
        Some(ChannelMode::Single) => apply_single(loop_definition, answers),
        Some(ChannelMode::Multiple) => {
            Err(parked(texts::configure::multiple_channels_not_available()))
        }
        None => Err(unanswered(texts::prompts::control_channels())),
    }
}

fn apply_single(
    loop_definition: &mut LoopDefinition,
    answers: &SetupStepAnswers,
) -> Result<(), Failure> {
    // `actuatorSetup(…, switching)`: a BOOL manipulated variable is an ON/OFF actuator, which the TypeScript forces.
    if loop_definition.signals.manipulated_variable.iec_type == "BOOL" {
        return Err(parked(texts::configure::on_off_actuator_not_available()));
    }
    match answers.actuator {
        Some(ActuatorChoice::Continuous) => {}
        Some(ActuatorChoice::OnOff) => {
            return Err(parked(texts::configure::on_off_actuator_not_available()));
        }
        None => return Err(unanswered(texts::prompts::actuator_capability())),
    }
    let rising = answers
        .rising_rate
        .ok_or_else(|| unanswered(texts::prompts::rising_rate()))?;
    let falling = answers
        .falling_rate
        .ok_or_else(|| unanswered(texts::prompts::falling_rate()))?;
    loop_definition.limits.rate_up_per_s = per_second(rising);
    loop_definition.limits.rate_down_per_s = per_second(falling);
    apply_feedback(loop_definition, answers)?;
    loop_definition.signals.additional = Some(Vec::new());
    Ok(())
}

/// The rate a limit declares: none for a blank answer.
fn per_second(answer: RateAnswer) -> Option<f64> {
    match answer {
        RateAnswer::Unlimited => None,
        RateAnswer::PerSecond(rate) => Some(rate),
    }
}

/// The applied-action feedback: the controller output itself, or an input that reports the applied action in the output's
/// unit over the output's range (`configureLoopChannels.ts`, `binding` with direction read).
fn apply_feedback(
    loop_definition: &mut LoopDefinition,
    answers: &SetupStepAnswers,
) -> Result<(), Failure> {
    let output_symbol = loop_definition.signals.manipulated_variable.symbol.clone();
    let feedback = answers
        .feedback
        .as_ref()
        .ok_or_else(|| unanswered(&texts::prompts::applied_feedback(&output_symbol)))?;
    let FeedbackChoice::Signal(signal) = feedback else {
        return Ok(());
    };
    if !is_applied_feedback_candidate(signal, &claimed_symbols(answers)) {
        return Err(Failure::new(
            &texts::configure::not_applied_feedback(&signal.symbol),
            Cause::Settings,
            NextAction::OpenLoopSetup,
        ));
    }
    let unit = loop_definition.signals.manipulated_variable.unit.clone();
    let range = loop_definition.signals.manipulated_variable.range;
    loop_definition.signals.actuator = Some(LoopSignal {
        symbol: signal.symbol.clone(),
        address: Some(signal.address.clone()),
        iec_type: signal.iec_type.clone(),
        direction: Direction::Read,
        unit,
        range,
    });
    Ok(())
}

/// A refusal of a part that the channel model, parked for V7, would carry. The cause is the same as the parked features of
/// the loop contract (`LoopDefinition::rust_engine_support`).
fn parked(sentence: &str) -> Failure {
    Failure::new(sentence, Cause::ControllerCapability, NextAction::None)
}

fn unanswered(title: &str) -> Failure {
    Failure::new(
        &texts::configure::unanswered(title),
        Cause::Settings,
        NextAction::OpenLoopSetup,
    )
}

#[cfg(test)]
#[path = "configure_channels_tests.rs"]
mod tests;
