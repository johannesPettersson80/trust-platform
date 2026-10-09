import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import { tuneTypedController } from "../../controlStudio/controllerAdapters";
import { recommendAutomaticController } from "../../controlStudio/automaticControllerRecommendation";
import { engineerPrediction } from "../../controlStudio/engineerPrediction";
import { classifyMainWorkflowFailure, nextActionLabel } from "../../controlStudio/mainWorkflowFailure";
import { tuneControllerAdapter } from "../../controlStudio/controllerTuning";

/** A Control.PID loop with a 20 ms task and ±20 V output. */
function pidLoop(): any {
  return JSON.parse(fs.readFileSync(path.resolve(__dirname, "../../../src/test/fixtures/control-studio-pid/control-loop.json"), "utf8"));
}

/** 1.5 m/V, 4 s lag and 0.5 s delay: SIMC's More robust PI settles too slowly for a 12 s deadline. */
const model: any = { kind: "transfer-function", numerator: [1.5], denominator: [4, 1], delaySeconds: 0.5, revision: "fopdt",
  source: "fitted", validation: "validated", evidence: ["recording"], operatingPoint: { inputs: [0], outputs: [0] },
  inputs: [{ id: "Main.MV", unit: "V", operatingRange: { minimum: -20, maximum: 20 } }],
  outputs: [{ id: "Main.PV", unit: "m", operatingRange: { minimum: -20, maximum: 20 } }] };

/** The largest predicted deviation from the target from the deadline to the end of the test, as Validate checks it. */
function remainingError(loop: any, candidate: any, task: any): number {
  const prediction = engineerPrediction(model, loop, candidate, task)!;
  assert.ok(prediction, "the model predicts the candidate");
  return Math.max(...prediction.timeResponse.filter(point => point.timeSeconds >= task.settlingTimeSeconds - 1e-9)
    .map(point => Math.abs(point.candidatePv - task.referenceChange)));
}

suite("Control Studio Automatic tuning meets the requirements", () => {
  const task = { referenceChange: 1, maximumError: 0.1, settlingBand: 0.05, settlingTimeSeconds: 12, durationSeconds: 24,
    tuningObjective: "robust" as const };

  test("uses a faster response than the declared one when that is needed to settle by the deadline", () => {
    const loop = pidLoop();
    const tuned = tuneTypedController(loop, model, "PI", "manual", task);
    const error = remainingError(loop, tuned.design.candidate, task);
    assert.ok(error <= task.maximumError, `the tuned PI is ${error} m from the target after 12 s; 0.1 m is allowed`);
    const recommended = recommendAutomaticController(loop, model, task, "manual");
    assert.strictEqual(recommended.family, "PI");
    assert.ok(remainingError(loop, recommended.design.candidate, task) <= task.maximumError, "the recommended controller meets the deadline");
  });

  test("keeps the declared response when it meets the requirements", () => {
    const loop = pidLoop();
    const relaxed = { ...task, settlingTimeSeconds: 24, durationSeconds: 48 };
    const tuned = tuneTypedController(loop, model, "PI", "manual", relaxed);
    const lambda = 4 * Math.max(0.5, 4 / 3);
    assert.ok(Math.abs(tuned.design.candidate.proportionalGain - 4 / (1.5 * (lambda + 0.5))) < 1e-6,
      `More robust SIMC: Kp = T/(K(λ + L)) with λ = ${lambda} s; got ${tuned.design.candidate.proportionalGain}`);
  });

  test("refuses with the predicted deviation when no response can meet the deadline", () => {
    const loop = pidLoop();
    assert.throws(() => tuneTypedController(loop, model, "PI", "manual", { ...task, settlingTimeSeconds: 2, durationSeconds: 4 }),
      (error: Error) => /PI cannot meet the requirements/.test(error.message) && /after 2 s/.test(error.message) && /0\.1 m is allowed/.test(error.message));
  });

  test("sends the engineer to the requirements when no response can meet them", () => {
    const loop = pidLoop();
    assert.throws(() => tuneTypedController(loop, model, "PI", "manual", { ...task, settlingTimeSeconds: 2, durationSeconds: 4 }), (error: Error) => {
      const next = classifyMainWorkflowFailure(error).nextAction;
      assert.strictEqual(next, "open-requirements",
        "the requirements, not another Tune, decide whether a PI can meet them");
      return true;
    });
  });

  test("refuses PD when holding a declared load within the allowed error needs a gain beyond its range, and recommends PID", () => {
    const loop = pidLoop(); // Kp, Ki and Kd are each declared from 0 to 100.
    // The shaft of Lessons 34–37, G(s) = 1/(s² + s), lifting a weight that pulls with 1.962 N·m on the drive.
    const shaft = { ...model, numerator: [1], denominator: [1, 1, 0], delaySeconds: 0, revision: "shaft" };
    const loaded = { referenceChange: 1, maximumError: 0.02, settlingBand: 0.01, settlingTimeSeconds: 4, durationSeconds: 8,
      constantInputDisturbance: -1.962 };
    // PD holds the load with an error d/Kp: within 0.02 − 0.01 m only with Kp = 1.962/0.01 = 196.2 V/m.
    assert.throws(() => tuneTypedController(loop, shaft, "PD", "manual", loaded),
      (error: Error) => /PD cannot meet the requirements/.test(error.message) && /196\.2/.test(error.message) && /100/.test(error.message)
        && /integral action/.test(error.message),
      "the refusal names the gain PD needs, the declared maximum and the remedy");
    const pid = tuneTypedController(loop, shaft, "PID", "manual", loaded);
    assert.ok(pid.design.candidate.integralGain! > 0 && pid.design.candidate.proportionalGain <= 100, "PID meets the same requirements within range");
    assert.strictEqual(recommendAutomaticController(loop, shaft, loaded, "manual").family, "PID",
      "the simplest family that meets every declared requirement, the load included");
  });

  test("explains why P is refused: the offset on a delayed process, the missing damping on the shaft", () => {
    const loop = pidLoop();
    const refusal = (plant: any, task: any) => {
      try { tuneControllerAdapter(loop, plant, task, { family: "P", applicationMode: "manual" }); }
      catch (error) { return { message: (error as Error).message, next: nextActionLabel(classifyMainWorkflowFailure(error).nextAction) }; }
      assert.fail("P must be refused");
    };
    // 1.5 m/V with a 0.5 s delay: P keeps the offset Δr/(1 + K·Kp); within 0.1 − 0.05 m that needs Kp ≥ (1/0.05 − 1)/1.5.
    const delayed = refusal(model, task);
    assert.match(delayed.message, /P is unavailable for a model with a transport delay/);
    assert.match(delayed.message, /Kp ≥ 12\.67 V\/m/);
    assert.strictEqual(delayed.next, "Choose another method");
    // The shaft G(s) = 1/(s² + s): with P its error decays at B/(2J) = 0.5 1/s whatever Kp; settling by 4 s needs 1 1/s.
    const shaft = { ...model, numerator: [1], denominator: [1, 1, 0], delaySeconds: 0, revision: "shaft" };
    const undamped = refusal(shaft, { referenceChange: 1, maximumError: 0.02, settlingBand: 0.01, settlingTimeSeconds: 4, durationSeconds: 8 });
    assert.match(undamped.message, /decay rate at 0\.5 1\/s/);
    assert.match(undamped.message, /at least 1 1\/s/);
    assert.strictEqual(undamped.next, "Choose another method");
  });

  test("designs P for a delayed integrating tank and refuses it under a declared outflow change", () => {
    const loop = pidLoop();
    // A buffer tank: the level rises 0.01 m/s per V of pump command, 2 s after it; empty outlet, so it holds with 0 V.
    const tank = { ...model, numerator: [0.01], denominator: [1, 0], delaySeconds: 2, revision: "tank" };
    const level = { referenceChange: 0.1, maximumError: 0.01, settlingBand: 0.005, settlingTimeSeconds: 60, durationSeconds: 120,
      tuningObjective: "robust" as const };
    const p = tuneControllerAdapter(loop, tank, level, { family: "P", applicationMode: "manual" });
    // SIMC's more robust response, lambda = 4L = 8 s: Kp = 1/(0.01 (8 + 2)) = 10 V/m, no integral.
    assert.strictEqual(p.family, "P");
    const candidate = p.design.candidate as any;
    assert.ok(Math.abs(candidate.proportionalGain - 10) < 1e-9 && candidate.integralGain === 0, JSON.stringify(candidate));
    // The outlet pump starts with 2 V: P holds the new outflow only from an error of 2/Kp.
    assert.throws(() => tuneControllerAdapter(loop, tank, { ...level, constantInputDisturbance: -2 }, { family: "P", applicationMode: "manual" }),
      (error: Error) => /P cannot meet the requirements/.test(error.message) && /input disturbance/.test(error.message) && /integral action/.test(error.message));
    const pi = tuneTypedController(loop, tank, "PI", "manual", { ...level, constantInputDisturbance: -2 });
    assert.ok(pi.design.candidate.integralGain > 0, "PI removes the offset");
  });

  test("places the closed-loop poles for the declared settling band when no formula response settles in time", () => {
    const loop = pidLoop();
    // A shaft of 1 kg·m² with 1 N·m·s/rad friction, G(s) = 1/(s² + s), stepped 1 m with a ±1 % settling band.
    const shaft = { ...model, numerator: [1], denominator: [1, 1, 0], delaySeconds: 0, revision: "shaft" };
    const tight = { referenceChange: 1, maximumError: 0.02, settlingBand: 0.01, settlingTimeSeconds: 4, durationSeconds: 8 };
    const tuned = tuneTypedController(loop, shaft, "PD", "manual", tight);
    const prediction = engineerPrediction(shaft, loop, tuned.design.candidate, tight)!;
    const remaining = prediction.timeResponse.filter(point => point.timeSeconds >= tight.settlingTimeSeconds - 1e-9).map(point => point.candidatePv);
    const final = remaining[remaining.length - 1];
    const error = Math.max(...remaining.map(value => Math.abs(value - tight.referenceChange)));
    const movement = Math.max(...remaining.map(value => Math.abs(value - final)));
    assert.ok(error <= tight.maximumError && movement <= tight.settlingBand,
      `the PD stays within 0.02 m of the target and 0.01 m of its final value after 4 s: error ${error}, movement ${movement}`);
    // ζω = ln(1/0.01)/4 with ζ = 0.8 still overshoots through the controller's zero; 25 % faster, ω = 1.25·ln(100)/3.2
    // ≈ 1.80 rad/s, meets the band: Kp = ω² ≈ 3.24 and Kd = 2ζω − 1 ≈ 1.88.
    assert.ok(Math.abs(tuned.design.candidate.proportionalGain - (1.25 * Math.log(100) / 3.2) ** 2) < 1e-6, String(tuned.design.candidate.proportionalGain));
  });

  test("leaves an explicitly selected SIMC formula unchanged", () => {
    const loop = pidLoop();
    const tuned = tuneTypedController(loop, model, "PI", "manual", { ...task, tuningMethod: "SIMC" });
    const lambda = 4 * Math.max(0.5, 4 / 3);
    assert.ok(Math.abs(tuned.design.candidate.proportionalGain - 4 / (1.5 * (lambda + 0.5))) < 1e-6);
  });
});
