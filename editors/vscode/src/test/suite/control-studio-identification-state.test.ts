import * as assert from "assert";
import { IdentificationWorkflow, type IdentificationSample } from "../../controlStudio/identification";
import { createWorkflowResult, decodeWorkflowResult } from "../../controlStudio/workflowResult";
import { buildWorkflowResultSnapshot } from "../../controlStudio/workflowResultSnapshot";
import { initializeStepResponseControls } from "../../controlStudio/webview/stepResponseControls";
import { StudioAnalysisSessions } from "../../controlStudio/studioAnalysisSessions";
import { routeIdentificationRecordingImport } from "../../controlStudio/identificationRecordingImport";

const settings = {
  baselineDurationSeconds: 5, finalDurationSeconds: 5, maximumWindowRange: .1,
  maximumWindowSlopePerSecond: .01, measurementBound: .05, onsetThreshold: .15, onsetSamples: 3,
  delay: { minimum: 0, maximum: 2, step: .2 }, timeConstant: { minimum: 4, maximum: 6, step: .2 },
  baselineStep: .01, gainStep: .001, timestampShiftSeconds: 0,
};
function samples(change: number): IdentificationSample[] {
  return Array.from({ length: 452 }, (_, i) => ({ timeSeconds: i / 10,
    input: i >= 50 && i < 450 ? 75 + change : 75,
    output: i <= 60 ? 35 : 35 + .2 * change * (1 - Math.exp(-(i / 10 - 6) / 5)),
    quality: "good", inputUnit: "%", outputUnit: "degC" }));
}
function preview(): any {
  const workflow = new IdentificationWorkflow() as any;
  workflow.qualify(samples(10), [], "Simulation", "fopdt", "source-estimation", settings);
  workflow.fit(); return workflow;
}
function snapshot(workflow: any): any {
  assert.strictEqual(typeof workflow.snapshot, "function", "the existing identification workflow must expose its restorable input and review state");
  return JSON.parse(JSON.stringify(workflow.snapshot()));
}
function originalRecording(change: number, id: string): any {
  const units = { sp: "", pv: "degC", mv: "%" };
  return { kind: "project_sequence", data: {
    rawSamples: samples(change).map((sample, i) => ({ elapsedSeconds: sample.timeSeconds, observedAtMs: 1000 + 100 * i,
      actualCycleSeconds: .1, scan: i + 10, source: "Simulation", loopRevision: "sha256:loop", sp: null,
      pv: sample.output, appliedMv: sample.input, requestedMv: null, units, quality: "good", freshnessMs: null,
      controllerMode: "Unavailable", outputLimits: { minimum: 0, maximum: 100 }, rateLimits: {}, safetyState: "Unavailable", events: [] })),
    selectedIntervals: [], acquisition: { recordingId: id, method: "runtime_scan_buffer", completeness: "complete",
      observedSampleCount: 452, scanRange: { first: 10, last: 461 }, missingScanRanges: [], droppedObservedSamples: 0,
      actualCycleTimingAvailable: true, freshnessAvailable: false, reason: "Every native scan retained." },
  } };
}
function acceptedWithOriginals(): any {
  const workflow = new IdentificationWorkflow() as any;
  workflow.qualify(samples(10), [], "Simulation", "fopdt", "source-estimation", settings, originalRecording(10, "source-estimation")); workflow.fit();
  workflow.qualify(samples(12), [], "Simulation", "fopdt", "source-validation", settings, originalRecording(12, "source-validation")); workflow.fit(); workflow.accept();
  return workflow;
}

suite("Control Studio identification state recovery", () => {
  test("public import retains original estimation and first-order settings when the export currently displays validation", async () => {
    const source = snapshot(acceptedWithOriginals()), data = source.current.recording.data;
    const sourceLoop: any = { id: "loop", revision: "sha256:loop", topology: "open-loop", projectId: "project",
      controller: { instance: "P1", revision: "sha256:program" },
      signals: { processVariable: { unit: "degC" }, manipulatedVariable: { unit: "%" } } };
    const result = await buildWorkflowResultSnapshot({ extension: { packageJSON: { version: "0.24.65" } } } as any, sourceLoop,
      { source: "Simulation", excludedIntervals: [], recording: { samples: [], rawSamples: data.rawSamples, evidence: data.acquisition }, identificationStudy: source } as any);
    assert.ok(decodeWorkflowResult(result), "the fixture must be a valid exported two-record study");
    const loop: any = { ...sourceLoop, id: "model-structure", projectId: "next-lesson", runtimeId: "simulator", revision: "next-revision" };
    const scope = { projectId: loop.projectId, loopId: loop.id, loopRevision: loop.revision, editorSessionId: "next-editor" };
    const sessions = new StudioAnalysisSessions(); let imported: any, reference: any;
    try {
      await routeIdentificationRecordingImport({ schemaVersion: 1, kind: "control-studio.identify.import", messageId: "import",
        projectId: scope.projectId, loopId: scope.loopId, editorSessionId: scope.editorSessionId, expectedLoopRevision: scope.loopRevision,
        payload: { json: JSON.stringify(result) } }, scope, loop, false, (async (recording: any, original: any) => {
          imported = recording; reference = original;
          (sessions.selectIdentificationRecording as Function)(recording, original);
        }) as any, async () => undefined);
      assert.strictEqual(imported?.rawSamples[100].appliedMv, source.estimation.recording.data.rawSamples[100].appliedMv,
        "Candidate A must import the 85-percent estimation command, not the 87-percent validation command");
      assert.deepStrictEqual(imported?.rawSamples, source.estimation.recording.data.rawSamples,
        "Candidate A must use the original 10-point estimation step, not the later 12-point validation");
      assert.deepStrictEqual(reference?.stepResponseSettings, settings);
      assert.strictEqual(sessions.identification.currentState.phase, "selected", "imported inputs still require Check");
      const current = sessions.identification.snapshot()!.current;
      assert.deepStrictEqual(current.stepResponseSettings, settings);
      assert.strictEqual(current.recording?.kind, source.estimation.recording.kind);
      assert.strictEqual(current.recording?.data.acquisition?.recordingId, "source-estimation");
      assert.strictEqual(sessions.identification.acceptedModel, undefined);
      assert.strictEqual(sessions.model.selectedModel(), undefined);
      const extended = { ...settings, delay: { minimum: .2, maximum: 1.8, step: .1 }, sensorTimeConstant: { minimum: .1, maximum: 1, step: .1 } };
      sessions.qualifyIdentification(current.estimation as IdentificationSample[], [], "Simulation", "sopdt", current.estimationRecordingId,
        extended, current.recording);
      assert.deepStrictEqual(sessions.identification.snapshot()!.current.firstOrderStepResponseSettings, settings,
        "refining B immediately after import must preserve the original A grid");
    } finally { sessions.dispose(); }
  });

  test("selecting a validation recording retains estimation but requires Check again, including after restore", () => {
    const workflow = preview();
    const estimation = snapshot(workflow).estimation;
    assert.strictEqual(typeof workflow.selectRecording, "function", "recording selection needs an unqualified state owned by identification");
    workflow.selectRecording(samples(12), [], "Simulation", "fopdt", "source-validation", settings);
    assert.strictEqual(workflow.nextFitRole, "validation");
    assert.strictEqual(workflow.acceptedModel, undefined);
    assert.throws(() => workflow.fit(), /Qualify a suitable recording/);
    // Select again after the intentionally rejected Fit so the saved state is pending Check.
    workflow.selectRecording(samples(12), [], "Simulation", "fopdt", "source-validation", settings);
    const selected = snapshot(workflow);
    assert.strictEqual(selected.phase, "selected");
    assert.deepStrictEqual(selected.estimation, estimation);
    const restored = new IdentificationWorkflow() as any;
    restored.restore(selected);
    assert.deepStrictEqual(snapshot(restored), selected);
    restored.qualify(samples(12), [], "Simulation", "fopdt", "source-validation", settings);
    restored.fit();
    assert.strictEqual(restored.accept().review.result.partition, "independent-runs");
  });

  test("opening a second imported FOPDT recording preserves the existing estimation and its step settings", () => {
    const sessions = new StudioAnalysisSessions();
    const first = preview();
    const firstState = snapshot(first);
    sessions.identification.restore(firstState);
    const reference: any = { ...firstState.current, firstOrderStepResponseSettings: settings };
    const data = originalRecording(12, "second-validation").data;
    sessions.selectIdentificationRecording({ samples: [], rawSamples: data.rawSamples, evidence: data.acquisition } as any, reference);
    const selected = snapshot(sessions.identification);
    assert.ok(selected.estimation, "the first imported estimation must survive selecting the second recording");
    assert.strictEqual(selected.estimation.estimationRecordingId, "source-estimation");
    assert.strictEqual(selected.current.estimationRecordingId, "second-validation");
    assert.deepStrictEqual(selected.current.stepResponseSettings, settings);
    assert.strictEqual(selected.current.firstOrderStepResponseSettings, undefined, "FOPDT has no separate first-order reference fit; do not import foreign SOPDT settings");
    sessions.dispose();
  });

  test("recording replacement clears an accepted design and checks runtime admission before mutation", () => {
    const sessions = new StudioAnalysisSessions() as any;
    sessions.identification.restore(snapshot(acceptedWithOriginals()));
    const signal = (symbol: string, unit: string) => ({ symbol, unit, range: { minimum: 0, maximum: 100 } });
    sessions.acceptIdentification(sessions.identification.accept(), { signals: {
      processVariable: signal("P1.PV", "degC"), manipulatedVariable: signal("P1.MV", "%") } });
    assert.strictEqual(typeof sessions.selectIdentificationRecording, "function", "import must replace recordings through the selected-model owner");
    const data = originalRecording(12, "new-selection").data;
    const recording = { samples: [], rawSamples: data.rawSamples, evidence: data.acquisition };
    const before = snapshot(sessions.identification), model = sessions.model.selectedModel();
    sessions.engineer = { assertModelReplacementAllowed: () => { throw new Error("operation active"); } };
    assert.throws(() => sessions.selectIdentificationRecording(recording), /operation active/);
    assert.deepStrictEqual(snapshot(sessions.identification), before);
    assert.deepStrictEqual(sessions.model.selectedModel(), model);
    sessions.engineer = undefined;
    sessions.selectIdentificationRecording(recording);
    assert.strictEqual(sessions.model.selectedModel(), undefined);
    assert.strictEqual(sessions.identification.acceptedModel, undefined);
    assert.strictEqual(sessions.identification.nextFitRole, "estimation");
    assert.strictEqual(snapshot(sessions.identification).current.estimationRecordingId, "new-selection");
    assert.deepStrictEqual(snapshot(sessions.identification).current.recording.data.rawSamples, data.rawSamples);
  });
  test("restores a preview and uses the next distinct record for validation", () => {
    const original = preview(), saved = snapshot(original);
    assert.strictEqual(saved.phase, "preview");
    assert.strictEqual(saved.estimation.estimationRecordingId, "source-estimation");
    assert.deepStrictEqual(saved.estimation.stepResponseSettings, settings);
    const restored = new IdentificationWorkflow() as any;
    restored.restore(saved);
    assert.strictEqual(restored.nextFitRole, "validation");
    assert.strictEqual(restored.acceptedModel, undefined);
    restored.qualify(samples(12), [], "Simulation", "fopdt", "source-validation", settings);
    assert.strictEqual(restored.fit().stage, "acceptance_required");
    assert.strictEqual(restored.acceptedModel, undefined, "restoring a preview must not accept a model");
    restored.accept();
    saved.estimation.estimation[0].output = 99;
    assert.strictEqual(snapshot(original).estimation.estimation[0].output, 35);
  });

  test("restores qualified validation and accepted state without fitting a new estimation record", () => {
    const original = preview();
    original.qualify(samples(12), [], "Simulation", "fopdt", "source-validation", settings);
    const qualified = snapshot(original);
    assert.strictEqual(qualified.phase, "validation-qualified");
    const restored = new IdentificationWorkflow() as any; restored.restore(qualified);
    const review = restored.fit(); restored.accept();
    const accepted = snapshot(restored);
    assert.strictEqual(accepted.phase, "accepted");
    assert.strictEqual(accepted.current.estimationRecordingId, "source-validation");
    const reopened = new IdentificationWorkflow() as any; reopened.restore(accepted);
    assert.deepStrictEqual(reopened.acceptedModel, review.selected);
    assert.deepStrictEqual(snapshot(reopened), accepted);
  });

  test("rejects malformed or invalid accepted input atomically and preserves the current analysis", () => {
    const active = preview(), before = snapshot(active);
    const accepted = preview(); accepted.qualify(samples(12), [], "Simulation", "fopdt", "source-validation", settings);
    accepted.fit(); accepted.accept();
    for (const mutate of [
      (value: any) => { value.schemaVersion = 99; },
      (value: any) => { delete value.estimation; },
      (value: any) => { value.current.estimationRecordingId = value.estimation.estimationRecordingId; },
      (value: any) => { value.current.estimation[100].output += 5; },
      (value: any) => { value.estimation.stepResponseSettings.measurementBound = 0; },
    ]) {
      const invalid = snapshot(accepted); mutate(invalid);
      assert.throws(() => active.restore(invalid));
      assert.deepStrictEqual(snapshot(active), before, "failed restore cannot erase or partly replace the active preview");
    }
  });

  test("retains both complete original acquisition records inside the identification session", () => {
    const workflow = acceptedWithOriginals(), saved = snapshot(workflow);
    assert.deepStrictEqual(saved.estimation.recording, originalRecording(10, "source-estimation"), "the estimation input must retain its original complete scan record");
    assert.deepStrictEqual(saved.current.recording, originalRecording(12, "source-validation"));
    const restored = new IdentificationWorkflow() as any; restored.restore(saved);
    assert.deepStrictEqual(snapshot(restored), saved);
  });

  test("saved workflow results include identification state and reject mismatched source data", async () => {
    const saved = snapshot(acceptedWithOriginals());
    const data = originalRecording(12, "source-validation").data;
    const loop: any = { id: "loop", revision: "sha256:loop", topology: "open-loop", projectId: "project",
      controller: { instance: "P1", revision: "sha256:program" },
      signals: { processVariable: { unit: "degC" }, manipulatedVariable: { unit: "%" } } };
    const context: any = { extension: { packageJSON: { version: "0.24.65" } } };
    const result: any = await buildWorkflowResultSnapshot(context, loop, { source: "Simulation", excludedIntervals: [],
      recording: { samples: [], rawSamples: data.rawSamples, evidence: data.acquisition }, identificationStudy: saved } as any);
    assert.deepStrictEqual(result.identificationStudy, saved, "Save must keep the complete identification session, not only an accepted model");
    assert.ok(decodeWorkflowResult(JSON.parse(JSON.stringify(result))));
    for (const mutate of [
      (value: any) => { value.identificationStudy.schemaVersion = 99; },
      (value: any) => { value.identificationStudy.estimation.recording.data.rawSamples[20].pv += 10; },
      (value: any) => { value.identificationStudy.estimation.recording.data.acquisition.recordingId = "other"; },
      (value: any) => { value.identificationStudy.estimation.recording.data.rawSamples[20].loopRevision = "sha256:other"; },
    ]) {
      const invalid = JSON.parse(JSON.stringify(result)); mutate(invalid);
      assert.strictEqual(decodeWorkflowResult(invalid), undefined, "saved analysis must bind its inputs to the original acquisition");
    }
    assert.doesNotThrow(() => createWorkflowResult({ ...result, resultRevision: 2 }, result.createdAt));
  });

  test("reopened held-step controls show the saved measurement settings instead of defaults", () => {
    const { JSDOM } = require("jsdom");
    const dom = new JSDOM('<main data-control-studio data-project-id="p" data-loop-id="l" data-editor-session-id="s" data-loop-revision="r"><section data-workflow-pane="Identify"><div class="field"><select id="identify-model"><option value="automatic">Automatic</option><option value="fopdt">FOPDT</option></select></div><details><input id="identify-exclude-start"></details><div class="summary-panel"></div></section></main>');
    const previous = { document: (globalThis as any).document, window: (globalThis as any).window };
    Object.assign(globalThis, { document: dom.window.document, window: dom.window });
    try {
      initializeStepResponseControls();
      dom.window.dispatchEvent(new dom.window.MessageEvent("message", { data: {
        kind: "control-studio.workflow-result", projectId: "p", loopId: "l", editorSessionId: "s", expectedLoopRevision: "r", phase: "restored",
        payload: { result: { identificationStudy: snapshot(preview()), units: { pv: "degC", mv: "%" } } },
      } }));
      assert.strictEqual(dom.window.document.querySelector('[data-step-response-enabled]').checked, true);
      assert.strictEqual(dom.window.document.querySelector('[data-step-field="delay.step"]').value, "0.2");
      assert.strictEqual(dom.window.document.querySelector('#identify-model').value, "fopdt");
      assert.strictEqual(dom.window.document.querySelector('#identify-exclude-start').closest('details').hidden, true);
    } finally { Object.assign(globalThis, previous); dom.window.close(); }
  });

  test("new identification inputs invalidate the selected design model through the session owner", () => {
    const sessions = new StudioAnalysisSessions() as any;
    sessions.identification.restore(snapshot(acceptedWithOriginals()));
    const signal = (symbol: string, unit: string) => ({ symbol, unit, range: { minimum: 0, maximum: 100 } });
    sessions.acceptIdentification(sessions.identification.accept(), { signals: {
      processVariable: signal("P1.PV", "degC"), manipulatedVariable: signal("P1.MV", "%") } });
    assert.ok(sessions.model.selectedModel());
    assert.strictEqual(typeof sessions.qualifyIdentification, "function", "identification changes must pass through the owner of the selected design model");
    sessions.qualifyIdentification(samples(12), [], "Simulation", "fopdt", "next", settings, originalRecording(12, "next"));
    assert.strictEqual(sessions.identification.acceptedModel, undefined);
    assert.strictEqual(sessions.model.selectedModel(), undefined, "an old accepted model cannot remain active or be saved as the selected model after inputs change");
    assert.strictEqual(sessions.model.snapshot(), undefined);
  });

  test("busy-operation admission rejects identification changes and restores before mutating current state", () => {
    const sessions = new StudioAnalysisSessions() as any;
    sessions.identification.restore(snapshot(preview()));
    const before = snapshot(sessions.identification);
    sessions.engineer = { assertModelReplacementAllowed: () => { throw new Error("operation active"); },
      assertStudyRestoreAllowed: () => { throw new Error("operation active"); } };
    assert.strictEqual(typeof sessions.qualifyIdentification, "function");
    assert.strictEqual(typeof sessions.restoreIdentification, "function");
    assert.throws(() => sessions.qualifyIdentification(samples(12), [], "Simulation", "fopdt", "next", settings), /operation active/);
    assert.deepStrictEqual(snapshot(sessions.identification), before);
    assert.throws(() => sessions.restoreIdentification({ identificationStudy: snapshot(acceptedWithOriginals()) }), /operation active/);
    assert.deepStrictEqual(snapshot(sessions.identification), before);
  });
});
