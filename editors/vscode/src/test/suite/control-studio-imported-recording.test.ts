import * as assert from "assert";
import { createWorkflowResult, decodeWorkflowResult, exportWorkflowJson } from "../../controlStudio/workflowResult";
import { buildWorkflowResultSnapshot } from "../../controlStudio/workflowResultSnapshot";
import { identificationSamplesFromEvidence } from "../../controlStudio/panelWorkflowSupport";

function importedResult(): any {
  const units = { sp: "", pv: "°C", mv: "%" };
  const original = createWorkflowResult({
    resultId: "record-source", resultRevision: 3, projectId: "project:experiment", source: "Simulation",
    loop: { id: "experiment-loop", revision: "sha256:experiment", topology: "open-loop" },
    controller: { instance: "P1", revision: "sha256:source-program" },
    software: { extensionVersion: "0.24.65", numericalAdapters: {} }, units,
    recording: { rawSamples: [0, 1].map(i => ({ elapsedSeconds: i / 10, observedAtMs: 1000 + i * 100,
      actualCycleSeconds: 0.1, scan: 80 + i, source: "Simulation", loopRevision: "sha256:experiment",
      sp: null, pv: 35, requestedMv: null, appliedMv: 75, units, quality: "good", freshnessMs: 0,
      controllerMode: "Unavailable", outputLimits: { minimum: 0, maximum: 100 }, rateLimits: {},
      safetyState: "Unavailable", events: [] })), selectedIntervals: [],
      acquisition: { method: "runtime_scan_buffer", completeness: "complete", programRevision: "sha256:source-program",
        observedSampleCount: 2, scanRange: { first: 80, last: 81 }, missingScanRanges: [], droppedObservedSamples: 0,
        actualCycleTimingAvailable: true, freshnessAvailable: true, reason: "Every runtime scan retained." } },
    evidence: { observation: "Observed", model: "Not evaluated" }, conclusion: "Measured experiment.",
  }, "2026-09-12T12:00:00.000Z");
  return { ...original, resultId: "analysis-result", resultRevision: 1, projectId: "project:analysis",
    loop: { id: "analysis-loop", revision: "sha256:analysis", topology: "open-loop" },
    controller: { instance: "P1", revision: "sha256:analysis-program" },
    recording: { ...original.recording, provenance: { schemaVersion: 1,
      resultId: original.resultId, resultRevision: original.resultRevision, createdAt: original.createdAt,
      projectId: original.projectId, source: original.source, loop: original.loop,
      controller: original.controller, software: original.software, units: original.units } } };
}

suite("Control Studio imported recording provenance", () => {
  test("the currently selected native recording takes precedence over an older Test role", () => {
    const value = importedResult();
    const selected: any = { samples: [], rawSamples: value.recording.rawSamples,
      evidence: { ...value.recording.acquisition, recordingId: "current-native-recording" } };
    const older: any = { experimentId: "old-test", role: "validation",
      samples: [{ timeSeconds: 0, input: 1, output: 2, quality: "good" }] };
    assert.deepStrictEqual(identificationSamplesFromEvidence(older, selected, []).map(s => [s.timeSeconds, s.input, s.output]),
      [[0, 75, 35], [.1, 75, 35]], "Identify must use the displayed current native recording, not an older validation ledger entry");
  });

  test("an explicitly imported recording takes precedence over an older local Test record", () => {
    const value = importedResult();
    const oldTest: any = { samples: [{ timeSeconds: 0, input: 5, output: 12, quality: "good", inputUnit: "%", outputUnit: "°C" }] };
    const imported: any = { samples: [], rawSamples: value.recording.rawSamples,
      evidence: value.recording.acquisition, provenance: value.recording.provenance };
    const samples = identificationSamplesFromEvidence(oldTest, imported, []);
    assert.deepStrictEqual(samples.map(sample => [sample.timeSeconds, sample.input, sample.output]), [[0, 75, 35], [0.1, 75, 35]],
      "Identify must check the explicitly selected foreign recording, not a previous local Test");
    assert.deepStrictEqual(identificationSamplesFromEvidence(oldTest, undefined, []), oldTest.samples,
      "local Test selection keeps its existing behavior when no imported recording is selected");
  });

  test("saving and reopening an imported recording preserves acquisition source rather than the live runtime source", async () => {
    const value = importedResult();
    const loop: any = { id: value.loop.id, revision: value.loop.revision, topology: "open-loop", projectId: value.projectId,
      controller: value.controller, signals: { processVariable: { unit: "°C" }, manipulatedVariable: { unit: "%" } } };
    const context: any = { extension: { packageJSON: { version: "0.24.65" } } };
    const state: any = { source: "Physical", excludedIntervals: [], recording: { samples: [],
      rawSamples: value.recording.rawSamples, evidence: value.recording.acquisition, provenance: value.recording.provenance } };
    let saved: any;
    await assert.doesNotReject(async () => { saved = await buildWorkflowResultSnapshot(context, loop, state); },
      "saving imported Simulation data must not relabel it with the currently live Physical source");
    assert.strictEqual(saved.source, "Simulation");
    assert.strictEqual(saved.projectId, loop.projectId);
    assert.deepStrictEqual(saved.recording.provenance, value.recording.provenance);
    assert.deepStrictEqual(saved.recording.rawSamples, value.recording.rawSamples);
    const reopened = decodeWorkflowResult(JSON.parse(exportWorkflowJson(saved)))!;
    const resaved = await buildWorkflowResultSnapshot(context, loop,
      { source: "Physical", previousResult: reopened, excludedIntervals: [] });
    assert.deepStrictEqual(resaved.recording, saved.recording, "another Save after Open keeps the original acquisition identity");
    assert.strictEqual(resaved.source, "Simulation");
    assert.strictEqual(resaved.resultRevision, saved.resultRevision + 1);
  });

  test("round-trips a foreign recording without relabelling its source loop or adopting design state", () => {
    const value = importedResult();
    const decoded: any = decodeWorkflowResult(JSON.parse(exportWorkflowJson(value)));
    assert.ok(decoded, "an explicitly attributed foreign recording must survive Save/Open");
    assert.strictEqual(decoded.projectId, "project:analysis");
    assert.strictEqual(decoded.recording.provenance.projectId, "project:experiment");
    assert.deepStrictEqual(decoded.recording, value.recording);
    assert.strictEqual(decoded.recording.rawSamples[0].loopRevision, "sha256:experiment");
    for (const key of ["model", "candidate", "validation", "commissioning", "engineerStudy"]) assert.strictEqual(decoded[key], undefined);
    value.recording.rawSamples[0].pv = 99;
    assert.strictEqual(decoded.recording.rawSamples[0].pv, 35, "saved evidence is detached from the import draft");
  });

  test("rejects inconsistent or malformed recording provenance without weakening ordinary result ownership", () => {
    const valid = importedResult();
    for (const change of [
      (v: any) => { delete v.recording.provenance.projectId; },
      (v: any) => { v.recording.provenance.schemaVersion = 2; },
      (v: any) => { v.recording.provenance.resultRevision = 0; },
      (v: any) => { v.recording.provenance.source = "Physical"; },
      (v: any) => { v.recording.provenance.loop.revision = "sha256:other"; },
      (v: any) => { v.recording.provenance.units.pv = "K"; },
      (v: any) => { v.recording.provenance.candidate = {}; },
      (v: any) => { v.recording.acquisition.recordingId = 12; },
      (v: any) => { v.recording.acquisition.recordingId = ""; },
      (v: any) => { delete v.recording.provenance; },
    ]) {
      const value = structuredClone(valid); change(value);
      assert.strictEqual(decodeWorkflowResult(value), undefined, "invalid provenance cannot authorize foreign samples");
    }
  });
});
