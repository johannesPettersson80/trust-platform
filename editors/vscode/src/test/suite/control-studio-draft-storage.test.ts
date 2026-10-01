import * as assert from "assert";
import { storeEngineerDraft, restoreEngineerDraft, restoreWorkflowDraft } from "../../controlStudio/engineerDraftStorage";
import type { ControlLoopDefinition } from "../../controlStudio/loopDefinition";
import type { EngineerStudy } from "../../controlStudio/engineerStudyPersistence";
import type { ExtensionContext } from "vscode";
import { IdentificationWorkflow, type IdentificationSample } from "../../controlStudio/identification";
import { createWorkflowResult } from "../../controlStudio/workflowResult";
import { buildWorkflowDraftSnapshot, buildWorkflowResultSnapshot } from "../../controlStudio/workflowResultSnapshot";
import { evaluateRecordedRun } from "../../controlStudio/runValidation";

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
  function storage(write?: () => Promise<void>) {
    const entries = new Map<string, unknown>();
    return { entries, context: { workspaceState: {
      get: (key: string) => entries.get(key),
      update: async (key: string, value: unknown) => { await write?.(); entries.set(key, value); },
    } } as unknown as Pick<ExtensionContext, "workspaceState"> };
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
    const { context, entries } = storage();
    const evidence = { campaigns: [], scenarioResults: Array.from({ length: 8 }, (_, i) => verificationResult(`scenario-${i}`)) } as unknown as EngineerStudy;
    await storeEngineerDraft(context, loop, evidence);
    const stored = Buffer.byteLength(JSON.stringify([...entries.values()][0]), "utf8");
    assert.ok(stored < 16 * 1024 * 1024, `Autosave must stay within workspace storage bounds; stored ${stored} bytes`);
    assert.deepStrictEqual(restoreEngineerDraft(context, loop), evidence, "Reopening restores exactly the saved evidence");
  });
  test("restores edited settings and task without adding runtime approval", async () => {
    const { context } = storage();
    await storeEngineerDraft(context, loop, study);
    const restored = restoreEngineerDraft(context, loop)!;
    assert.deepStrictEqual(restored, study);
    assert.ok(!("review" in restored) && !("verifiedInputs" in restored));
    restored.task!.referenceChange = 99;
    assert.strictEqual(restoreEngineerDraft(context, loop)!.task!.referenceChange, 1);
  });
  test("latest queued edits are available during close and reopen", async () => {
    let release!: () => void;
    const barrier = new Promise<void>(resolve => { release = resolve; });
    const { context } = storage(() => barrier);
    const first = storeEngineerDraft(context, loop, study);
    const latest = { ...study, controllerChoice: "PID" as const };
    const second = storeEngineerDraft(context, loop, latest);
    assert.strictEqual(restoreEngineerDraft(context, loop)?.controllerChoice, "PID");
    release(); await Promise.all([first, second]);
    assert.strictEqual(restoreEngineerDraft(context, loop)?.controllerChoice, "PID");
  });
  test("changed loop revisions cannot reuse old parameter pins", async () => {
    const { context, entries } = storage();
    await storeEngineerDraft(context, loop, study);
    assert.strictEqual(restoreEngineerDraft(context, { ...loop, revision: "loop-v2" }), undefined);
    assert.strictEqual(entries.size, 1, "different-loop evidence must remain stored");
  });
  test("an empty current study supersedes old saved state during and after queued writes", async () => {
    let release!: () => void;
    const barrier = new Promise<void>(resolve => { release = resolve; });
    let delayed = false;
    const { context } = storage(() => delayed ? barrier : Promise.resolve());
    await storeEngineerDraft(context, loop, study);
    delayed = true;
    const obsolete = storeEngineerDraft(context, loop, { ...study, controllerChoice: "PID" });
    const empty = storeEngineerDraft(context, loop, undefined);
    try {
      assert.deepStrictEqual(restoreEngineerDraft(context, loop) ?? study, { campaigns: [] },
        "an empty current draft must hide a queued old draft and prevent fallback to an older saved result");
    } finally { release(); await Promise.all([obsolete, empty]); }
    assert.deepStrictEqual(restoreEngineerDraft(context, loop) ?? study, { campaigns: [] },
      "reopening after persistence must not resurrect a superseded study");
  });

  test("round-trips a complete workflow draft with raw recording and identification state", async () => {
    const { context } = storage();
    const workflow = workflowDraft();
    await storeEngineerDraft(context, loop, draftStudy, workflow);
    const restored = restoreWorkflowDraft(context, loop)!;
    assert.deepStrictEqual(restored, workflow);
    assert.deepStrictEqual(restoreEngineerDraft(context, loop), draftStudy);
    (restored.recording.rawSamples as any)[0].pv = 99;
    assert.strictEqual(restoreWorkflowDraft(context, loop)!.recording.rawSamples[0].pv, 3);
  });

  test("an explicit full empty workflow supersedes an older accepted draft", async () => {
    const { context } = storage();
    await storeEngineerDraft(context, loop, draftStudy, workflowDraft());
    const empty = workflowDraft(draftStudy, undefined);
    await storeEngineerDraft(context, loop, { campaigns: [] }, empty);
    assert.deepStrictEqual(restoreWorkflowDraft(context, loop), empty);
    assert.deepStrictEqual(restoreEngineerDraft(context, loop), { campaigns: [] });
  });

  test("latest full workflow is visible from pending storage before queued writes finish", async () => {
    let release!: () => void;
    let started!: () => void;
    const writingStarted = new Promise<void>(resolve => { started = resolve; });
    let blocked = true;
    const { context } = storage(() => {
      if (!blocked) return Promise.resolve();
      blocked = false; started(); return new Promise<void>(resolve => { release = resolve; });
    });
    const first = storeEngineerDraft(context, loop, draftStudy, workflowDraft());
    await writingStarted;
    const latest = workflowDraft({ ...draftStudy, controllerChoice: "PID" });
    const second = storeEngineerDraft(context, loop, latest.engineerStudy!, latest);
    assert.strictEqual(restoreWorkflowDraft(context, loop)!.engineerStudy?.controllerChoice, "PID");
    release(); await Promise.all([first, second]);
    assert.strictEqual(restoreWorkflowDraft(context, loop)!.engineerStudy?.controllerChoice, "PID");
  });

  test("rejects malformed, mismatched, or shared-study workflow drafts", async () => {
    const { context, entries } = storage();
    const workflow = workflowDraft();
    await storeEngineerDraft(context, loop, draftStudy, workflow);
    for (const invalid of [
      { ...workflow, projectId: "other-project" },
      { ...workflow, loop: { ...workflow.loop, revision: "other-revision" } },
      { ...workflow, engineerStudy: { campaigns: [], controllerChoice: "PID" } },
      { ...workflow, recording: { ...workflow.recording, rawSamples: [{ bad: true }] } },
    ]) {
      await assert.rejects(() => storeEngineerDraft(context, loop, draftStudy, invalid as any));
      assert.deepStrictEqual(restoreWorkflowDraft(context, loop), workflow);
    }
    entries.set("trust.controlStudio.draft.project.sim.loop", { loopRevision: loop.revision, study: draftStudy, workflow: { malformed: true } });
    assert.throws(() => restoreWorkflowDraft(context, loop), /draft|decode|invalid/i);
  });

  test("sync draft snapshot preserves the async result snapshot contract", async () => {
    const { context } = storage();
    const state: any = { source: "Simulation", excludedIntervals: [], engineerStudy: draftStudy, identificationStudy: identificationStudy(),
      recording: undefined, previousResult: workflowDraft() };
    const sync = buildWorkflowDraftSnapshot(workflowLoop, state);
    const asyncResult = await buildWorkflowResultSnapshot({ extension: { packageJSON: { version: "0.24.65" } } } as any, workflowLoop, state);
    assert.deepStrictEqual({ ...sync, createdAt: "" }, { ...asyncResult, createdAt: "" });
  });

  test("a new native recording cannot inherit an imported recording's plant metadata", () => {
    const previous = workflowDraft();
    previous.plant = { source: "Simulation", id: "foreign-plant", configurationRevision: "sha256:foreign",
      model: "first_order_dead_time", configuration: { gain: 10 }, uncertainties: [], scheduledEvents: [], actuator: "0..100%", validityRange: "20..40 C" };
    previous.recording.provenance = { schemaVersion: 1, resultId: "foreign-result", resultRevision: 1,
      createdAt: previous.createdAt, projectId: "foreign-project", source: "Simulation", loop: previous.loop,
      controller: previous.controller, software: previous.software, units: previous.units, plant: previous.plant };
    const native = identificationStudy().current.recording.data;
    const result = buildWorkflowDraftSnapshot(workflowLoop, { previousResult: previous, source: "Simulation", excludedIntervals: [],
      recording: { samples: [], rawSamples: native.rawSamples, evidence: native.acquisition }, engineerStudy: draftStudy });
    assert.strictEqual(result.plant, undefined, "unknown current metadata must not be borrowed from another acquisition");
    assert.strictEqual(result.recording.provenance, undefined);
    assert.deepStrictEqual(result.recording.rawSamples, native.rawSamples);
  });
});
