import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import { decodeGeneralModel } from "../../controlStudio/generalModel";
import { materializeSelectedModelSimulation } from "../../controlStudio/modelSimulation";
import { designSimpleProportionalController } from "../../controlStudio/simpleControllerDesign";
import { designManualProportionalCandidate } from "../../controlStudio/manualProportionalDesign";
import { engineerVerificationPlan, engineerRequirements } from "../../controlStudio/engineerVerificationPlan";

import { transferTolerance, proportionalInitializationPlan } from "../../controlStudio/proportionalTransfer";

const channel = (id: string, unit: string) => ({ id, unit, operatingRange: { minimum: -20, maximum: 20 } });
const model = () => ({ kind: "transfer-function", numerator: [1], denominator: [1, 1], delaySeconds: 0,
  revision: "offset-model", source: "provided", validation: "validated", evidence: ["bench"],
  inputs: [channel("u", "V")], outputs: [channel("y", "m")], operatingPoint: { inputs: [2], outputs: [10] } });
const requirements = { referenceChange: 1, constantInputDisturbance: 0, maximumSteadyError: 0.1,
  settlingBand: 0.02, settlingTimeSeconds: 2, proportionalGainBounds: { minimum: -100, maximum: 100 } };

suite("Control Studio operating-point coordinates", () => {
  test("rejects invalid operating-point dimensions and physical coordinates", () => {
    assert.ok(decodeGeneralModel(model()));
    for (const operatingPoint of [{ inputs: [], outputs: [10] }, { inputs: [2], outputs: [Infinity] },
      { inputs: [2], outputs: [21] }, { inputs: [2] }]) {
      assert.strictEqual(decodeGeneralModel({ ...model(), operatingPoint }), undefined,
        "An invalid operating point must not be ignored as arbitrary model metadata");
    }
  });
  test("materializes model deviations using the existing physical IO offsets", () => {
    const materialized = materializeSelectedModelSimulation(model(), { plantId: "offset", commandSources: ["%QL0"],
      feedbackTargets: ["%IL0"], initialState: [0], initialInput: [0], stepMs: 20 });
    assert.match(materialized.simulation_override.simulation_toml, /input_offset = \[-2\]/);
    assert.match(materialized.simulation_override.simulation_toml, /output_offset = \[10\]/);
  });
  test("accounts for the operating command without inventing P controller bias", () => {
    const automatic = designSimpleProportionalController(model(), requirements);
    assert.strictEqual(automatic.candidate.proportionalGain, 29);
    assert.ok(automatic.prediction.steadyErrorBound <= 0.1);
    const manual = designManualProportionalCandidate(model(), 0.5, requirements);
    assert.strictEqual(manual.prediction.steadyErrorBound, 2);
  });
  test("retains estimation operating coordinates through accepted identification", () => {
    const { IdentificationWorkflow } = require("../../controlStudio/identification");
    const { acceptedIdentificationModel } = require("../../controlStudio/selectedModelAnalysis");
    const workflow = new IdentificationWorkflow();
    const samples = (amplitude: number, output0: number) => Array.from({ length: 301 }, (_, i) => ({
      timeSeconds: i * 0.1, input: 2 + (i >= 20 ? amplitude : 0),
      output: output0 + (i >= 20 ? amplitude * (1 - Math.exp(-(i - 20) * 0.1)) : 0),
      quality: "good", inputUnit: "V", outputUnit: "m" }));
    workflow.qualify(samples(1, 10), [], "Simulation", "first_order"); workflow.fit();
    workflow.qualify(samples(0.75, 11), [], "Simulation", "first_order"); workflow.fit();
    const accepted = workflow.accept();
    const selected = acceptedIdentificationModel(accepted, {
      inputs: [channel("u", "V")], outputs: [channel("y", "m")],
    });
    assert.deepStrictEqual(selected.operatingPoint, { inputs: [2], outputs: [10] },
      "Independent validation coordinates must not replace the fitted estimation point");
    const { StudioAnalysisSessions } = require("../../controlStudio/studioAnalysisSessions");
    const loop = JSON.parse(fs.readFileSync(path.resolve(__dirname, "../../../src/test/fixtures/control-studio-engineer/control-loop.json"), "utf8"));
    const sessions = new StudioAnalysisSessions(loop);
    try {
      sessions.acceptIdentification(accepted, loop);
      assert.ok(sessions.engineer.modelChanged().identificationPresentation,
        "Accepting a model in Advanced Identify must expose the same measured comparison in the guided workflow");
    } finally { sessions.dispose(); }

  });
  test("bounds manual Auto-entry tolerance and preserves zero as an explicit choice", () => {
    const loop = JSON.parse(fs.readFileSync(path.resolve(__dirname, "../../../src/test/fixtures/control-studio-engineer/control-loop.json"), "utf8"));
    loop.controller.manualOutputType = "LREAL";
    const task = { referenceChange: 1, maximumError: 0.1, settlingBand: 0.02, settlingTimeSeconds: 2, durationSeconds: 4 };
    for (const value of [-1, Infinity, NaN, "0.01", null]) {
      assert.throws(() => engineerRequirements({ ...task, transferTolerance: value }), /tolerance/);
    }
    const explicit = engineerRequirements({ ...task, transferTolerance: 0 });
    assert.strictEqual(explicit.transferTolerance, 0);
    assert.strictEqual(transferTolerance(loop, explicit.transferTolerance), 0);
    assert.ok(Math.abs(transferTolerance(loop) - 0.00004) < 1e-18);
    assert.throws(() => proportionalInitializationPlan(loop, decodeGeneralModel(model())!,
      { ...task, transferTolerance: 41 }, { "Main.Kp": 36.5 }), /output span/);
  });
  test("uses matched physical starting command and an absolute reference in actual ST plans", () => {
    const loop = JSON.parse(fs.readFileSync(path.resolve(__dirname, "../../../src/test/fixtures/control-studio-engineer/control-loop.json"), "utf8"));
    const selected = decodeGeneralModel(model())!;
    const task = { referenceChange: 1, maximumError: 0.1, settlingBand: 0.02, settlingTimeSeconds: 2, durationSeconds: 4 };
    const baseline = engineerVerificationPlan(loop, selected, task, { "Main.Kp": 1 });
    const candidate = engineerVerificationPlan(loop, selected, task, { "Main.Kp": 36.5 });
    assert.strictEqual(candidate.request.initial_values["Main.SP"], 11);
    assert.strictEqual(candidate.request.initial_values["Main.MV"], 2);
    assert.deepStrictEqual(candidate.request.initial_values, baseline.request.initial_values);
    assert.deepStrictEqual(candidate.request.simulation_override, baseline.request.simulation_override);
    assert.strictEqual((candidate.requirements.find(value => value.kind === "settling") as any).target, 11);
    assert.ok(candidate.request.observations.some(value => value.symbol === "Main.Kp"),
      "Actual-ST verification must observe the fixed gain, not just request an override");
    assert.ok(candidate.requirements.some(value => value.kind === "bounds" && value.channel === "Main.Kp"
      && value.minimum === 36.5 && value.maximum === 36.5));
  });
});
