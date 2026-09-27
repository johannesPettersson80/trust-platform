import * as assert from "assert";
import { designPidForTask } from "../../controlStudio/pidTaskDesign";
import { engineerWorkflowControls } from "../../controlStudio/engineerWorkflowPresentation";
import { initializeEngineerNavigation } from "../../controlStudio/webview/engineerNavigation";

suite("Control Studio explicit tuning method", () => {
  const model: any = { kind: "transfer-function", numerator: [1], denominator: [1, 0], delaySeconds: 0,
    revision: "integrator", source: "provided", validation: "validated", evidence: ["fixture"],
    inputs: [{ id: "u", unit: "V", operatingRange: { minimum: -20, maximum: 20 } }],
    outputs: [{ id: "y", unit: "m", operatingRange: { minimum: -20, maximum: 20 } }] };
  const task = { referenceChange: 1, maximumError: 0.1, settlingBand: 0.02, settlingTimeSeconds: 2, durationSeconds: 4 };
  test("does not replace an explicit unsupported rule with pole matching", () => {
    assert.match(designPidForTask(model, "PI", task, 20).scope, /coefficient-matched/);
    assert.throws(() => designPidForTask(model, "PI", { ...task, tuningMethod: "SIMC" }, 20), /positive effective delay/);
    assert.throws(() => designPidForTask(model, "PI", { ...task, tuningMethod: "AMIGO" }, 20), /AMIGO/);
  });
  test("defaults to Automatic and hides response preference where the rule does not use it", () => {
    const { JSDOM } = require("jsdom");
    const dom = new JSDOM(`<div data-control-studio>${engineerWorkflowControls("m", true, "V", true)}</div>`);
    const prior = new Map<string, PropertyDescriptor | undefined>();
    for (const key of ["window", "HTMLElement", "CustomEvent"]) prior.set(key, Object.getOwnPropertyDescriptor(globalThis, key));
    Object.assign(globalThis, { window: dom.window, HTMLElement: dom.window.HTMLElement, CustomEvent: dom.window.CustomEvent });
    try {
      const section = dom.window.document.querySelector("[data-engineer-workflow]");
      const method = section.querySelector("[data-engineer-tuning-method]");
      assert.strictEqual(method.value, "automatic");
      const update = initializeEngineerNavigation(section, () => {});
      section.querySelector("[data-engineer-family]").value = "PI";
      const objectiveGroup = section.querySelector("[data-engineer-tuning-objective]").closest("[data-engineer-applicability=pid]");
      for (const [choice, hidden] of [["automatic", true], ["SIMC", false], ["AMIGO", true]] as const) {
        method.value = choice;
        update({ status: "model-ready", stageEvidence: { hasModel: true } });
        section.querySelector("[data-engineer-stage-tab=tune]").click();
        assert.strictEqual(objectiveGroup.hidden, hidden, choice);
      }
    } finally {
      for (const [key, descriptor] of prior) {
        if (descriptor) Object.defineProperty(globalThis, key, descriptor); else delete (globalThis as any)[key];
      }
      dom.window.close();
    }
  });
});
