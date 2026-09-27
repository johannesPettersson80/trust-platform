import * as assert from "assert";
import { StudioAnalysisSessions } from "../../controlStudio/studioAnalysisSessions";
import { IdentificationWorkflow, type IdentificationSample } from "../../controlStudio/identification";
import { createManualWorkflowState, handleManualWorkflowCommand } from "../../controlStudio/manualWorkflowCommands";
import { ExperimentIdentificationLedger } from "../../controlStudio/experiment";
import { openControlStudioCommand } from "../../controlStudio/openCommand";
import { decodeEngineeringCommand } from "../../controlStudio/messages";
import { setIdentificationActionState } from "../../controlStudio/webview/identificationActions";

const settings = {
  baselineDurationSeconds: 5, finalDurationSeconds: 5, maximumWindowRange: .1,
  maximumWindowSlopePerSecond: .01, measurementBound: .05, onsetThreshold: .15, onsetSamples: 3,
  delay: { minimum: 0, maximum: 2, step: .5 }, timeConstant: { minimum: 1, maximum: 9, step: .5 },
  sensorTimeConstant: { minimum: .5, maximum: 6, step: .5 }, baselineStep: .01, gainStep: .001, timestampShiftSeconds: 0,
};
function samples(change = 10): IdentificationSample[] {
  return Array.from({ length: 252 }, (_, i) => ({ timeSeconds: i / 5,
    input: i >= 25 && i < 250 ? 75 + change : 75,
    output: i <= 30 ? 35 : 35 + .2 * change * (1 - Math.exp(-(i / 5 - 6) / 5)),
    quality: "good", inputUnit: "%", outputUnit: "degC" }));
}
function fixture() {
  const sessions = new StudioAnalysisSessions();
  sessions.qualifyIdentification(samples(), [], "Simulation", "sopdt", "estimation", settings);
  const posted: any[] = [];
  let retained = 0;
  const context: Parameters<typeof handleManualWorkflowCommand>[0] = {
    command: { action: "identify-fit", messageId: "fit" },
    loop: { signals: { manipulatedVariable: { unit: "%" }, processVariable: { unit: "degC" } } } as any,
    sessions, state: createManualWorkflowState(), selectedRecording: undefined, currentSavedResult: undefined,
    currentLoopSample: undefined, testLedger: new ExperimentIdentificationLedger(),
    post: async (_phase, payload) => { posted.push(payload); }, retainDraft: async () => { retained += 1; },
    runtimeClient: async () => { throw new Error("Identification must not write to a runtime."); },
    runtimeCandidate: async () => { throw new Error("Identification must not read or adopt a controller."); },
  };
  return { sessions, posted, context, retained: () => retained };
}

suite("Control Studio identification worker", () => {
  test("keeps the host event loop responsive while the public Fit command computes a comparison", async () => {
    const f = fixture(); let ticked = false;
    const timer = setTimeout(() => { ticked = true; }, 0);
    try {
      await handleManualWorkflowCommand(f.context);
      assert.strictEqual(ticked, true, "Fit must yield the host event loop while calculating");
      assert.strictEqual(f.sessions.identification.currentState.phase, "preview");
      assert.ok(f.sessions.identification.currentState.review?.fit.modelStructure);
      assert.strictEqual(f.retained(), 1);
      assert.strictEqual(f.posted.filter(value => value.phase === "validation_required").length, 1);
    } finally { clearTimeout(timer); f.sessions.dispose(); }
  });

  test("changing qualified inputs cancels the old calculation without publishing or persisting its preview", async () => {
    const f = fixture();
    try {
      const changed = new Promise<void>(resolve => setTimeout(() => {
        f.sessions.qualifyIdentification(samples(12), [], "Simulation", "sopdt", "new-estimation", settings); resolve();
      }, 0));
      await Promise.all([handleManualWorkflowCommand(f.context), changed]);
      assert.strictEqual(f.sessions.identification.snapshot()?.current.estimationRecordingId, "new-estimation");
      assert.strictEqual(f.sessions.identification.currentState.phase, "qualified");
      assert.strictEqual(f.posted.filter(value => value.phase === "validation_required").length, 0);
      assert.strictEqual(f.retained(), 0);
    } finally { f.sessions.dispose(); }
  });

  test("disposing the editor cancels its calculation without a late result", async () => {
    const f = fixture();
    const disposed = new Promise<void>(resolve => setTimeout(() => { f.sessions.dispose(); resolve(); }, 0));
    await Promise.all([handleManualWorkflowCommand(f.context), disposed]);
    assert.strictEqual(f.posted.filter(value => value.phase === "validation_required").length, 0);
    assert.strictEqual(f.retained(), 0);
  });

  test("replays a saved comparison without blocking the host or refitting on different inputs", async () => {
    const f = fixture(), source = new IdentificationWorkflow();
    source.qualify(samples(), [], "Simulation", "sopdt", "saved-estimation", settings); source.fit();
    const saved = source.snapshot(); let ticked = false;
    const timer = setTimeout(() => { ticked = true; }, 0);
    try {
      const restore = (f.sessions as any).restoreIdentificationAsync ?? f.sessions.restoreIdentification;
      await restore.call(f.sessions, { identificationStudy: saved });
      assert.strictEqual(ticked, true, "Open must yield the host event loop while replaying the saved comparison");
      assert.deepStrictEqual(f.sessions.identification.snapshot(), saved);
      assert.deepStrictEqual(f.sessions.identification.currentState.review, source.currentState.review);
    } finally { clearTimeout(timer); f.sessions.dispose(); }
  });

  test("an edit during the running notification cannot turn the earlier Fit into an unchecked fit of newer inputs", async () => {
    const f = fixture();
    const originalPost = f.context.post;
    f.context.post = async (phase, payload: any) => {
      await originalPost(phase, payload);
      if (payload.status === "running") f.sessions.qualifyIdentification(samples(12), [], "Simulation", "sopdt", "new-input", settings);
    };
    try {
      await handleManualWorkflowCommand(f.context);
      assert.strictEqual(f.sessions.identification.currentState.phase, "qualified");
      assert.strictEqual(f.posted.filter(value => value.phase === "validation_required").length, 0);
      assert.strictEqual(f.retained(), 0);
    } finally { f.sessions.dispose(); }
  });

  test("the public Open command waits for asynchronous editor restoration", async () => {
    let opened = false;
    await openControlStudioCommand({} as any, { project: "explicit-request" }, async () => {
      await new Promise(resolve => setTimeout(resolve, 0)); opened = true;
    });
    assert.strictEqual(opened, true, "command completion must include the restored editor");
  });

  test("the newer saved study wins when two Opens overlap in the same editor", async () => {
    const f = fixture();
    const saved = (change: number, id: string) => {
      const workflow = new IdentificationWorkflow();
      workflow.qualify(samples(change), [], "Simulation", "sopdt", id, settings); workflow.fit();
      return { identificationStudy: workflow.snapshot() };
    };
    const first = saved(10, "older-open"), second = saved(12, "newer-open");
    try {
      const older = f.sessions.restoreIdentificationAsync(first);
      const newer = f.sessions.restoreIdentificationAsync(second);
      assert.deepStrictEqual(await Promise.all([older, newer]), [false, true]);
      assert.strictEqual(f.sessions.identification.snapshot()?.current.estimationRecordingId, "newer-open");
    } finally { f.sessions.dispose(); }
  });

  test("a malformed Open leaves a currently running Fit alive", async () => {
    const f = fixture();
    try {
      const fit = handleManualWorkflowCommand(f.context);
      await assert.rejects(f.sessions.restoreIdentificationAsync({ identificationStudy: { schemaVersion: 99 } as any }), /malformed|incompatible/);
      await fit;
      assert.strictEqual(f.sessions.identification.currentState.phase, "preview");
      assert.strictEqual(f.posted.filter(value => value.phase === "validation_required").length, 1);
    } finally { f.sessions.dispose(); }
  });

  test("preparing a saved study leaves the current owner intact until an explicit still-current adoption", async () => {
    const f = fixture(), source = new IdentificationWorkflow();
    source.qualify(samples(12), [], "Simulation", "sopdt", "prepared-study", settings); source.fit();
    const before = f.sessions.identification.snapshot();
    try {
      const prepare = (f.sessions as any).prepareIdentificationRestoreAsync ?? f.sessions.restoreIdentificationAsync;
      const adopt = await prepare.call(f.sessions, { identificationStudy: source.snapshot() });
      assert.strictEqual(f.sessions.identification.snapshot()?.current.estimationRecordingId, before?.current.estimationRecordingId,
        "a prepared replay must not partially adopt the new study");
      f.sessions.qualifyIdentification(samples(15), [], "Simulation", "sopdt", "newer-edit", settings);
      assert.strictEqual(adopt(), false, "an edited owner must reject the prepared older study");
      assert.strictEqual(f.sessions.identification.snapshot()?.current.estimationRecordingId, "newer-edit");
    } finally { f.sessions.dispose(); }
  });

  test("accepts a scoped Cancel command and stops fitting without changing the retained inputs", async () => {
    const scope = { projectId: "p", loopId: "l", editorSessionId: "e", loopRevision: "r" };
    const command = decodeEngineeringCommand({ schemaVersion: 1, kind: "control-studio.identify.cancel", messageId: "cancel",
      projectId: "p", loopId: "l", editorSessionId: "e", expectedLoopRevision: "r", payload: {} }, scope);
    assert.strictEqual(command?.action, "identify-cancel");
    const f = fixture(), before = f.sessions.identification.snapshot();
    const originalPost = f.context.post;
    f.context.post = async (phase, payload: any) => {
      await originalPost(phase, payload);
      if (payload.status === "running") await handleManualWorkflowCommand({ ...f.context, command: command! });
    };
    try {
      await handleManualWorkflowCommand(f.context);
      assert.deepStrictEqual(f.sessions.identification.snapshot(), before);
      assert.strictEqual(f.posted.filter(value => value.phase === "validation_required").length, 0);
      assert.strictEqual(f.posted[f.posted.length - 1].cancelled, true, "the completed Cancel action must identify its outcome in the existing status projection");
      assert.strictEqual(f.retained(), 0);
    } finally { f.sessions.dispose(); }
  });

  test("shows Cancel during calculation and returns keyboard focus to Fit when cancellation finishes", () => {
    const { JSDOM } = require("jsdom");
    const dom = new JSDOM('<button data-identify-import>Import</button><button data-identify-run>Check</button><button data-identify-fit>Fit</button><button data-identify-record-validation>Validate</button><button data-identify-accept>Accept</button><button data-identify-cancel hidden>Cancel</button>');
    const previous = (globalThis as any).document;
    (globalThis as any).document = dom.window.document;
    try {
      (setIdentificationActionState as Function)("running");
      const cancel = dom.window.document.querySelector('[data-identify-cancel]');
      assert.strictEqual(cancel.hidden, false);
      assert.strictEqual(dom.window.document.querySelector('[data-identify-fit]').hidden, true);
      assert.strictEqual(dom.window.document.querySelector('[data-identify-run]').disabled, true);
      cancel.focus(); setIdentificationActionState("qualified");
      assert.strictEqual(cancel.hidden, true);
      assert.strictEqual(dom.window.document.activeElement, dom.window.document.querySelector('[data-identify-fit]'));
    } finally { (globalThis as any).document = previous; dom.window.close(); }
  });
});
