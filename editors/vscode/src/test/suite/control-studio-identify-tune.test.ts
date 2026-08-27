import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";

import {
  identifyPlant,
  modelFitEvidence,
  qualifyIdentificationData,
  type IdentificationSample,
} from "../../controlStudio/identification";
import {
  frequencyResponse,
  tuneController,
} from "../../controlStudio/tuning";
import { runSavedSimulationAutomaticTune, tuneSavedSimulationPlant } from "../../controlStudio/automaticTune";
import { decodeEngineeringCommand, decodeLoopAutomaticTuneApprove } from "../../controlStudio/messages";
import {
  candidateFromControlPid,
  selectSimplestPassingCandidate,
  updateControlPid,
  type ControlPidParameters,
  type ControlPidState,
} from "../../controlStudio/controllerCandidate";
import { buildEngineeringPlotEvidence } from "../../controlStudio/engineeringPlots";
import { compareControlPidCandidates } from "../../controlStudio/validation";

function fopdtSamples(): IdentificationSample[] {
  const result: IdentificationSample[] = [];
  const dt = 0.1;
  const delayed: number[] = Array(5).fill(0);
  let pv = 0;
  for (let index = 0; index <= 300; index += 1) {
    const input = index >= 20 ? 2 : 0;
    delayed.push(input);
    const applied = delayed.shift() ?? 0;
    pv += dt * (1.5 * applied - pv) / 4;
    result.push({ timeSeconds: index * dt, input, output: pv, quality: "good" });
  }
  return result;
}

suite("Control Studio identification and tuning", () => {
  test("qualifies data and excludes intervals without changing the raw recording", () => {
    const raw = fopdtSamples();
    const qualified = qualifyIdentificationData(raw, [{ startSeconds: 8, endSeconds: 9 }]);
    assert.strictEqual(raw.length, 301);
    assert.strictEqual(qualified.rawSamples.length, 301);
    assert.ok(qualified.samples.length < raw.length);
    assert.strictEqual(qualified.suitable, true);
    assert.strictEqual(qualified.inputUnit, "unit");
    assert.strictEqual(qualified.outputUnit, "unit");
    assert.ok(qualified.sampleTimeSeconds > 0);
  });

  test("applies selected exclusions without rewriting raw samples and labels physical evidence", () => {
    const samples = fopdtSamples();
    const original = JSON.stringify(samples);
    const result = identifyPlant(samples, [{ startSeconds: 0, endSeconds: 0.1 }], "Physical");
    assert.strictEqual(result.source, "Identified from physical data");
    assert.strictEqual(JSON.stringify(samples), original);
  });

  test("fits and selects the simplest acceptable low-order model", () => {
    const result = identifyPlant(fopdtSamples());
    assert.strictEqual(result.source, "Identified from simulation data");
    assert.strictEqual(result.recommended.kind, "first_order_dead_time");
    assert.ok(Math.abs(result.recommended.gain - 1.5) < 0.15);
    assert.ok(Math.abs(result.recommended.timeConstantSeconds! - 4) < 0.8);
    assert.ok(Math.abs(result.recommended.deadTimeSeconds! - 0.5) < 0.35);
    assert.ok(result.models.some((model) => model.kind === "integrating"));
    assert.ok(result.models.some((model) => model.kind === "first_order"));
    assert.ok(result.models.some((model) => model.kind === "second_order"));
    assert.ok(result.models.some((model) => model.kind === "second_order_dead_time"));
    assert.ok(result.models.every((model) => Number.isFinite(model.normalizedError)));
  });

  test("rejects unusable data with a deterministic reason and retains it", () => {
    const samples = [
      { timeSeconds: 0, input: 1, output: 1, quality: "good" as const },
      { timeSeconds: 1, input: 1, output: Number.NaN, quality: "bad" as const },
    ];
    const qualification = qualifyIdentificationData(samples, []);
    assert.strictEqual(qualification.suitable, false);
    assert.match(qualification.reason ?? "", /finite good samples/i);
    assert.strictEqual(qualification.rawSamples.length, 2);
    assert.throws(() => identifyPlant(samples), /finite good samples/i);
  });

  test("generates unapplied SIMC and AMIGO PI candidates with model-backed margins", () => {
    const model = identifyPlant(fopdtSamples()).recommended;
    const simc = tuneController(model, { method: "SIMC", objective: "balanced" });
    const amigo = tuneController(model, { method: "AMIGO", objective: "balanced" });
    assert.strictEqual(simc.applied, false);
    assert.strictEqual(simc.controller.structure, "PI");
    assert.ok(simc.controller.kp > 0 && simc.controller.tiSeconds! > 0);
    assert.ok(amigo.controller.kp > 0 && amigo.controller.tiSeconds! > 0);
    assert.ok(simc.frequency.gainMarginDb > 0);
    assert.ok(simc.frequency.phaseMarginDegrees > 0);
    assert.ok(simc.frequency.points.length >= 40);
  });

  test("withholds frequency results without a valid model", () => {
    assert.throws(() => frequencyResponse({
      kind: "first_order", gain: 0, stable: true, normalizedError: 0,
    }), /gain/i);
  });

  test("Automatic Tune reuses the same tune and validation services without applying", () => {
    const result = tuneSavedSimulationPlant({
      schema_version: 1, operation: "plant.read", project_identity: "fixture", project_root: "/fixture",
      saved_configuration_revision: "sha256:plant-1", saved_configuration: { plants: [{ id: "plant-1",
        model: "first_order_dead_time", parameters: { gain: 1.5, time_constant_s: 4, dead_time_s: 0.5 } }] },
    } as never, "plant-1", "balanced");
    assert.strictEqual(result.applied, false);
    assert.strictEqual(result.modelEvidence, "Exact simulation model");
    assert.deepStrictEqual(result.evaluatedStructures, ["P", "PI", "PD", "PID"]);
    assert.strictEqual(result.structure, "PI", "the simplest passing structure should win this FOPDT case");
    assert.ok(result.frequency.points.length >= 40);
    assert.ok(result.requirements.length >= 4);
    assert.ok(result.requirements.every((requirement) => requirement.status !== "Fail" && requirement.status !== "Inconclusive"));
    assert.match(result.rationale, /simplest passing/i);
    const attempts = (result as unknown as { attempts?: readonly {
      method: string; status: string; score?: number; reason?: string }[] }).attempts;
    assert.ok(attempts, "Automatic Tune does not retain method comparison attempts");
    assert.ok(attempts.some((attempt) => attempt.method === "SIMC"));
    assert.ok(attempts.some((attempt) => attempt.method === "AMIGO"));
    assert.ok(attempts.some((attempt) => attempt.method === "Constrained comparison"));
    assert.ok(attempts.every((attempt) => ["Feasible", "Rejected", "Unavailable"].includes(attempt.status)));
    assert.ok(result.validationResult.metrics?.candidate,
      "the recommendation must retain the common comparison metrics");
  });

  test("compares every method with the running controller units, limits, and timing semantics", () => {
    const current = candidateFromControlPid({
      proportionalGain: 0.667, integralGain: 0.667, derivativeGain: 0,
      proportionalSetpointWeight: 0.8, derivativeSetpointWeight: 0, bias: 0,
      outputMinimum: -10, outputMaximum: 10, rateMinimum: -25, rateMaximum: 25,
      trackingTime: 0.4, antiWindup: "Combined",
    }, "Imported");
    const tuneWithUnits = tuneSavedSimulationPlant as unknown as (
      response: Parameters<typeof tuneSavedSimulationPlant>[0], plantId: string,
      objective: "balanced", controller: ReturnType<typeof candidateFromControlPid>,
      units: { processUnit: string; outputUnit: string }
    ) => ReturnType<typeof tuneSavedSimulationPlant>;
    const result = tuneWithUnits({
      schema_version: 1, operation: "plant.read", project_identity: "fixture", project_root: "/fixture",
      saved_configuration_revision: "sha256:plant-1", saved_configuration: { plants: [{ id: "plant-1",
        model: "first_order_dead_time", parameters: { gain: 1, time_constant_s: 1, dead_time_s: 0.5 } }] },
    } as never, "plant-1", "balanced", current, { processUnit: "unit", outputUnit: "%" });
    assert.deepStrictEqual({
      proportionalSetpointWeight: result.parameters.proportionalSetpointWeight,
      derivativeSetpointWeight: result.parameters.derivativeSetpointWeight,
      bias: result.parameters.bias,
      outputMinimum: result.parameters.outputMinimum,
      outputMaximum: result.parameters.outputMaximum,
      rateMinimum: result.parameters.rateMinimum,
      rateMaximum: result.parameters.rateMaximum,
      antiWindup: result.parameters.antiWindup,
    }, {
      proportionalSetpointWeight: 0.8, derivativeSetpointWeight: 0, bias: 0,
      outputMinimum: -10, outputMaximum: 10, rateMinimum: -25, rateMaximum: 25,
      antiWindup: "Combined",
    });
    const units = Object.fromEntries(result.requirements
      .filter((requirement) => ["MV excursion", "MV rate", "Noise response"].includes(requirement.name))
      .map((requirement) => [requirement.name, requirement.unit]));
    assert.deepStrictEqual(units, {
      "MV excursion": "%", "MV rate": "%/s", "Noise response": "% RMS",
    }, "the comparison must retain the loop's controller-output engineering unit");
  });

  test("Automatic Tune stops at the failed stage and retains completed work", () => {
    const outcome = runSavedSimulationAutomaticTune({
      schema_version: 1, operation: "plant.read", project_identity: "fixture", project_root: "/fixture",
      saved_configuration_revision: "sha256:plant-1", saved_configuration: { plants: [{ id: "plant-1",
        model: "state_space", parameters: {} }] },
    } as never, "plant-1", "balanced");
    assert.strictEqual(outcome.status, "failed");
    if (outcome.status !== "failed") return;
    assert.deepStrictEqual(outcome.completedStages, ["Test"]);
    assert.strictEqual(outcome.failedStage, "Identify");
    assert.match(outcome.reason, /first-order-plus-dead-time/i);
    assert.match(outcome.recovery, /configure|record|identify/i);
  });

  test("keeps the automatic method comparison behind one compact disclosure", () => {
    const extensionRoot = path.resolve(__dirname, "..", "..", "..");
    const panel = fs.readFileSync(path.join(extensionRoot, "src", "controlStudio", "panel.ts"), "utf8");
    const workflow = fs.readFileSync(path.join(extensionRoot, "src", "controlStudio", "webview", "workflow.ts"), "utf8");
    assert.match(panel, /data-auto-tune-attempts/);
    assert.match(panel, /<summary>Method comparison<\/summary>/);
    assert.match(workflow, /result\.attempts/);
    assert.doesNotMatch(panel, /candidateFromControlPid\(result\.parameters, "SIMC"\)/,
      "Automatic Tune must retain the winning method instead of relabelling every result SIMC");
    assert.match(panel, /tuningComparison:\s*currentTuningComparison/,
      "saved results must retain the exact automatic comparison");
    assert.match(panel, /data-saved-tuning-attempts/,
      "reopened results need a hidden professional comparison disclosure");
    assert.match(workflow, /result\.tuningComparison/,
      "reopened comparisons must be rendered from the saved result");
  });

  test("accepts Automatic Tune approval from the configured loop scope", () => {
    const scope = { projectId: "project", editorSessionId: "editor", loopId: "loop", loopRevision: "sha256:loop" };
    assert.deepStrictEqual(decodeLoopAutomaticTuneApprove({ schemaVersion: 1, kind: "control-studio.automatic.approve",
      messageId: "message", projectId: "project", editorSessionId: "editor", loopId: "loop",
      expectedLoopRevision: "sha256:loop", payload: { objective: "balanced" } }, scope),
    { messageId: "message", objective: "balanced" });
  });
  test("accepts only the three concise loop-scoped engineering commands", () => {
    const scope = { projectId: "project", editorSessionId: "editor", loopId: "loop", loopRevision: "sha256:loop" };
    const base = { schemaVersion: 1, messageId: "message", projectId: "project", editorSessionId: "editor",
      loopId: "loop", expectedLoopRevision: "sha256:loop" };
    assert.deepStrictEqual(decodeEngineeringCommand({ ...base, kind: "control-studio.identify.run",
      payload: { model: "automatic" } }, scope), { action: "identify", messageId: "message", model: "automatic" });
    assert.deepStrictEqual(decodeEngineeringCommand({ ...base, kind: "control-studio.identify.run",
      payload: { model: "automatic", excludedIntervals: [{ startSeconds: 0, endSeconds: 1 }] } }, scope),
    { action: "identify", messageId: "message", model: "automatic",
      excludedIntervals: [{ startSeconds: 0, endSeconds: 1 }] });
    assert.deepStrictEqual(decodeEngineeringCommand({ ...base, kind: "control-studio.tune.run",
      payload: { method: "SIMC", objective: "balanced" } }, scope),
    { action: "tune", messageId: "message", method: "SIMC", objective: "balanced" });
    assert.deepStrictEqual(decodeEngineeringCommand({ ...base, kind: "control-studio.validate.run", payload: {} }, scope),
      { action: "validate", messageId: "message" });
    const settings = {
      proportionalGain: 2, integralGain: 0.5, derivativeGain: 0,
      proportionalSetpointWeight: 1, derivativeSetpointWeight: 0, bias: 0,
      outputMinimum: 0, outputMaximum: 100, rateMinimum: -20, rateMaximum: 20,
      trackingTime: 1, antiWindup: "BackCalculation",
    };
    assert.deepStrictEqual(decodeEngineeringCommand({ ...base, kind: "control-studio.tune.run",
      payload: { method: "Manual", settings } }, scope),
    { action: "tune-settings", messageId: "message", method: "Manual", settings });
    assert.deepStrictEqual(decodeEngineeringCommand({ ...base, kind: "control-studio.tune.run",
      payload: { method: "Imported", settings } }, scope),
    { action: "tune-settings", messageId: "message", method: "Imported", settings });
  });

  test("preserves manual and imported Control.PID settings without conversion", () => {
    const parameters: ControlPidParameters = {
      proportionalGain: 2,
      integralGain: 0.5,
      derivativeGain: 0.1,
      proportionalSetpointWeight: 0.5,
      derivativeSetpointWeight: 0,
      bias: 1,
      outputMinimum: -10,
      outputMaximum: 10,
      rateMinimum: -20,
      rateMaximum: 20,
      trackingTime: 1,
      antiWindup: "BackCalculation",
    };
    const manual = candidateFromControlPid(parameters, "Manual");
    const imported = candidateFromControlPid(parameters, "Imported");
    assert.strictEqual(manual.structure, "PID");
    assert.strictEqual(manual.applied, false);
    assert.deepStrictEqual(manual.parameters, parameters);
    assert.deepStrictEqual(imported.parameters, parameters);
    assert.notStrictEqual(imported.parameters, parameters);

    assert.strictEqual(candidateFromControlPid({ ...parameters, integralGain: 0 }, "Manual").structure, "PD");
    assert.strictEqual(candidateFromControlPid({ ...parameters, derivativeGain: 0 }, "Manual").structure, "PI");
    assert.throws(() => candidateFromControlPid({ ...parameters, trackingTime: 0 }, "Manual"), /tracking time/i);
  });

  test("predicts the published Control.PID positional and incremental vectors exactly", () => {
    const zeroState: ControlPidState = {
      setpoint: 0, setpointRate: 0, processValue: 0, processValueRate: 0,
      feedForward: 0, nominalOutput: 0, saturatedOutput: 0,
    };
    const common: ControlPidParameters = {
      proportionalGain: 2, integralGain: 0, derivativeGain: 0.5,
      proportionalSetpointWeight: 0, derivativeSetpointWeight: 0.25, bias: 1,
      outputMinimum: -100, outputMaximum: 100, rateMinimum: -1000, rateMaximum: 1000,
      trackingTime: 1, antiWindup: "BackCalculation",
    };
    const positional1 = updateControlPid(common, zeroState, {
      setpoint: 1, processValue: 0.2, feedForward: 0.3, manualOutput: 0,
      trackingOutput: 0, mode: "Auto", cycleTime: 0.1,
    });
    const positional2 = updateControlPid(common, positional1.state, {
      setpoint: 1, processValue: 0.4, feedForward: 0.3, manualOutput: 0,
      trackingOutput: 0, mode: "Auto", cycleTime: 0.2,
    });
    assert.ok(Math.abs(positional1.output - 3.15) < 1e-12);
    assert.ok(Math.abs(positional2.output - 2) < 1e-12);

    const incremental = { ...common, integralGain: 0.5, derivativeGain: 0.1,
      proportionalSetpointWeight: 0.5, derivativeSetpointWeight: 0 };
    const incremental1 = updateControlPid(incremental, zeroState, {
      setpoint: 1, processValue: 0, feedForward: 0.2, manualOutput: 0,
      trackingOutput: 0, mode: "Auto", cycleTime: 0.1,
    });
    const incremental2 = updateControlPid(incremental, incremental1.state, {
      setpoint: 1, processValue: 0.2, feedForward: 0.3, manualOutput: 0,
      trackingOutput: 0, mode: "Auto", cycleTime: 0.1,
    });
    assert.ok(Math.abs(incremental1.output - 1.25) < 1e-12);
    assert.ok(Math.abs(incremental2.output - 0.79) < 1e-12);
  });

  test("selects the simplest passing structure unless complexity has material benefit", () => {
    const parameters: ControlPidParameters = {
      proportionalGain: 1, integralGain: 1, derivativeGain: 0,
      proportionalSetpointWeight: 1, derivativeSetpointWeight: 0, bias: 0,
      outputMinimum: 0, outputMaximum: 100, rateMinimum: -100, rateMaximum: 100,
      trackingTime: 1, antiWindup: "BackCalculation",
    };
    const pi = candidateFromControlPid(parameters, "Manual");
    const pid = candidateFromControlPid({ ...parameters, derivativeGain: 0.2 }, "Manual");
    assert.strictEqual(selectSimplestPassingCandidate([
      { candidate: pi, passes: true, performanceScore: 1 },
      { candidate: pid, passes: true, performanceScore: 0.95 },
    ], 0.1)?.structure, "PI");
    assert.strictEqual(selectSimplestPassingCandidate([
      { candidate: pi, passes: true, performanceScore: 1 },
      { candidate: pid, passes: true, performanceScore: 0.7 },
    ], 0.1)?.structure, "PID");
    assert.strictEqual(selectSimplestPassingCandidate([
      { candidate: pi, passes: false, performanceScore: 1 },
      { candidate: pid, passes: false, performanceScore: 0.7 },
    ]), undefined);
  });

  test("reports unsafe frequency margins instead of coercing them positive", () => {
    const result = frequencyResponse({
      kind: "first_order_dead_time", gain: 100, timeConstantSeconds: 1,
      deadTimeSeconds: 5, stable: true, normalizedError: 0,
    });
    assert.ok(result.gainMarginDb < 0, `expected negative gain margin, got ${result.gainMarginDb}`);
  });

  test("qualifies SIMC and AMIGO against independent published-reference oracles", () => {
    const relative = (actual: number, expected: number): void => {
      assert.ok(Math.abs(actual - expected) / Math.max(Math.abs(expected), Number.EPSILON) <= 1e-7,
        `expected ${expected}, got ${actual}`);
    };
    const symmetric = { kind: "first_order_dead_time" as const, gain: 1, timeConstantSeconds: 1,
      deadTimeSeconds: 1, stable: true, normalizedError: 0 };
    const simc = tuneController(symmetric, { method: "SIMC", objective: "balanced" });
    const amigo = tuneController(symmetric, { method: "AMIGO", objective: "balanced" });
    relative(simc.controller.kp, 1 / 3);
    relative(simc.controller.tiSeconds!, 1);
    relative(simc.controller.kp / simc.controller.tiSeconds!, 1 / 3);
    relative(amigo.controller.kp, 0.25);
    relative(amigo.controller.tiSeconds!, 1);
    relative(amigo.controller.kp / amigo.controller.tiSeconds!, 0.25);

    const asymmetric = { ...symmetric, gain: 2, timeConstantSeconds: 10 };
    const asymmetricAmigo = tuneController(asymmetric, { method: "AMIGO", objective: "fast" });
    relative(asymmetricAmigo.controller.kp, 1.4117768595);
    relative(asymmetricAmigo.controller.tiSeconds!, 6.0768722467);
    relative(asymmetricAmigo.controller.kp / asymmetricAmigo.controller.tiSeconds!,
      1.4117768595 / 6.0768722467);
  });

  test("maps only SIMC response objectives and rejects models outside each published domain", () => {
    const model = { kind: "first_order_dead_time" as const, gain: 1, timeConstantSeconds: 1,
      deadTimeSeconds: 0.5, stable: true, normalizedError: 0 };
    const fast = tuneController(model, { method: "SIMC", objective: "fast" });
    const balanced = tuneController(model, { method: "SIMC", objective: "balanced" });
    const robust = tuneController(model, { method: "SIMC", objective: "robust" });
    assert.ok(fast.controller.kp > balanced.controller.kp);
    assert.ok(balanced.controller.kp > robust.controller.kp);
    const amigoFast = tuneController(model, { method: "AMIGO", objective: "fast" });
    const amigoRobust = tuneController(model, { method: "AMIGO", objective: "robust" });
    assert.deepStrictEqual(amigoFast.controller, amigoRobust.controller,
      "response labels must not silently rewrite the published AMIGO formula");
    assert.throws(() => tuneController({ ...model, stable: false },
      { method: "SIMC", objective: "balanced" }), /stable/i);
    assert.throws(() => tuneController({ ...model, gain: -1 },
      { method: "SIMC", objective: "balanced" }), /positive gain/i);
    assert.throws(() => tuneController({ ...model, normalizedError: 0.3 },
      { method: "SIMC", objective: "balanced" }), /model error/i);
    assert.throws(() => tuneController({ ...model, deadTimeSeconds: 0 },
      { method: "AMIGO", objective: "balanced" }), /resolved positive dead time/i);
    assert.throws(() => tuneController({ ...model, deadTimeSeconds: 2 },
      { method: "AMIGO", objective: "balanced" }), /delay.*domain/i);
  });

  test("keeps proportional control eligible for integrating plants", () => {
    const parameters: ControlPidParameters = {
      proportionalGain: 0.8, integralGain: 0, derivativeGain: 0,
      proportionalSetpointWeight: 1, derivativeSetpointWeight: 0, bias: 0,
      outputMinimum: -10, outputMaximum: 10, rateMinimum: -100, rateMaximum: 100,
      trackingTime: 1, antiWindup: "BackCalculation",
    };
    const proportional = candidateFromControlPid(parameters, "Manual");
    assert.strictEqual(proportional.structure, "P");
  });

  test("exposes one deterministic qualified-method comparison service", () => {
    const tuning = require("../../controlStudio/tuning") as Record<string, unknown>;
    assert.strictEqual(typeof tuning.compareQualifiedTunings, "function",
      "qualified tuning comparison is not implemented");
  });

  test("retains the complete common performance and actuator metric set", () => {
    const model = { kind: "first_order_dead_time" as const, gain: 1, timeConstantSeconds: 1,
      deadTimeSeconds: 0.5, stable: true, normalizedError: 0 };
    const base: ControlPidParameters = {
      proportionalGain: 0.667, integralGain: 0.667, derivativeGain: 0,
      proportionalSetpointWeight: 1, derivativeSetpointWeight: 0, bias: 0,
      outputMinimum: -10, outputMaximum: 10, rateMinimum: -100, rateMaximum: 100,
      trackingTime: 1, antiWindup: "BackCalculation",
    };
    const result = compareControlPidCandidates(model,
      candidateFromControlPid(base, "Imported"), candidateFromControlPid(base, "SIMC"), {
        evidence: "Model-predicted",
        scenarios: [{ kind: "setpoint", amplitude: 1 }, { kind: "disturbance", amplitude: 0.2 },
          { kind: "noise", amplitude: 0.02 }],
        variationFractions: [-0.2, 0, 0.2], outputUnit: "%",
      });
    const metrics = (result as unknown as { metrics?: { candidate?: Record<string, number | boolean> } }).metrics?.candidate;
    assert.ok(metrics, "validation does not retain candidate metrics");
    for (const name of ["iae", "ise", "itae", "mvMovement", "peakAbsoluteMv", "maximumMvRate",
      "noiseMvRms", "worstCaseVariationIae"] as const) {
      assert.ok(Number.isFinite(metrics?.[name]), `${name} must be finite`);
    }
  });

  test("ranks feasible candidates by objective with a stable documented tie rule", () => {
    const candidates = require("../../controlStudio/controllerCandidate") as {
      rankQualifiedCandidates?: (items: readonly unknown[], objective: "balanced") => {
        winner?: { source: string; structure: string }; ranked: readonly { id: string; feasible: boolean }[] };
    };
    assert.strictEqual(typeof candidates.rankQualifiedCandidates, "function");
    const base: ControlPidParameters = {
      proportionalGain: 1, integralGain: 1, derivativeGain: 0,
      proportionalSetpointWeight: 1, derivativeSetpointWeight: 0, bias: 0,
      outputMinimum: -10, outputMaximum: 10, rateMinimum: -100, rateMaximum: 100,
      trackingTime: 1, antiWindup: "BackCalculation",
    };
    const metric = { iae: 1, ise: 1, itae: 1, mvMovement: 1, peakAbsoluteMv: 1,
      maximumMvRate: 1, noiseMvRms: 1, worstCaseVariationIae: 1,
      gainMarginDb: 10, phaseMarginDegrees: 45 };
    const ranked = candidates.rankQualifiedCandidates!([
      { id: "amigo", method: "AMIGO", candidate: candidateFromControlPid(base, "AMIGO"),
        feasible: true, metrics: metric },
      { id: "rejected", method: "Constrained comparison",
        candidate: candidateFromControlPid({ ...base, proportionalGain: 10 }, "Constrained comparison"),
        feasible: false, metrics: { ...metric, iae: 0.01 } },
      { id: "simc", method: "SIMC", candidate: candidateFromControlPid(base, "SIMC"),
        feasible: true, metrics: metric },
    ], "balanced");
    assert.strictEqual(ranked.winner?.source, "SIMC");
    assert.deepStrictEqual(ranked.ranked.map((item) => item.id), ["simc", "amigo", "rejected"]);
    assert.strictEqual(ranked.ranked[2].feasible, false);

    const proportional = candidateFromControlPid({ ...base, integralGain: 0 }, "Constrained comparison");
    const pid = candidateFromControlPid({ ...base, derivativeGain: 0.2 }, "Constrained comparison");
    const complexityResult = candidates.rankQualifiedCandidates!([
      { id: "p", method: "Constrained comparison", candidate: proportional,
        feasible: true, metrics: { ...metric, iae: 1.05, ise: 1.05, itae: 1.05,
          mvMovement: 1.05, peakAbsoluteMv: 1.05, maximumMvRate: 1.05,
          noiseMvRms: 1.05, worstCaseVariationIae: 1.05 } },
      { id: "pid", method: "Constrained comparison", candidate: pid,
        feasible: true, metrics: metric },
    ], "balanced");
    assert.strictEqual(complexityResult.winner?.structure, "P",
      "a more complex controller must not win without at least ten percent material benefit");
  });

  test("retains measured, predicted, residual, range, delay, units, and warnings as model evidence", () => {
    const samples = fopdtSamples().map((sample) => ({ ...sample, inputUnit: "%", outputUnit: "degC" }));
    const identified = identifyPlant(samples);
    const evidence = modelFitEvidence(identified.recommended, samples);
    assert.strictEqual(evidence.points.length, samples.length);
    assert.ok(evidence.points.every((point) => Number.isFinite(point.predicted) && Number.isFinite(point.residual)));
    assert.ok(evidence.rootMeanSquareError >= 0);
    assert.strictEqual(evidence.outputUnit, "degC");
    assert.strictEqual(evidence.stable, true);
    assert.ok(evidence.delaySeconds >= 0);
    assert.deepStrictEqual(evidence.operatingRange, {
      inputMinimum: 0, inputMaximum: 2, outputMinimum: 0, outputMaximum: samples[samples.length - 1].output,
    });
  });

  test("builds bounded current/candidate time and Bode evidence from the same model", () => {
    const model = identifyPlant(fopdtSamples()).recommended;
    const base: ControlPidParameters = {
      proportionalGain: 0.4, integralGain: 0.1, derivativeGain: 0,
      proportionalSetpointWeight: 1, derivativeSetpointWeight: 0, bias: 0,
      outputMinimum: 0, outputMaximum: 100, rateMinimum: -100, rateMaximum: 100,
      trackingTime: 1, antiWindup: "BackCalculation",
    };
    const evidence = buildEngineeringPlotEvidence(
      model,
      candidateFromControlPid(base, "Imported"),
      candidateFromControlPid({ ...base, proportionalGain: 0.8, integralGain: 0.2 }, "Manual")
    );
    assert.strictEqual(evidence.evidence, "Model-predicted");
    assert.ok(evidence.timeResponse.length >= 100 && evidence.timeResponse.length <= 2000);
    assert.ok(evidence.timeResponse.every((point) => [point.timeSeconds, point.currentPv, point.candidatePv,
      point.currentMv, point.candidateMv].every(Number.isFinite)));
    assert.ok(evidence.plantBode.length >= 40);
    assert.strictEqual(evidence.candidateOpenLoopBode.length, evidence.plantBode.length);
    assert.ok(Number.isFinite(evidence.gainMarginDb));
    assert.ok(Number.isFinite(evidence.phaseMarginDegrees));
  });

  test("reports an unbounded gain margin when a stable no-delay loop has no phase crossover", () => {
    const model = {
      kind: "first_order" as const, gain: 1.45928, timeConstantSeconds: 4,
      stable: true, normalizedError: 0.0998,
    };
    const base: ControlPidParameters = {
      proportionalGain: 0.4, integralGain: 0.1, derivativeGain: 0,
      proportionalSetpointWeight: 1, derivativeSetpointWeight: 0, bias: 0,
      outputMinimum: 0, outputMaximum: 100, rateMinimum: -100, rateMaximum: 100,
      trackingTime: 1, antiWindup: "BackCalculation",
    };
    const current = candidateFromControlPid(base, "Imported");
    const candidate = candidateFromControlPid(
      { ...base, proportionalGain: 0.57285, integralGain: 0.38139 }, "SIMC");
    const evidence = buildEngineeringPlotEvidence(model, current, candidate);
    assert.strictEqual(evidence.gainMarginDb, Number.POSITIVE_INFINITY);
    assert.ok(Number.isFinite(evidence.phaseMarginDegrees));
    assert.ok(evidence.candidateOpenLoopBode.length >= 40);
    const messageEvidence = (require("../../controlStudio/engineeringPlots") as {
      engineeringPlotMessageEvidence?: (value: unknown) => Record<string, unknown>;
    }).engineeringPlotMessageEvidence;
    assert.strictEqual(typeof messageEvidence, "function", "unbounded margins need an explicit message representation");
    assert.strictEqual(messageEvidence?.(evidence).gainMarginDb, "unbounded");
    const validation = compareControlPidCandidates(model, current, candidate, {
      evidence: "Model-predicted", scenarios: [{ kind: "setpoint", amplitude: 1 }],
      variationFractions: [0],
    });
    const gainMargin = validation.requirements.find((item) => item.name === "Gain margin");
    assert.strictEqual(gainMargin?.status, "Pass");
    assert.strictEqual(gainMargin?.value, Number.POSITIVE_INFINITY,
      "the rendered requirement must state that the passing margin is unbounded");
  });
});
