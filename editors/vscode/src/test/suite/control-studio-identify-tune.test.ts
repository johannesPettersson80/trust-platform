import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";

import {
  IdentificationWorkflow,
  identifyPlant,
  modelFitEvidence,
  qualifyIdentificationData,
  type IdentificationSample,
} from "../../controlStudio/identification";
import { identificationPresentationState } from "../../controlStudio/identificationPresentationState";
import {
  describeTuningMethod,
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

function fopdtSamples(amplitude = 2): IdentificationSample[] {
  const result: IdentificationSample[] = [];
  const dt = 0.1;
  const delayed: number[] = Array(5).fill(0);
  let pv = 0;
  for (let index = 0; index <= 300; index += 1) {
    const input = index >= 20 ? amplitude : 0;
    delayed.push(input);
    const applied = delayed.shift() ?? 0;
    pv += dt * (1.5 * applied - pv) / 4;
    result.push({ timeSeconds: index * dt, input, output: pv, quality: "good" });
  }
  return result;
}

suite("Control Studio identification and tuning", () => {
  test("named root-locus sweep varies Kp while retaining measured-velocity feedback", () => {
    const model = { kind: "integrating" as const, gain: 1, timeConstantSeconds: 1, stable: true, normalizedError: 0 };
    const parameters: ControlPidParameters = {
      proportionalGain: 12, integralGain: 0, derivativeGain: 7,
      proportionalSetpointWeight: 1, derivativeSetpointWeight: 0, bias: 0,
      outputMinimum: -20, outputMaximum: 20, rateMinimum: -1000, rateMaximum: 1000,
      trackingTime: 1, antiWindup: "BackCalculation",
    };
    const candidate = candidateFromControlPid(parameters, "Manual");
    const evidence = buildEngineeringPlotEvidence(model, candidate, candidate, {
      parameter: "proportionalGain", values: [12, 16, 20], selectedValue: 16,
    });
    assert.strictEqual(evidence.rootLocus.status, "qualified");
    assert.deepStrictEqual(evidence.rootLocus.branches.map(branch => branch.gain), [12, 16, 20], "sweep uses the declared parameter values");
    assert.strictEqual(evidence.rootLocus.parameter, "proportionalGain");
    assert.strictEqual(evidence.rootLocus.fixedParameters.derivativeGain, 7);
    const expected = [[[-6, 0], [-2, 0]], [[-4, 0], [-4, 0]], [[-4, -2], [-4, 2]]];
    evidence.rootLocus.branches.forEach((branch, index) => {
      const poles = [...branch.poles].sort((a, b) => a.real-b.real || a.imaginary-b.imaginary);
      poles.forEach((pole, p) => {
        assert.ok(Math.abs(pole.real - expected[index][p][0]) < 1e-6);
        assert.ok(Math.abs(pole.imaginary - expected[index][p][1]) < 1e-6);
      });
    });
    assert.strictEqual(evidence.rootLocus.selectedValue, 16);
    const comparison = (evidence as any).comparison;
    assert.ok(comparison?.context, "plot evidence must carry its authoritative comparison context");
    assert.deepStrictEqual(comparison.candidate, candidate);
    const next = candidateFromControlPid({ ...parameters, proportionalGain: 20 }, "Manual");
    assert.deepStrictEqual((buildEngineeringPlotEvidence(model, candidate, next) as any).comparison.context, comparison.context,
      "candidate edits retain one common model, baseline, and comparison method");
    assert.notDeepStrictEqual((buildEngineeringPlotEvidence({ ...model, gain: 1.1 }, candidate, next) as any).comparison.context, comparison.context,
      "a changed model invalidates comparisons");
    assert.notDeepStrictEqual((buildEngineeringPlotEvidence(model, next, next) as any).comparison.context, comparison.context,
      "a changed runtime baseline invalidates comparisons");
  });

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
    const qualification = qualified as any;
    assert.deepStrictEqual(Object.keys(qualification.metrics ?? {}), [
      "rawSampleCount", "retainedSampleCount", "durationSeconds", "medianSampleTimeSeconds",
      "maximumSampleTimeDeviationPercent", "gapCount", "rejectedQualitySampleCount",
      "clippedSampleCount", "saturationSampleCount", "baselineDriftPerSecond", "inputSpan",
      "outputSpan", "signalToNoiseDb", "excitation", "coherence", "independentValidationAvailable",
    ]);
    assert.strictEqual(qualification.metrics?.rawSampleCount, 301);
    assert.strictEqual(qualification.metrics?.retainedSampleCount, qualified.samples.length);
    assert.ok(qualification.metrics?.durationSeconds > 20);
    assert.strictEqual(qualification.metrics?.gapCount, 1,
      "excluding a disturbed interval must leave one visible timing gap");
    assert.ok(qualification.metrics?.inputSpan > 0);
    assert.ok(qualification.metrics?.outputSpan > 0);
    assert.ok(Number.isFinite(qualification.metrics?.signalToNoiseDb));
    assert.strictEqual(qualification.metrics?.excitation, "step");
    assert.strictEqual(qualification.metrics?.coherence, "Not evaluated for time-domain excitation");
    assert.strictEqual(qualification.metrics?.independentValidationAvailable, false);
    assert.strictEqual(qualification.suitableForDesign, false);
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

  test("retains an acceptable model when a bounded experiment restores its input", () => {
    const samples: IdentificationSample[] = [];
    for (let index = 0; index <= 172; index += 1) {
      const timeSeconds = index * 0.3;
      const input = timeSeconds >= 11.4 && timeSeconds < 50.4 ? 30 : 20;
      const responseTime = Math.max(0, timeSeconds - 11.4 - 0.9);
      const output = 30 + 2 * (1 - Math.exp(-responseTime / 5));
      samples.push({ timeSeconds, input, output, quality: "good" });
    }

    const result = identifyPlant(samples);
    assert.strictEqual(result.recommended.kind, "first_order_dead_time");
    assert.ok(result.recommended.normalizedError <= 0.05);
  });

  test("fits the held part of a bounded step before its final restoration", () => {
    const samples = fopdtSamples(2);
    samples.push({ timeSeconds: 30.1, input: 0, output: samples[samples.length - 1].output, quality: "good" });
    const result = identifyPlant(samples);
    assert.ok(result.recommended.gain > 0.5 && result.recommended.gain < 1.5,
      "the final restoration must not replace the held-step response used for gain estimation");
  });

  test("uses the declared 0.05 eligibility limit before the resolved-delay override", () => {
    const select = (require("../../controlStudio/identification") as any).selectLowOrderModel;
    assert.strictEqual(typeof select, "function", "the declared candidate-selection contract is missing");
    const candidate = (kind: string, normalizedError: number) => ({
      kind, gain: 1, timeConstantSeconds: 4, deadTimeSeconds: 0,
      normalizedError, stable: true,
    });
    const models = [
      candidate("integrating", 0.8),
      candidate("first_order", 0.04),
      candidate("first_order_dead_time", 0.039),
      candidate("second_order", 0.02),
    ];
    assert.strictEqual(select(models, 0.1, 0.1).kind, "first_order",
      "the simplest stable finite candidate at or below 0.05 must win without resolved delay");
    models[2] = { ...models[2], deadTimeSeconds: 0.3, normalizedError: 0.03 };
    assert.strictEqual(select(models, 0.1, 0.3).kind, "first_order_dead_time",
      "resolved dead time with at least 2 percent error reduction must override simplicity");
  });

  test("qualifies a Control Studio model on a separate immutable validation run", () => {
    const result = identifyPlant(fopdtSamples(2), [], "Simulation", fopdtSamples(1.25), 0.02);
    assert.strictEqual(result.partition, "independent-runs");
    assert.strictEqual(result.recommended.kind, "first_order");
    const normalizedValidationError = result.recommended.normalizedValidationError;
    assert.ok(normalizedValidationError !== undefined);
    assert.ok(normalizedValidationError <= 0.03);
    const validationResiduals = result.recommended.validationResiduals;
    assert.ok(validationResiduals !== undefined);
    assert.ok(validationResiduals.length === 301);
    const uncertainty = result.recommended.uncertainty;
    assert.ok(uncertainty !== undefined);
    assert.strictEqual(uncertainty.method, "grid-resolution-and-validation-residual");
  });

  test("presents an identified model with engineering and residual evidence", () => {
    const module = require("../../controlStudio/identification") as any;
    const result = identifyPlant(fopdtSamples(2), [], "Simulation", fopdtSamples(1.25), 0.02);
    const model = result.recommended;
    const fit = modelFitEvidence(model, fopdtSamples(1.25));
    const present = module.identifiedModelPresentation;
    assert.strictEqual(typeof present, "function", "identified model presentation is missing");
    const evidence = present(model, fit, "kW", "°C");
    assert.match(evidence.equation, /G\(s\) = .*\/ \(/);
    assert.deepStrictEqual(evidence.units, { input: "kW", output: "°C", gain: "°C/kW", time: "s" });
    assert.strictEqual(evidence.poles.length, 1);
    assert.deepStrictEqual(evidence.zeros, []);
    assert.strictEqual(evidence.delaySeconds, 0);
    assert.match(evidence.uncertainty.label, /search-resolution and held-out-residual/i);
    assert.ok(Number.isFinite(evidence.estimation.residualLagOneCorrelation));
    assert.ok(Number.isFinite(evidence.validation.residualLagOneCorrelation));
    assert.strictEqual(evidence.predictedResponse.length, fit.points.length);
    assert.deepStrictEqual(evidence.validityRange, fit.operatingRange);
    const compact = present({ ...model, gain: 1 / 3, timeConstantSeconds: 2 / 3, deadTimeSeconds: 1 / 7 },
      fit, "kW", "°C");
    assert.doesNotMatch(compact.equation, /333333333|666666666|142857142/,
      "model equations must use readable engineering precision");
  });

  test("reconstructs step and restoration superposition for first-order and integrating fits", () => {
    const samples = (outputs: readonly number[]) => outputs.map((output, index) => ({
      timeSeconds: index, input: index < 2 || index >= 4 ? 0 : 1, output, quality: "good" as const,
    }));
    const firstOrder = modelFitEvidence({ kind: "first_order", gain: 1, timeConstantSeconds: 1,
      normalizedError: 0, stable: true }, samples([0, 0, 0, 0.6321, 0.8647, 0.3181]));
    assert.ok(firstOrder.points[4].predicted > 0.8, "restoring the input must not erase the held first-order response");
    assert.ok(Math.abs(firstOrder.points[5].predicted - (1 - Math.exp(-3) - (1 - Math.exp(-1)))) < 0.01);
    const integrating = modelFitEvidence({ kind: "integrating", gain: 1,
      normalizedError: 0, stable: true }, samples([0, 0, 0, 1, 2, 2]));
    assert.strictEqual(integrating.points[4].predicted, 2);
    assert.strictEqual(integrating.points[5].predicted, 2);
  });

  test("requires a distinct validation run and explicit review before design use", () => {
    const prepare = (require("../../controlStudio/identification") as any).prepareIdentificationReview;
    assert.strictEqual(typeof prepare, "function", "the two-run identification workflow is missing");
    const estimation = fopdtSamples(2);
    const preview = prepare?.({ estimation, excluded: [], source: "Simulation", requestedKind: "automatic" });
    assert.strictEqual(preview.stage, "validation_required");
    assert.strictEqual(preview.estimationQualification.suitable, true);
    assert.strictEqual(preview.estimationQualification.suitableForDesign, false);
    assert.strictEqual(preview.preview.partition, "single-run");
    assert.throws(() => prepare?.({ estimation, validation: estimation, excluded: [],
      source: "Simulation", requestedKind: "automatic" }), /distinct immutable validation run/i);

    const qualified = prepare?.({ estimation, validation: fopdtSamples(1.25), excluded: [],
      source: "Simulation", requestedKind: "automatic", maximumNormalizedError: 0.02 });
    assert.strictEqual(qualified.stage, "acceptance_required");
    assert.strictEqual(qualified.validationQualification.metrics.independentValidationAvailable, true);
    assert.strictEqual(qualified.result.partition, "independent-runs");
    assert.strictEqual(qualified.selected.kind, "first_order");
    assert.ok(qualified.fit.points.length > 0);
    assert.strictEqual(qualified.accepted, false,
      "a qualified model must remain unapplied to Tune until the user accepts it");
  });

  test("rejects a requested wrong model order on independent validation", () => {
    const prepare = (require("../../controlStudio/identification") as any)
      .prepareIdentificationReview;
    assert.throws(() => prepare({
      estimation: fopdtSamples(2),
      validation: fopdtSamples(1.25),
      excluded: [],
      source: "Simulation",
      requestedKind: "integrating",
      maximumNormalizedError: 0.02,
    }), /requested integrating model.*independent validation/i);
  });

  test("moves identification through qualification, independent validation, and explicit acceptance", () => {
    const IdentificationWorkflow = (require("../../controlStudio/identification") as any).IdentificationWorkflow;
    assert.strictEqual(typeof IdentificationWorkflow, "function", "the identification workflow state is missing");
    const workflow = new IdentificationWorkflow(0.02);
    const estimation = fopdtSamples(2);
    const qualification = workflow.qualify(estimation, [], "Simulation", "automatic");
    assert.strictEqual(qualification.suitableForDesign, false);
    const preview = workflow.fit();
    assert.strictEqual(preview.stage, "validation_required");
    assert.throws(() => workflow.accept(), /independent validation/i);

    workflow.qualify(fopdtSamples(1.25), [], "Simulation", "automatic");
    const review = workflow.fit();
    assert.strictEqual(review.stage, "acceptance_required");
    assert.strictEqual(workflow.acceptedModel, undefined);
    const accepted = workflow.accept();
    assert.strictEqual(accepted.model.kind, "static_gain");
    assert.strictEqual(accepted.evidence, "Identified from simulation data");
    assert.strictEqual(workflow.acceptedModel?.kind, "static_gain");
  });

  test("accepts equal deterministic samples only when their immutable Test recording identities differ", () => {
    const IdentificationWorkflow = (require("../../controlStudio/identification") as any).IdentificationWorkflow;
    const workflow = new IdentificationWorkflow();
    const deterministic = fopdtSamples(2);
    workflow.qualify(deterministic, [], "Simulation", "automatic", "test-recording-estimation");
    assert.strictEqual(workflow.fit().stage, "validation_required");
    workflow.qualify(deterministic, [], "Simulation", "automatic", "test-recording-validation");
    assert.strictEqual(workflow.fit().stage, "acceptance_required",
      "separate Test recording identities establish independent validation even when a deterministic simulator returns equal values");
  });

  test("renders the approved Identify page shell and keeps model evidence separate from Tune", () => {
    const extensionRoot = path.resolve(__dirname, "..", "..", "..");
    const template = fs.readFileSync(path.join(extensionRoot, "src", "controlStudio", "threePageTemplate.ts"), "utf8");
    const workflow = fs.readFileSync(path.join(extensionRoot, "src", "controlStudio", "webview", "threePageWorkflow.ts"), "utf8");
    assert.match(template, /Identify.*Tune.*Validate/);
    assert.match(template, /data-studio-inputs/);
    assert.match(template, /data-studio-outputs/);
    assert.match(template, /data-engineer-acquisition-method/);
    assert.match(template, /data-engineer-find[^>]*>Identify plant</);
    assert.match(template, /data-identify-import[^>]*>Open recording…</);
    assert.match(template, /data-studio-limits[^>]*>Limits…</);
    assert.match(template, /data-studio-model-result/);
    assert.match(template, /data-control-studio-chart/);
    assert.doesNotMatch(template, /Check recording|Fit estimation run|Accept model for design/);
    assert.doesNotMatch(template, /data-identify-qualification|plot-toolbar|bottom status/);
    assert.match(workflow, /new ThreePageOperation/);
    assert.match(workflow, /new ThreePageBindings/);
    assert.match(workflow, /new ThreePageApplication/);
    const identification = new IdentificationWorkflow();
    identification.qualify(fopdtSamples(2), [], "Simulation", "automatic", "estimation");
    const preview = identification.fit();
    const presentation = identificationPresentationState(identification, "%", "degC");
    assert.strictEqual(presentation?.phase, "validation_required");
    assert.deepStrictEqual((presentation?.presentation as { predictedResponse: unknown }).predictedResponse, preview.fit.points);
    assert.strictEqual(identification.acceptedModel, undefined, "presenting the preview cannot enable Tune by accepting it");
    const hostState = fs.readFileSync(path.join(extensionRoot, "src", "controlStudio", "webview", "threePageHostState.ts"), "utf8");
    assert.match(hostState, /renderIdentifiedModel/);
    assert.match(hostState, /data-studio-model-type/);
    assert.match(hostState, /data-studio-model-equation/);
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

  test("describes formula assumptions, units, inputs, results, and unavailable reasons", () => {
    const describe = (require("../../controlStudio/tuning") as any).describeTuningMethod;
    assert.strictEqual(typeof describe, "function", "tuning method presentation is missing");
    const model = identifyPlant(fopdtSamples()).recommended;
    const simc = describe(model, "SIMC", "balanced", "%", "°C");
    assert.strictEqual(simc.status, "Feasible");
    assert.strictEqual(simc.title, "SIMC PI (Skogestad 2003)");
    assert.strictEqual(simc.convention, "Parallel Control.PID");
    assert.match(simc.assumptions.join(" "), /stable.*positive-gain.*first-order/i);
    assert.deepStrictEqual(simc.units, { plantGain: "°C/%", proportionalGain: "%/°C",
      integralGain: "%/(°C·s)", derivativeGain: "(%·s)/°C" });
    assert.ok(simc.inputs.some((input: any) => input.name === "lambda" && input.unit === "s"));
    assert.strictEqual(simc.result.derivativeGain, 0);
    assert.strictEqual(simc.applied, false);

    const unavailable = describe({ ...model, deadTimeSeconds: 0 }, "AMIGO", "balanced", "%", "°C");
    assert.strictEqual(unavailable.status, "Unavailable");
    assert.match(unavailable.reason, /resolved positive dead time/i);
    assert.strictEqual(unavailable.result, undefined);
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

  test("Automatic Tune materializes a P candidate without an integral time", () => {
    const automaticTune = require("../../controlStudio/automaticTune") as {
      materializeAutomaticTuneController?: (candidate: ReturnType<typeof candidateFromControlPid>) =>
        { kp: number; tiSeconds?: number };
    };
    assert.strictEqual(typeof automaticTune.materializeAutomaticTuneController, "function");
    const candidate = candidateFromControlPid({
      proportionalGain: 0.1, integralGain: 0, derivativeGain: 0,
      proportionalSetpointWeight: 1, derivativeSetpointWeight: 0, bias: 0,
      outputMinimum: 0, outputMaximum: 100, rateMinimum: -100, rateMaximum: 100,
      trackingTime: 1, antiWindup: "BackCalculation",
    }, "Constrained comparison");
    assert.deepStrictEqual(automaticTune.materializeAutomaticTuneController!(candidate), { kp: 0.1 });
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

  test("keeps automatic method choices and comparison evidence in the current Tune result owner", () => {
    const extensionRoot = path.resolve(__dirname, "..", "..", "..");
    const template = fs.readFileSync(path.join(extensionRoot, "src", "controlStudio", "threePageTemplate.ts"), "utf8");
    const workflow = fs.readFileSync(path.join(extensionRoot, "src", "controlStudio", "webview", "threePageWorkflow.ts"), "utf8");
    assert.match(template, /data-engineer-tuning-method[\s\S]*value="SIMC"/);
    assert.match(template, /data-engineer-tuning-method[\s\S]*value="AMIGO"/);
    assert.match(template, /data-engineer-tuning-method[\s\S]*value="manual"/);
    assert.match(template, /data-studio-tuning-result/);
    assert.match(workflow, /new ThreePageTuning/);
    assert.match(workflow, /tuningPreferences/);
    assert.doesNotMatch(template, /data-auto-tune-attempts|data-saved-tuning-attempts|Method comparison/,
      "retained comparison evidence belongs in the current Tune result owner, not the retired panel markup");
  });

  test("renders the approved Tune controls and keeps tuning evidence in the result area", () => {
    const extensionRoot = path.resolve(__dirname, "..", "..", "..");
    const template = fs.readFileSync(path.join(extensionRoot, "src", "controlStudio", "threePageTemplate.ts"), "utf8");
    const workflow = fs.readFileSync(path.join(extensionRoot, "src", "controlStudio", "webview", "threePageWorkflow.ts"), "utf8");
    assert.match(template, /data-studio-controller/);
    assert.match(template, /data-engineer-tuning-method/);
    assert.match(template, /data-engineer-tune[^>]*>Tune controller</);
    assert.match(template, /data-studio-requirements[^>]*>Requirements…</);
    assert.match(template, /data-studio-tuning-result/);
    assert.match(template, /data-control-studio-chart/);
    assert.doesNotMatch(template, /Design method|data-tune-method-evidence|Assumptions|Design inputs|Equations/);
    assert.match(workflow, /new ThreePageTuning/);
    assert.match(workflow, /tuningPreferences/);
    assert.match(workflow, /data-studio-requirements/);
  });

  test("accepts Automatic Tune approval from the configured loop scope", () => {
    const scope = { projectId: "project", editorSessionId: "editor", loopId: "loop", loopRevision: "sha256:loop" };
    assert.deepStrictEqual(decodeLoopAutomaticTuneApprove({ schemaVersion: 1, kind: "control-studio.automatic.approve",
      messageId: "message", projectId: "project", editorSessionId: "editor", loopId: "loop",
      expectedLoopRevision: "sha256:loop", payload: { objective: "balanced" } }, scope),
    { messageId: "message", objective: "balanced" });
  });
  test("accepts a strict loop-scoped raw identification edit with blank fields", () => {
    const scope = { projectId: "project", editorSessionId: "editor", loopId: "loop", loopRevision: "sha256:loop" };
    const values = Object.fromEntries([
      "baselineDurationSeconds", "finalDurationSeconds", "maximumWindowRange", "maximumWindowSlopePerSecond",
      "measurementBound", "onsetThreshold", "onsetSamples", "baselineStep", "gainStep", "timestampShiftSeconds",
      "delay.minimum", "delay.maximum", "delay.step", "timeConstant.minimum", "timeConstant.maximum", "timeConstant.step",
    ].map(key => [key, key === "delay.minimum" ? "" : "5"]));
    const base = { schemaVersion: 1, messageId: "message", projectId: "project", editorSessionId: "editor",
      loopId: "loop", expectedLoopRevision: "sha256:loop" };
    assert.deepStrictEqual(decodeEngineeringCommand({ ...base, kind: "control-studio.identify.edit",
      payload: { form: { schemaVersion: 1, model: "fopdt", heldStep: true, values, excludedStart: "", excludedEnd: "" } } }, scope), {
      action: "identify-edit", messageId: "message",
      form: { schemaVersion: 1, model: "fopdt", heldStep: true, values, excludedStart: "", excludedEnd: "" },
    });
    assert.strictEqual(decodeEngineeringCommand({ ...base, kind: "control-studio.identify.edit",
      payload: { form: { schemaVersion: 1, model: "fopdt", heldStep: true, values: { ...values, extra: "" }, excludedStart: "", excludedEnd: "" } } }, scope), undefined);
  });

  test("accepts the concise loop-scoped qualification, fit, acceptance, tune, and validate commands", () => {
    const scope = { projectId: "project", editorSessionId: "editor", loopId: "loop", loopRevision: "sha256:loop" };
    const base = { schemaVersion: 1, messageId: "message", projectId: "project", editorSessionId: "editor",
      loopId: "loop", expectedLoopRevision: "sha256:loop" };
    assert.deepStrictEqual(decodeEngineeringCommand({ ...base, kind: "control-studio.identify.run",
      payload: { model: "automatic" } }, scope),
    { action: "identify-qualify", messageId: "message", model: "automatic" });
    assert.deepStrictEqual(decodeEngineeringCommand({ ...base, kind: "control-studio.identify.run",
      payload: { model: "automatic", excludedIntervals: [{ startSeconds: 0, endSeconds: 1 }] } }, scope),
    { action: "identify-qualify", messageId: "message", model: "automatic",
      excludedIntervals: [{ startSeconds: 0, endSeconds: 1 }] });
    assert.deepStrictEqual(decodeEngineeringCommand({ ...base, kind: "control-studio.identify.fit",
      payload: {} }, scope), { action: "identify-fit", messageId: "message" });
    assert.deepStrictEqual(decodeEngineeringCommand({ ...base, kind: "control-studio.identify.accept",
      payload: {} }, scope), { action: "identify-accept", messageId: "message" });
    assert.strictEqual(decodeEngineeringCommand({ ...base, kind: "control-studio.identify.run",
      payload: { model: "invented" } }, scope), undefined);
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

  test("selects the simplest passing structure and uses performance only within that complexity", () => {
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
    ], 0.1)?.structure, "PI");
    const pd = candidateFromControlPid({ ...parameters, integralGain: 0, derivativeGain: 0.2 }, "Manual");
    assert.strictEqual(selectSimplestPassingCandidate([
      { candidate: pi, passes: true, performanceScore: 1 },
      { candidate: pd, passes: true, performanceScore: 0.95 },
    ], 0.1)?.structure, "PD");
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

  test("qualifies SIMC PI for an integrating tank without treating rate gain as static gain", () => {
    const tank = { kind: "integrating" as const, gain: 0.05, deadTimeSeconds: 1,
      stable: true, normalizedError: 0.04 };
    const fast = tuneController(tank, { method: "SIMC", objective: "fast" });
    const balanced = tuneController(tank, { method: "SIMC", objective: "balanced" });
    const robust = tuneController(tank, { method: "SIMC", objective: "robust" });
    assert.deepStrictEqual(fast.controller, { structure: "PI", kp: 10, tiSeconds: 8 });
    assert.deepStrictEqual(balanced.controller,
      { structure: "PI", kp: 6.6666667, tiSeconds: 12 });
    assert.deepStrictEqual(robust.controller, { structure: "PI", kp: 4, tiSeconds: 20 });
    assert.deepStrictEqual([fast.closedLoopTimeSeconds, balanced.closedLoopTimeSeconds,
      robust.closedLoopTimeSeconds], [1, 2, 4]);
    assert.strictEqual(balanced.applied, false);

    const described = describeTuningMethod(tank, "SIMC", "balanced", "valve-%", "level-%");
    assert.strictEqual(described.status, "Feasible");
    assert.strictEqual(described.units.plantGain, "level-%/(valve-%·s)");
    assert.match(described.assumptions.join(" "), /integrating.*positive.*rate gain/i);
    assert.ok(described.inputs.some((input) => input.name === "Kprime"));

    const openLoop = frequencyResponse(tank);
    assert.ok(openLoop.points[0].magnitudeDb -
      openLoop.points[openLoop.points.length - 1].magnitudeDb > 100,
      "an integrating model must lose 20 dB per decade before controller effects");
    assert.throws(() => tuneController({ ...tank, deadTimeSeconds: 0 },
      { method: "SIMC", objective: "balanced" }), /resolved positive effective delay/i);
    assert.throws(() => tuneController(tank,
      { method: "AMIGO", objective: "balanced" }), /integrating/i);
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

  test("ranks feasible candidates by structural complexity and objective", () => {
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

    const nearTie = candidates.rankQualifiedCandidates!([
      { id: "amigo-near", method: "AMIGO", candidate: candidateFromControlPid(base, "AMIGO"),
        feasible: true, metrics: metric },
      { id: "simc-near", method: "SIMC", candidate: candidateFromControlPid(base, "SIMC"),
        feasible: true, metrics: { ...metric, iae: 1.05, ise: 1.05, itae: 1.05,
          mvMovement: 1.05, peakAbsoluteMv: 1.05, maximumMvRate: 1.05,
          noiseMvRms: 1.05, worstCaseVariationIae: 1.05 } },
    ], "balanced");
    assert.strictEqual(nearTie.winner?.source, "AMIGO",
      "equally simple qualifying candidates are selected by their declared objective");
    assert.strictEqual(nearTie.ranked[0].id, "amigo-near");

    const proportional = candidateFromControlPid({ ...base, integralGain: 0 }, "Constrained comparison");
    const pid = candidateFromControlPid({ ...base, derivativeGain: 0.2 }, "Constrained comparison");
    const complexityResult = candidates.rankQualifiedCandidates!([
      { id: "p", method: "Constrained comparison", candidate: proportional,
        feasible: true, metrics: { ...metric, iae: 2, ise: 2, itae: 2,
          mvMovement: 2, peakAbsoluteMv: 2, maximumMvRate: 2,
          noiseMvRms: 2, worstCaseVariationIae: 2 } },
      { id: "pid", method: "Constrained comparison", candidate: pid,
        feasible: true, metrics: metric },
    ], "balanced");
    assert.strictEqual(complexityResult.winner?.structure, "P",
      "a more complex controller must not displace a simpler qualifying controller");

    const pi = candidateFromControlPid(base, "Constrained comparison");
    const pd = candidateFromControlPid({ ...base, integralGain: 0, derivativeGain: 0.2 }, "Constrained comparison");
    const equallySimpleResult = candidates.rankQualifiedCandidates!([
      { id: "pi", method: "Constrained comparison", candidate: pi,
        feasible: true, metrics: { ...metric, iae: 1.05, ise: 1.05, itae: 1.05,
          mvMovement: 1.05, peakAbsoluteMv: 1.05, maximumMvRate: 1.05,
          noiseMvRms: 1.05, worstCaseVariationIae: 1.05 } },
      { id: "pd", method: "Constrained comparison", candidate: pd,
        feasible: true, metrics: metric },
    ], "balanced");
    assert.strictEqual(equallySimpleResult.winner?.structure, "PD",
      "PI and PD are equally simple, so the better matching objective wins");
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
    assert.ok(Math.min(...evidence.candidateOpenLoopBode.map((point) => point.phaseDegrees)) >= -720,
      "the displayed Bode range must stay near the useful dynamics instead of extending through many dead-time rotations");
    const advanced = evidence as typeof evidence & {
      nichols?: readonly unknown[];
      nyquist?: readonly unknown[];
      sensitivity?: readonly { sensitivityMagnitudeDb: number;
        complementarySensitivityMagnitudeDb: number; controlSensitivityMagnitudeDb: number }[];
      rootLocus?: { status: string; branches: readonly unknown[] };
      requirements?: readonly { name: string; plot: string; status: string }[];
      uncertainty?: { status: string; points: readonly { lower: number; nominal: number; upper: number }[] };
    };
    assert.strictEqual(advanced.nichols?.length, evidence.candidateOpenLoopBode.length);
    assert.ok((advanced.nyquist?.length ?? 0) >= evidence.candidateOpenLoopBode.length * 2 - 1);
    assert.strictEqual(advanced.sensitivity?.length, evidence.candidateOpenLoopBode.length);
    assert.ok(advanced.sensitivity?.every((point) => [point.sensitivityMagnitudeDb,
      point.complementarySensitivityMagnitudeDb, point.controlSensitivityMagnitudeDb].every(Number.isFinite)));
    assert.ok(advanced.rootLocus?.status === "qualified" && advanced.rootLocus.branches.length >= 40);
    assert.ok(advanced.requirements?.some((item) => item.name === "Overshoot" && item.plot === "response"));
    assert.ok(advanced.requirements?.some((item) => item.name === "Phase margin" && item.plot === "bode"));
    assert.ok(advanced.requirements?.every((item) => ["Pass", "Fail", "Inconclusive"].includes(item.status)));
    assert.strictEqual(advanced.uncertainty?.status, "evaluated");
    assert.strictEqual(advanced.uncertainty?.points.length, evidence.timeResponse.length);
    assert.ok(advanced.uncertainty?.points.every((point) => point.lower <= point.nominal && point.nominal <= point.upper));
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
