import * as assert from "assert";
import { IdentificationWorkflow, identifiedModelPresentation, prepareIdentificationReview, type IdentificationSample } from "../../controlStudio/identification";
import { StudioAnalysisSessions } from "../../controlStudio/studioAnalysisSessions";
import { decodeEngineeringCommand } from "../../controlStudio/messages";
import type { StepResponseSettings } from "../../controlStudio/stepResponseIdentification";

const settings = {
  baselineDurationSeconds: 5, finalDurationSeconds: 5,
  maximumWindowRange: .1, maximumWindowSlopePerSecond: .01,
  measurementBound: .05, onsetThreshold: .15, onsetSamples: 3,
  delay: { minimum: 0, maximum: 3, step: .5 },
  timeConstant: { minimum: 1, maximum: 9, step: .5 },
  sensorTimeConstant: { minimum: .5, maximum: 6, step: .5 },
  baselineStep: .01, gainStep: .001, timestampShiftSeconds: 0,
};

function recording(water = 5, sensor = 2, gain = .2, delay = 1): IdentificationSample[] {
  return Array.from({ length: 252 }, (_, i) => {
    const time = i / 5 - 5, elapsed = Math.max(0, time - delay);
    const fraction = sensor === 0 ? 1 - Math.exp(-elapsed / water)
      : sensor === water ? 1 - (1 + elapsed / water) * Math.exp(-elapsed / water)
      : 1 - water / (water - sensor) * Math.exp(-elapsed / water)
        + sensor / (water - sensor) * Math.exp(-elapsed / sensor);
    return { timeSeconds: time, input: time >= 0 && time < 45 ? 85 : 75,
      output: 35 + 10 * gain * fraction, quality: "good", inputUnit: "%", outputUnit: "degC" };
  });
}

function review(samples: IdentificationSample[], suppliedSettings: StepResponseSettings = settings, requestedKind: "sopdt" | "fopdt" = "sopdt"): any {
  return prepareIdentificationReview({ estimation: samples, excluded: [], source: "Simulation",
    requestedKind, estimationRecordingId: "retained-estimation", stepResponseSettings: suppliedSettings });
}

suite("Control Studio sensor-lag comparison", () => {
  test("carries both complete candidate fits to the plot presentation", () => {
    const result = review(recording());
    const presentation: any = identifiedModelPresentation(result.selected, result.fit, "%", "degC");
    assert.ok(presentation.modelStructure, "paired plots need the actual two candidate fits from this review");
    for (const name of ["candidateA", "candidateB"]) {
      const candidate = presentation.modelStructure[name];
      assert.ok(candidate.model);
      assert.match(candidate.equation ?? "", /^G\(s\) = /, "each candidate must expose its own model equation");
      assert.ok(candidate.fit.points.some((point: any) => point.timeSeconds < candidate.analysis.stepTimeSeconds),
        "each candidate must carry the baseline as well as the held response");
      assert.strictEqual(candidate.fit.modelStructure, undefined, "candidate fits cannot recursively contain their comparison");
      assert.deepStrictEqual(candidate.fit.points.map((point: any) => [point.timeSeconds, point.measured]),
        presentation.predictedResponse.map((point: any) => [point.timeSeconds, point.measured]));
    }
  });

  test("requires an explicit provisional choice, preserves it on worker replay, and clears it when inputs change", async () => {
    const sessions = new StudioAnalysisSessions();
    try {
      sessions.qualifyIdentification(recording(5, 0), [], "Simulation", "sopdt", "first-order-record", settings);
      const before = await sessions.fitIdentification();
      assert.strictEqual((before?.fit.modelStructure as any)?.choice, undefined);
      const scope = { projectId: "p", loopId: "l", loopRevision: "r", editorSessionId: "e" };
      const command = decodeEngineeringCommand({ schemaVersion: 1, kind: "control-studio.identify.choose-structure", messageId: "choice",
        projectId: "p", loopId: "l", expectedLoopRevision: "r", editorSessionId: "e", payload: { choice: "first_order" } }, scope);
      assert.strictEqual(command?.action, "identify-choose-structure");
      assert.strictEqual(typeof (sessions as any).chooseIdentificationStructure, "function");
      (sessions as any).chooseIdentificationStructure("first_order");
      const saved: any = sessions.identification.snapshot();
      assert.strictEqual(saved.structureChoice, "first_order");
      assert.strictEqual(sessions.identification.currentState.phase, "preview");
      assert.strictEqual(sessions.identification.acceptedModel, undefined);
      assert.strictEqual(sessions.model.selectedModel(), undefined);
      assert.strictEqual(await sessions.restoreIdentificationAsync({ identificationStudy: saved }), true);
      assert.strictEqual((sessions.identification.currentState.review?.fit.modelStructure as any)?.choice, "first_order");
      sessions.qualifyIdentification(recording(5, 0), [], "Simulation", "sopdt", "first-order-record", { ...settings, gainStep: .0005 });
      assert.strictEqual((sessions.identification.snapshot() as any).structureChoice, undefined);
    } finally { sessions.dispose(); }
  });

  test("rejects an unsupported extra state without altering the retained comparison", () => {
    const workflow = new IdentificationWorkflow();
    workflow.qualify(recording(5, 0), [], "Simulation", "sopdt", "first-order-record", settings); workflow.fit();
    assert.strictEqual(typeof (workflow as any).chooseStructure, "function");
    const before = workflow.snapshot();
    assert.throws(() => (workflow as any).chooseStructure("additional_lag"), /first.order|supported|adequate/i);
    assert.deepStrictEqual(workflow.snapshot(), before);
  });

  test("retains a supported additional lag as provisional with unresolved physical origin", () => {
    const workflow = new IdentificationWorkflow();
    const strict = { ...settings, measurementBound: .01, modelAdequacy: { maximumAbsoluteResidual: .02, maximumFinalError: .01 } };
    workflow.qualify(recording(), [], "Simulation", "sopdt", "two-lag-record", strict);
    const comparison = workflow.fit().fit.modelStructure!;
    assert.strictEqual(comparison.recommended, "additional_lag");
    assert.strictEqual(typeof (workflow as any).chooseStructure, "function");
    (workflow as any).chooseStructure("additional_lag");
    const selected = workflow.currentState.review!;
    assert.strictEqual(selected.selected.kind, "second_order_dead_time");
    assert.strictEqual((selected.fit.modelStructure as any).choice, "additional_lag");
    assert.strictEqual(selected.fit.modelStructure?.physicalOrigin, "unresolved");
    assert.strictEqual(workflow.acceptedModel, undefined);
    assert.throws(() => workflow.accept(), /Independent validation/);
  });

  test("refits both physical lags while retaining the first-order reference and source record", () => {
    const samples = recording(), original = JSON.stringify(samples);
    const firstOrder = review(samples, settings, "fopdt");
    const result = review(samples);
    assert.strictEqual(result.stage, "validation_required");
    assert.strictEqual(result.accepted, false);
    assert.strictEqual(result.preview.models.length, 2);
    const comparison = result.fit.modelStructure;
    assert.ok(comparison, "the bounded SOPDT request must produce the physical structure comparison");
    assert.deepStrictEqual(comparison.candidateA.analysis, firstOrder.stepResponse);
    assert.deepStrictEqual(result.preview.models[0], firstOrder.selected);
    const best = comparison.candidateB.analysis.best;
    assert.deepStrictEqual([best.timeConstantSeconds, best.secondTimeConstantSeconds].sort((a, b) => a - b), [2, 5]);
    assert.strictEqual(best.delaySeconds, 1);
    assert.ok(Math.abs(best.gain - .2) < .0001);
    assert.ok(comparison.candidateB.analysis.consistency.count > 0);
    assert.strictEqual(comparison.physicalOrigin, "unresolved");
    assert.strictEqual(comparison.provisional, true);
    assert.strictEqual(JSON.stringify(samples), original);
  });

  test("keeps the simplest adequate first-order fit without confusing measurement and approximation bounds", () => {
    const result = review(recording(5, 0));
    const comparison = result.fit.modelStructure;
    assert.strictEqual(comparison.candidateA.adequate, true);
    assert.strictEqual(comparison.recommended, "first_order");
    assert.strictEqual(result.preview.recommended.kind, "first_order_dead_time");
    assert.strictEqual(comparison.requirements.maximumAbsoluteResidual, .2);
    assert.strictEqual(comparison.requirements.maximumFinalError, .1);
    assert.strictEqual(comparison.candidateB.analysis.settings.measurementBound, .05);
  });

  test("fits the repeated-pole limit without nonfinite predictions", () => {
    const comparison = review(recording(3, 3)).fit.modelStructure;
    const best = comparison.candidateB.analysis.best;
    assert.strictEqual(best.timeConstantSeconds, 3);
    assert.strictEqual(best.secondTimeConstantSeconds, 3);
    assert.ok(comparison.candidateB.analysis.points.every((point: any) => Number.isFinite(point.predicted)));
    assert.ok(comparison.candidateB.maximumAbsoluteResidual < .001);
  });

  test("retains both physically indistinguishable time-constant assignments in the consistent grid", () => {
    const equalRanges = { ...settings, timeConstant: { minimum: 1, maximum: 6, step: 1 },
      sensorTimeConstant: { minimum: 1, maximum: 6, step: 1 } };
    const left = review(recording(5, 2), equalRanges).fit.modelStructure;
    const right = review(recording(2, 5), equalRanges).fit.modelStructure;
    assert.ok(Math.abs(left.candidateB.analysis.best.squaredError - right.candidateB.analysis.best.squaredError) < 1e-9);
    for (const interval of [left.candidateB.analysis.consistency.timeConstantSeconds,
      left.candidateB.analysis.consistency.secondTimeConstantSeconds]) {
      assert.ok(interval.minimum <= 2 && interval.maximum >= 5);
    }
    assert.strictEqual(left.physicalOrigin, "unresolved");
  });

  test("fits a falling temperature response with signed gain on the same positive lag grid", () => {
    const comparison = review(recording(5, 2, -.2)).fit.modelStructure;
    assert.ok(Math.abs(comparison.candidateB.analysis.best.gain + .2) < .0001);
    assert.strictEqual(comparison.candidateB.adequate, true);
  });

  test("matches exhaustive grid fitting and every measurement-consistent combination", () => {
    const grids = { ...settings, timeConstant: { minimum: 4, maximum: 6, step: 1 },
      sensorTimeConstant: { minimum: 1, maximum: 3, step: 1 }, delay: { minimum: .5, maximum: 1.5, step: .5 },
      baselineStep: .05, gainStep: .005 };
    const samples = recording();
    const analysis = review(samples, grids).fit.modelStructure.candidateB.analysis;
    const valid = samples.filter(sample => sample.timeSeconds < 45);
    const held = valid.filter(sample => sample.timeSeconds >= 0);
    const gainMinimum = (analysis.final.mean - 35 - .1) / 10;
    let count = 0, bestError = Infinity;
    const accepted: Array<{ gain: number; baseline: number; delaySeconds: number; timeConstantSeconds: number; secondTimeConstantSeconds: number }> = [];
    for (const baseline of [34.95, 35, 35.05]) for (let g = 0; g <= 4; g += 1) {
      const gain = gainMinimum + g * .005;
      if (Math.abs(baseline + 10 * gain - analysis.final.mean) > .05 + 1e-9) continue;
      for (const delay of [.5, 1, 1.5]) for (const water of [4, 5, 6]) for (const sensor of [1, 2, 3]) {
        const predict = (time: number) => {
          const elapsed = Math.max(0, time - delay);
          return baseline + 10 * gain * (1 - water / (water - sensor) * Math.exp(-elapsed / water)
            + sensor / (water - sensor) * Math.exp(-elapsed / sensor));
        };
        bestError = Math.min(bestError, held.reduce((sum, sample) => sum + (sample.output - predict(sample.timeSeconds)) ** 2, 0));
        if (valid.every(sample => Math.abs(sample.output - predict(sample.timeSeconds)) <= .05 + 1e-9)) {
          count += 1;
          accepted.push({ gain, baseline, delaySeconds: delay, timeConstantSeconds: water, secondTimeConstantSeconds: sensor });
        }
      }
    }
    assert.ok(count > 0);
    assert.strictEqual(analysis.consistency.count, count);
    assert.ok(Math.abs(analysis.best.squaredError - bestError) < 1e-9);
    for (const key of ["gain", "baseline", "delaySeconds", "timeConstantSeconds", "secondTimeConstantSeconds"] as const) {
      assert.ok(Math.abs(analysis.consistency[key].minimum - Math.min(...accepted.map(candidate => candidate[key]))) < 1e-9);
      assert.ok(Math.abs(analysis.consistency[key].maximum - Math.max(...accepted.map(candidate => candidate[key]))) < 1e-9);
    }
  });

  test("retains an empty measurement-consistent set even when the model is adequate for a looser task", () => {
    const samples = recording();
    samples[60].output += .15;
    const result = review(samples, { ...settings, measurementBound: .005, baselineStep: .001, gainStep: .0001 });
    assert.strictEqual(result.fit.modelStructure.candidateB.analysis.consistency.count, 0);
    assert.strictEqual(result.fit.modelStructure.candidateB.analysis.consistency.gain, undefined);
    assert.strictEqual(result.fit.modelStructure.candidateB.adequate, true);
    assert.ok(result.fit.modelStructure.candidateB.analysis.warnings.some((warning: string) => /measurement bound/.test(warning)));
  });

  test("states the unresolved origin of an additional lag without naming a domain", () => {
    const warnings: string[] = review(recording(), settings).fit.modelStructure.candidateB.analysis.warnings;
    assert.strictEqual(warnings[0], "Additional lag is a hypothesis; the recording alone cannot assign the two time constants to the process and the sensor.");
    assert.deepStrictEqual(warnings.filter(warning => /water|heater|tank|temperature/i.test(warning)), [], "Control Studio serves any plant");
  });

  test("rejects malformed or excessive sensor grids with the original record untouched", () => {
    const samples = recording(), before = JSON.stringify(samples);
    for (const sensorTimeConstant of [
      { minimum: 0, maximum: 10, step: .1 }, { minimum: 2, maximum: 1, step: .1 },
      { minimum: .1, maximum: 10, step: .13 }, { minimum: .1, maximum: 10000, step: .1 },
    ]) assert.throws(() => review(samples, { ...settings, sensorTimeConstant }), /settings|search.*large/i);
    assert.strictEqual(JSON.stringify(samples), before);
  });

  test("reopens the same provisional comparison through the existing identification session", () => {
    const workflow = new IdentificationWorkflow();
    assert.strictEqual(workflow.qualify(recording(), [], "Simulation", "sopdt", "retained-estimation", settings).suitable, true);
    const original = workflow.fit();
    const saved = workflow.snapshot();
    const reopened = new IdentificationWorkflow();
    reopened.restore(saved);
    assert.deepStrictEqual(reopened.currentState.review, original);
    assert.strictEqual(reopened.acceptedModel, undefined);
    assert.throws(() => reopened.accept(), /Independent validation/);
    assert.deepStrictEqual(reopened.snapshot(), saved);
  });

  test("changing from first-order fitting to structure comparison reuses estimation instead of treating it as validation", () => {
    const workflow = new IdentificationWorkflow(), samples = recording();
    workflow.qualify(samples, [], "Simulation", "fopdt", "retained-estimation", settings);
    const reference = workflow.fit();
    assert.strictEqual(workflow.qualify(samples, [], "Simulation", "sopdt", "retained-estimation", settings).suitable, true);
    assert.strictEqual(workflow.nextFitRole, "estimation");
    const comparison = workflow.fit();
    assert.deepStrictEqual(comparison.fit.modelStructure?.candidateA.analysis, reference.stepResponse);
    assert.strictEqual(comparison.stage, "validation_required");
  });

  test("uses the complete declared default sensor-time grid without substituting nominal parameters", function () {
    this.timeout(60000);
    const { sensorTimeConstant: _unused, ...defaults } = settings;
    const result = review(recording(5, 0), { ...defaults,
      delay: { minimum: 0, maximum: 2, step: .1 }, timeConstant: { minimum: .1, maximum: 20, step: .1 } });
    assert.deepStrictEqual(result.fit.modelStructure.candidateB.analysis.settings.sensorTimeConstant,
      { minimum: .1, maximum: 10, step: .1 });
    assert.strictEqual(result.fit.modelStructure.recommended, "first_order");
  });

  test("supports the added lag when the simpler fit fails the declared task and improvement exceeds measurement uncertainty", () => {
    const comparison = review(recording(), { ...settings, measurementBound: .005, baselineStep: .001, gainStep: .0001,
      modelAdequacy: { maximumAbsoluteResidual: .01, maximumFinalError: .01 } }).fit.modelStructure;
    assert.strictEqual(comparison.candidateA.adequate, false);
    assert.strictEqual(comparison.candidateB.adequate, true);
    assert.ok(comparison.rmsImprovement > .005);
    assert.strictEqual(comparison.recommended, "additional_lag");
    assert.strictEqual(comparison.provisional, true);
  });

  test("rejects a malformed saved sensor grid atomically without replacing the current comparison", () => {
    const workflow = new IdentificationWorkflow();
    workflow.qualify(recording(), [], "Simulation", "sopdt", "retained-estimation", settings);
    workflow.fit();
    const before = workflow.snapshot();
    const malformed = JSON.parse(JSON.stringify(before));
    malformed.current.stepResponseSettings.sensorTimeConstant.minimum = -1;
    assert.throws(() => workflow.restore(malformed), /malformed|incompatible/);
    assert.deepStrictEqual(workflow.snapshot(), before);
  });

  test("reports a free baseline hitting its declared uncertainty boundary", () => {
    const samples = recording();
    samples.forEach(sample => { if (sample.timeSeconds < 0) sample.output += .05; });
    const analysis = review(samples).fit.modelStructure.candidateB.analysis;
    assert.ok(Math.abs(analysis.best.baseline - (analysis.baseline.mean - .05)) < 1e-9);
    assert.strictEqual(analysis.boundaryLimited, true);
    assert.ok(analysis.warnings.some((warning: string) => /baseline|gain/.test(warning)));
  });

  test("retains a one-sided zero-delay finding without treating fixed zero delay as an unresolved positive boundary", () => {
    const analysis = review(recording(5, 2, .2, 0), { ...settings,
      delay: { minimum: 0, maximum: 0, step: .1 } }).fit.modelStructure.candidateB.analysis;
    assert.strictEqual(analysis.best.delaySeconds, 0);
    assert.strictEqual(analysis.zeroDelayLowerBound, true);
    assert.strictEqual(analysis.boundaryLimited, false);
  });

  test("refining Candidate B keeps Candidate A's original fit and uncertainty through saved-session replay", () => {
    const workflow = new IdentificationWorkflow(), samples = recording();
    workflow.qualify(samples, [], "Simulation", "fopdt", "retained-estimation", settings);
    const reference = workflow.fit();
    const refined = { ...settings, timeConstant: { ...settings.timeConstant, step: .25 },
      sensorTimeConstant: { ...settings.sensorTimeConstant, step: .25 } };
    workflow.qualify(samples, [], "Simulation", "sopdt", "retained-estimation", refined);
    const comparison = workflow.fit();
    assert.deepStrictEqual(comparison.fit.modelStructure?.candidateA.analysis, reference.stepResponse,
      "refining B must not recalculate A on B's new search grid");
    assert.deepStrictEqual(comparison.fit.modelStructure?.candidateB.analysis.settings.timeConstant, refined.timeConstant);
    const reopened = new IdentificationWorkflow();
    reopened.restore(workflow.snapshot());
    assert.deepStrictEqual(reopened.currentState.review, comparison);
  });

  test("requires a new first-order reference when the comparison changes the qualified windows", () => {
    const workflow = new IdentificationWorkflow(), samples = recording();
    workflow.qualify(samples, [], "Simulation", "fopdt", "retained-estimation", settings);
    workflow.fit();
    workflow.qualify(samples, [], "Simulation", "sopdt", "retained-estimation", { ...settings, baselineDurationSeconds: 4 });
    assert.throws(() => workflow.fit(), /first-order reference|same.*window/i);
    assert.strictEqual(workflow.snapshot()?.current.estimation.length, samples.length);
  });
});
