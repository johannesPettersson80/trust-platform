import * as assert from "assert";

import { configuredPlantRequest, usesConfiguredPlant, type VerificationPlant } from "../../controlStudio/configuredPlantVerification";

/** B5.3: the configured plant is the plant the isolated checks run against when the target is simulated and it drives the loop. */
const CONFIGURED: VerificationPlant = { kind: "configured", plantId: "P1" };
const MODEL: VerificationPlant = { kind: "model" };

suite("Control Studio configured-plant routing", () => {
  const loop = (scenarios: unknown[]) => ({ verificationScenarios: scenarios }) as never;

  test("a declared scenario named like a generated case still runs on the project's plant", () => {
    const declared = loop([{ id: "load-rejection", initialValues: {}, events: [{ atMs: 0, symbol: "P1.LoadTorqueNm", value: 1.962 }],
      requirements: [{ id: "late-error" }] }]);
    assert.strictEqual(usesConfiguredPlant(declared, "load-rejection", CONFIGURED), true,
      "the declared drum-load trial needs the plant's load input, which the supplied model does not have");
  });

  test("generated model-variation cases and declared model cases keep their model", () => {
    assert.strictEqual(usesConfiguredPlant(loop([]), "load-rejection", CONFIGURED), false, "the generated load case disturbs the model");
    assert.strictEqual(usesConfiguredPlant(loop([{ id: "heavy-payload", model: {}, initialValues: {}, events: [],
      requirements: [{ id: "late-error" }] }]), "heavy-payload", CONFIGURED), false, "a declared model case uses its own model");
    assert.strictEqual(usesConfiguredPlant(loop([]), "target-response", CONFIGURED), true, "the nominal case runs on the plant");
  });

  test("a model choice runs every scenario on its model (B5.3: the target is physical or the plant does not drive the loop)", () => {
    assert.strictEqual(usesConfiguredPlant(loop([]), "target-response", MODEL), false, "the nominal case keeps the qualified model");
    const request = { step_ms: 100, duration_ms: 1000, simulation_override: { model_revision: "vessel", model_provenance: "fitted", simulation_toml: "[simulation]" } };
    assert.deepStrictEqual(configuredPlantRequest(loop([]), request as never, MODEL), request, "the model request is sent unchanged");
  });

  test("validates a cascade's outer loop on its model, which contains the closed inner loop", () => {
    const request = { step_ms: 100, duration_ms: 1000, simulation_override: { model_revision: "vessel", model_provenance: "fitted", simulation_toml: "[simulation]" } };
    const outer = { cascade: { role: "outer", partnerLoopId: "loop:inner" } };
    const inner = { cascade: { role: "inner", partnerLoopId: "loop:outer", referenceHold: { modeSymbol: "P1.OuterAuto", manualValue: false, manualOutputSymbol: "P1.OuterManualC" } } };
    const route = (loop: unknown, value: typeof request, plant: VerificationPlant) => configuredPlantRequest(loop as never, value as never, plant) as { simulation_override?: unknown };
    assert.deepStrictEqual(route(outer, request, CONFIGURED), request, "the outer loop keeps its model");
    assert.strictEqual(usesConfiguredPlant(outer as never, "target-response", CONFIGURED), false);
    assert.strictEqual(route(inner, request, CONFIGURED).simulation_override, undefined, "the inner loop runs on the project's plant");
    assert.ok(route(inner, request, MODEL).simulation_override, "a model choice keeps the inner loop's model");
  });
});
