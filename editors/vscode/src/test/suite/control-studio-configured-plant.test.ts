import * as assert from "assert";

import { configuredPlantRequest, usesConfiguredPlant } from "../../controlStudio/configuredPlantVerification";

suite("Control Studio configured-plant routing", () => {
  const loop = (scenarios: unknown[]) => ({ verificationScenarios: scenarios }) as never;

  test("a declared scenario named like a generated case still runs on the project's plant", () => {
    const declared = loop([{ id: "load-rejection", initialValues: {}, events: [{ atMs: 0, symbol: "P1.LoadTorqueNm", value: 1.962 }],
      requirements: [{ id: "late-error" }] }]);
    assert.strictEqual(usesConfiguredPlant(declared, "load-rejection"), true,
      "the declared drum-load trial needs the plant's load input, which the supplied model does not have");
  });

  test("generated model-variation cases and declared model cases keep their model", () => {
    assert.strictEqual(usesConfiguredPlant(loop([]), "load-rejection"), false, "the generated load case disturbs the model");
    assert.strictEqual(usesConfiguredPlant(loop([{ id: "heavy-payload", model: {}, initialValues: {}, events: [],
      requirements: [{ id: "late-error" }] }]), "heavy-payload"), false, "a declared model case uses its own model");
    assert.strictEqual(usesConfiguredPlant(loop([]), "target-response"), true, "the nominal case runs on the plant");
  });

  test("validates a cascade's outer loop on its model, which contains the closed inner loop", () => {
    const request = { step_ms: 100, duration_ms: 1000, simulation_override: { model_revision: "vessel", model_provenance: "fitted", simulation_toml: "[simulation]" } };
    const outer = { cascade: { role: "outer", partnerLoopId: "loop:inner" } };
    const inner = { cascade: { role: "inner", partnerLoopId: "loop:outer", referenceHold: { modeSymbol: "P1.OuterAuto", manualValue: false, manualOutputSymbol: "P1.OuterManualC" } } };
    const route = (loop: unknown, value: typeof request) => configuredPlantRequest(loop as never, value as never) as { simulation_override?: unknown };
    assert.deepStrictEqual(route(outer, request), request, "the outer loop keeps its model");
    assert.strictEqual(usesConfiguredPlant(outer as never, "target-response"), false);
    assert.strictEqual(route(inner, request).simulation_override, undefined, "the inner loop runs on the project's plant");
  });
});
