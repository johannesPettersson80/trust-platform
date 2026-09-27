import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";

const engineerLoop = () => JSON.parse(fs.readFileSync(path.resolve(__dirname, "../../../src/test/fixtures/control-studio-engineer/control-loop.json"), "utf8"));
const pidLoop = () => JSON.parse(fs.readFileSync(path.resolve(__dirname, "../../../src/test/fixtures/control-studio-pid/control-loop.json"), "utf8"));
const channel = (id: string, unit: string) => ({ id, unit, operatingRange: { minimum: -100, maximum: 100 } });

suite("Control Studio controller and model mappings", () => {
  test("manual project-owned PID retains the entire declared composition", () => {
    const { manualControllerCandidate } = require("../../controlStudio/manualControllerCandidate");
    const loop = pidLoop();
    loop.parameterBinding.parameters.push({ symbol: "Main.FilterTime", iec_type: "LREAL", unit: "s", convention: "filter time constant", minimum: 0, maximum: 10 });
    loop.controller.project = { family: "PI", convention: "project-owned-native-controller", initialization: {
      kind: "declared-state-v1", states: [{ symbol: "Main.ProjectState", iec_type: "LREAL", unit: "V", minimum: -20, maximum: 20, initial: 0, postScan: "bounded" }],
    }, axes: [], objective: { requirementId: "response-deadline", direction: "minimize" }, maximumCandidates: 4, maximumCycles: 1000, wallBudgetMs: 1000 };
    const model = { kind: "transfer-function", numerator: [1], denominator: [1, 1], delaySeconds: 0,
      revision: "manual-model", source: "provided", validation: "validated", evidence: ["record"], inputs: [channel("u", "V")], outputs: [channel("y", "m")] };
    const entered = Object.fromEntries(loop.parameterBinding.parameters.map((parameter: {symbol: string}) => [parameter.symbol, parameter.symbol === "Main.Kd" ? 0 : 0.5]));
    const candidate = manualControllerCandidate(loop, model, "PI", entered, { "Main.FilterTime": 0.5 });
    assert.deepStrictEqual(candidate.values, entered, "The declared project adapter receives gains, filter settings and pins together");
    assert.strictEqual(candidate.design.candidate.applied, false);
  });
  test("tunes a positive one-state sampled model through its exact held-input equivalent", () => {
    const { linearTransferModel } = require("../../controlStudio/linearTransferModel");
    const step = 0.02, pole = Math.exp(-step / 2);
    const sampled = { kind: "discrete-state-space", revision: "sampled-fit", source: "fitted", validation: "validated", evidence: ["estimation", "independent"],
      delaySeconds: 0, sampleTimeSeconds: step, a: [[pole]], b: [[3 * (1 - pole)]], c: [[1]], d: [[0]],
      inputs: [channel("u", "V")], outputs: [channel("y", "m")], states: [channel("x", "m")] };
    const before = structuredClone(sampled);
    const transfer = linearTransferModel(sampled);
    assert.ok(Math.abs(transfer.denominator[1] - 0.5) < 1e-12);
    assert.ok(Math.abs(transfer.numerator[0] - 1.5) < 1e-12);
    assert.strictEqual(transfer.revision, sampled.revision);
    assert.deepStrictEqual(sampled, before, "the selected sampled model remains unchanged for actual ST");
    const integrator = linearTransferModel({ ...sampled, a: [[1]], b: [[2 * step]] });
    assert.deepStrictEqual(integrator.denominator, [1, 0]);
    assert.strictEqual(integrator.numerator[0], 2);
    assert.throws(() => linearTransferModel({ ...sampled, a: [[-0.5]] }), /sampled|discrete|continuous/i);
    assert.throws(() => linearTransferModel({ ...sampled, d: [[1]] }), /sampled|discrete|continuous/i);
  });

  test("maps project plant and inverse gains in the declared engineering direction", () => {
    const { projectModelParameters } = require("../../controlStudio/projectModelParameters");
    const loop = engineerLoop();
    loop.parameterBinding.parameters[0].unit = "m/V";
    const model = { kind: "transfer-function", numerator: [2], denominator: [1, 1], delaySeconds: 0,
      revision: "gain-model", source: "provided", validation: "validated", evidence: ["record"],
      inputs: [channel("u", "V")], outputs: [channel("y", "m")] };
    assert.deepStrictEqual(projectModelParameters(loop, model, { "Main.Kp": { kind: "plant-gain" } }), { "Main.Kp": 2 });
    loop.parameterBinding.parameters[0].unit = "V/m";
    assert.deepStrictEqual(projectModelParameters(loop, model, { "Main.Kp": { kind: "inverse-plant-gain" } }), { "Main.Kp": 0.5 });
    loop.parameterBinding.parameters[0].unit = "m3/s";
    assert.throws(() => projectModelParameters(loop, { ...model, physical: { kind: "gravity-tank", areaM2: 2, outflowCoefficient: 1 } },
      { "Main.Kp": { kind: "physical", parameter: "outflowCoefficient" } }), /unit/i);
  });

  test("accepts dimensionless equal-coordinate sampled coefficients and rejects wrong coordinates", () => {
    const { projectModelParameters } = require("../../controlStudio/projectModelParameters");
    const loop = engineerLoop();
    loop.parameterBinding.parameters[0].unit = "1";
    const model = { kind: "continuous-state-space", revision: "matrix-model", source: "provided", validation: "validated", evidence: ["record"], delaySeconds: 0,
      inputs: [channel("u", "m")], outputs: [channel("y", "m")], states: [channel("x", "m")],
      a: [[-1]], b: [[1]], c: [[1]], d: [[0]] };
    const sampled = projectModelParameters(loop, model, { "Main.Kp": { kind: "sampled-matrix", matrix: "a", row: 0, column: 0 } });
    assert.ok(Math.abs(sampled["Main.Kp"] - Math.exp(-0.02)) < 1e-12, "continuous state models map through exact ZOH at the task period");
    loop.parameterBinding.parameters[0].unit = "V";
    assert.throws(() => projectModelParameters(loop, model, { "Main.Kp": { kind: "sampled-matrix", matrix: "a", row: 0, column: 0 } }), /units/i);
  });

  test("calculates declared PID integral, weighted-error, and notch initial states from one operating point", () => {
    const { decodeControllerInitialization, initializationValues } = require("../../controlStudio/controllerInitialization");
    const loop = pidLoop();
    loop.parameterBinding.parameters.push(
      { symbol: "Main.A1", iec_type: "LREAL", unit: "1", convention: "notch denominator", minimum: -0.9, maximum: 0.9 },
      { symbol: "Main.A2", iec_type: "LREAL", unit: "1", convention: "notch denominator", minimum: -0.9, maximum: 0.9 });
    const definition = decodeControllerInitialization({ kind: "declared-state-v1", states: [
      { symbol: "Main.I", iec_type: "LREAL", unit: "V", minimum: -20, maximum: 20,
        initial: { source: "pid-integral", index: 0, inputIndex: 0, gainSymbol: "Main.Kp", weight: 2 }, postScan: "bounded" },
      { symbol: "Main.E", iec_type: "LREAL", unit: "m", minimum: -20, maximum: 20,
        initial: { source: "weighted-error", index: 0, weight: 0.5 }, postScan: "unchanged" },
      { symbol: "Main.N", iec_type: "LREAL", unit: "m", minimum: -20, maximum: 20,
        initial: { source: "notch-history", index: 0, denominatorSymbols: ["Main.A1", "Main.A2"] }, postScan: "unchanged" },
    ] });
    assert.ok(definition);
    const model = { kind: "transfer-function", numerator: [1], denominator: [1, 1], delaySeconds: 0,
      revision: "initial-model", source: "provided", validation: "validated", evidence: ["record"],
      inputs: [channel("u", "V")], outputs: [channel("y", "m")], operatingPoint: { inputs: [3], outputs: [4] } };
    assert.deepStrictEqual(initializationValues(loop, model, definition, { "Main.Kp": 2, "Main.A1": 0.1, "Main.A2": 0.2 }),
      { "Main.I": -5, "Main.E": -2, "Main.N": 4 / 1.3 });
  });

  test("keeps pinned PID values bounded and rejects pins from another family", () => {
    const { pinnedControllerGains } = require("../../controlStudio/controllerSettingsPins");
    const loop = pidLoop();
    const gains = { proportionalGain: 1, integralGain: 2, derivativeGain: 0 };
    const pinned = pinnedControllerGains(loop, "PI", gains, { loopRevision: loop.revision, family: "PI", values: { "Main.Kp": 3, "Main.Ki": 4 } });
    assert.deepStrictEqual(pinned, { proportionalGain: 3, integralGain: 4, derivativeGain: 0 });
    assert.throws(() => pinnedControllerGains(loop, "PI", gains, { loopRevision: loop.revision, family: "PID", values: { "Main.Kp": 3 } }), /different loop or controller/i);
    assert.throws(() => pinnedControllerGains(loop, "PI", gains, { loopRevision: loop.revision, family: "PI", values: { "Main.Kp": 1000 } }), /bound|representable/i);
  });

  test("rejects a physical identification tolerance outside the declared adequacy interval", () => {
    const { identifyPhysicalModel } = require("../../controlStudio/physicalIdentification");
    const run = { id: "record", conditions: { programRevision: "program", modelRevision: "model", sampleTimesSeconds: [] }, samples: [], channels: [] };
    assert.throws(() => identifyPhysicalModel(run, { ...run, id: "validation" },
      { inputs: [{ id: "u", unit: "W" }], outputs: [{ id: "y", unit: "K" }] }, { kind: "thermal-balance" }, 1), /tolerance/i);
  });

  test("preserves every declared project-controller family behind the bounded native adapter", () => {
    const { PROJECT_CONTROLLER_FAMILIES } = require("../../controlStudio/controllerFamilies");
    const { controllerAdapter } = require("../../controlStudio/controllerAdapters");
    const { decodeProjectController } = require("../../controlStudio/projectController");
    for (const family of PROJECT_CONTROLLER_FAMILIES) {
      const loop = engineerLoop();
      loop.controller = { ...loop.controller, design: "project", project: {
        family, convention: "project-owned-native-controller", initialization: {
          kind: "declared-state-v1", states: [{ symbol: "Main.ProjectState", iec_type: "LREAL", unit: "V",
            minimum: -20, maximum: 20, initial: 0, postScan: "bounded" }],
        }, axes: [{ symbol: "Main.Kp", values: [1] }],
        objective: { requirementId: "response-deadline", direction: "minimize" }, maximumCandidates: 4, maximumCycles: 1000, wallBudgetMs: 1000,
      } };
      const definition = decodeProjectController(loop.controller.project, loop.parameterBinding);
      assert.ok(definition, `${family} must decode through the shared project contract`);
      const adapter = controllerAdapter(loop);
      assert.deepStrictEqual(adapter.families, [family]);
      adapter.checkInitialization();
      assert.deepStrictEqual(adapter.materialize({ parameters: { "Main.Kp": 1 } }, {}), { "Main.Kp": 1 });
      assert.throws(() => adapter.materialize({ parameters: { "Main.Unknown": 1 } }, {}), /complete|declared|parameter/i);
    }
  });

  test("derives ON/OFF hysteresis and task-aligned dwell through its typed adapter", () => {
    const { controllerAdapter } = require("../../controlStudio/controllerAdapters");
    const { tuneControllerAdapter } = require("../../controlStudio/controllerTuning");
    const loop = engineerLoop();
    loop.parameterBinding.parameters.push(
      { symbol: "Main.Hysteresis", iec_type: "LREAL", unit: "m", convention: "switching band", minimum: 0, maximum: 5 },
      { symbol: "Main.MinOn", iec_type: "LREAL", unit: "s", convention: "minimum on time", minimum: 0, maximum: 10 },
      { symbol: "Main.MinOff", iec_type: "LREAL", unit: "s", convention: "minimum off time", minimum: 0, maximum: 10 });
    loop.controller = { ...loop.controller, onOff: {
      hysteresisSymbol: "Main.Hysteresis", minimumOnTimeSymbol: "Main.MinOn", minimumOffTimeSymbol: "Main.MinOff",
      bandConvention: "half-band", lowOutput: 0, highOutput: 20, noiseBand: 0.1,
      initialization: { kind: "declared-state-v1", states: [{ symbol: "Main.SwitchState", iec_type: "BOOL", unit: "1",
        minimum: 0, maximum: 1, initial: 0, postScan: "bounded" }] },
    } };
    const model = { kind: "transfer-function", numerator: [1], denominator: [1, 1], delaySeconds: 0,
      revision: "on-off-model", source: "provided", validation: "validated", evidence: ["record"],
      inputs: [channel("u", "V")], outputs: [channel("y", "m")] };
    assert.deepStrictEqual(controllerAdapter(loop).families, ["ON/OFF"]);
    const result = tuneControllerAdapter(loop, model, { referenceChange: 1, maximumError: 1, settlingBand: 0.2,
      settlingTimeSeconds: 2, durationSeconds: 4 }, { family: "automatic", applicationMode: "manual" });
    assert.strictEqual(result.family, "ON/OFF");
    assert.ok(result.values["Main.Hysteresis"] >= 0.2);
    assert.ok(Math.abs(result.values["Main.MinOn"] / 0.02 - Math.round(result.values["Main.MinOn"] / 0.02)) < 1e-9);
    assert.ok(Math.abs(result.values["Main.MinOff"] / 0.02 - Math.round(result.values["Main.MinOff"] / 0.02)) < 1e-9);
  });
  test("preserves pinned ON/OFF hysteresis and dwell settings during automatic Tune", () => {
    const { tuneControllerAdapter } = require("../../controlStudio/controllerTuning");
    const loop = engineerLoop();
    loop.parameterBinding.parameters.push(
      { symbol: "Main.Hysteresis", iec_type: "LREAL", unit: "m", convention: "switching band", minimum: 0, maximum: 5 },
      { symbol: "Main.MinOn", iec_type: "LREAL", unit: "s", convention: "minimum on time", minimum: 0, maximum: 10 },
      { symbol: "Main.MinOff", iec_type: "LREAL", unit: "s", convention: "minimum off time", minimum: 0, maximum: 10 });
    loop.controller = { ...loop.controller, onOff: { hysteresisSymbol: "Main.Hysteresis", minimumOnTimeSymbol: "Main.MinOn", minimumOffTimeSymbol: "Main.MinOff", bandConvention: "half-band", lowOutput: 0, highOutput: 20, noiseBand: 0.1, initialization: { kind: "declared-state-v1", states: [{ symbol: "Main.SwitchState", iec_type: "BOOL", unit: "1", minimum: 0, maximum: 1, initial: 0, postScan: "bounded" }] } } };
    const model = { kind: "transfer-function", numerator: [1], denominator: [1, 1], delaySeconds: 0, revision: "on-off-pinned", source: "provided", validation: "validated", evidence: ["record"], inputs: [channel("u", "V")], outputs: [channel("y", "m")] };
    const result = tuneControllerAdapter(loop, model, { referenceChange: 1, maximumError: 1, settlingBand: 0.2, settlingTimeSeconds: 2, durationSeconds: 4 }, { family: "automatic", applicationMode: "manual", pinnedValues: { "Main.Hysteresis": 0.8, "Main.MinOn": 0.04, "Main.MinOff": 0.06 } });
    assert.deepStrictEqual({ hysteresis: result.values["Main.Hysteresis"], minOn: result.values["Main.MinOn"], minOff: result.values["Main.MinOff"] }, { hysteresis: 0.8, minOn: 0.04, minOff: 0.06 });
  });
});
