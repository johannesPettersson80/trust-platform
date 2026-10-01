import * as assert from "assert";
import { controllerTuningPreferences } from "../../controlStudio/controllerTuningPreferences";
import { tuneControllerAdapter } from "../../controlStudio/controllerTuning";
import { pidLoop, channel } from "./control-studio-pid-fixtures";
import { solveDiscreteLqr } from "../../controlStudio/numerics";
import { initializeThreePageStateDesign } from "../../controlStudio/webview/threePageStateDesign";

interface StateDesignPreferences {
  desiredPoles?: readonly number[];
  stateWeights?: readonly (readonly number[])[];
  inputWeights?: readonly (readonly number[])[];
  processNoise?: readonly (readonly number[])[];
  measurementNoise?: readonly (readonly number[])[];
}

function decoder(): ((value: unknown, dimensions: { states: number; inputs: number; outputs: number }) => StateDesignPreferences | undefined) | undefined {
  try { return require("../../controlStudio/stateDesignPreferences").decodeStateDesignPreferences; } catch { return undefined; }
}

suite("Control Studio state design preferences", () => {
  test("keeps incomplete state-design input through saved acknowledgements and supports finite design fields", () => {
    const { JSDOM } = require("jsdom");
    const dom = new JSDOM('<main><section data-studio-state-design></section></main>');
    try {
      const root = dom.window.document.querySelector("main")!;
      let changes = 0;
      const editor = initializeThreePageStateDesign(root, undefined, () => { changes++; }, () => "LQR") as any;
      const mode = root.querySelector('[data-state-design-mode]');
      assert.ok(mode, "The solver policy must be selectable in the state-design form");
      mode.value = "finite"; mode.dispatchEvent(new dom.window.Event("change"));
      const steps = root.querySelector('[data-state-design-field="horizonSteps"]');
      const terminal = root.querySelector('[data-state-design-field="terminalWeights"]');
      steps.value = "2"; terminal.value = "[[1]]";
      terminal.dispatchEvent(new dom.window.Event("input"));
      assert.deepStrictEqual(editor.read(true).finiteHorizon, { steps: 2, terminalWeights: [[1]] });
      const q = root.querySelector('[data-state-design-field="stateWeights"]');
      q.value = "[["; q.dispatchEvent(new dom.window.Event("input"));
      assert.ok(changes >= 3, "Every edit retires stale candidate evidence, including incomplete input");
      const draft = editor.draft();
      assert.strictEqual(draft.stateWeights, "[[");
      const saved = controllerTuningPreferences({ method: "state-design", stateDesignDraft: draft });
      assert.ok(saved, "Incomplete input is editing state, never a numeric solver setting");
      editor.receive(undefined, (saved as any).stateDesignDraft);
      assert.strictEqual(q.value, "[[", "A host acknowledgement must not erase a pending edit");
      assert.throws(() => editor.read(true), /State weights/);
      assert.strictEqual(q.getAttribute("aria-invalid"), "true");
      q.value = "[[9]]"; q.dispatchEvent(new dom.window.Event("input"));
      assert.deepStrictEqual(editor.read(true).stateWeights, [[9]]);
      assert.strictEqual(q.hasAttribute("aria-invalid"), false);
    } finally { dom.window.close(); }
  });
  test("shows a state-design error only under the field it concerns and clears it once the design reads", () => {
    const { JSDOM } = require("jsdom");
    const dom = new JSDOM('<main><section data-studio-state-design></section></main>');
    try {
      const root = dom.window.document.querySelector("main")!;
      initializeThreePageStateDesign(root, undefined, () => undefined, () => "LQR");
      const field = (name: string) => root.querySelector(`[data-state-design-field="${name}"]`);
      const shown = () => [...root.querySelectorAll("[data-state-design-error]")].filter((error: any) => !error.hidden)
        .map((error: any) => [error.dataset.stateDesignError, error.textContent]);
      const mode = root.querySelector("[data-state-design-mode]");
      mode.value = "finite"; mode.dispatchEvent(new dom.window.Event("change"));
      for (const [name, value] of [["stateWeights", "[[400,0],[0,400]]"], ["inputWeights", "[[40000]]"], ["horizonSteps", "20"]]) {
        field(name).value = value; field(name).dispatchEvent(new dom.window.Event("input"));
      }
      assert.deepStrictEqual(shown(), [["terminalWeights", "Enter the terminal weights matrix."]], "Only the missing terminal weights are named");
      field("terminalWeights").value = "[[400,0],[0,400]]"; field("terminalWeights").dispatchEvent(new dom.window.Event("input"));
      assert.deepStrictEqual(shown(), [], "A complete design shows no error");
    } finally { dom.window.close(); }
  });

  test("shows declared Q and R defaults while preserving blank drafts as explicit edits", () => {
    const { JSDOM } = require("jsdom");
    const dom = new JSDOM('<main><section data-studio-state-design></section></main>');
    try {
      const root = dom.window.document.querySelector("main")!;
      const editor = initializeThreePageStateDesign(root, { stateWeights: [[2]], inputWeights: [[3]] }, undefined, () => "LQR") as any;
      assert.strictEqual(root.querySelector('[data-state-design-field="stateWeights"]').value, "[[2]]");
      assert.strictEqual(root.querySelector('[data-state-design-field="inputWeights"]').value, "[[3]]");
      const input = root.querySelector('[data-state-design-field="inputWeights"]'); input.value = ""; input.dispatchEvent(new dom.window.Event("input"));
      assert.strictEqual(editor.draft().inputWeights, "", "an explicit blank remains a draft and does not silently restore the declared default");
    } finally { dom.window.close(); }
  });
  test("retains finite-horizon Riccati stages and the repeated first gain policy", () => {
    const finite = (solveDiscreteLqr as any)([[1]], [[1]], [[1]], [[1]],
      { absolute: 1e-12, relative: 1e-10, conditionLimit: 1e12 }, { steps: 2, terminalWeights: [[1]] });
    assert.strictEqual(finite.status, "qualified");
    assert.ok(Math.abs(finite.gain[0][0] - 0.6) < 1e-12, "Use the finite first gain instead of the algebraic steady-state gain");
    assert.ok(finite.gainSequence.every((gain: number[][], k: number) => Math.abs(gain[0][0] - [0.6, 0.5][k]) < 1e-12));
    assert.ok(finite.riccatiSequence.every((cost: number[][], k: number) => Math.abs(cost[0][0] - [1.6, 1.5, 1][k]) < 1e-12));
    assert.strictEqual(finite.runtimePolicy, "repeat-first-finite-gain");
    assert.deepStrictEqual(controllerTuningPreferences({ method: "state-design", stateDesign: { finiteHorizon: { steps: 2, terminalWeights: [[1]] } } }),
      { method: "state-design", stateDesign: { finiteHorizon: { steps: 2, terminalWeights: [[1]] } } });
  });
  test("changes mapped LQR gains using the saved weights without changing the declared adapter", () => {
    const loop = pidLoop(); delete loop.controller.law;
    loop.controller.design = "project";
    loop.controller.stateFeedback = { family: "LQR", convention: "sampled-deviation-negative-feedback",
      gainSymbols: [["Main.Kp"]], referenceGainSymbols: [["Main.N"]], stateWeights: [[1]], inputWeights: [[1]] };
    loop.parameterBinding.parameters = [
      { symbol: "Main.Kp", iec_type: "LREAL", unit: "V/m", convention: "state feedback gain", minimum: -100, maximum: 100 },
      { symbol: "Main.N", iec_type: "LREAL", unit: "V/m", convention: "reference gain", minimum: -100, maximum: 100 },
    ];
    const model: any = { kind: "continuous-state-space", revision: "lqr-weights", source: "provided", validation: "validated", evidence: ["independent"], delaySeconds: 0,
      inputs: [channel("u", "V")], outputs: [channel("y", "m")], states: [channel("x", "m")], a: [[-1]], b: [[1]], c: [[1]], d: [[0]] };
    const task = { referenceChange: 1, maximumError: .1, settlingBand: .02, settlingTimeSeconds: 2, durationSeconds: 4 };
    const original = JSON.stringify(loop);
    const baseline = tuneControllerAdapter(loop, model, task, { family: "LQR", applicationMode: "manual" });
    const changed = tuneControllerAdapter(loop, model, task, { family: "LQR", applicationMode: "manual", stateDesign: { stateWeights: [[9]], inputWeights: [[1]] } } as any);
    assert.ok(changed.values["Main.Kp"] > baseline.values["Main.Kp"], "A stronger state-error weight changes the actual mapped controller gain");
    assert.strictEqual(JSON.stringify(loop), original);
    assert.strictEqual(changed.design.candidate.applied, false);
  });
  test("preserves a pinned finite-horizon gain while recomputing its mapped poles", () => {
    const loop = pidLoop(); delete loop.controller.law; loop.controller.design = "project";
    loop.controller.stateFeedback = { family: "LQR", convention: "sampled-deviation-negative-feedback", gainSymbols: [["Main.Kp"]], referenceGainSymbols: [["Main.N"]], stateWeights: [[1]], inputWeights: [[1]] };
    loop.parameterBinding.parameters = [
      { symbol: "Main.Kp", iec_type: "LREAL", unit: "V/m", convention: "state feedback gain", minimum: -100, maximum: 100 },
      { symbol: "Main.N", iec_type: "LREAL", unit: "V/m", convention: "reference gain", minimum: -100, maximum: 100 },
    ];
    const model: any = { kind: "continuous-state-space", revision: "lqr-pinned", source: "provided", validation: "validated", evidence: ["independent"], delaySeconds: 0,
      inputs: [channel("u", "V")], outputs: [channel("y", "m")], states: [channel("x", "m")], a: [[-1]], b: [[1]], c: [[1]], d: [[0]] };
    const task = { referenceChange: 1, maximumError: .1, settlingBand: .02, settlingTimeSeconds: 2, durationSeconds: 4 };
    const result = tuneControllerAdapter(loop, model, task, { family: "LQR", applicationMode: "manual", pinnedValues: { "Main.Kp": 7, "Main.N": 2 }, stateDesign: { finiteHorizon: { steps: 2, terminalWeights: [[1]] } } } as any);
    assert.strictEqual(result.values["Main.Kp"], 7, "finite tuning must preserve the pinned gain");
    assert.ok((result.design as { closedLoopPoles?: readonly unknown[] }).closedLoopPoles, "poles must be recomputed from the realized pinned coefficients");
  });
  test("persists state design overrides alongside the existing tuning method", () => {
    const input = { method: "automatic", values: { "P1.Kp": 1 }, stateDesign: {
      desiredPoles: [.5, .6], stateWeights: [[1, 0], [0, 1]], inputWeights: [[1]],
    } };
    const decoded = controllerTuningPreferences(input);
    assert.deepStrictEqual(decoded, input, "automatic tuning must retain state design drafts without granting runtime authority");
    assert.deepStrictEqual(decoded?.values, input.values, "existing manual values and pins remain unaffected");
  });

  test("accepts declared state design inputs in model dimensions", () => {
    const decode = decoder(); assert.strictEqual(typeof decode, "function", "state design preference decoder must exist");
    const value = decode!({ desiredPoles: [.7, .8], stateWeights: [[2, 0], [0, 1]], inputWeights: [[3]], processNoise: [[.1, 0], [0, .2]], measurementNoise: [[.3]] }, { states: 2, inputs: 1, outputs: 1 });
    assert.deepStrictEqual(value?.desiredPoles, [.7, .8]);
    assert.deepStrictEqual(value?.stateWeights, [[2, 0], [0, 1]]);
  });

  test("rejects wrong dimensions, nonsymmetric weights, and nonfinite poles", () => {
    const decode = decoder(); assert.strictEqual(typeof decode, "function");
    const dimensions = { states: 2, inputs: 1, outputs: 1 };
    assert.strictEqual(decode!({ stateWeights: [[1]] }, dimensions), undefined);
    assert.strictEqual(decode!({ stateWeights: [[1, 1], [0, 1]] }, dimensions), undefined);
    assert.strictEqual(decode!({ desiredPoles: [.7, Number.NaN] }, dimensions), undefined);
    assert.strictEqual(decode!({ inputWeights: [[0]] }, dimensions), undefined);
  });

  test("preserves omitted fields for declared defaults and rejects unknown preference keys", () => {
    const decode = decoder(); assert.strictEqual(typeof decode, "function");
    const value = decode!({ inputWeights: [[2]] }, { states: 2, inputs: 1, outputs: 1 });
    assert.deepStrictEqual(value, { inputWeights: [[2]] });
    assert.strictEqual(decode!({ inputWeights: [[2]], runtimeWrite: true }, { states: 2, inputs: 1, outputs: 1 }), undefined);
  });

  test("rejects malformed finite horizons and terminal weights before Tune", () => {
    const decode = decoder(); assert.strictEqual(typeof decode, "function");
    const dimensions = { states: 2, inputs: 1, outputs: 1 };
    assert.strictEqual(decode!({ finiteHorizon: { steps: 0, terminalWeights: [[1, 0], [0, 1]] } }, dimensions), undefined);
    assert.strictEqual(decode!({ finiteHorizon: { steps: 2, terminalWeights: [[1, 0]] } }, dimensions), undefined);
    assert.strictEqual(decode!({ finiteHorizon: { steps: 2, terminalWeights: [[1, 2], [0, 1]] } }, dimensions), undefined);
    assert.strictEqual(decode!({ finiteHorizon: { steps: 2, terminalWeights: [[-1, 0], [0, 1]] } }, dimensions), undefined);
  });
});
