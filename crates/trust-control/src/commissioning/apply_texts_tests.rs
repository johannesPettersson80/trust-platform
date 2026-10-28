use super::*;

#[test]
fn a_candidate_without_a_current_passed_validation_asks_for_validate() {
    // B6.7: "Candidate or validation no longer current | Validate".
    let failure = validation_not_current();
    assert_eq!(
        (failure.cause, failure.next_action),
        (Cause::Settings, NextAction::Validate)
    );
    assert!(
        failure.message.contains("no longer current"),
        "{}",
        failure.message
    );
}

#[test]
fn an_unresolved_checkpoint_names_itself_and_its_stage_and_offers_restore() {
    // A8.5, B6.4: the journal still owns the earlier Apply's operations, so Restore resolves them before a new Apply.
    let failure = checkpoint_unresolved("commissioning-1", "transfer");
    assert_eq!(
        failure.message,
        "An earlier Apply of this loop is unresolved: its checkpoint commissioning-1 stopped at the transfer stage."
    );
    assert_eq!(
        failure.detail.as_deref(),
        Some("An earlier Apply of this loop is unresolved: its checkpoint commissioning-1 stopped at the transfer stage. Restore previous settings first, so that its writes are resolved before a new Apply.")
    );
    assert_eq!(failure.next_action, NextAction::Restore);
}

#[test]
fn a_recording_the_runtime_ended_names_the_runtimes_reason_and_asks_for_the_runtime() {
    // TS control-studio-apply-process-time, "a recording the runtime ended reports the runtime's reason and asks for the runtime to be
    // connected again (A6.7)": the text holds "The runtime ended the recording: scan_reset." and the next action connects the runtime.
    let ended = recording_ended(Some("scan_reset"));
    let text = ended
        .detail
        .clone()
        .unwrap_or_else(|| ended.message.clone());
    assert_eq!(
        text,
        "The runtime ended the recording: scan_reset. Connect the runtime again before any further live action."
    );
    assert_eq!(
        (ended.cause, ended.next_action),
        (Cause::Runtime, NextAction::ConnectRuntime)
    );
    let without_reason = recording_ended(None);
    assert_eq!(without_reason.message, "The runtime ended the recording.");
    assert_eq!(recording_ended(Some("")).message, without_reason.message);
}

#[test]
fn a_status_without_the_runtime_clock_cannot_be_observed() {
    // runtimeDuration.ts, `runtimeClockNs` (A3.2): process time runs only from the runtime's own timestamps.
    let failure = runtime_clock_missing();
    assert_eq!(
        failure.message,
        "The runtime does not report its clock in the transfer status."
    );
    assert_eq!(failure.cause, Cause::Runtime);
}

#[test]
fn a_reattachment_without_an_observation_says_so() {
    // TS control-studio-apply-process-time, "reopening resumes a journalled observation": /holds no applied response to resume/.
    assert_eq!(
        no_observation_to_resume().message,
        "The commissioning journal holds no applied response to resume."
    );
}

#[test]
fn a_passed_reattachment_names_its_error_to_five_fraction_digits_in_the_process_unit() {
    // engineerActions/recovery.ts, `settlePass`: formatEngineeringNumber(error, 5) and the process unit (A11.1).
    assert_eq!(
        reattach_passed(0.001_234_56, "m"),
        "Auto commissioning passed after reopening: recorded target error 0.00123 m."
    );
    assert_eq!(
        reattach_passed(0.0, ""),
        "Auto commissioning passed after reopening: recorded target error 0."
    );
}
