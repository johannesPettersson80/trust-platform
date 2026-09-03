import * as assert from "assert";

suite("Control Studio failure and plan presentation", () => {
  test("maps every identified model structure to a human label", () => {
    const api = failureMessagesApi();
    assert.strictEqual(api.modelKindLabel("static_gain"), "Static gain");
    assert.strictEqual(api.modelKindLabel("integrating"), "Integrating");
    assert.strictEqual(api.modelKindLabel("first_order"), "First order");
    assert.strictEqual(api.modelKindLabel("first_order_dead_time"), "First order with dead time");
    assert.strictEqual(api.modelKindLabel("second_order"), "Second order");
    assert.strictEqual(api.modelKindLabel("second_order_dead_time"), "Second order with dead time");
    assert.strictEqual(api.modelKindLabel("mystery_model"), "Unrecognized model structure (mystery_model)");
  });

  test("maps every terminal experiment status to a human label", () => {
    const api = failureMessagesApi();
    assert.strictEqual(api.experimentStatusLabel("completed"), "Completed");
    assert.strictEqual(api.experimentStatusLabel("aborted"), "Aborted");
    assert.strictEqual(api.experimentStatusLabel("failed"), "Failed");
    assert.strictEqual(api.experimentStatusLabel("failed_to_restore"), "Failed to restore");
    assert.strictEqual(api.experimentStatusLabel("runtime_disconnect"), "Stopped (runtime_disconnect)");
  });

  test("maps every known runtime stop/restoration reason code to a sentence", () => {
    const api = failureMessagesApi();
    assert.strictEqual(api.experimentReasonSentence("user_abort"), "you selected Abort test");
    assert.strictEqual(api.experimentReasonSentence("duration_complete"), "the reviewed maximum duration was reached");
    assert.strictEqual(api.experimentReasonSentence("signal_forced"), "the affected signal became forced on the runtime");
    assert.strictEqual(api.experimentReasonSentence("stale_runtime_data"), "runtime data stopped updating");
    assert.strictEqual(api.experimentReasonSentence("bad_runtime_data"), "the runtime reported a non-numeric or invalid value");
    assert.strictEqual(api.experimentReasonSentence("signal_outside_limits"), "the signal moved outside its reviewed limits");
    assert.strictEqual(api.experimentReasonSentence("relay_feedback_forced"), "the relay feedback signal became forced");
    assert.strictEqual(api.experimentReasonSentence("bad_relay_feedback"), "the relay feedback signal became invalid");
    assert.strictEqual(api.experimentReasonSentence("confirmation_timeout"), "the runtime did not confirm the restored value in time");
    // An unrecognized wire value must still read as a sentence, never a bare raw identifier.
    assert.strictEqual(api.experimentReasonSentence("runtime_disconnect"), 'the runtime reported "runtime_disconnect"');
  });

  test("maps every restoration outcome to a sentence with the required recovery action", () => {
    const api = failureMessagesApi();
    assert.strictEqual(api.experimentRestorationMessage("restored"),
      "The affected signal was confirmed restored to its original value.");
    assert.strictEqual(api.experimentRestorationMessage("not_required"),
      "No write was made, so no restoration was required.");
    assert.strictEqual(api.experimentRestorationMessage("pending"),
      "Restoration is still being confirmed against the runtime.");
    assert.strictEqual(api.experimentRestorationMessage("failed"),
      "Restoration failed. Check the affected output on the runtime and restore it manually before starting another test.");
    assert.strictEqual(api.experimentRestorationMessage("failed", "confirmation_timeout"),
      "Restoration failed — the runtime did not confirm the restored value in time. Check the affected output on the runtime and restore it manually before starting another test.");
    // An unknown restoration status must fail safe as "failed", not silently disappear.
    assert.strictEqual(api.experimentRestorationMessage("mystery"),
      "Restoration failed. Check the affected output on the runtime and restore it manually before starting another test.");
  });

  test("turns the exact reviewed failure state into one sentence with what happened and what to do", () => {
    const api = failureMessagesApi();
    // Reproduces the review's evidence: "failed · runtime_disconnect · restoration failed"
    // with no next action; the fix must name what happened and the recovery step.
    const message = api.experimentTerminalMessage({
      status: "failed", stopReason: "runtime_disconnect", restoration: "failed",
    });
    assert.strictEqual(message,
      'Failed — the runtime reported "runtime_disconnect". Restoration failed. Check the affected output on the runtime and restore it manually before starting another test.');
    assert.ok(!message.includes("· "), "the raw enum-joined format must be gone");

    const known = api.experimentTerminalMessage({
      status: "failed", stopReason: "stale_runtime_data", restoration: "failed", restorationReason: "confirmation_timeout",
    });
    assert.strictEqual(known,
      "Failed — runtime data stopped updating. Restoration failed — the runtime did not confirm the restored value in time. Check the affected output on the runtime and restore it manually before starting another test.");
  });

  test("describes the restoring phase as a sentence, not a raw status/reason pair", () => {
    const api = failureMessagesApi();
    assert.strictEqual(
      api.experimentRestoringMessage("aborted", "user_abort"),
      "Aborted — you selected Abort test. Confirming restoration with the runtime…"
    );
  });

  test("clarifies a stale candidate without discarding its values", () => {
    const api = failureMessagesApi();
    assert.strictEqual(
      api.staleCandidateNotice("Tune", "No acceptable controller found."),
      "This candidate is from an earlier attempt. The latest Tune attempt failed: No acceptable controller found."
    );
    assert.strictEqual(
      api.staleCandidateNotice("Automatic Tune", "Identify failed."),
      "This candidate is from an earlier attempt. The latest Automatic Tune attempt failed: Identify failed."
    );
  });

  test("binds the Test plan card to the exact form that was reviewed", () => {
    const api = planPresentationApi();
    const reviewed = { kind: "observation", amplitude: 0, durationSeconds: 30 };
    assert.strictEqual(api.experimentFormMatchesReviewedPlan(reviewed, reviewed), true);
    assert.strictEqual(api.experimentFormMatchesReviewedPlan({ ...reviewed }, { ...reviewed }), true);
    // The exact scenario from the review: form shows "Observe without excitation ·
    // Amplitude 5 · 30 s" while a stale "Setpoint · 5 -> 7 degC · 10 s" plan is displayed.
    const changedKind = { kind: "setpoint_step", amplitude: 5, durationSeconds: 10 };
    assert.strictEqual(api.experimentFormMatchesReviewedPlan(changedKind, reviewed), false);
    const changedAmplitude = { ...reviewed, amplitude: 5 };
    assert.strictEqual(api.experimentFormMatchesReviewedPlan(changedAmplitude, reviewed), false);
    const changedDuration = { ...reviewed, durationSeconds: 45 };
    assert.strictEqual(api.experimentFormMatchesReviewedPlan(changedDuration, reviewed), false);
    // No reviewed plan yet: nothing can match, so approval must stay blocked.
    assert.strictEqual(api.experimentFormMatchesReviewedPlan(reviewed, undefined), false);
  });

  test("gives the physical authorizing action a distinct treatment from the ordinary primary action", () => {
    const api = planPresentationApi();
    assert.strictEqual(api.experimentApprovalButtonClass("simulation"), "primary");
    assert.strictEqual(api.experimentApprovalButtonClass("physical"), "danger");
  });

  test("formats a saved-result timestamp as local time without milliseconds, keeping the ISO value", () => {
    const api = timestampFormatApi();
    const iso = "2026-09-01T05:13:01.387Z";
    const formatted = api.formatLocalTimestamp(iso);
    assert.strictEqual(formatted.title, iso);
    assert.ok(!formatted.display.includes("."), "milliseconds must not be shown");
    assert.ok(!formatted.display.includes("T"), "the raw ISO separator must not be shown");
    assert.ok(!formatted.display.includes("Z"), "the raw ISO UTC marker must not be shown");
    // The display must be a local reading of the same instant: recompute it the same
    // way the implementation does so this assertion is timezone-independent.
    const date = new Date(iso);
    const pad = (value: number) => String(value).padStart(2, "0");
    const expected = `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ` +
      `${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`;
    assert.strictEqual(formatted.display, expected);
  });

  test("falls back to the raw text for an unparsable timestamp", () => {
    const api = timestampFormatApi();
    const formatted = api.formatLocalTimestamp("not-a-timestamp");
    assert.strictEqual(formatted.display, "not-a-timestamp");
    assert.strictEqual(formatted.title, "not-a-timestamp");
  });
});

function failureMessagesApi(): any {
  try { return require("../../controlStudio/failureMessages"); } catch { return {}; }
}

function planPresentationApi(): any {
  try { return require("../../controlStudio/planPresentation"); } catch { return {}; }
}

function timestampFormatApi(): any {
  try { return require("../../controlStudio/timestampFormat"); } catch { return {}; }
}
