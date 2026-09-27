import * as assert from "assert";
import { GeneralModelSession, isSavedGeneralModelAnalysis } from "../../controlStudio/generalModelSession";
import { analyzeGeneralFeedback } from "../../controlStudio/generalFeedback";
import { LeadLagSession } from "../../controlStudio/leadLagSession";
import { retainedPeriodicRecordingSource } from "../../controlStudio/periodicRecordingSource";
const scope = { projectId: "p", editorSessionId: "e", loopId: "l", loopRevision: "r" };
const model = { kind: "transfer-function", numerator: [2], denominator: [1, 2], delaySeconds: 0, revision: "m", source: "provided", validation: "unvalidated", evidence: [], inputs: [{ id: "drive", unit: "V", operatingRange: { minimum: 0, maximum: 10 } }], outputs: [{ id: "speed", unit: "rpm", operatingRange: { minimum: 0, maximum: 100 } }] };
const message = (action: string, payload: Record<string, unknown>) => ({ schemaVersion: 1, kind: `control-studio.general-model.${action}`, messageId: "m", projectId: "p", editorSessionId: "e", loopId: "l", expectedLoopRevision: "r", payload });
suite("Control Studio supplied model jobs", () => {
  test("binds periodic analysis to complete evidence identity and rejects gaps or timing drift", () => {
    const raw = Array.from({ length: 32 }, (_, index) => ({ quality: "good", appliedMv: 1, pv: 2, actualCycleSeconds: 0.1,
      scan: index + 1, loopRevision: "loop", outputLimits: { minimum: 0, maximum: 10 } }));
    const recording = { rawSamples: raw, evidence: { recordingId: "recording", completeness: "complete", missingScanRanges: [], droppedObservedSamples: 0, actualCycleTimingAvailable: true } } as any;
    const retained = retainedPeriodicRecordingSource(recording, "recording", "loop");
    assert.strictEqual(retained?.sampleTimeSeconds, 0.1);
    assert.strictEqual(retained?.input.length, 32);
    assert.strictEqual(retainedPeriodicRecordingSource({ ...recording, rawSamples: raw.map((sample, index) => index === 12 ? { ...sample, scan: sample.scan + 1 } : sample) }, "recording", "loop"), undefined);
    assert.strictEqual(retainedPeriodicRecordingSource({ ...recording, rawSamples: raw.map((sample, index) => index === 12 ? { ...sample, actualCycleSeconds: 0.2 } : sample) }, "recording", "loop"), undefined);
  });
  test("routes lead/lag design through the host owner and retains the selected model", async () => {
    let selected = model as any; const session = new LeadLagSession(() => selected); const posts: Record<string, any>[] = [];
    const post = async (value: Record<string, unknown>) => { posts.push(value); };
    const request = { schemaVersion: 1, kind: "control-studio.lead-lag.run", messageId: "lead-lag", projectId: "p", editorSessionId: "e", loopId: "l", expectedLoopRevision: "r", payload: { mode: "lead", crossoverRadPerSecond: 1, value: 30 } };
    try {
      assert.strictEqual(await session.handle(request, scope, post), true);
      const completed = posts[posts.length - 1];
      assert.strictEqual(completed?.status, "complete");
      assert.strictEqual(completed?.saved?.model.revision, model.revision);
      assert.ok(Number.isFinite(completed?.saved?.result.gain));
      const saved = session.snapshot(); assert.ok(saved);
      session.restore(saved);
      assert.strictEqual(session.snapshot()?.mode, "lead");
      selected = { ...model, revision: "replacement" };
      assert.strictEqual(session.snapshot(), undefined, "A saved design must not survive a selected-model replacement");
    } finally { session.dispose(); }
  });
  test("separates initial-state and delayed zero-state responses through the selected-model session", async () => {
    const selected = { ...model, kind: "continuous-state-space", delaySeconds: .5,
      states: [{ id: "speed-state", unit: "rpm", operatingRange: { minimum: 0, maximum: 100 } }], a: [[-1]], b: [[1]], c: [[1]], d: [[0]] };
    const session = new GeneralModelSession(); const posts: Record<string, any>[] = [];
    const post = async (value: Record<string, unknown>) => { posts.push(value); };
    try {
      await session.handle(message("import", { json: JSON.stringify(selected) }), scope, post);
      const handled = await session.handle(message("response", { stepSeconds: .25, durationSeconds: 1, initialState: [2], input: [1], connection: "open-loop" }), scope, post);
      assert.strictEqual(handled, true, "The selected-model response operation must be reachable");
      const result = posts[posts.length - 1]?.saved?.response;
      assert.strictEqual(result?.model.revision, selected.revision);
      for (const point of result.points) {
        const forced = point.timeSeconds <= .5 ? 0 : 1 - Math.exp(-(point.timeSeconds - .5));
        assert.ok(Math.abs(point.zeroState[0] - forced) < 1e-10);
        assert.ok(Math.abs(point.initialCondition[0] - 2 * Math.exp(-point.timeSeconds)) < 1e-10, "Input delay must not shift the homogeneous response");
        assert.ok(Math.abs(point.total[0] - point.zeroState[0] - point.initialCondition[0]) < 1e-10);
      }
      assert.ok((session.snapshot() as any)?.response, "Retain the response and its input/initial-state settings");
      const controller = { ...model, numerator: [1], denominator: [1], revision: "unit-feedback", inputs: selected.outputs, outputs: selected.inputs };
      await session.handle(message("import-controller", { json: JSON.stringify(controller) }), scope, post);
      await session.handle(message("response", { stepSeconds: .25, durationSeconds: 1, initialState: [2], input: [1], connection: "feedback" }), scope, post);
      const feedback = posts[posts.length - 1]?.saved?.response;
      assert.ok(feedback, JSON.stringify(posts[posts.length - 1]));
      assert.ok(Math.abs(feedback.points[1].initialCondition[0] - 2 * Math.exp(-.25)) < 1e-10, "A delayed feedback loop retains its unshifted initial response");
      assert.ok(Math.abs(feedback.points[4].total[0] - result.points[4].total[0]) > .01, "Recompute feedback with the delayed plant instead of shifting a delay-free response");
    } finally { session.dispose(); }
  });
  test("keeps the full unstable model when the engineer selects a minimal pole-zero view", async () => {
    const selected = { ...model, numerator: [1, -1], denominator: [1, 0, -1] };
    const session = new GeneralModelSession(); const posts: Record<string, any>[] = [];
    const post = async (value: Record<string, unknown>) => { posts.push(value); };
    try {
      await session.handle(message("import", { json: JSON.stringify(selected) }), scope, post);
      assert.strictEqual(await session.handle(message("pole-view", { view: "minimal" }), scope, post), true);
      const result = posts[posts.length - 1]?.saved?.poleView;
      assert.deepStrictEqual(result.unreduced.denominator, [1, 0, -1]);
      assert.deepStrictEqual(result.minimal.denominator, [1, 1]);
      assert.deepStrictEqual(result.cancelledFactor, [1, -1]);
      assert.strictEqual(result.hiddenUnstableMode, true);
      assert.deepStrictEqual(session.selectedModel(), selected, "Changing a view must not replace the selected plant or erase its internal mode");
      assert.ok((session.snapshot() as any)?.poleView);
    } finally { session.dispose(); }
  });
  test("selecting the same model preserves its analysis while changed content invalidates it", async () => {
    const session = new GeneralModelSession();
    await session.handle(message("import", { json: JSON.stringify(model) }), scope, async () => undefined);
    await session.handle(message("analyze", { minimumFrequency: 1, maximumFrequency: 10, points: 3 }), scope, async () => undefined);
    const saved = session.snapshot(); assert.ok(saved);
    session.selectModel(Object.fromEntries(Object.entries(model).reverse()) as any);
    assert.deepStrictEqual(session.snapshot(), saved, "reconciling the same selected model cannot discard its existing frequency study");
    session.selectModel({ ...model, numerator: [3] } as any);
    assert.strictEqual(session.snapshot(), undefined, "changed content invalidates analysis even when revision text is unchanged");
    session.dispose();
  });
  test("converts independently accepted identification into the shared selected model without approximating delay", () => {
    const { IdentificationWorkflow } = require("../../controlStudio/identification");
    const bridge = require("../../controlStudio/selectedModelAnalysis");
    const workflow = new IdentificationWorkflow();
    const samples = (amplitude: number) => Array.from({ length: 301 }, (_, i) => ({
      timeSeconds: i * 0.1, input: i >= 20 ? amplitude : 0,
      output: i >= 25 ? 2 * amplitude * (1 - Math.exp(-(i - 25) * 0.1 / 3)) : 0,
      quality: "good", inputUnit: "V", outputUnit: "m" }));
    workflow.qualify(samples(1), [], "Simulation", "fopdt");
    workflow.fit();
    workflow.qualify(samples(1.5), [], "Simulation", "fopdt");
    workflow.fit();
    const accepted = workflow.accept();
    assert.strictEqual(typeof bridge.acceptedIdentificationModel, "function",
      "accepted identification must enter the same selected-model contract as supplied models");
    const selected = bridge.acceptedIdentificationModel(accepted, {
      inputs: [{ id: "command", unit: "V", operatingRange: { minimum: -10, maximum: 10 } }],
      outputs: [{ id: "position", unit: "m", operatingRange: { minimum: -10, maximum: 10 } }],
    });
    assert.strictEqual(selected.source, "fitted");
    assert.strictEqual(selected.validation, "validated");
    assert.strictEqual(selected.kind, "transfer-function");
    assert.deepStrictEqual(selected.numerator, [accepted.model.gain]);
    assert.deepStrictEqual(selected.denominator, [accepted.model.timeConstantSeconds, 1]);
    assert.strictEqual(selected.delaySeconds, accepted.model.deadTimeSeconds);
    assert.strictEqual(selected.evidence.length, 2);
    assert.notStrictEqual(selected.evidence[0], selected.evidence[1], "independent record identities remain distinct");
    assert.ok(selected.revision.startsWith("sha256:"));
  });

  test("rejects a blocked external model replacement before discarding current evidence and permits retry", async () => {
    let busy = false;
    const session = new (GeneralModelSession as any)(() => { if (busy) throw new Error("Finish or cancel the running operation, then retry"); });
    const posts: Record<string, unknown>[] = [];
    const post = async (p: Record<string, unknown>) => { posts.push(p); };
    await session.handle(message("import", { json: JSON.stringify(model) }), scope, post);
    await session.handle(message("analyze", { minimumFrequency: 1, maximumFrequency: 10, points: 3 }), scope, post);
    await session.handle(message("analyze", { minimumFrequency: 1, maximumFrequency: 10, points: 3 }), scope, post);
    const saved = session.snapshot(); assert.ok(saved);
    const replacement = { ...model, revision: "replacement", numerator: [3] };
    busy = true;
    await session.handle(message("import", { json: JSON.stringify(replacement) }), scope, post);
    assert.deepStrictEqual(session.selectedModel(), model, "Blocked import must not replace the model");
    assert.deepStrictEqual(session.snapshot(), saved, "Blocked import must preserve existing analysis");
    assert.strictEqual(posts[posts.length - 1].status, "failed");
    busy = false;
    await session.handle(message("import", { json: JSON.stringify(replacement) }), scope, post);
    assert.deepStrictEqual(session.selectedModel(), replacement);
    session.dispose();
  });

  test("imports and calculates finite model evidence in a bounded worker", async () => {
    const session = new GeneralModelSession();
    const posts: Record<string, unknown>[] = [];
    const post = async (p: Record<string, unknown>) => { posts.push(p); };
    assert.strictEqual(await session.handle(message("import", { json: JSON.stringify(model) }), scope, post), true);
    await session.handle(message("analyze", { minimumFrequency: 1, maximumFrequency: 10, points: 3 }), scope, post);
    assert.strictEqual(posts[posts.length - 1]?.status, "complete");
    assert.ok(isSavedGeneralModelAnalysis(session.snapshot()));
    const forged = session.snapshot()!;
    (forged.result as any).relativeGainMatrix = "broken";
    assert.strictEqual(isSavedGeneralModelAnalysis(forged), false);
    session.dispose();
  });
  test("rejects malformed restore without cancelling pending analysis or replacing evidence", async () => {
    const session = new GeneralModelSession();
    const posts: Record<string, unknown>[] = [];
    let started!: () => void;
    const running = new Promise<void>(resolve => { started = resolve; });
    const post = async (p: Record<string, unknown>) => { posts.push(p); if (p.status === "running") started(); };
    await session.handle(message("import", { json: JSON.stringify(model) }), scope, post);
    const pending = session.handle(message("analyze", { minimumFrequency: 1, maximumFrequency: 10, points: 512 }), scope, post);
    await running;
    assert.throws(() => session.restore({ broken: true } as any), /invalid/i);
    await pending;
    assert.strictEqual(posts[posts.length - 1]?.status, "complete", "invalid restore must not cancel the active analysis");
    assert.ok(session.snapshot(), "completed analysis remains owned after rejected restore");
    session.dispose();
  });
  test("cancels pending worker work without publishing late evidence", async () => {
    const session = new GeneralModelSession();
    const posts: Record<string, unknown>[] = [];
    let started!: () => void;
    const running = new Promise<void>(resolve => { started = resolve; });
    const post = async (p: Record<string, unknown>) => { posts.push(p); if (p.status === "running") started(); };
    await session.handle(message("import", { json: JSON.stringify(model) }), scope, post);
    const pending = session.handle(message("analyze", { minimumFrequency: 1, maximumFrequency: 10, points: 512 }), scope, post);
    await running;
    await session.handle(message("cancel", {}), scope, post);
    await pending;
    assert.strictEqual(posts[posts.length - 1].status, "cancelled");
    assert.strictEqual(posts.some(p => p.status === "complete"), false);
    assert.strictEqual(session.snapshot(), undefined);
    session.dispose();
  });
  test("attaches an explicit controller and retains separate feedback evidence", async () => {
    const session=new GeneralModelSession();const post=async(_p:Record<string,unknown>)=>{};
    await session.handle(message("import",{json:JSON.stringify(model)}),scope,post);
    const controller={...model,revision:"custom-controller",inputs:model.outputs,outputs:model.inputs,numerator:[2],denominator:[1]};
    assert.strictEqual(await session.handle(message("import-controller",{json:JSON.stringify(controller)}),scope,post),true);
    await session.handle(message("analyze",{minimumFrequency:1,maximumFrequency:10,points:3,plantInputId:"drive",plantOutputId:"speed",controllerInputId:"speed",controllerOutputId:"drive"}),scope,post);
    const saved=session.snapshot() as any;
    assert.ok(saved?.feedback);assert.strictEqual(saved.feedback.result.closedLoop.stable,true);
    assert.ok(Math.abs(saved.feedback.result.closedLoop.poles[0].real+6)<1e-9);
    session.dispose();
  });
  test("routes a declared controller law through the bounded feedback worker", async () => {
    let declaredFeedback: { declaredFeedbackDefinition?: (analysis: unknown, model: unknown) => unknown } | undefined;
    try { declaredFeedback = require("../../controlStudio/declaredFeedbackModel") as typeof declaredFeedback; } catch { declaredFeedback = undefined; }
    assert.strictEqual(typeof declaredFeedback?.declaredFeedbackDefinition, "function",
      "declared controller equations need an explicit GeneralFeedback adapter");
    const analysis = {
      plant: { kind: "transfer-function", numerator: [1], denominator: [1, 2], delaySeconds: 0,
        inputUnit: "V", outputUnit: "rpm", revision: "declared-plant", source: "provided", validation: "validated" },
      controller: { numerator: [{ parameter: "Kp" }], denominator: [1],
        parameters: { Kp: { value: 2, minimum: 0, maximum: 10, unit: "V/rpm" } } },
      rootLocus: { parameter: "Kp", values: [0, 2, 4], selectedValue: 2 },
    };
    const feedback = declaredFeedback!.declaredFeedbackDefinition!(analysis, model);
    assert.ok(feedback, "the selected plant and declared law must produce an explicit feedback definition");
    const session = new (GeneralModelSession as any)(undefined, () => feedback);
    const posts: Record<string, unknown>[] = [];
    const post = async (payload: Record<string, unknown>) => { posts.push(payload); };
    await session.handle(message("import", { json: JSON.stringify(model) }), scope, post);
    await session.handle(message("analyze", { minimumFrequency: 1, maximumFrequency: 10, points: 3 }), scope, post);
    const saved = session.snapshot() as any;
    assert.strictEqual(posts[posts.length - 1]?.status, "complete");
    assert.ok(saved?.feedback, "declared feedback must be retained beside plant channels");
    assert.deepStrictEqual(saved.feedback.definition.controller.numerator, [1]);
    assert.deepStrictEqual(saved.feedback.definition.declaredController?.numerator, [2]);
    assert.strictEqual(saved.feedback.result.closedLoop.status, "qualified");
    assert.strictEqual(saved.feedback.result.closedLoop.stable, true);
    assert.ok(saved.feedback.result.openLoop.every((point: any) => [point.real, point.imaginary, point.magnitude].every(Number.isFinite)));
    assert.ok(saved.feedback.result.sensitivity?.every((point: any) => [point.sensitivityMagnitudeDb, point.complementarySensitivityMagnitudeDb, point.controlSensitivityMagnitudeDb].every(Number.isFinite)));
    assert.ok(saved.feedback.result.nyquistStability, "delay-free declared transfer feedback must use the shared Nyquist stability owner");
    assert.match(saved.feedback.result.nyquistScope, /contour|sampled|curve/i);
    session.dispose();
  });
  test("retains a validated periodic frequency measurement with startup samples excluded", async () => {
    const session = new GeneralModelSession(undefined, undefined, (recordingId, recordingRevision) => ({ input: [...Array(16).fill(0), ...Array.from({ length: 64 }, (_, index) => Math.sin(index * 0.1 * 5))], output: [...Array(16).fill(0), ...Array.from({ length: 64 }, (_, index) => 2 * Math.sin(index * 0.1 * 5))], sampleTimeSeconds: 0.1, recordingId, recordingRevision }));
    const posts: Record<string, unknown>[] = [];
    const post = async (payload: Record<string, unknown>) => { posts.push(payload); };
    await session.handle(message("import", { json: JSON.stringify(model) }), scope, post);
    await session.handle(message("analyze", { minimumFrequency: 1, maximumFrequency: 10, points: 3 }), scope, post);
    const steadyInput = Array.from({ length: 64 }, (_, index) => Math.sin(index * 0.1 * 5));
    const input = [...Array(16).fill(0), ...steadyInput];
    const output = [...Array(16).fill(0), ...steadyInput.map(value => 2 * value)];
    assert.strictEqual(await session.handle(message("periodic", {
      input, output, sampleTimeSeconds: 0.1, frequencies: [5], segmentCount: 2,
      minimumCoherence: 0, startupSamples: 16,
      qualityChecked: true,
      recordingId: "recording-1", recordingRevision: "recording-revision-1",
    }), scope, post), true);
    assert.strictEqual(posts[posts.length - 1]?.status, "periodic-complete");
    const saved = session.snapshot() as any;
    assert.ok(saved?.periodic, "periodic evidence must be retained with the selected model analysis");
    assert.strictEqual(saved.periodic.startupSamples, 16);
    assert.strictEqual(saved.periodic.result.status, "qualified");
    assert.ok(Math.abs(saved.periodic.result.points[0].magnitude - 2) < 0.05);
    session.dispose();
  });
  test("retains and analyzes an improper declared derivative plus proportional law", async () => {
    const { declaredFeedbackDefinition } = require("../../controlStudio/declaredFeedbackModel") as {
      declaredFeedbackDefinition: (analysis: unknown, model: unknown) => any;
    };
    const pd = {
      plant: { kind: "transfer-function", numerator: [1], denominator: [1, 1, 0], delaySeconds: 0,
        inputUnit: "V", outputUnit: "rpm", revision: "declared-pd", source: "provided", validation: "validated" },
      controller: { numerator: [{ parameter: "Kd" }, { parameter: "Kp" }], denominator: [1],
        parameters: { Kp: { value: 2, minimum: 0, maximum: 10, unit: "V/rpm" }, Kd: { value: 3, minimum: 3, maximum: 3, unit: "V·s/rpm" } } },
      rootLocus: { parameter: "Kp", values: [0, 2, 4], selectedValue: 2 },
    };
    const definition = declaredFeedbackDefinition(pd, model);
    assert.ok(definition, "the selected plant and PD law must remain analyzable without a tuned candidate");
    assert.deepStrictEqual(definition.plant.numerator, [2]);
    assert.deepStrictEqual(definition.plant.denominator, [1, 2]);
    assert.deepStrictEqual(definition.controller.numerator, [1]);
    assert.deepStrictEqual(definition.declaredController.numerator, [3, 2]);
    const session = new (GeneralModelSession as any)(undefined, () => definition);
    const posts: Record<string, unknown>[] = [];
    const post = async (payload: Record<string, unknown>) => { posts.push(payload); };
    await session.handle(message("import", { json: JSON.stringify(model) }), scope, post);
    await session.handle(message("analyze", { minimumFrequency: 1, maximumFrequency: 10, points: 3 }), scope, post);
    const saved = session.snapshot();
    assert.strictEqual(posts[posts.length - 1]?.status, "complete");
    assert.ok(isSavedGeneralModelAnalysis(saved), "the saved analysis must retain the selected plant and declared controller law");
    if (!saved?.feedback) throw new Error("declared feedback evidence was not saved");
    assert.deepStrictEqual(saved.feedback.definition.plant, model);
    if (!saved.feedback.definition.declaredController || !saved.feedback.result.sensitivity) throw new Error("declared controller or sensitivity evidence was not saved");
    assert.deepStrictEqual(saved.feedback.definition.declaredController.numerator, [3, 2]);
    const sensitivity = saved.feedback.result.sensitivity.find((point: any) => point.frequencyRadPerSecond === 1);
    const expectedControlSensitivityDb = 20 * Math.log10(Math.hypot(2, 3) / Math.hypot(3.8, 1.6));
    if (!sensitivity) throw new Error("frequency point at 1 rad/s was not saved");
    assert.ok(Math.abs(sensitivity.controlSensitivityMagnitudeDb - expectedControlSensitivityDb) < 1e-9);
    session.dispose();

    const unstable = {
      ...pd,
      plant: { ...pd.plant, denominator: [1, 1], revision: "declared-unstable" },
      controller: { ...pd.controller, numerator: [-3], parameters: { Kp: { value: -3, minimum: -3, maximum: 0, unit: "V/rpm" }, Kd: { value: 0, minimum: 0, maximum: 0, unit: "V·s/rpm" } } },
      rootLocus: { parameter: "Kp", values: [-3], selectedValue: -3 },
    };
    const unstableDefinition = declaredFeedbackDefinition(unstable, { ...model, numerator: [1], denominator: [1, 1], revision: "declared-unstable" });
    const unstableResult = analyzeGeneralFeedback(unstableDefinition!, [1, 10]);
    assert.strictEqual(unstableResult.status, "qualified");
    assert.ok(unstableResult.closedLoop.status === "qualified" && unstableResult.closedLoop.poles.some((pole: any) => pole.real > 1),
      "declared closed-loop roots must retain a positive pole rather than collapsing to a stable identity-controller result");
  });
  test("rejects oversized grids and never accepts another editor scope", async () => {
    const session = new GeneralModelSession();
    const posts: Record<string, unknown>[] = [];
    const post = async (p: Record<string, unknown>) => { posts.push(p); };
    assert.strictEqual(await session.handle(message("import", { json: JSON.stringify(model) }), { ...scope, editorSessionId: "other" }, post), false);
    await session.handle(message("import", { json: JSON.stringify(model) }), scope, post);
    await session.handle(message("analyze", { minimumFrequency: 1, maximumFrequency: 10, points: 100000 }), scope, post);
    assert.strictEqual(posts[posts.length - 1]?.status, "failed");
    assert.strictEqual(session.snapshot(), undefined);
    session.dispose();
  });
});
