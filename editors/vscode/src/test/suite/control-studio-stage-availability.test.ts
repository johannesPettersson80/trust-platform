import * as assert from "assert";

import {
  ControlStudioView,
  escapeHtml,
} from "../../controlStudio/panelTemplate";

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
