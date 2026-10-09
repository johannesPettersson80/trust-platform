import { decodeEngineerStudy } from "../../controlStudio/engineerStudyPersistence";
import { identifiedModelPresentation } from "../../controlStudio/identification";
import { restoredEngineerView } from "../../controlStudio/restoredEngineerView";
import * as assert from "assert";
import { createWorkflowResult, decodeWorkflowResult } from "../../controlStudio/workflowResult";
import { evaluateRecordedRun } from "../../controlStudio/runValidation";

type EngineerStudyApi = {
  decodeEngineerStudy?: (value: unknown) => unknown;
};

function studyApi(): EngineerStudyApi {
  try { return require("../../controlStudio/engineerStudyPersistence") as EngineerStudyApi; }
  catch { return {}; }
}

const channel = (id: string, unit: string) => ({ id, unit, operatingRange: { minimum: -10, maximum: 10 } });

function model() {
  return { revision: "model:provided:1", source: "provided", validation: "unvalidated", evidence: [], delaySeconds: 0,
    kind: "transfer-function", numerator: [1], denominator: [1, 1], inputs: [channel("command", "V")], outputs: [channel("position", "m")] };
}

function run(id: string, gain: number) {
  return { id, evidence: "ST-executed", parameters: { "Main.Kp": gain },
    conditions: { initialState: { "Main.PV": 0 }, events: [{ atSeconds: 0, channel: "Main.SP", value: 1 }],
      durationSeconds: 2, sampleTimesSeconds: [0, 1, 2], seed: 7,
      modelRevision: "model:provided:1", programRevision: "program:1", parameterSchemaRevision: "parameters:1" },
    channels: [{ id: "Main.PV", unit: "m" }],
    samples: [0, 1, 1].map((value, index) => ({ timeSeconds: index, sequence: index + 1,
      values: { "Main.PV": { value, quality: "good" } } })) };
}

function verification(id: string, gain: number) {
  const raw = run(id, gain);
  const requirements = [{ id: "response-error", kind: "error", channel: "Main.PV", window: { start: 1, end: 2 },
    reference: 1, statistic: "maximum", maximum: 0.1 }];
  return { projectRevision: "project:1", request: { step_ms: 1000, duration_ms: 2000, seed: 7, max_total_cycles: 3,
    wall_budget_ms: 1000, initial_values: { "Main.PV": 0 }, events: [{ at_ms: 0, symbol: "Main.SP", value: 1 }],
    observations: [{ symbol: "Main.PV", unit: "m" }], parameter_overrides: [{ symbol: "Main.Kp", value: gain }],
    execution: "isolated-simulation", expected_project_revision: "project:1" }, run: raw, evaluation: { status: "Pass", run: raw, specification: requirements,
      requirements: [{ id: "response-error", kind: "error", status: "Pass", value: 0, limit: 0.1, unit: "m",
        reason: "Observed measurement satisfies the declared limit." }],
      scope: "Only the declared observed conditions were evaluated; Pass does not establish general stability." } };
}

function workflow(engineerStudy: unknown) {
  return createWorkflowResult({ resultId: "study-result", resultRevision: 1, projectId: "project:1", source: "Simulation",
    loop: { id: "loop:1", revision: "sha256:loop-1" }, controller: { instance: "Main", revision: "sha256:controller-1" },
    software: { extensionVersion: "0.1.0", numericalAdapters: {} }, units: { sp: "m", pv: "m", mv: "V" },
    recording: { rawSamples: [], selectedIntervals: [] }, evidence: { observation: "Not observed", model: "Not evaluated" },
    conclusion: "Engineering study retained without applying parameters." }, "2026-09-07T12:00:00.000Z");
}

function workflowWithStudy(engineerStudy: unknown) {
  return { ...workflow(undefined), engineerStudy };
}

suite("Control Studio engineer-study persistence", () => {
  test("restores completed controller initialization checks with the historical validation", () => {
    const initialization = verification("initialization", 1) as any;
    const restored = restoredEngineerView({ campaigns: [], initialization, requiredScenarios: ["controller-transfer"] }) as any;
    assert.deepStrictEqual(restored.initialization, initialization.evaluation);
    assert.strictEqual(restored.stageEvidence.validationPassed, false);
  });
  test("retains unfinished mandatory scenario identities without claiming their completion", () => {
    const source = { campaigns: [], selectedModel: model(), requiredScenarios: ["target-response", "load-change", "controller-transfer"] };
    const saved = decodeEngineerStudy(source) as any;
    assert.deepStrictEqual(saved?.requiredScenarios, source.requiredScenarios);
    assert.deepStrictEqual((restoredEngineerView(saved) as any).requiredScenarios, source.requiredScenarios);
    assert.strictEqual(restoredEngineerView(saved).stageEvidence.validationPassed, false);
    assert.strictEqual(decodeEngineerStudy({ ...source, requiredScenarios: ["load", "load"] }), undefined);
  });
  test("retains matched model prediction runs separately from configured-plant validation", () => {
    const study = { campaigns: [], modelPrediction: { baseline: verification("prediction-current", 1), candidate: verification("prediction-tuned", 2) },
      baseline: verification("validation-current", 1), candidate: verification("validation-tuned", 2) };
    assert.deepStrictEqual(decodeEngineerStudy(study), study);
    const mismatched = structuredClone(study);
    mismatched.modelPrediction.candidate.run.conditions.modelRevision = "different-model";
    assert.strictEqual(decodeEngineerStudy(mismatched), undefined, "A prediction comparison requires the same plant model");
  });
  test("retains the model-bound measured comparison through study restore", () => {
    const selectedModel = { ...model(), source: "fitted" };
    const presentation = identifiedModelPresentation({ kind: "first_order", gain: 1, timeConstantSeconds: 1, normalizedError: 0.1 } as any,
      { points: [{ timeSeconds: 0, measured: 0, predicted: 0, residual: 0 }, { timeSeconds: 2, measured: 1, predicted: 0.9, residual: 0.1 }],
        rootMeanSquareError: 0.1, outputUnit: "m", stable: true, delaySeconds: 0,
        operatingRange: { inputMinimum: 0, inputMaximum: 1, outputMinimum: 0, outputMaximum: 1 }, warnings: [] }, "V", "m");
    const modelComparison = { modelRevision: selectedModel.revision, presentation };
    const input = { selectedModel, campaigns: [], modelComparison };
    const decoded = decodeEngineerStudy(input);
    assert.ok(decoded, "The exact accepted measured comparison must survive the study boundary");
    assert.deepStrictEqual((decoded as any).modelComparison, modelComparison);
    assert.deepStrictEqual((restoredEngineerView(decoded) as any).identificationPresentation, presentation);
    assert.strictEqual(decodeEngineerStudy({ ...input, modelComparison: { ...modelComparison, modelRevision: "different-model" } }), undefined);
    assert.strictEqual(decodeEngineerStudy({ ...input, modelComparison: { ...modelComparison, presentation: { ...presentation, predictedResponse: [{ timeSeconds: 0, measured: NaN, predicted: 0, residual: 0 }] } } }), undefined);
  });

  test("retains a model-bound manual gain pin and preserves it in automatic mode", () => {
    const pin = { parameterSymbol: "Main.Kp", unit: "V/m", loopRevision: "sha256:loop-1", modelRevision: model().revision, value: 0.5 };
    for (const mode of ["manual", "automatic"]) {
      const study = { selectedModel: model(), campaigns: [], gainChoice: { mode, pin } };
      assert.deepStrictEqual(decodeWorkflowResult(workflowWithStudy(study))?.engineerStudy, study,
        "The study must preserve its explicit gain choice without restoring approval");
    }
    for (const gainChoice of [{ mode: "manual" }, { mode: "manual", pin: { ...pin, modelRevision: "other" } },
      { mode: "manual", pin: { ...pin, value: Infinity } }, { mode: "manual", pin: { ...pin, approved: true } }]) {
      assert.strictEqual(decodeWorkflowResult(workflowWithStudy({ selectedModel: model(), campaigns: [], gainChoice })), undefined);
    }
  });

  test("retains cancelled verification outcomes without inventing raw samples or approval", () => {
    const native = verification("cancelled-record", 1);
    const attempt = { status: "cancelled", jobId: "job:1", projectRevision: native.projectRevision,
      request: native.request, reason: "Engineer cancelled the run." };
    const study = { campaigns: [], verificationAttempts: [attempt] };
    assert.deepStrictEqual(decodeWorkflowResult(workflowWithStudy(study))?.engineerStudy, study);
    for (const status of ["completed", "failed", "cancellation-unconfirmed"]) {
      const retained = { campaigns: [], verificationAttempts: [{ ...attempt, status, run: native.run }] };
      assert.deepStrictEqual(decodeWorkflowResult(workflowWithStudy(retained))?.engineerStudy, retained);
    }
    assert.strictEqual(decodeWorkflowResult(workflowWithStudy({ ...study, verificationAttempts: [attempt, attempt] })), undefined);
    assert.strictEqual(decodeWorkflowResult(workflowWithStudy({ ...study, verificationAttempts: [{ ...attempt, approved: true }] })), undefined);
    assert.strictEqual(decodeWorkflowResult(workflowWithStudy({ ...study, verificationAttempts: [{ ...attempt, run: { ...native.run, parameters: { "Main.Kp": 99 } } }] })), undefined);
  });

  test("keeps engineer studies exclusive to the schema-four contract", () => {
    const saved = workflowWithStudy({ campaigns: [] });
    assert.strictEqual(decodeWorkflowResult({ ...saved, schemaVersion: 3 }), undefined,
      "Legacy schema three cannot claim the schema-four engineer study contract");
  });

  test("persists an unanalysed supplied model with declared P task and matched historical verification", () => {
    assert.strictEqual(typeof studyApi().decodeEngineerStudy, "function",
      "workflow persistence must expose a strict Engineer Study decoder before saving study evidence");
    const study = { selectedModel: model(), family: "P", task: { referenceChange: 1, maximumError: 0.1,
      settlingBand: 0.02, settlingTimeSeconds: 1, durationSeconds: 2 }, campaigns: [{ id: "campaign:complete", status: "completed", runs: [verification("find-1", 1)] }],
      baseline: verification("baseline", 1), candidate: verification("candidate", 2) };
    const saved = workflowWithStudy(study);
    assert.deepStrictEqual(decodeWorkflowResult(saved)?.engineerStudy, study,
      "saving must retain original supplied-model provenance and historical baseline/candidate evidence without applying it");
  });

  test("retains typed PI PD and PID family evidence without restoring application authority", () => {
    const decode = studyApi().decodeEngineerStudy;
    assert.ok(decode);
    for (const family of ["PI", "PD", "PID"] as const) {
      const study = { selectedModel: model(), family, task: { referenceChange: 1, maximumError: 0.1,
        settlingBand: 0.02, settlingTimeSeconds: 1, durationSeconds: 2 }, campaigns: [], applicationMode: "auto" };
      assert.deepStrictEqual(decode!(study), study, `${family} history must reopen as evidence, not a new approval`);
    }
  });

  test("accepts explicit immutable evaluations for each selected known controller family", () => {
    const decode = studyApi().decodeEngineerStudy!;
    const base = { selectedModel: model(), campaigns: [], familyEvaluations: [
      { family: "P", status: "actual-pass", reason: "actual ST pass" },
      { family: "PI", status: "model-qualified", reason: "model qualified" },
      { family: "PD", status: "actual-fail", reason: "steady error" },
      { family: "PID", status: "model-qualified", reason: "model qualified" },
      { family: "MPC", status: "unavailable", reason: "adapter requires a project-owned mapping" },
    ] };
    assert.deepStrictEqual(decode(base), base);
    assert.deepStrictEqual(new Set(base.familyEvaluations.map(entry => entry.family)), new Set(["P", "PI", "PD", "PID", "MPC"]));
    for (const mutate of [
      (value: any) => { value.familyEvaluations[0].family = "unknown"; },
      (value: any) => { value.familyEvaluations[4].family = "P"; },
      (value: any) => { value.familyEvaluations[0].status = "pass"; },
    ]) {
      const invalid = JSON.parse(JSON.stringify(base)); mutate(invalid);
      assert.strictEqual(decode(invalid), undefined);
    }
  });

  test("retains failed Find evidence without requiring a selected model or task", () => {
    const study = { campaigns: [{ id: "campaign:failed", status: "failed", reason: "Process bound exceeded.",
      runs: [verification("failed-run", 1)] }] };
    assert.deepStrictEqual(decodeWorkflowResult(workflowWithStudy(study))?.engineerStudy, study);
  });

  test("rejects forged evaluation, oversized campaign history, and approval/client/job state", () => {
    const valid = { campaigns: [{ id: "campaign:failed", status: "failed", reason: "Process bound exceeded.",
      runs: [verification("failed-run", 1)] }] } as any;
    const forged = structuredClone(valid);
    forged.campaigns[0].runs[0].evaluation.status = "Fail";
    assert.strictEqual(decodeWorkflowResult(workflowWithStudy(forged)), undefined, "stored evaluation must equal a fresh evaluation of raw evidence and requirements");
    assert.strictEqual(decodeWorkflowResult(workflowWithStudy({ ...valid, campaigns: Array.from({ length: 17 }, (_, i) => ({ ...valid.campaigns[0], id: `campaign:${i}` })) })), undefined,
      "stored campaign history has an explicit bounded contract and must not silently truncate");
    assert.strictEqual(decodeWorkflowResult(workflowWithStudy({ ...valid, approval: { accepted: true } })), undefined);
    assert.strictEqual(decodeWorkflowResult(workflowWithStudy({ ...valid, client: { token: "forbidden" } })), undefined);
    assert.strictEqual(decodeWorkflowResult(workflowWithStudy({ ...valid, jobId: "runtime-job" })), undefined);
  });

  test("preserves failed cross-revision evidence but rejects a completed mixed-revision campaign", () => {
    const failed = verification("failed-run", 1) as any;
    const later = verification("later-run", 1) as any;
    later.projectRevision = "project:2";
    later.request.expected_project_revision = "project:2";
    const failedStudy = { campaigns: [{ id: "campaign:failed", status: "failed", reason: "The project changed during retry.",
      runs: [failed, later] }] };
    assert.ok(decodeWorkflowResult(workflowWithStudy(failedStudy)), "failure history must retain actual trial records across a project revision change");
    assert.strictEqual(decodeWorkflowResult(workflowWithStudy({ campaigns: [{ ...failedStudy.campaigns[0], status: "completed", reason: undefined }] })), undefined,
      "completed campaigns require one project revision across their qualifying trials");
  });

  test("keeps a long run that the runtime recorded at a record period as historical evidence (A4.2, A7)", () => {
    // Five executions averaged two per sample: the samples sit at the mean time of the executions each one averages.
    const averaged = verification("averaged", 1) as any;
    const times = [0.5, 2.5, 4];
    averaged.request.duration_ms = 4000; averaged.request.max_total_cycles = 5; averaged.request.record_every = 2;
    averaged.run.conditions.durationSeconds = 4; averaged.run.conditions.sampleTimesSeconds = times; averaged.run.conditions.recordEvery = 2;
    // Each sample carries the last sequence value of the executions it averages (protocol annex E4.4).
    averaged.run.samples = times.map((timeSeconds, index) => ({ timeSeconds, sequence: [2, 4, 5][index],
      values: { "Main.PV": { value: index === 0 ? 0 : 1, quality: "good" } } }));
    const specification = [{ id: "response-error", kind: "error", channel: "Main.PV", window: { start: 2, end: 4 },
      reference: 1, statistic: "maximum", maximum: 0.1 }];
    averaged.evaluation = evaluateRecordedRun(averaged.run, specification as never);
    assert.ok(decodeWorkflowResult(workflowWithStudy({ campaigns: [{ id: "campaign:averaged", status: "failed", reason: "Observed.", runs: [averaged] }] })),
      "a run averaged two executions per sample keeps its evidence");
  });

  test("validates native request limits, zero seed, and selected simulation identity before evaluating saved evidence", () => {
    const zeroSeed = verification("zero-seed", 1) as any;
    zeroSeed.request.seed = 0;
    zeroSeed.run.conditions.seed = 0;
    zeroSeed.evaluation.run.conditions.seed = 0;
    assert.ok(decodeWorkflowResult(workflowWithStudy({ campaigns: [{ id: "campaign:zero", status: "failed", reason: "Observed failure.", runs: [zeroSeed] }] })),
      "native seed zero remains valid historical evidence");
    const oversized = verification("oversized", 1) as any;
    oversized.request.duration_ms = 600001;
    oversized.run.conditions.durationSeconds = 600.001;
    oversized.evaluation.run.conditions.durationSeconds = 600.001;
    assert.strictEqual(decodeWorkflowResult(workflowWithStudy({ campaigns: [{ id: "campaign:oversized", status: "failed", reason: "Rejected.", runs: [oversized] }] })), undefined,
      "unbounded saved request duration must be rejected before sample-time expansion");
    const forgedModel = verification("forged-model", 1) as any;
    forgedModel.request.simulation_override = { model_revision: "model:other", model_provenance: "provided", simulation_toml: "[simulation]\nenabled = true\n" };
    assert.strictEqual(decodeWorkflowResult(workflowWithStudy({ campaigns: [{ id: "campaign:forged-model", status: "failed", reason: "Rejected.", runs: [forgedModel] }] })), undefined,
      "a saved run must retain the selected simulation model/configuration identity that produced it");
  });
});
