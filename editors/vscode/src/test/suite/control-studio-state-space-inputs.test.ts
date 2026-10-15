import * as assert from "assert";
import { engineHoldsChange, inputsAfterChange } from "../../controlStudio/engine/routes/inputsRoute";

suite("Control Studio state-space study inputs", () => {
  test("keeps a state law's family, draft text and pins with the study's state design, not as a structure's settings", () => {
    const draft = { stateWeights: "[[400, 0], [0, 400]]", solverMode: "finite", horizonSteps: "20", terminalWeights: "[[400, 0], [0, 400]]" };
    const next = inputsAfterChange({}, {
      family: "LQR",
      tuningPreferences: { method: "state-space", values: {}, stateDesignDraft: draft },
      pinnedValues: { "P1.N": 0.1 },
    }, "revision-1");
    assert.strictEqual(next.method, "state-space");
    assert.strictEqual(next.structure, undefined, "a state family is no PID structure");
    assert.strictEqual(next.pins, undefined, "a state law's pins are not the pins of a structure");
    assert.deepStrictEqual(next.stateDesign, { family: "LQR", draft, pins: { "P1.N": 0.1 } });
  });

  test("clears a state law's pins when none are kept, and keeps the draft text as typed", () => {
    const recorded = { method: "state-space", stateDesign: { family: "LQG", draft: { desiredPoles: "[0.9" }, pins: { "P1.L1": 0.5 } } } as any;
    const next = inputsAfterChange(recorded, {
      family: "LQG",
      tuningPreferences: { method: "state-space", values: {}, stateDesignDraft: { desiredPoles: "[0.9" } },
      pinnedValues: null,
    }, "revision-1");
    assert.deepStrictEqual(next.stateDesign, { family: "LQG", draft: { desiredPoles: "[0.9" }, pins: undefined });
  });

  test("holds the state-space method, and the retired method name stays with the TypeScript route", () => {
    assert.strictEqual(engineHoldsChange({ tuningPreferences: { method: "state-space", values: {} } }), true);
    assert.strictEqual(engineHoldsChange({ tuningPreferences: { method: "state-design", values: {} } }), false);
  });
});
