import * as assert from "assert";

import {
  CONTROL_STUDIO_STAGE_ORDER,
  controlStudioAutomaticTuneAvailability,
  controlStudioStageAvailability,
  controlStudioStageStatusLabel,
} from "../../controlStudio/stageAvailability";
import {
  ControlStudioView,
  escapeHtml,
  renderAutomaticTuneButton,
  renderControlStudioHtml,
  renderModeTabs,
} from "../../controlStudio/panelTemplate";

suite("Control Studio stage availability", () => {
  test("manual parameter review is ready without a fitted model or invented candidate", () => {
    const evidence = { loopBound: true, hasManualParameters: true };
    const stages = controlStudioStageAvailability("project-scripted", evidence);
    assert.strictEqual(stages.Tune.status, "ready");
    assert.strictEqual(stages.Identify.status, "needs-data");
    assert.strictEqual(stages.Validate.status, "needs-data");
  });
  test("a provided analysis makes Tune ready without inventing an identification result", () => {
    const evidence = { loopBound: true, hasDeclaredAnalysis: true };
    const stages = controlStudioStageAvailability("project-scripted", evidence);
    assert.strictEqual(stages.Tune.status, "ready");
    assert.strictEqual(stages.Identify.status, "needs-data");
  });

  test("keeps every stage available for a control-studio loop", () => {
    const availability = controlStudioStageAvailability("control-studio");
    for (const stage of CONTROL_STUDIO_STAGE_ORDER) {
      assert.strictEqual(availability[stage].disabled, false, `${stage} must stay available`);
    }
  });

  test("keeps Automatic Tune the primary header action for a control-studio loop", () => {
    const automaticTune = controlStudioAutomaticTuneAvailability("control-studio");
    assert.strictEqual(automaticTune.primary, true);
  });

  test("keeps project-owned analysis stages reachable with missing-data status", () => {
    const availability = controlStudioStageAvailability("project-scripted");
    for (const stage of ["Identify", "Tune", "Validate"] as const) {
      assert.strictEqual(availability[stage].disabled, false);
      assert.strictEqual(availability[stage].status, "needs-data");
    }
  });

  test("keeps the approved three analysis stages available for a project-scripted loop", () => {
    const availability = controlStudioStageAvailability("project-scripted", { loopBound: true });
    for (const stage of ["Identify", "Tune", "Validate"] as const) {
      assert.strictEqual(availability[stage].disabled, false, `${stage} must remain available`);
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
    for (const status of ["ready", "needs-data", "in-progress", "complete", "warning", "failed"] as const) {
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

  test("renderModeTabs keeps every project-owned workflow tab enabled", () => {
    const html = renderModeTabs(controlStudioStageAvailability("project-scripted"));
    for (const stage of CONTROL_STUDIO_STAGE_ORDER) {
      const button = html.match(new RegExp(`<button[^>]*>${stage}</button>`))?.[0] ?? "";
      assert.ok(button);
      assert.doesNotMatch(button, /\bdisabled\b/);
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

suite("Control Studio current three-page presentation", () => {
  test("does not render the retired value strip or runtime mode tabs", () => {
    const { JSDOM } = require("jsdom");
    const view = {
      cspSource: "", nonce: "n", chartScript: "chart.js", workflowScript: "workflow.js", plant: "Heater", sourceLabel: "Simulation", runtime: "simulator",
      controllerInstance: "P1.Controller", project: "project:x", session: "s", revision: "sha256:a", loop: "loop", loopRevision: "sha256:b", processUnit: "degC", outputUnit: "%",
      experimentDefaults: { execution: "control-studio", kind: "observation", amplitude: 0, durationSeconds: 20, seed: 1 }, experimentOptions: "", pidDesign: { available: true }, tuneDescription: "", tuneDisabledReason: "", tuneStateText: "", automaticTune: { disabled: false, primary: false, title: "" }, guidedWorkflow: false,
      loopDefinition: { schemaVersion: 1, id: "loop", name: "Loop", revision: "sha256:b", projectId: "project:x", runtimeId: "simulator", controller: { instance: "P1.Controller", type: "Control.PID", revision: "sha256:a", action: "direct", taskPeriodMs: 100 }, signals: { processVariable: { symbol: "P1.PV", address: "%IL0", unit: "degC", iecType: "LREAL", direction: "read", range: { minimum: 0, maximum: 80 } }, manipulatedVariable: { symbol: "P1.MV", address: "%QL0", unit: "%", iecType: "LREAL", direction: "write", range: { minimum: 0, maximum: 100 } } }, limits: { outputMinimum: 0, outputMaximum: 100 }, observedSampleTiming: { nominalPeriodMs: 100 } },
      stageAvailability: controlStudioStageAvailability("control-studio"),
    } as unknown as ControlStudioView;
    const { renderThreePageStudioHtml } = require("../../controlStudio/threePageTemplate");
    const dom = new JSDOM(renderThreePageStudioHtml(view, (value: string) => value));
    try {
      assert.deepStrictEqual([...dom.window.document.querySelectorAll("[data-engineer-stage-tab]")].map((node: HTMLElement) => node.textContent?.trim()), ["Identify", "Tune", "Validate"]);
      assert.strictEqual(dom.window.document.querySelector(".values"), null);
      assert.strictEqual(dom.window.document.querySelector("[data-control-studio-mode]"), null);
    } finally { dom.window.close(); }
  });
});
