import * as assert from "assert";
import { decodeControlLoopDefinition } from "../../controlStudio/loopDefinition";
import { projectControllerSeed } from "../../controlStudio/projectControllerSeed";
import { designPidToRequirements } from "../../controlStudio/automaticPidResponse";
import { candidateMargins } from "../../controlStudio/candidateMargins";
import { declaredInitializationPlan } from "../../controlStudio/controllerInitialization";
import { engineerRequirements } from "../../controlStudio/engineerVerificationPlan";
import { ownLawLoopDefinition } from "../../controlStudio/loopSetup/pidLoopSetup";

const TASK = [{ name: "MainTask", intervalMs: 20 }];
/** A drum axis whose program moves its reference along a motion profile and adds K_v·ṙ + K_a·r̈ to its own PID. */
const axis = {
  symbols: [
    { symbol: "P1.AngleRad", type: "LREAL", address: "%IL0" }, { symbol: "P1.TorqueNm", type: "LREAL", address: "%QL0" },
    { symbol: "P1.SetpointRad", type: "LREAL", address: "%ML0" }, { symbol: "P1.ManualTorqueNm", type: "LREAL", address: "%ML8" },
    { symbol: "P1.AutoMode", type: "BOOL" }, { symbol: "P1.Started", type: "BOOL" },
    { symbol: "P1.Kp", type: "LREAL" }, { symbol: "P1.Ki", type: "LREAL" }, { symbol: "P1.Kd", type: "LREAL" },
    { symbol: "P1.Integral", type: "LREAL" }, { symbol: "P1.PreviousAngleRad", type: "LREAL" },
  ],
  controllers: [{ instance: "P1", type: "Main", tasks: TASK }],
  links: [],
  writes: ["P1.TorqueNm", "P1.Integral", "P1.PreviousAngleRad", "P1.Started"],
};
const draft = { projectId: "project:axis", runtimeId: "runtime-1", name: "Axis", action: "direct", law: "PID", instance: "P1",
  processVariable: { symbol: "P1.AngleRad", unit: "rad", minimum: -2, maximum: 2 },
  manipulatedVariable: { symbol: "P1.TorqueNm", unit: "N·m", minimum: -20, maximum: 20 },
  setpoint: { symbol: "P1.SetpointRad" }, mode: { symbol: "P1.AutoMode", whenTrue: "Auto" }, manualOutput: "P1.ManualTorqueNm",
  proportionalGain: { symbol: "P1.Kp", minimum: 0, maximum: 100 }, integralGain: { symbol: "P1.Ki", minimum: 0, maximum: 100 },
  derivativeGain: { symbol: "P1.Kd", minimum: 0, maximum: 100 }, integralTerm: "P1.Integral",
  derivativeState: "P1.PreviousAngleRad", derivativeOn: "measurement", experimentStep: 1, experimentDurationSeconds: 8, startupFlag: "P1.Started" };

function trajectoryLoop(): any {
  const loop: any = ownLawLoopDefinition(axis as any, draft as any);
  loop.parameterBinding.parameters.push(
    { symbol: "P1.RateGain", iec_type: "LREAL", unit: "N·m·s/rad", convention: "Rate feedforward gain", minimum: 0, maximum: 10 },
    { symbol: "P1.AccelerationGain", iec_type: "LREAL", unit: "N·m·s²/rad", convention: "Acceleration feedforward gain", minimum: 0, maximum: 10 });
  loop.controller.project.seed = { kind: "trajectory-feedforward-pid", family: "PID", proportionalSymbol: "P1.Kp", integralSymbol: "P1.Ki",
    derivativeSymbol: "P1.Kd", rateSymbol: "P1.RateGain", accelerationSymbol: "P1.AccelerationGain" };
  return loop;
}
const transfer = (numerator: number[], denominator: number[]): any => ({ kind: "transfer-function", numerator, denominator, delaySeconds: 0,
  revision: "shaft", source: "provided", validation: "validated", evidence: ["Lesson 34"],
  inputs: [{ id: "P1.TorqueNm", unit: "N·m", operatingRange: { minimum: -20, maximum: 20 } }],
  outputs: [{ id: "P1.AngleRad", unit: "rad", operatingRange: { minimum: -2, maximum: 2 } }], operatingPoint: { inputs: [0], outputs: [0] } });
const task: any = { referenceChange: 1, maximumError: 0.02, settlingBand: 0.01, settlingTimeSeconds: 4, durationSeconds: 8, tuningObjective: "robust" };

suite("Control Studio trajectory feedforward", () => {
  test("accepts a loop whose program adds its trajectory's feedforward, with five distinct gains", () => {
    assert.strictEqual(decodeControlLoopDefinition(trajectoryLoop()).kind, "accepted", "Specification 35 §17.16 declares the trajectory-feedforward-pid seed");
    const duplicate = trajectoryLoop();
    duplicate.controller.project.seed.accelerationSymbol = "P1.RateGain";
    assert.strictEqual(decodeControlLoopDefinition(duplicate).kind, "rejected");
  });

  test("designs the PID as Automatic does and the feedforward as the model's inverse", () => {
    const loop = trajectoryLoop();
    // 2/(0.5·s² + s): K_v = b/n = 0.5 N·m·s/rad and K_a = a/n = 0.25 N·m·s²/rad.
    const model = transfer([2], [0.5, 1, 0]);
    const pid = designPidToRequirements(loop, model, "PID", task).candidate;
    assert.deepStrictEqual(projectControllerSeed(loop, model, task, loop.controller.project.seed, "PID"),
      { "P1.Kp": pid.proportionalGain, "P1.Ki": pid.integralGain, "P1.Kd": pid.derivativeGain, "P1.RateGain": 0.5, "P1.AccelerationGain": 0.25 });
  });

  test("refuses a PID whose designed gain leaves its declared range, as for Control.PID", () => {
    const loop = trajectoryLoop();
    loop.parameterBinding.parameters.find((parameter: any) => parameter.symbol === "P1.Kp").maximum = 2;
    assert.throws(() => projectControllerSeed(loop, transfer([1], [1, 1, 0]), task, loop.controller.project.seed, "PID"), /above its declared maximum of 2/);
  });

  test("names the PID's gains and the two feedforward gains on Tune", () => {
    const { JSDOM } = require("jsdom");
    const { renderTunedController } = require("../../controlStudio/webview/threePageResults");
    const dom = new JSDOM(`<main><div data-studio-tuning-result hidden></div></main>`);
    const previous = (globalThis as any).document; (globalThis as any).document = dom.window.document;
    try {
      const root = dom.window.document.querySelector("main");
      renderTunedController(root, trajectoryLoop(), { status: "candidate", family: "PID",
        values: { "P1.Kp": 18, "P1.Ki": 12, "P1.Kd": 7, "P1.RateGain": 1, "P1.AccelerationGain": 1 } });
      assert.deepStrictEqual(Array.from(root.querySelectorAll("dt")).map((term: any) => term.textContent), ["Proportional gain (Kp)",
        "Integral gain (Ki)", "Derivative gain (Kd)", "Rate feedforward (Kv)", "Acceleration feedforward (Ka)"]);
    } finally { (globalThis as any).document = previous; dom.window.close(); }
  });

  test("names a model it has no trajectory feedforward rule for", () => {
    const loop = trajectoryLoop();
    const heater = { referenceChange: 3, maximumError: 0.2, settlingBand: 0.1, settlingTimeSeconds: 30, durationSeconds: 60, tuningObjective: "robust" };
    assert.throws(() => projectControllerSeed(loop, transfer([0.2], [5, 1]), heater as any, loop.controller.project.seed, "PI"), /no trajectory feedforward rule/);
  });

  test("shows the PID's margins and Auto-entry allowance, since the feedforward acts outside the loop", () => {
    const loop = trajectoryLoop();
    const model = transfer([1], [1, 1, 0]);
    const values = { "P1.Kp": 18, "P1.Ki": 12, "P1.Kd": 7, "P1.RateGain": 1, "P1.AccelerationGain": 1 };
    const margins = candidateMargins(model, loop, { candidate: { parameters: values } }, task, undefined);
    assert.ok(margins && typeof margins.phaseMarginDegrees === "number", `the PID's margins on the model: ${JSON.stringify(margins)}`);
    const noise = 0.001, cycle = 0.02;
    const plan: any = declaredInitializationPlan(loop, model, engineerRequirements(task), values, loop.controller.project.initialization, noise);
    const continuity = plan.requirements.find((requirement: any) => requirement.id === "transfer-continuity");
    const allowance = 2 * 18 * noise + 12 * cycle * noise + 4 * 7 * noise / cycle;
    assert.ok(Math.abs((continuity.maximum - continuity.minimum) / 2 - allowance) < 1e-9,
      `the first output may move by what the noise does through the PID (${allowance}), got ${JSON.stringify(continuity)}`);
  });
});
