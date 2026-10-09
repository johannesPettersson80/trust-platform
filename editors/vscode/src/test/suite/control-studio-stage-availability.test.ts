import * as assert from "assert";

import {
  CONTROL_STUDIO_STAGE_ORDER,
  controlStudioStageAvailability,
} from "../../controlStudio/stageAvailability";
import {
  ControlStudioView,
  escapeHtml,
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
});

suite("Control Studio panel template pieces", () => {
  test("escapeHtml escapes the five reserved HTML characters", () => {
    assert.strictEqual(escapeHtml(`<a href="x">&'</a>`), "&lt;a href=&quot;x&quot;&gt;&amp;&#39;&lt;/a&gt;");
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
