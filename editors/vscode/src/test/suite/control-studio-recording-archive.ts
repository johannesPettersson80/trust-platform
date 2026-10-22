/** The loop fields a synthetic recording names: its identity, its controller and its two signals (symbol and unit). */
export interface RecordingArchiveLoop {
  id: string;
  revision: string;
  projectId: string;
  controller: { instance: string; revision: string };
  signals: {
    manipulatedVariable: { symbol: string; unit: string };
    processVariable: { symbol: string; unit: string };
  };
}

/**
 * Synthetic fitting fixture; native execution is established by the separate real PID journey. The file is a retained study
 * recording with one campaign. Its run verdict is written out for this synthetic run, whose only requirement (bounds of the
 * process variable) holds by construction: the response stays between 0 and 4 for an amplitude of 2.
 */
export function recordingArchive(loop: RecordingArchiveLoop, id: string, amplitude: number): string {
  const mv = loop.signals.manipulatedVariable, pv = loop.signals.processVariable;
  const units = { sp: pv.unit, pv: pv.unit, mv: mv.unit };
  const times = Array.from({ length: 201 }, (_, index) => index / 10);
  const run = { id, evidence: "ST-executed", parameters: {},
    conditions: { initialState: {}, events: [], durationSeconds: 20, sampleTimesSeconds: times, seed: 1,
      modelRevision: "synthetic-plant", programRevision: "synthetic-program", parameterSchemaRevision: "synthetic-parameters" },
    channels: [{ id: mv.symbol, unit: mv.unit }, { id: pv.symbol, unit: pv.unit }],
    samples: times.map((timeSeconds, index) => ({ timeSeconds, sequence: index + 1, values: {
      [mv.symbol]: { value: timeSeconds < 1 ? 0 : amplitude, quality: "good" },
      [pv.symbol]: { value: timeSeconds < 1 ? 0 : 2 * amplitude * (1 - Math.exp(-(timeSeconds - 1) / 2)), quality: "good" },
    } })),
  };
  const requirement = { id: "data-bounds", kind: "bounds" as const, channel: pv.symbol, window: { start: 0, end: 20 }, minimum: -20, maximum: 20 };
  // The engine evaluates the run again and compares its verdict with this one field by field (`decode_isolated_run`), so the verdict
  // is the engine's own: a bounds requirement that holds measures a violation of 0 against the limit 0, in the channel's unit
  // (`evaluate_observed_requirement`; the sentences are `measurement_within_limit` and `evaluation_scope` of `validation/texts.rs`).
  const evaluation = { status: "Pass", run, specification: [requirement],
    requirements: [{ id: requirement.id, kind: requirement.kind, status: "Pass", value: 0, limit: 0, unit: pv.unit,
      reason: "Observed measurement satisfies the declared limit." }],
    scope: "Only the declared observed conditions were evaluated; Pass does not establish general stability." };
  const result = { projectRevision: "synthetic-project", run, evaluation,
    request: { execution: "isolated-simulation", expected_project_revision: "synthetic-project", step_ms: 100, duration_ms: 20000, seed: 1,
      max_total_cycles: 201, wall_budget_ms: 1000, initial_values: {}, events: [], observations: run.channels.map(channel => ({ symbol: channel.id, unit: channel.unit })), parameter_overrides: [] } };
  const file = { schemaVersion: 4, createdAt: "2026-09-13T12:00:00Z", resultId: id, resultRevision: 1, projectId: loop.projectId, source: "Simulation",
    loop: { id: loop.id, revision: loop.revision }, controller: { instance: loop.controller.instance, revision: loop.controller.revision },
    software: { extensionVersion: "0.24.65", numericalAdapters: {} }, units,
    recording: { selectedIntervals: [], rawSamples: run.samples.map(sample => ({ elapsedSeconds: sample.timeSeconds, observedAtMs: sample.timeSeconds * 1000,
      actualCycleSeconds: .1, scan: sample.sequence, source: "Simulation", loopRevision: loop.revision,
      sp: 0, pv: sample.values[pv.symbol].value, requestedMv: sample.values[mv.symbol].value, appliedMv: sample.values[mv.symbol].value,
      units, quality: "good", freshnessMs: 0, controllerMode: "Manual", outputLimits: { minimum: -20, maximum: 20 }, rateLimits: {}, safetyState: "Good", events: [] })),
      acquisition: { method: "runtime_scan_buffer", completeness: "complete", recordingId: id, programRevision: "synthetic-program", observedSampleCount: 201,
        scanRange: { first: 1, last: 201 }, missingScanRanges: [], droppedObservedSamples: 0, actualCycleTimingAvailable: true, freshnessAvailable: true, reason: "Synthetic fitting fixture." } },
    evidence: { observation: "Observed", model: "Not evaluated" }, conclusion: "Synthetic fitting fixture." };
  return JSON.stringify({ ...file, engineerStudy: { campaigns: [{ id, status: "completed", runs: [result] }] } });
}
