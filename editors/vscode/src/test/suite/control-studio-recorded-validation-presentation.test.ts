import * as assert from "assert";
import { renderThreePageStudioHtml } from "../../controlStudio/threePageTemplate";
import type { ControlStudioView } from "../../controlStudio/panelTemplate";

const { JSDOM } = require("jsdom");

suite("Control Studio current three-page presentation", () => {
  test("renders the approved Identify, Tune and Validate controls", () => {
    const dom = new JSDOM(renderThreePageStudioHtml(view(), value => value));
    try {
      const document = dom.window.document;
      assert.deepStrictEqual([...document.querySelectorAll("[data-engineer-stage-tab]")].map(tab => tab.textContent?.trim()), ["Identify", "Tune", "Validate"]);
      assert.ok(document.querySelector("[data-engineer-find]"));
      assert.ok(document.querySelector("[data-engineer-tune]"));
      assert.ok(document.querySelector("[data-engineer-verify]"));
      assert.ok(document.querySelector("[data-engineer-apply]"));
      assert.strictEqual(document.querySelector("[data-workflow-pane=Test]"), null);
      assert.strictEqual(document.querySelector("[data-control-studio-mode]"), null);
    } finally { dom.window.close(); }
  });

  test("initial Validate omits empty results and keeps validation and application unavailable", () => {
    const dom = new JSDOM(renderThreePageStudioHtml(view(), value => value));
    try {
      const validate = dom.window.document.querySelector("[data-studio-page=verify]") as HTMLElement;
      assert.strictEqual((validate.querySelector("[data-studio-validation-result]") as HTMLTableElement).hidden, true);
      assert.strictEqual((validate.querySelector("[data-engineer-verify]") as HTMLButtonElement).disabled, true);
      assert.strictEqual((dom.window.document.querySelector("[data-engineer-apply]") as HTMLButtonElement).disabled, true);
      assert.strictEqual(dom.window.document.querySelector("[data-engineer-results]"), null);
    } finally { dom.window.close(); }
  });
});

function view(): ControlStudioView {
  const signal = {
    symbol: "P1.AppliedHeaterPower", address: "%QL0", iecType: "LREAL" as const,
    direction: "read" as const, unit: "%", range: { minimum: 0, maximum: 100 },
  };
  return {
    cspSource: "", nonce: "n", chartScript: "chart.js", workflowScript: "workflow.js",
    project: "project:x", session: "s",
    revision: "sha256:a", loop: "loop", loopRevision: "sha256:b",
    processUnit: "degC", outputUnit: "%",
    loopDefinition: {
      schemaVersion: 1, id: "loop", name: "Loop", revision: "sha256:b",
      projectId: "project:x", runtimeId: "simulator",
      controller: { instance: "P1.Controller", type: "Control.PID", revision: "sha256:a", action: "direct", taskPeriodMs: 100 },
      signals: {
        processVariable: { symbol: "P1.TemperatureC", address: "%IL0", unit: "degC", iecType: "LREAL", direction: "read", range: { minimum: 0, maximum: 80 } },
        manipulatedVariable: signal,
      },
      limits: { outputMinimum: 0, outputMaximum: 100 },
      observedSampleTiming: { nominalPeriodMs: 100 },
    },
  };
}
