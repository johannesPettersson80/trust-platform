import * as assert from "assert";

import {
  CONTROL_STUDIO_STAGE_ORDER,
  controlStudioAutomaticTuneAvailability,
  controlStudioStageAvailability,
  controlStudioStageStatusLabel,
} from "../../controlStudio/stageAvailability";
import {
  escapeHtml,
  renderAutomaticTuneButton,
  renderModeTabs,
} from "../../controlStudio/panelTemplate";

suite("Control Studio stage availability", () => {
  test("keeps every stage available for a control-studio loop", () => {
    const availability = controlStudioStageAvailability("control-studio");
    for (const stage of CONTROL_STUDIO_STAGE_ORDER) {
      assert.strictEqual(availability[stage].disabled, false, `${stage} must stay available`);
      assert.notStrictEqual(availability[stage].status, "not-in-lesson");
    }
  });

  test("keeps Automatic Tune the primary header action for a control-studio loop", () => {
    const automaticTune = controlStudioAutomaticTuneAvailability("control-studio");
    assert.strictEqual(automaticTune.primary, true);
  });

  test("disables Identify, Tune, and Validate with a visible reason for a project-scripted loop", () => {
    const availability = controlStudioStageAvailability("project-scripted");
    for (const stage of ["Identify", "Tune", "Validate"] as const) {
      assert.strictEqual(availability[stage].disabled, true, `${stage} must be disabled`);
      assert.strictEqual(availability[stage].status, "not-in-lesson");
      assert.ok(availability[stage].reason && availability[stage].reason!.length > 0,
        `${stage} must explain why it is unavailable`);
      assert.ok(availability[stage].reason!.includes("runnable project"),
        "the reason must name the runnable project as the controller owner");
    }
  });

  test("keeps Monitor and Test primary and available for a project-scripted loop", () => {
    const availability = controlStudioStageAvailability("project-scripted", { loopBound: true });
    for (const stage of ["Monitor", "Test"] as const) {
      assert.strictEqual(availability[stage].disabled, false, `${stage} must remain available`);
      assert.notStrictEqual(availability[stage].status, "not-in-lesson");
    }
  });

  test("demotes Automatic Tune to a secondary action for a project-scripted loop", () => {
    const automaticTune = controlStudioAutomaticTuneAvailability("project-scripted");
    assert.strictEqual(automaticTune.primary, false);
    assert.ok(automaticTune.reason.length > 0, "a demoted button must still explain itself");
  });

  test("derives the Specification 35 section 4.2 status vocabulary from workflow evidence", () => {
    const noEvidence = controlStudioStageAvailability("control-studio");
    assert.strictEqual(noEvidence.Identify.status, "needs-data");
    assert.strictEqual(noEvidence.Tune.status, "needs-data");
    assert.strictEqual(noEvidence.Validate.status, "needs-data");

    const withModel = controlStudioStageAvailability("control-studio", { hasModel: true });
    assert.strictEqual(withModel.Identify.status, "complete");
    assert.strictEqual(withModel.Tune.status, "ready");

    const withCandidate = controlStudioStageAvailability("control-studio", { hasModel: true, hasCandidate: true });
    assert.strictEqual(withCandidate.Tune.status, "complete");
    assert.strictEqual(withCandidate.Validate.status, "ready", "a candidate exists, so Validate is ready to run, not missing data");

    const passedValidation = controlStudioStageAvailability("control-studio",
      { hasModel: true, hasCandidate: true, validationHasResult: true, validationPassed: true });
    assert.strictEqual(passedValidation.Validate.status, "complete");

    const failedValidation = controlStudioStageAvailability("control-studio",
      { hasModel: true, hasCandidate: true, validationHasResult: true, validationPassed: false });
    assert.strictEqual(failedValidation.Validate.status, "failed");
  });

  test("every status resolves to a non-empty human-readable label", () => {
    for (const status of ["ready", "needs-data", "in-progress", "complete", "warning", "failed", "not-in-lesson"] as const) {
      assert.ok(controlStudioStageStatusLabel(status).length > 0);
    }
  });
});

suite("Control Studio panel template pieces", () => {
  test("escapeHtml escapes the five reserved HTML characters", () => {
    assert.strictEqual(escapeHtml(`<a href="x">&'</a>`), "&lt;a href=&quot;x&quot;&gt;&amp;&#39;&lt;/a&gt;");
  });

  test("renderModeTabs keeps each button's text content exactly the stage name", () => {
    const html = renderModeTabs(controlStudioStageAvailability("control-studio"));
    for (const stage of CONTROL_STUDIO_STAGE_ORDER) {
      assert.ok(html.includes(`>${stage}</button>`),
        `${stage} tab text must stay exactly the stage name so workflow.ts's textContent-based mode lookup keeps working`);
    }
  });

  test("renderModeTabs exposes a visible status word for every tab, not a bare dot", () => {
    const html = renderModeTabs(controlStudioStageAvailability("control-studio"));
    const buttons = html.match(/<button[^>]*>/g) ?? [];
    assert.strictEqual(buttons.length, 5);
    for (const button of buttons) {
      assert.match(button, /data-status="[a-z-]+"/, "every tab must declare a status word");
      assert.match(button, /aria-label="[A-Za-z]+, [^"]+"/, "every tab must expose an accessible status description");
    }
  });

  test("renderModeTabs disables the lesson-excluded stages with a visible reason", () => {
    const html = renderModeTabs(controlStudioStageAvailability("project-scripted"));
    for (const stage of ["Identify", "Tune", "Validate"]) {
      const button = html.match(new RegExp(`<button[^>]*>${stage}</button>`))?.[0] ?? "";
      assert.match(button, /\bdisabled\b/, `${stage} must be disabled for a project-scripted loop`);
      assert.match(button, /title="[^"]+"/, `${stage} must carry a visible reason`);
    }
    for (const stage of ["Monitor", "Test"]) {
      const button = html.match(new RegExp(`<button[^>]*>${stage}</button>`))?.[0] ?? "";
      assert.doesNotMatch(button, /\bdisabled\b/, `${stage} must stay enabled for a project-scripted loop`);
    }
  });

  test("renderAutomaticTuneButton renders the primary style when Automatic Tune is the prescribed path", () => {
    const html = renderAutomaticTuneButton({ disabled: false, primary: true, title: "Build a complete tuning plan." });
    assert.match(html, /class="automatic-tune"/);
    assert.doesNotMatch(html, /secondary/);
    assert.doesNotMatch(html, /\bdisabled\b/);
  });

  test("renderAutomaticTuneButton renders secondary styling and stays enabled when demoted", () => {
    const html = renderAutomaticTuneButton({ disabled: false, primary: false, title: "Secondary here." });
    assert.match(html, /class="automatic-tune secondary"/);
    assert.doesNotMatch(html, /\bdisabled\b/);
  });

  test("renderAutomaticTuneButton disables the button and keeps the reason visible", () => {
    const html = renderAutomaticTuneButton({ disabled: true, primary: true, title: "No saved plant is available." });
    assert.match(html, /\bdisabled\b/);
    assert.match(html, /title="No saved plant is available\."/);
  });
});
