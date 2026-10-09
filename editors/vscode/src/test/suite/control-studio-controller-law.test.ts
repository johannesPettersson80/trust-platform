import { AutomaticControllerSelectionError } from "../../controlStudio/automaticControllerRecommendation";
import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import { validateControlLoopDefinition } from "../../controlStudio/loopDefinition";
import { proportionalParameter } from "../../controlStudio/engineerVerificationPlan";
import { calculateDeclaredRouth } from "../../controlStudio/routhHurwitz";

function loop(): any {
  const value = JSON.parse(fs.readFileSync(path.resolve(__dirname, "../../../src/test/fixtures/control-studio-engineer/control-loop.json"), "utf8"));
  delete value.analysis; delete value.analysisParameterSymbols;
  value.controller.law = { kind: "proportional", gainSymbol: "Main.Kp" };
  return value;
}
suite("Control Studio model-free controller law", () => {
  test("makes the same automatic choice for equivalent transfer and state-space models", () => {
    const { recommendAutomaticController } = require("../../controlStudio/automaticControllerRecommendation");
    const bound = JSON.parse(fs.readFileSync(path.resolve(__dirname, "../../../src/test/fixtures/control-studio-pid/control-loop.json"), "utf8"));
    for (const p of bound.parameterBinding.parameters) {
      if (p.symbol === bound.controller.law.integralGainSymbol) { p.minimum = 0; p.maximum = 0; }
      if (p.symbol === bound.controller.law.proportionalGainSymbol) p.maximum = 2;
    }
    const channel = (id: string, unit: string) => ({ id, unit, operatingRange: { minimum: -20, maximum: 20 } });
    const model = { kind: "transfer-function", numerator: [1], denominator: [1, 1], delaySeconds: 0,
      revision: "same-model", source: "provided", validation: "validated", evidence: ["record"],
      inputs: [channel("u", "V")], outputs: [channel("y", "m")] };
    const stateSpace = { ...model, kind: "continuous-state-space", states: [channel("x", "m")],
      a: [[-1]], b: [[2]], c: [[0.5]], d: [[0]] };
    const task = { referenceChange: 1, maximumError: 0.5, settlingBand: 0.05, settlingTimeSeconds: 2, durationSeconds: 4 };
    const expected = recommendAutomaticController(bound, model, task, "manual");
    assert.strictEqual(expected.family, "P");
    let actual: any;
    assert.doesNotThrow(() => { actual = recommendAutomaticController(bound, stateSpace, task, "manual"); });
    assert.strictEqual(actual.family, expected.family);
    assert.deepStrictEqual(actual.values, expected.values);
    assert.strictEqual(actual.design.actualSTVerify, true);
  });

  test("shares state-space dynamics across PI tuning, manual P and Routh analysis", () => {
    const channel = (id: string, unit: string) => ({ id, unit, operatingRange: { minimum: -20, maximum: 20 } });
    const model = { kind: "continuous-state-space", a: [[-1]], b: [[2]], c: [[0.5]], d: [[0]],
      states: [channel("x", "m")], revision: "state-space-loop", source: "provided", validation: "validated", evidence: ["record"],
      inputs: [channel("u", "V")], outputs: [channel("y", "m")], delaySeconds: 0 };
    const { designTypedPiCandidate } = require("../../controlStudio/pidPiDesign");
    let pi: any;
    assert.doesNotThrow(() => { pi = designTypedPiCandidate(model, { method: "SIMC", objective: "balanced", action: "direct" }); });
    assert.strictEqual(pi.candidate.proportionalGain, 1.5);
    const { proportionalLoopAnalysis } = require("../../controlStudio/controllerLaw");
    const routh = calculateDeclaredRouth(proportionalLoopAnalysis(loop(), model, 3), 3);
    assert.deepStrictEqual(routh.polynomial.coefficients, [1, 4]);
    const { designManualProportionalCandidate } = require("../../controlStudio/manualProportionalDesign");
    const manual = designManualProportionalCandidate({ ...model, a: [[0]] }, 2,
      { referenceChange: 1, constantInputDisturbance: 0, maximumSteadyError: 0.1,
        settlingBand: 0.02, settlingTimeSeconds: 2, proportionalGainBounds: { minimum: -10, maximum: 10 } });
    assert.strictEqual(manual.prediction.closedLoopPole, -2);
    assert.strictEqual(manual.selectedModel.revision, model.revision);
    assert.strictEqual(model.kind, "continuous-state-space");
  });

  test("binds an explicit proportional law before any plant model is known", () => {
    const value = loop();
    assert.strictEqual(validateControlLoopDefinition(value, { projectId: value.projectId, runtimeId: value.runtimeId }).kind, "accepted");
    assert.doesNotThrow(() => proportionalParameter(value), "An explicit controller law must not require invented plant coefficients");
    assert.strictEqual(proportionalParameter(value).symbol, "Main.Kp");
    assert.strictEqual(value.analysis, undefined);
  });
  test("rejects an unknown or unbound controller law", () => {
    for (const law of [{ kind: "guessed", gainSymbol: "Main.Kp" }, { kind: "proportional", gainSymbol: "NotBound" }]) {
      const value = loop(); value.controller.law = law;
      assert.strictEqual(validateControlLoopDefinition(value, { projectId: value.projectId, runtimeId: value.runtimeId }).kind, "rejected",
        "Unsupported or unbound controller semantics cannot enter automatic design");
    }
  });
  test("accepts only an explicit PID record adapter with unique bound gain fields", () => {
    const value = loop();
    value.controller = { ...value.controller, type: "Control.PID", design: "pid" };
    value.parameterBinding = {
      ...value.parameterBinding,
      parameters: [
        { symbol: "Main.Kp", iec_type: "LREAL", unit: "V/m", convention: "proportional gain", minimum: 0, maximum: 100 },
        { symbol: "Main.Ki", iec_type: "LREAL", unit: "V/(m*s)", convention: "integral gain", minimum: 0, maximum: 100 },
        { symbol: "Main.Kd", iec_type: "LREAL", unit: "V*s/m", convention: "derivative gain", minimum: 0, maximum: 100 },
      ],
    };
    value.controller.law = {
      kind: "pid", parameterRecordSymbol: "Main.Parameters", parameterRevisionSymbol: "Main.ParameterRevision",
      proportionalGainSymbol: "Main.Kp", integralGainSymbol: "Main.Ki", derivativeGainSymbol: "Main.Kd",
    };
    const adapter: any = require("../../controlStudio/controllerLaw");
    assert.strictEqual(typeof adapter.decodePidLaw, "function", "PID execution needs a typed parameter-record adapter");
    assert.strictEqual(validateControlLoopDefinition(value, { projectId: value.projectId, runtimeId: value.runtimeId }).kind, "accepted");
    assert.deepStrictEqual(adapter.decodePidLaw(value.controller.law, value.parameterBinding), value.controller.law);

    value.controller.law = { ...value.controller.law, derivativeGainSymbol: "Main.Ki" };
    assert.strictEqual(validateControlLoopDefinition(value, { projectId: value.projectId, runtimeId: value.runtimeId }).kind, "rejected",
      "one compiled parameter cannot stand in for two PID terms");
  });
  test("materializes an exact PID gain record without changing unmapped observed values", () => {
    const value = loop();
    value.controller = { ...value.controller, type: "Control.PID", design: "pid" };
    value.parameterBinding = { ...value.parameterBinding, parameters: [
      { symbol: "Main.Kp", iec_type: "LREAL", unit: "V/m", convention: "proportional gain", minimum: 0, maximum: 100 },
      { symbol: "Main.Ki", iec_type: "LREAL", unit: "V/(m*s)", convention: "integral gain", minimum: 0, maximum: 100 },
      { symbol: "Main.Kd", iec_type: "LREAL", unit: "V*s/m", convention: "derivative gain", minimum: 0, maximum: 100 },
    ] };
    value.controller.law = { kind: "pid", parameterRecordSymbol: "Main.Parameters", parameterRevisionSymbol: "Main.ParameterRevision",
      proportionalGainSymbol: "Main.Kp", integralGainSymbol: "Main.Ki", derivativeGainSymbol: "Main.Kd" };
    const adapter: any = require("../../controlStudio/controllerLaw");
    assert.strictEqual(typeof adapter.materializePidGainRecord, "function",
      "Tune, Verify and Apply must share one typed PID-record materializer");
    assert.deepStrictEqual(adapter.materializePidGainRecord(value, {
      proportionalGain: 12, integralGain: 3, derivativeGain: 0.5,
    }, { "Main.Kp": 1, "Main.Ki": 0, "Main.Kd": 0, "Main.Unrelated": 7 }), {
      "Main.Kp": 12, "Main.Ki": 3, "Main.Kd": 0.5, "Main.Unrelated": 7,
    });
  });
  test("maps a back-calculation gain only through an explicit typed anti-windup adapter", () => {
    const value = loop();
    value.controller = { ...value.controller, type: "Control.PID", design: "pid" };
    value.parameterBinding = { ...value.parameterBinding, parameters: [
      { symbol: "Main.Kp", iec_type: "LREAL", unit: "V/m", convention: "proportional gain", minimum: 0, maximum: 100 },
      { symbol: "Main.Ki", iec_type: "LREAL", unit: "V/(m*s)", convention: "integral gain", minimum: 0, maximum: 100 },
      { symbol: "Main.Kd", iec_type: "LREAL", unit: "V*s/m", convention: "derivative gain", minimum: 0, maximum: 100 },
      { symbol: "Main.BackCalculationGain", iec_type: "LREAL", unit: "1/s", convention: "back-calculation gain", minimum: 0, maximum: 100 },
    ] };
    value.controller.law = { kind: "pid", parameterRecordSymbol: "Main.Parameters", parameterRevisionSymbol: "Main.ParameterRevision",
      proportionalGainSymbol: "Main.Kp", integralGainSymbol: "Main.Ki", derivativeGainSymbol: "Main.Kd",
      antiWindup: { method: "back-calculation", trackingGainSymbol: "Main.BackCalculationGain" } };
    const adapter: any = require("../../controlStudio/controllerLaw");
    assert.deepStrictEqual(adapter.decodePidLaw(value.controller.law, value.parameterBinding), value.controller.law);
    assert.deepStrictEqual(adapter.materializePidGainRecord(value, {
      proportionalGain: 12, integralGain: 3, derivativeGain: 0.5, backCalculationGain: 0.4,
    }, { "Main.Kp": 1, "Main.Ki": 0, "Main.Kd": 0, "Main.BackCalculationGain": 0, "Main.Unrelated": 7 }), {
      "Main.Kp": 12, "Main.Ki": 3, "Main.Kd": 0.5, "Main.BackCalculationGain": 0.4, "Main.Unrelated": 7,
    });
    const wrongUnit = structuredClone(value);
    wrongUnit.parameterBinding.parameters[3].unit = "s";
    assert.strictEqual(adapter.decodePidLaw(wrongUnit.controller.law, wrongUnit.parameterBinding), undefined,
      "A tracking gain must be declared in reciprocal seconds, not inferred from its name.");
  });
  test("constructs Routh mathematics from the selected model and explicit gain binding", () => {
    let adapter: any;
    try { adapter = require("../../controlStudio/controllerLaw"); } catch { adapter = {}; }
    assert.strictEqual(typeof adapter.proportionalLoopAnalysis, "function", "Model-free setup needs a selected-model mathematical adapter");
    const model = { kind: "transfer-function", numerator: [4], denominator: [2, 3], delaySeconds: 0,
      revision: "provided-nonshaft", source: "provided", validation: "unvalidated", evidence: [],
      inputs: [{ id: "drive", unit: "V", operatingRange: { minimum: -20, maximum: 20 } }],
      outputs: [{ id: "distance", unit: "m", operatingRange: { minimum: -20, maximum: 20 } }] };
    const analysis = adapter.proportionalLoopAnalysis(loop(), model, 3);
    const result = calculateDeclaredRouth(analysis, 3);
    assert.deepStrictEqual(result.polynomial.coefficients, [2, 15]);
    assert.strictEqual(result.polynomial.stable, true);
    assert.strictEqual(analysis.plant.revision, model.revision);
    assert.deepStrictEqual(analysis.controller.numerator, [{ parameter: "Kp" }]);
  });
  test("maps a selected delayed integrator into the existing SIMC PI tuning contract", () => {
    const { designTypedPiCandidate } = require("../../controlStudio/pidPiDesign");
    for (const source of ["provided", "fitted"]) {
      const model = { kind: "transfer-function", numerator: [4], denominator: [2, 0], delaySeconds: 0.5,
        revision: source, source, validation: "validated", evidence: ["independent record"],
        inputs: [{ id: "u", unit: "V", operatingRange: { minimum: -20, maximum: 20 } }],
        outputs: [{ id: "y", unit: "m", operatingRange: { minimum: -20, maximum: 20 } }] };
      let result: any;
      assert.doesNotThrow(() => { result = designTypedPiCandidate(model, { method: "SIMC", objective: "balanced", action: "direct" }); }, "selected integrating models must reach SIMC PI");
      assert.strictEqual(result.candidate.proportionalGain, Number((1 / 3).toPrecision(8)));
      assert.strictEqual(result.candidate.integralGain, result.candidate.proportionalGain / 6);
      assert.strictEqual(result.candidate.derivativeGain, 0);
      assert.strictEqual(result.selectedModel.revision, source);
      assert.strictEqual(result.actualSTVerify, true);
      assert.strictEqual(result.candidate.applied, false);
      assert.throws(() => designTypedPiCandidate({ ...model, delaySeconds: 0 }, { method: "SIMC", objective: "balanced", action: "direct" }), /positive.*delay/i);
    }
  });

  test("reduces a second-order model by Skogestad's half rule before the SIMC PI formula", () => {
    const { designTypedPiCandidate } = require("../../controlStudio/pidPiDesign");
    const channel = (id: string, unit: string) => ({ id, unit, operatingRange: { minimum: -20, maximum: 20 } });
    // G(s) = 2·e^(−0.5s) / ((5s + 1)(s + 1)): the half rule gives τ = 5 + 1/2 = 5.5 s and θ = 0.5 + 1/2 = 1 s.
    const model = { kind: "transfer-function", numerator: [2], denominator: [5, 6, 1], delaySeconds: 0.5, revision: "second-order",
      source: "fitted", validation: "validated", evidence: ["independent record"], inputs: [channel("u", "%")], outputs: [channel("y", "degC")] };
    let result: any;
    assert.doesNotThrow(() => { result = designTypedPiCandidate(model, { method: "SIMC", objective: "balanced", action: "direct" }); }, "a second-order model is within SIMC's scope");
    const closedLoop = 2 * Math.max(1, 5.5 / 3), kp = 5.5 / (2 * (closedLoop + 1));
    const close = (actual: number, expected: number) => Math.abs(actual - expected) <= 1e-7 * Math.abs(expected);
    assert.ok(close(result.candidate.proportionalGain, kp), `Kp ${result.candidate.proportionalGain} ≠ ${kp}`);
    assert.ok(close(result.candidate.integralGain, kp / 5.5), "Ti = min(τ, 4(τc + θ)) = 5.5 s");
    assert.match(result.scope, /half rule/i, "the reduction is named, not hidden");
    assert.throws(() => designTypedPiCandidate({ ...model, denominator: [1, 0.4, 1] }, { method: "SIMC", objective: "balanced", action: "direct" }), /complex poles/i,
      "an oscillatory second-order model has no half-rule reduction");
  });

  test("considers a compatible PD alternative instead of claiming all finite error tasks require integral action", () => {
    const { recommendAutomaticController } = require("../../controlStudio/automaticControllerRecommendation");
    const fs = require("fs"), path = require("path");
    const bound = JSON.parse(fs.readFileSync(path.resolve(__dirname, "../../../src/test/fixtures/control-studio-pid/control-loop.json"), "utf8"));
    const law = bound.controller.law;
    const integral = bound.parameterBinding.parameters.find((p: any) => p.symbol === law.integralGainSymbol);
    integral.minimum = 0; integral.maximum = 0;
    const model = { kind: "transfer-function", numerator: [1], denominator: [1, 1], delaySeconds: 0.1,
      revision: "delayed-pd", source: "provided", validation: "validated", evidence: ["independent record"],
      inputs: [{ id: "u", unit: "V", operatingRange: { minimum: -20, maximum: 20 } }],
      outputs: [{ id: "y", unit: "m", operatingRange: { minimum: -20, maximum: 20 } }] };
    let result: any;
    assert.doesNotThrow(() => { result = recommendAutomaticController(bound, model,
      { referenceChange: 1, maximumError: 0.5, settlingBand: 0.02, settlingTimeSeconds: 2, durationSeconds: 4 }, "manual"); });
    assert.strictEqual(result.family, "PD");
    assert.strictEqual(result.design.candidate.integralGain, 0);
    assert.strictEqual(result.design.actualSTVerify, true);
    assert.strictEqual(result.design.candidate.applied, false);
    assert.strictEqual(result.attempts.length, 4);
    assert.throws(() => recommendAutomaticController(bound, model,
      { referenceChange: 1, maximumError: 0.1, settlingBand: 0.02, settlingTimeSeconds: 2, durationSeconds: 4 }, "manual"),
      /steady.*error/i, "PD must still be rejected when its calculated steady error exceeds the task limit");
  });

  test("retains one rejected P attempt when the actual gain record cannot represent P", () => {
    const { recommendAutomaticController } = require("../../controlStudio/automaticControllerRecommendation");
    const fs = require("fs"), path = require("path");
    const bound = JSON.parse(fs.readFileSync(path.resolve(__dirname, "../../../src/test/fixtures/control-studio-pid/control-loop.json"), "utf8"));
    bound.parameterBinding.parameters.find((p: any) => p.symbol === bound.controller.law.integralGainSymbol).minimum = 0.1;
    const model = { kind: "transfer-function", numerator: [1], denominator: [1, 1], delaySeconds: 0,
      revision: "pi-bound", source: "provided", validation: "validated", evidence: ["independent record"],
      inputs: [{ id: "u", unit: "V", operatingRange: { minimum: -20, maximum: 20 } }],
      outputs: [{ id: "y", unit: "m", operatingRange: { minimum: -20, maximum: 20 } }] };
    const result = recommendAutomaticController(bound, model,
      { referenceChange: 1, maximumError: 0.1, settlingBand: 0.02, settlingTimeSeconds: 2, durationSeconds: 4 }, "manual");
    assert.strictEqual(result.family, "PI");
    assert.strictEqual(result.attempts.length, 4, "each controller family must have exactly one retained outcome");
    assert.deepStrictEqual(result.attempts.filter((a: any) => a.structure === "P").map((a: any) => a.status), ["Rejected"]);
  });

  test("retains every controller rejection when no automatic candidate can be constructed", () => {
    const { recommendAutomaticController } = require("../../controlStudio/automaticControllerRecommendation");
    const fs = require("fs"), path = require("path");
    const bound = JSON.parse(fs.readFileSync(path.resolve(__dirname, "../../../src/test/fixtures/control-studio-pid/control-loop.json"), "utf8"));
    for (const parameter of bound.parameterBinding.parameters) { parameter.minimum = 0; parameter.maximum = 0; }
    const model = { kind: "transfer-function", numerator: [1], denominator: [1, 1], delaySeconds: 0,
      revision: "no-controller", source: "provided", validation: "validated", evidence: ["record"],
      inputs: [{ id: "u", unit: "V", operatingRange: { minimum: -20, maximum: 20 } }],
      outputs: [{ id: "y", unit: "m", operatingRange: { minimum: -20, maximum: 20 } }] };
    let failed: any;
    try { recommendAutomaticController(bound, model,
      { referenceChange: 1, maximumError: 0.1, settlingBand: 0.02, settlingTimeSeconds: 2, durationSeconds: 4 }, "manual"); }
    catch (error) { failed = error; }
    assert.ok(failed instanceof AutomaticControllerSelectionError);
    assert.ok(Array.isArray(failed.attempts), "rejected designs need structured retained outcomes, not only a concatenated message");
    assert.deepStrictEqual(failed.attempts.map((a: any) => a.structure), ["P", "PI", "PD", "PID"]);
    assert.ok(failed.attempts.every((a: any) => a.status === "Rejected" && a.reason.length > 0));
  });

  test("checks Auto initialization capability for P implemented by a PID controller", () => {
    const { recommendAutomaticController } = require("../../controlStudio/automaticControllerRecommendation");
    const fs = require("fs"), path = require("path");
    const bound = JSON.parse(fs.readFileSync(path.resolve(__dirname, "../../../src/test/fixtures/control-studio-pid/control-loop.json"), "utf8"));
    delete bound.controller.pidInitialization;
    const model = { kind: "transfer-function", numerator: [1], denominator: [1, 1], delaySeconds: 0,
      revision: "no-initialization", source: "provided", validation: "validated", evidence: ["record"],
      inputs: [{ id: "u", unit: "V", operatingRange: { minimum: -20, maximum: 20 } }],
      outputs: [{ id: "y", unit: "m", operatingRange: { minimum: -20, maximum: 20 } }] };
    const task = { referenceChange: 1, maximumError: 0.1, settlingBand: 0.02, settlingTimeSeconds: 2, durationSeconds: 4 };
    assert.throws(() => recommendAutomaticController(bound, model, task, "auto"), /initialization/i,
      "Auto must not recommend a PID-backed P candidate without its transfer contract");
    assert.strictEqual(recommendAutomaticController(bound, model, task, "manual").family, "P");
  });

});
