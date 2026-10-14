//! The sentences of the step-by-step loop setup (specification 35, B1.3; A11.1), one for each question that
//! `configureLoop.ts` and `setupPromptTypes.ts` ask. The sentences are the TypeScript's, word for word, so that switching
//! the extension over (checklist V6) changes no wording. Placeholders and the chrome of the `QuickPick` and the input box
//! are presentation and stay in TypeScript.

/// The name of a phase of the step-by-step setup (`SETUP_PHASE_NAMES`, `setupPromptTypes.ts`), numbered 1 to 4.
#[must_use]
pub fn phase_name(number: u8) -> &'static str {
    match number {
        1 => "Loop and signals",
        2 => "Units and bounds",
        3 => "Controller and experiment",
        4 => "Channels and actuators",
        _ => "",
    }
}

/// A text answer that is blank once trimmed (`input`, `configureLoop.ts`).
#[must_use]
pub fn enter_value() -> &'static str {
    "Enter a value."
}

/// A number that is not finite or does not pass the question's rule (`number`, `configureLoop.ts`).
#[must_use]
pub fn invalid_number() -> &'static str {
    "Enter a valid finite number."
}

/// A rate that is not a finite number of zero or more (`configureLoopChannels.ts`, `actuatorSetup`).
#[must_use]
pub fn nonnegative_value() -> &'static str {
    "Enter a nonnegative finite value."
}

/// The hint under the rate questions: a blank answer declares no limit (`configureLoopChannels.ts`).
#[must_use]
pub fn leave_blank_for_no_limit() -> &'static str {
    "Leave blank when no limit is specified."
}

/// The loop's name question (`configureLoop.ts`).
#[must_use]
pub fn loop_name() -> &'static str {
    "Loop name"
}

/// The title of the compiled controller pick (`configureLoop.ts`).
#[must_use]
pub fn controller() -> &'static str {
    "Controller"
}

/// The last item of the controller pick (`configureLoop.ts`).
#[must_use]
pub fn custom_controller() -> &'static str {
    "Enter a custom controller"
}

/// The description of the custom controller item (`configureLoop.ts`).
#[must_use]
pub fn custom_controller_description() -> &'static str {
    "Declare its identity explicitly"
}

/// The question for a custom controller's instance (`configureLoop.ts`).
#[must_use]
pub fn controller_instance() -> &'static str {
    "Controller instance"
}

/// The question for a custom controller's type (`configureLoop.ts`).
#[must_use]
pub fn controller_type() -> &'static str {
    "Controller type"
}

/// The question for the task period when the controller has no single periodic task (`configureLoop.ts`).
#[must_use]
pub fn task_period() -> &'static str {
    "Task period (ms)"
}

/// The process variable question (`configureLoop.ts`).
#[must_use]
pub fn process_variable() -> &'static str {
    "Process variable"
}

/// The controller output question (`configureLoop.ts`).
#[must_use]
pub fn controller_output() -> &'static str {
    "Controller output"
}

/// The setpoint question (`configureLoop.ts`).
#[must_use]
pub fn setpoint() -> &'static str {
    "Setpoint"
}

/// The first setpoint item: the loop has no reference (`configureLoop.ts`).
#[must_use]
pub fn no_setpoint() -> &'static str {
    "No setpoint (open loop)"
}

/// The workflow question (`configureLoop.ts`, `selectWorkflow`).
#[must_use]
pub fn control_workflow() -> &'static str {
    "Control workflow"
}

/// The observation workflow item (`selectWorkflow`).
#[must_use]
pub fn observation() -> &'static str {
    "Observation"
}

/// The observation workflow's description (`selectWorkflow`).
#[must_use]
pub fn observation_description() -> &'static str {
    "Record the existing program without declaring its control law"
}

/// The adapter workflow item (`selectWorkflow`).
#[must_use]
pub fn adapter_workflow() -> &'static str {
    "Use an existing controller adapter"
}

/// The adapter workflow's description (`selectWorkflow`).
#[must_use]
pub fn adapter_workflow_description() -> &'static str {
    "Import a typed state, industrial or predictive controller mapping"
}

/// The ON/OFF workflow item (`selectWorkflow`).
#[must_use]
pub fn on_off_workflow() -> &'static str {
    "Tune existing ON/OFF controller"
}

/// The ON/OFF workflow's description (`selectWorkflow`).
#[must_use]
pub fn on_off_workflow_description() -> &'static str {
    "Bind hysteresis, dwell and retained switching states"
}

/// The proportional workflow item (`selectWorkflow`).
#[must_use]
pub fn proportional_workflow() -> &'static str {
    "Tune existing proportional controller"
}

/// The proportional workflow's description (`selectWorkflow`).
#[must_use]
pub fn proportional_workflow_description() -> &'static str {
    "Confirm u = Kp * (SP - PV) and bind its existing parameters"
}

/// The process unit question (`configureLoop.ts`, phase 2).
#[must_use]
pub fn process_unit() -> &'static str {
    "Process unit"
}

/// The process minimum question (`configureLoop.ts`, phase 2).
#[must_use]
pub fn process_minimum() -> &'static str {
    "Process minimum"
}

/// The process maximum question (`configureLoop.ts`, phase 2).
#[must_use]
pub fn process_maximum() -> &'static str {
    "Process maximum"
}

/// The output unit question (`configureLoop.ts`, phase 2).
#[must_use]
pub fn output_unit() -> &'static str {
    "Output unit"
}

/// The output minimum question (`configureLoop.ts`, phase 2).
#[must_use]
pub fn output_minimum() -> &'static str {
    "Output minimum"
}

/// The output maximum question (`configureLoop.ts`, phase 2).
#[must_use]
pub fn output_maximum() -> &'static str {
    "Output maximum"
}

/// The proportional gain question (`proportionalSetup`, `configureLoop.ts`).
#[must_use]
pub fn proportional_gain() -> &'static str {
    "Proportional gain"
}

/// The gain minimum question (`proportionalSetup`).
#[must_use]
pub fn gain_minimum() -> &'static str {
    "Gain minimum"
}

/// The gain maximum question (`proportionalSetup`).
#[must_use]
pub fn gain_maximum() -> &'static str {
    "Gain maximum"
}

/// The manual command question (`proportionalSetup`).
#[must_use]
pub fn manual_command() -> &'static str {
    "Manual command"
}

/// The manual mode question (`proportionalSetup`).
#[must_use]
pub fn manual_mode() -> &'static str {
    "Manual mode"
}

/// The manual mode polarity question (`proportionalSetup`).
#[must_use]
pub fn manual_mode_polarity() -> &'static str {
    "Manual mode polarity"
}

/// The polarity item whose TRUE value is Manual (`proportionalSetup`).
#[must_use]
pub fn true_means_manual() -> &'static str {
    "TRUE means Manual"
}

/// The polarity item whose TRUE value is Auto (`proportionalSetup`).
#[must_use]
pub fn true_means_auto() -> &'static str {
    "TRUE means Auto"
}

/// The controller task question, asked when the controller has no single periodic task (`proportionalSetup`).
#[must_use]
pub fn controller_task() -> &'static str {
    "Controller task"
}

/// The observation duration question (`configureLoop.ts`; C12).
#[must_use]
pub fn observation_duration() -> &'static str {
    "Observation duration (seconds)"
}

/// The title of the adapter mapping file dialog (`configureControllerAdapter.ts`).
#[must_use]
pub fn adapter_mapping() -> &'static str {
    "Existing controller adapter mapping"
}

/// The channel mode question (`configureLoopChannels.ts`).
#[must_use]
pub fn control_channels() -> &'static str {
    "Control channels"
}

/// The single channel item (`configureLoopChannels.ts`).
#[must_use]
pub fn single_channel() -> &'static str {
    "Single channel"
}

/// The single channel item's description (`configureLoopChannels.ts`).
#[must_use]
pub fn single_channel_description() -> &'static str {
    "Use the selected PV, MV and setpoint"
}

/// The multiple channel item (`configureLoopChannels.ts`).
#[must_use]
pub fn multiple_channels() -> &'static str {
    "Multiple channels"
}

/// The multiple channel item's description (`configureLoopChannels.ts`).
#[must_use]
pub fn multiple_channels_description() -> &'static str {
    "Bind ordered model inputs, outputs and per-channel requirements"
}

/// The actuator capability question (`actuatorSetup`).
#[must_use]
pub fn actuator_capability() -> &'static str {
    "Actuator capability"
}

/// The continuous output item (`actuatorSetup`).
#[must_use]
pub fn continuous_output() -> &'static str {
    "Continuous output"
}

/// The ON/OFF output item (`actuatorSetup`).
#[must_use]
pub fn on_off_output() -> &'static str {
    "ON/OFF output"
}

/// The rising rate question (`actuatorSetup`).
#[must_use]
pub fn rising_rate() -> &'static str {
    "Maximum rising output rate (output units/s)"
}

/// The falling rate question (`actuatorSetup`).
#[must_use]
pub fn falling_rate() -> &'static str {
    "Maximum falling output rate (output units/s)"
}

/// The applied-actuator feedback question for the output `output_symbol` (`configureLoopChannels.ts`).
#[must_use]
pub fn applied_feedback(output_symbol: &str) -> String {
    format!("{output_symbol}: applied actuator feedback")
}

/// The item that declares no separate feedback (`configureLoopChannels.ts`).
#[must_use]
pub fn use_controller_output() -> &'static str {
    "Use the controller output"
}

/// The description of the item that declares no separate feedback (`configureLoopChannels.ts`).
#[must_use]
pub fn use_controller_output_description() -> &'static str {
    "No separate applied-action feedback is declared"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_phases_carry_the_names_of_setup_phase_names() {
        assert_eq!(phase_name(1), "Loop and signals");
        assert_eq!(phase_name(2), "Units and bounds");
        assert_eq!(phase_name(3), "Controller and experiment");
        assert_eq!(phase_name(4), "Channels and actuators");
        assert_eq!(phase_name(5), "");
    }

    #[test]
    fn the_applied_feedback_question_names_its_output() {
        assert_eq!(
            applied_feedback("PumpStation.Power"),
            "PumpStation.Power: applied actuator feedback"
        );
    }
}
