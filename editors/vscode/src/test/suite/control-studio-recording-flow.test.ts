import { decodeEngineerStudy } from "../../controlStudio/engineerStudyPersistence";
import * as assert from "assert";
import { StudioAnalysisSessions } from "../../controlStudio/studioAnalysisSessions";
import { createWorkflowResult } from "../../controlStudio/workflowResult";
import { evaluateRecordedRun, type RecordedRun } from "../../controlStudio/runValidation";
import type { ControlLoopDefinition } from "../../controlStudio/loopDefinition";
import { pidLoop } from "./control-studio-pid-fixtures";

/** Synthetic fitting fixture; native execution is established by the separate real PID journey. */
export function recordingArchive(loop: ControlLoopDefinition, id: string, amplitude: number): string {
  const mv = loop.signals.manipulatedVariable, pv = loop.signals.processVariable;
  const units = { sp: pv.unit, pv: pv.unit, mv: mv.unit };
  const times = Array.from({ length: 201 }, (_, index) => index / 10);
  const run: RecordedRun = { id, evidence: "ST-executed", parameters: {},
    conditions: { initialState: {}, events: [], durationSeconds: 20, sampleTimesSeconds: times, seed: 1,
      modelRevision: "synthetic-plant", programRevision: "synthetic-program", parameterSchemaRevision: "synthetic-parameters" },
    channels: [{ id: mv.symbol, unit: mv.unit }, { id: pv.symbol, unit: pv.unit }],
    samples: times.map((timeSeconds, index) => ({ timeSeconds, sequence: index + 1, values: {
      [mv.symbol]: { value: timeSeconds < 1 ? 0 : amplitude, quality: "good" },
      [pv.symbol]: { value: timeSeconds < 1 ? 0 : 2 * amplitude * (1 - Math.exp(-(timeSeconds - 1) / 2)), quality: "good" },
    } })),
  };
  const requirement = { id: "data-bounds", kind: "bounds" as const, channel: pv.symbol, window: { start: 0, end: 20 }, minimum: -20, maximum: 20 };
  const result = { projectRevision: "synthetic-project", run, evaluation: evaluateRecordedRun(run, [requirement]),
    request: { execution: "isolated-simulation", expected_project_revision: "synthetic-project", step_ms: 100, duration_ms: 20000, seed: 1,
      max_total_cycles: 201, wall_budget_ms: 1000, initial_values: {}, events: [], observations: run.channels.map(channel => ({ symbol: channel.id, unit: channel.unit })), parameter_overrides: [] } };
  const file = createWorkflowResult({ resultId: id, resultRevision: 1, projectId: loop.projectId, source: "Simulation",
    loop: { id: loop.id, revision: loop.revision }, controller: { instance: loop.controller.instance, revision: loop.controller.revision },
    software: { extensionVersion: "0.24.65", numericalAdapters: {} }, units,
    recording: { selectedIntervals: [], rawSamples: run.samples.map(sample => ({ elapsedSeconds: sample.timeSeconds, observedAtMs: sample.timeSeconds * 1000,
      actualCycleSeconds: .1, scan: sample.sequence, source: "Simulation" as const, loopRevision: loop.revision,
      sp: 0, pv: sample.values[pv.symbol].value, requestedMv: sample.values[mv.symbol].value, appliedMv: sample.values[mv.symbol].value,
      units, quality: "good" as const, freshnessMs: 0, controllerMode: "Manual" as const, outputLimits: { minimum: -20, maximum: 20 }, rateLimits: {}, safetyState: "Good", events: [] })),
      acquisition: { method: "runtime_scan_buffer", completeness: "complete", recordingId: id, programRevision: "synthetic-program", observedSampleCount: 201,
        scanRange: { first: 1, last: 201 }, missingScanRanges: [], droppedObservedSamples: 0, actualCycleTimingAvailable: true, freshnessAvailable: true, reason: "Synthetic fitting fixture." } },
    evidence: { observation: "Observed", model: "Not evaluated" }, conclusion: "Synthetic fitting fixture." }, "2026-09-13T12:00:00Z");
  return JSON.stringify({ ...file, schemaVersion: 4, engineerStudy: { campaigns: [{ id, status: "completed", runs: [result] }] } });
}

suite("Control Studio recording flow", () => {
  test("fits two independent opened recordings through the shared owner without a live Find", async () => {
    const loop = pidLoop() as ControlLoopDefinition;
    const sessions = new StudioAnalysisSessions(loop);
    const scope = { projectId: loop.projectId, editorSessionId: "recording-editor", loopId: loop.id, loopRevision: loop.revision };
    const posted: Record<string, unknown>[] = [];
    const messages: string[] = [];
    const handle = sessions.engineer!.handle.bind(sessions.engineer);
    sessions.engineer!.handle = async (message, expected, post) => {
      const kind = (message as {kind: string}).kind; messages.push(kind);
      assert.ok(!kind.endsWith(".find") && !kind.endsWith(".apply") && !kind.endsWith(".verify"), "Opening data cannot start live acquisition or parameter writes");
      return handle(message, expected, post);
    };
    const open = async (id: string, amplitude: number) => sessions.handle({ schemaVersion: 1, kind: "control-studio.flow.start", messageId: id,
      ...scope, expectedLoopRevision: loop.revision, payload: { from: "recording", json: recordingArchive(loop, id, amplitude) } }, scope,
    async (stage, payload) => { posted.push({ stage, ...payload }); });
    try {
      assert.strictEqual(await open("estimation", 2), true);
      assert.ok(posted.some(payload => payload.status === "recording-needed"), "Request an independent recording in the same action area");
      assert.strictEqual(sessions.model.selectedModel(), undefined);
      await open("validation", 1.5);
      assert.ok(sessions.model.selectedModel(), JSON.stringify(posted));
      assert.ok(posted.some(payload => payload.status === "model-ready"), JSON.stringify(posted));
      const comparison = sessions.engineer!.snapshotStudy()!.modelComparison!;
      const experiment = (comparison.presentation as any).experiment;
      assert.ok(experiment?.inputs?.length, "Opened-recording model review includes its recorded plant input");
      assert.strictEqual(Math.max(...experiment.inputs[0].points.map((point: any) => point.value)), 1.5, "Display the independent validation input, not the estimation trial");
      assert.strictEqual(experiment.outputs[0].unit, loop.signals.processVariable.unit);
      const saved = sessions.engineer!.snapshotStudy()!;
      assert.deepStrictEqual((decodeEngineerStudy(saved)!.modelComparison!.presentation as any).experiment, experiment, "Retain the same experiment when the study is decoded");
      const malformed = structuredClone(saved);
      (malformed.modelComparison!.presentation as any).experiment.inputs[0].points[0].value = Infinity;
      assert.strictEqual(decodeEngineerStudy(malformed), undefined, "Reject non-finite recorded display values");

      assert.strictEqual(posted[posted.length - 1].flowStage, "Identify", "Keep the recorded model available for review");
      assert.strictEqual(posted[posted.length - 1].state, "completed");
      assert.strictEqual(posted[posted.length - 1].missing, undefined, "Opening recordings does not request Tune requirements");
      assert.ok(!messages.some(kind => kind.endsWith(".find")));
    } finally { sessions.dispose(); }
  });

  test("experiment parameter application invalidates dependent design state but keeps recordings", async () => {
    const loop = pidLoop() as ControlLoopDefinition;
    loop.parameterPurpose = "experiment";
    const binding = loop.parameterBinding!;
    binding.mode = { ...binding.mode, manual_value: false };
    loop.experiment = { execution: "project-scripted", kind: "observation", amplitude: 0, durationSeconds: 30, seed: 1,
      activation: { symbol: binding.mode.symbol, address: "%MX0.0", iecType: "BOOL", direction: "write" } };
    const sessions = new StudioAnalysisSessions(loop);
    const posted: Array<{ stage: string; payload: Record<string, unknown> }> = [];
    const forward = (sessions as any).forwardSessionUpdate.bind(sessions);
    try {
      await forward("parameters", { status: "inspected" }, async (stage: string, payload: Record<string, unknown>) => posted.push({ stage, payload }));
      await forward("parameters", { status: "reviewed", reviewId: "review-1" }, async (stage: string, payload: Record<string, unknown>) => posted.push({ stage, payload }));
      assert.deepStrictEqual(posted.map(item => item.stage), ["parameters", "parameters"]);
      await forward("parameters", { status: "applied", observed: { values: {} } }, async (stage: string, payload: Record<string, unknown>) => posted.push({ stage, payload }));
      assert.strictEqual(posted[2].stage, "parameters");
      assert.strictEqual(posted[3].stage, "engineer", "confirmed experiment parameters must invalidate dependent candidate/verification state");
      assert.strictEqual(posted[3].payload.status, "edited", "Preparing a project experiment does not require an identified model");
      assert.strictEqual((posted[3].payload.stageEvidence as any).hasCandidate, false);
    } finally { sessions.dispose(); }
  });
});
