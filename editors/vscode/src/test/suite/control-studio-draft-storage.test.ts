import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import { storeEngineerDraft, restoreEngineerDraft, restoreWorkflowDraft } from "../../controlStudio/engineerDraftStorage";
import type { ControlLoopDefinition } from "../../controlStudio/loopDefinition";
import { decodeEngineerStudy, type EngineerStudy } from "../../controlStudio/engineerStudyPersistence";
import { IdentificationWorkflow, type IdentificationSample } from "../../controlStudio/identification";
import { createWorkflowResult } from "../../controlStudio/workflowResult";
import { buildWorkflowDraftSnapshot, buildWorkflowResultSnapshot } from "../../controlStudio/workflowResultSnapshot";
import { evaluateRecordedRun } from "../../controlStudio/runValidation";
import { openProjectStudy, studyDirectory, updateProjectStudy } from "../../controlStudio/studyStore";
import { STUDY_STORAGE_LIMIT_BYTES } from "../../controlStudio/controlStudioLimits";

suite("Control Studio automatic draft storage", () => {
  const loop = { projectId: "project", runtimeId: "sim", id: "loop", revision: "sha256:loop-v1" } as ControlLoopDefinition;
  const study: EngineerStudy = { campaigns: [], controllerChoice: "PI", applicationMode: "manual",
    task: { referenceChange: 1, maximumError: 0.2, settlingBand: 0.05, settlingTimeSeconds: 1, durationSeconds: 3 },
    settingsPins: { loopRevision: "sha256:loop-v1", family: "PI", values: { "Main.Kp": 2 } },
    acquisitionSettings: { method: "mv_step", amplitude: 1, durationSeconds: 3, reuseRecordings: true } };
  const draftStudy: EngineerStudy = { campaigns: [] };
  const workflowLoop: any = { ...loop, topology: "open-loop", controller: { instance: "P1", revision: "sha256:controller-v1" },
    signals: { processVariable: { unit: "degC" }, manipulatedVariable: { unit: "%" } } };
  function identificationStudy(): any {
    const samples: IdentificationSample[] = Array.from({ length: 24 }, (_, i) => ({ timeSeconds: i / 10,
      input: i < 8 ? 1 : 2, output: i < 8 ? 3 : 3.5, quality: "good", inputUnit: "%", outputUnit: "degC" }));
    const workflow = new IdentificationWorkflow(); workflow.qualify(samples, [], "Simulation", "first_order", "recording-1");
    const saved = JSON.parse(JSON.stringify(workflow.snapshot()));
    const units = { sp: "", pv: "degC", mv: "%" };
    saved.current.recording = { kind: "project_sequence", data: {
      rawSamples: samples.map((sample, i) => ({ elapsedSeconds: sample.timeSeconds, observedAtMs: 1000 + i * 100,
        actualCycleSeconds: .1, scan: i + 1, source: "Simulation", loopRevision: "sha256:loop-v1", sp: null,
        pv: sample.output, requestedMv: null, appliedMv: sample.input, units, quality: "good", freshnessMs: null,
        controllerMode: "Unavailable", outputLimits: { minimum: 0, maximum: 100 }, rateLimits: {}, safetyState: "Unavailable", events: [] })),
      selectedIntervals: [], acquisition: { recordingId: "recording-1", method: "runtime_scan_buffer", completeness: "complete",
        observedSampleCount: samples.length, scanRange: { first: 1, last: samples.length }, missingScanRanges: [], droppedObservedSamples: 0,
        actualCycleTimingAvailable: true, freshnessAvailable: false, reason: "Every native scan retained." },
    } };
    return saved;
  }
  function workflowDraft(currentStudy: EngineerStudy = draftStudy, identification = identificationStudy()): any {
    return createWorkflowResult({ resultId: "draft-result", resultRevision: 4, projectId: loop.projectId, source: "Simulation",
      loop: { id: loop.id, revision: loop.revision, topology: "open-loop" }, controller: workflowLoop.controller,
      software: { extensionVersion: "0.24.65", numericalAdapters: {} }, units: { sp: "", pv: "degC", mv: "%" },
      recording: { rawSamples: [0, 1].map(i => ({ elapsedSeconds: i / 10, observedAtMs: 1000 + i * 100,
        actualCycleSeconds: .1, scan: i + 1, source: "Simulation", loopRevision: loop.revision, sp: null, pv: 3 + i,
        requestedMv: null, appliedMv: 1 + i, units: { sp: "", pv: "degC", mv: "%" }, quality: "good", freshnessMs: null,
        controllerMode: "Unavailable", outputLimits: { minimum: 0, maximum: 100 }, rateLimits: {}, safetyState: "Unavailable", events: [] })), selectedIntervals: [] },
      engineerStudy: currentStudy, identificationStudy: identification,
      evidence: { observation: "Observed", model: "Not evaluated" }, conclusion: "Draft workflow." }, "2026-09-12T12:00:00.000Z");
  }
  /** Opens a fresh project study for the loop; every draft test stores into a temporary project folder. */
  async function openStudy(): Promise<string> {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), "control-studio-draft-"));
    await openProjectStudy(root, loop);
    return root;
  }
  function directoryBytes(directory: string): number {
    if (!fs.existsSync(directory)) return 0;
    return fs.readdirSync(directory, { withFileTypes: true }).reduce((sum, entry) => {
      const file = path.join(directory, entry.name);
      return sum + (entry.isDirectory() ? directoryBytes(file) : fs.statSync(file).size);
    }, 0);
  }
  function verificationResult(id: string): any {
    const channels = Array.from({ length: 27 }, (_, i) => ({ id: `P1.Controller.StateValue.Channel${String(i).padStart(2, "0")}`, unit: "degC" }));
    const times = Array.from({ length: 601 }, (_, i) => i * 100 / 1000);
    const run = { id: `${id}-run`, evidence: "ST-executed", parameters: { "P1.Parameters.ProportionalGain": 5 },
      conditions: { initialState: { "P1.SetpointC": 38 }, events: [], durationSeconds: 60, sampleTimesSeconds: times, seed: 7,
        modelRevision: "sha256:model", programRevision: "sha256:program", parameterSchemaRevision: "sha256:schema" },
      channels, samples: times.map((timeSeconds, sequence) => ({ timeSeconds, sequence,
        values: Object.fromEntries(channels.map((channel, i) => [channel.id, { value: 35 + i / 100 + timeSeconds / 1000, quality: "good" }])) })) };
    const evaluation = evaluateRecordedRun(run, [{ id: `${id}-range`, kind: "bounds", channel: channels[0].id, window: { start: 0, end: 60 }, minimum: 0, maximum: 80 }]);
    const request = { step_ms: 100, duration_ms: 60000, seed: 7, initial_values: { "P1.SetpointC": 38 }, events: [],
      observations: channels.map(channel => ({ symbol: channel.id, unit: channel.unit })),
      parameter_overrides: [{ symbol: "P1.Parameters.ProportionalGain", value: 5 }], execution: "isolated-simulation", expected_project_revision: "sha256:project" };
    return { id, result: { projectRevision: "sha256:project", request, run: evaluation.run, evaluation } };
  }
  test("retains a realistic 60-second multi-scenario study within the storage bound", async function () {
    this.timeout(120_000);
    const root = await openStudy();
    const evidence = { campaigns: [], scenarioResults: Array.from({ length: 8 }, (_, i) => verificationResult(`scenario-${i}`)) } as unknown as EngineerStudy;
    await storeEngineerDraft(loop, evidence);
    const directory = studyDirectory(root, loop.id);
    const stored = directoryBytes(directory);
    assert.ok(stored < STUDY_STORAGE_LIMIT_BYTES, `The study must stay within the storage bound; stored ${stored} bytes`);
    assert.ok(fs.readdirSync(path.join(directory, "recordings")).length >= 8, "each scenario run is stored as its own recording file");
    assert.deepStrictEqual(restoreEngineerDraft(loop), evidence, "Reopening restores exactly the saved evidence");
  });
  test("checks each recorded result once and shares it with later snapshots and saves (A10.7)", async function () {
    this.timeout(120_000);
    await openStudy();
    const evidence = { campaigns: [], scenarioResults: Array.from({ length: 8 }, (_, i) => verificationResult(`scenario-${i}`)) } as unknown as EngineerStudy;
    const started = performance.now();
    const first = decodeEngineerStudy(evidence)!;
    const firstMs = performance.now() - started;
    const result = first.scenarioResults![0].result;
    assert.ok(Object.isFrozen(result) && Object.isFrozen(result.run.samples[0]), "a checked result is immutable");
    assert.strictEqual(result.evaluation.run, result.run, "the evaluation shares its run");
    const again = performance.now();
    const second = decodeEngineerStudy(first)!;
    const fromSource = decodeEngineerStudy(evidence)!;
    const repeatedMs = performance.now() - again;
    await storeEngineerDraft(loop, first, workflowDraft(first));
    const restored = restoreEngineerDraft(loop)!;
    for (const study of [second, fromSource, restored]) {
      assert.strictEqual(study.scenarioResults![0].result, result, "later snapshots and saves share the checked result");
    }
    assert.deepStrictEqual(restored, evidence, "the shared result is the evidence that was recorded");
    assert.throws(() => { (result.evaluation as { status: string }).status = "Pass"; }, TypeError);
    // Decoding a snapshot of checked results again copies only what is not shared.
    assert.ok(repeatedMs < firstMs / 4, `two repeated snapshots took ${repeatedMs.toFixed(0)} ms; the first check of eight long runs ${firstMs.toFixed(0)} ms`);
  });

  test("restores edited settings and task without adding runtime approval", async () => {
    await openStudy();
    await storeEngineerDraft(loop, study);
    const restored = restoreEngineerDraft(loop)!;
    assert.deepStrictEqual(restored, study);
    assert.ok(!("review" in restored) && !("verifiedInputs" in restored));
    restored.task!.referenceChange = 99;
    assert.strictEqual(restoreEngineerDraft(loop)!.task!.referenceChange, 1);
  });
  test("latest queued edits are available before and after their writes finish", async () => {
    await openStudy();
    const first = storeEngineerDraft(loop, study);
    const second = storeEngineerDraft(loop, { ...study, controllerChoice: "PID" as const });
    assert.strictEqual(restoreEngineerDraft(loop)?.controllerChoice, "PID");
    await Promise.all([first, second]);
    assert.strictEqual(restoreEngineerDraft(loop)?.controllerChoice, "PID");
  });
  test("changed loop revisions cannot reuse old parameter pins", async () => {
    const root = await openStudy();
    await storeEngineerDraft(loop, study);
    assert.strictEqual(restoreEngineerDraft({ ...loop, revision: "loop-v2" } as ControlLoopDefinition), undefined);
    const document = JSON.parse(fs.readFileSync(path.join(studyDirectory(root, loop.id), "study.json"), "utf8"));
    assert.strictEqual(document.state.draft.loopRevision, loop.revision, "different-loop evidence must remain stored");
  });
  test("an empty current study supersedes old saved state during and after queued writes", async () => {
    await openStudy();
    await storeEngineerDraft(loop, study);
    const obsolete = storeEngineerDraft(loop, { ...study, controllerChoice: "PID" as const });
    const empty = storeEngineerDraft(loop, undefined);
    assert.deepStrictEqual(restoreEngineerDraft(loop) ?? study, { campaigns: [] },
      "an empty current draft must hide a queued old draft and prevent fallback to an older saved result");
    await Promise.all([obsolete, empty]);
    assert.deepStrictEqual(restoreEngineerDraft(loop) ?? study, { campaigns: [] },
      "reopening after persistence must not resurrect a superseded study");
  });

  test("round-trips a complete workflow draft with raw recording and identification state", async () => {
    await openStudy();
    const workflow = workflowDraft();
    await storeEngineerDraft(loop, draftStudy, workflow);
    const restored = restoreWorkflowDraft(loop)!;
    assert.deepStrictEqual(restored, workflow);
    assert.deepStrictEqual(restoreEngineerDraft(loop), draftStudy);
    (restored.recording.rawSamples as any)[0].pv = 99;
    assert.strictEqual(restoreWorkflowDraft(loop)!.recording.rawSamples[0].pv, 3);
  });

  test("an explicit full empty workflow supersedes an older accepted draft", async () => {
    await openStudy();
    await storeEngineerDraft(loop, draftStudy, workflowDraft());
    const empty = workflowDraft(draftStudy, undefined);
    await storeEngineerDraft(loop, { campaigns: [] }, empty);
    assert.deepStrictEqual(restoreWorkflowDraft(loop), empty);
    assert.deepStrictEqual(restoreEngineerDraft(loop), { campaigns: [] });
  });

  test("latest full workflow is visible before queued writes finish", async () => {
    await openStudy();
    const first = storeEngineerDraft(loop, draftStudy, workflowDraft());
    const latest = workflowDraft({ ...draftStudy, controllerChoice: "PID" });
    const second = storeEngineerDraft(loop, latest.engineerStudy!, latest);
    assert.strictEqual(restoreWorkflowDraft(loop)!.engineerStudy?.controllerChoice, "PID");
    await Promise.all([first, second]);
    assert.strictEqual(restoreWorkflowDraft(loop)!.engineerStudy?.controllerChoice, "PID");
  });

  test("rejects malformed, mismatched, or shared-study workflow drafts", async () => {
    await openStudy();
    const workflow = workflowDraft();
    await storeEngineerDraft(loop, draftStudy, workflow);
    for (const invalid of [
      { ...workflow, projectId: "other-project" },
      { ...workflow, loop: { ...workflow.loop, revision: "other-revision" } },
      { ...workflow, engineerStudy: { campaigns: [], controllerChoice: "PID" } },
      { ...workflow, recording: { ...workflow.recording, rawSamples: [{ bad: true }] } },
    ]) {
      await assert.rejects(() => storeEngineerDraft(loop, draftStudy, invalid as any));
      assert.deepStrictEqual(restoreWorkflowDraft(loop), workflow);
    }
    await updateProjectStudy(loop, "draft", { loopRevision: loop.revision, study: draftStudy, workflow: { malformed: true } });
    assert.throws(() => restoreWorkflowDraft(loop), /draft|decode|invalid/i);
  });
});
