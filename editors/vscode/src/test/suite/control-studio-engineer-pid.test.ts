import { pidLoop, proportionalLoop, channel, pidInitialization } from "./control-studio-pid-fixtures";
import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import { EngineerWorkflowSession } from "../../controlStudio/engineerWorkflowSession";
import { runtimeLifecycleService } from "../../runtimeLifecycle";
import { attachControlStudio } from "./control-studio-rendered-harness";
import { captureRenderedWorkbenchCdp, captureRenderedWorkbench, connectCdp, devToolsPort, evaluateInWebview, httpJson, pollValue, requiredEnvironment } from "./plant-rendered-harness";

const AUTO_FAMILY = ["automatic", "P", "PID"].includes(process.env.TRUST_CONTROL_STUDIO_ENGINEER_FAMILY ?? "")
  ? process.env.TRUST_CONTROL_STUDIO_ENGINEER_FAMILY as "automatic" | "P" | "PID" : "PI";
const CANCEL_AUTO = process.env.TRUST_CONTROL_STUDIO_CANCEL_AUTO === "1";

suite("Control Studio connected PID engineer path", () => {
  test("invalidates a cleared model without reporting a failed tuning operation", () => {
    const session = new EngineerWorkflowSession(pidLoop(), () => undefined, () => undefined);
    try {
      const result = session.modelChanged();
      assert.strictEqual(result.status, "edited", "Preparing identification is valid before a model exists");
      assert.strictEqual(result.failure, undefined);
      assert.strictEqual((result.stageEvidence as any).hasModel, false);
    } finally { session.dispose(); }
  });

  test("exposes all typed PID gains through the actual ST runtime", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_ACTUAL_PID !== "1") this.skip();
    this.timeout(60_000);
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    const loop = JSON.parse(fs.readFileSync(path.join(vscode.workspace.workspaceFolders![0].uri.fsPath, "control-loop.json"), "utf8"));
    const start = await runtimeLifecycleService.startLocalSimulator();
    assert.strictEqual(start.ok, true, JSON.stringify(start));
    try {
      const connection = await require("../../controlStudio/runtimeParameterConnection").connectRuntimeParameters(loop);
      const status = await connection.client.status();
      assert.deepStrictEqual(status.values, { "Main.Kp": 1, "Main.Ki": 0, "Main.Kd": 0 });
    } finally { await runtimeLifecycleService.stopRuntime(); }
  });
  test("creates an unapplied PI candidate from a typed PID law", async () => {
    const loop = pidLoop();
    const model: any = { kind: "transfer-function", numerator: [1], denominator: [1, 1], delaySeconds: 0,
      revision: "first-order", source: "provided", validation: "validated", evidence: ["bench"],
      normalizedError: 0.01, inputs: [channel("u", "V")], outputs: [channel("y", "m")] };
    const session = new EngineerWorkflowSession(loop, () => model, () => undefined);
    const scope = { projectId: loop.projectId, editorSessionId: "pid", loopId: loop.id, loopRevision: loop.revision };
    const posts: any[] = [];
    try {
      await session.handle({ schemaVersion: 1, kind: "control-studio.engineer.tune", messageId: "pi",
        projectId: scope.projectId, editorSessionId: scope.editorSessionId, loopId: scope.loopId,
        expectedLoopRevision: scope.loopRevision, payload: { family: "PI", referenceChange: 1, maximumError: 0.1,
          settlingBand: 0.02, settlingTimeSeconds: 2, durationSeconds: 4, applicationMode: "manual" } }, scope,
      async value => { posts.push(value); });
      const result = posts[posts.length - 1];
      assert.strictEqual(result?.status, "candidate", String(result?.reason));
      assert.strictEqual(result?.design?.candidate?.structure, "PI");
      // Settling within ±0.02 m by 2 s needs SIMC's faster response, λ = τ/3.
      assert.deepStrictEqual(result?.values, { "Main.Kp": 3, "Main.Ki": 3, "Main.Kd": 0 });
      assert.match(String(result?.reason), /PI candidate ready|manual staging/i);
    } finally { session.dispose(); }
  });

  test("creates declared PD and PID alternatives from the same typed first-order adapter", async () => {
    const loop = pidLoop();
    const model: any = { kind: "transfer-function", numerator: [1], denominator: [1, 1], delaySeconds: 0,
      revision: "first-order", source: "provided", validation: "validated", evidence: ["bench"],
      normalizedError: 0.01, inputs: [channel("u", "V")], outputs: [channel("y", "m")] };
    const scope = { projectId: loop.projectId, editorSessionId: "pid-alternatives", loopId: loop.id, loopRevision: loop.revision };
    const session = new EngineerWorkflowSession(loop, () => model, () => undefined);
    const posts: any[] = [];
    const tune = (family: "PD" | "PID") => session.handle({ schemaVersion: 1, kind: "control-studio.engineer.tune", messageId: family,
      projectId: scope.projectId, editorSessionId: scope.editorSessionId, loopId: scope.loopId, expectedLoopRevision: scope.loopRevision,
      payload: { family, referenceChange: 1, maximumError: 0.5, settlingBand: 0.1, settlingTimeSeconds: 2,
        durationSeconds: 4, applicationMode: "manual" } }, scope, async value => { posts.push(value); });
    try {
      await tune("PD");
      assert.strictEqual(posts[posts.length - 1]?.status, "candidate");
      assert.strictEqual(posts[posts.length - 1]?.design?.candidate?.structure, "PD");
      assert.deepStrictEqual(posts[posts.length - 1]?.values, { "Main.Kp": 1.5, "Main.Ki": 0, "Main.Kd": 0.075 });
      assert.match(String(posts[posts.length - 1]?.reason), /PD candidate ready|constrained PD/i);
      await tune("PID");
      assert.strictEqual(posts[posts.length - 1]?.status, "candidate");
      assert.strictEqual(posts[posts.length - 1]?.design?.candidate?.structure, "PID");
      assert.deepStrictEqual(posts[posts.length - 1]?.values, { "Main.Kp": 1.5, "Main.Ki": 1.5, "Main.Kd": 0.075 });
      assert.match(String(posts[posts.length - 1]?.reason), /PID candidate ready|constrained PID/i);
    } finally { session.dispose(); }
  });

  test("keeps an automatically recommended proportional controller eligible for Auto commissioning without a PID state adapter", async () => {
    const loop = proportionalLoop();
    const model: any = { kind: "transfer-function", numerator: [1], denominator: [1, 1], delaySeconds: 0,
      revision: "first-order-p", source: "provided", validation: "validated", evidence: ["bench"],
      normalizedError: 0.01, inputs: [channel("u", "V")], outputs: [channel("y", "m")] };
    const session = new EngineerWorkflowSession(loop, () => model, () => undefined);
    const scope = { projectId: loop.projectId, editorSessionId: "p-automatic", loopId: loop.id, loopRevision: loop.revision };
    const posts: any[] = [];
    try {
      await session.handle({ schemaVersion: 1, kind: "control-studio.engineer.tune", messageId: "automatic-p",
        projectId: scope.projectId, editorSessionId: scope.editorSessionId, loopId: scope.loopId, expectedLoopRevision: scope.loopRevision,
        payload: { family: "automatic", referenceChange: 1, maximumError: 0.1, settlingBand: 0.02,
          settlingTimeSeconds: 2, durationSeconds: 4, applicationMode: "auto" } }, scope, async value => { posts.push(value); });
      const result = posts[posts.length - 1];
      assert.strictEqual(result?.status, "candidate", String(result?.reason));
      assert.strictEqual(result?.design?.candidate?.structure, "P");
      assert.match(String(result?.reason), /P is the simplest (?:model-screened|passing) candidate/i);
    } finally { session.dispose(); }
  });

  test("switches a typed PID controller from PI to explicit P with zero integral and derivative gains", async () => {
    const loop = pidLoop();
    const model: any = { kind: "transfer-function", numerator: [1], denominator: [1, 1], delaySeconds: 0,
      revision: "explicit-p", source: "provided", validation: "validated", evidence: ["bench"],
      inputs: [channel("u", "V")], outputs: [channel("y", "m")] };
    const session = new EngineerWorkflowSession(loop, () => model, () => undefined);
    const scope = { projectId: loop.projectId, editorSessionId: "explicit-p", loopId: loop.id, loopRevision: loop.revision };
    const posts: any[] = [];
    const tune = async (family: string) => session.handle({ schemaVersion: 1,
      kind: "control-studio.engineer.tune", messageId: family, projectId: scope.projectId,
      editorSessionId: scope.editorSessionId, loopId: scope.loopId, expectedLoopRevision: scope.loopRevision,
      payload: { family, applicationMode: "manual", referenceChange: 1, maximumError: 0.1,
        settlingBand: 0.02, settlingTimeSeconds: 2, durationSeconds: 4 } }, scope,
      async value => { posts.push(value); });
    try {
      await tune("PI");
      assert.strictEqual(posts[posts.length - 1]?.status, "candidate");
      await tune("P");
      const result = posts[posts.length - 1];
      assert.strictEqual(result?.status, "candidate", String(result?.reason));
      assert.strictEqual(result.design.candidate.structure, "P");
      assert.strictEqual(result.values["Main.Ki"], 0);
      assert.strictEqual(result.values["Main.Kd"], 0);
      assert.strictEqual(session.snapshotStudy()?.family, "P");
    } finally { session.dispose(); }
  });

  test("reopens a pinned P gain on a PID loop without restoring Apply authority", async () => {
    const loop = pidLoop();
    const model: any = { kind: "transfer-function", numerator: [1], denominator: [1, 1], delaySeconds: 0,
      revision: "pinned-p", source: "provided", validation: "validated", evidence: ["bench"],
      inputs: [channel("u", "V")], outputs: [channel("y", "m")] };
    const session = new EngineerWorkflowSession(loop, () => model, () => undefined);
    const reopened = new EngineerWorkflowSession(loop, () => model, () => undefined);
    const scope = { projectId: loop.projectId, editorSessionId: "pin-p", loopId: loop.id, loopRevision: loop.revision };
    const posts: any[] = [];
    const send = (owner: EngineerWorkflowSession, action: string, payload: Record<string, unknown>) =>
      owner.handle({ schemaVersion: 1, kind: `control-studio.engineer.${action}`, messageId: action,
        projectId: scope.projectId, editorSessionId: scope.editorSessionId, loopId: scope.loopId,
        expectedLoopRevision: scope.loopRevision, payload }, scope, async value => { posts.push(value); });
    try {
      await send(session, "tune", { family: "P", applicationMode: "manual", gainMode: "manual", proportionalGain: 12,
        referenceChange: 1, maximumError: 0.1, settlingBand: 0.02, settlingTimeSeconds: 2, durationSeconds: 4 });
      assert.strictEqual(posts[posts.length - 1]?.status, "candidate");
      const saved = session.snapshotStudy();
      assert.doesNotThrow(() => reopened.restoreStudy(saved), "a valid PID proportional pin must reopen");
      assert.deepStrictEqual(reopened.snapshotStudy()?.gainChoice, saved?.gainChoice);
      assert.strictEqual(reopened.snapshotStudy()?.family, "P");
      await send(reopened, "apply", {});
      assert.strictEqual(posts[posts.length - 1]?.status, "failed");
      assert.match(String(posts[posts.length - 1]?.reason), /verify|mandatory scenario/i);
      const incompatible = structuredClone(saved!);
      incompatible.gainChoice!.pin!.unit = "wrong-unit";
      assert.throws(() => reopened.restoreStudy(incompatible), /does not match/i);
      assert.deepStrictEqual(reopened.snapshotStudy()?.gainChoice, saved?.gainChoice);
    } finally { session.dispose(); reopened.dispose(); }
  });

  test("routes every main-workflow failure to its owning corrective operation", async () => {
    const model: any = { kind: "transfer-function", numerator: [1], denominator: [1, 1], delaySeconds: 0,
      revision: "first-order", source: "provided", validation: "validated", evidence: ["bench"],
      inputs: [channel("u", "V")], outputs: [channel("y", "m")] };
    const payload = { family: "P", referenceChange: 1, maximumError: 0.1, settlingBand: 0.02,
      settlingTimeSeconds: 2, durationSeconds: 4, applicationMode: "manual" };
    const run = async (loop: any, selectedModel: any, values: Record<string, unknown>) => {
      const session = new EngineerWorkflowSession(loop, () => selectedModel, () => undefined);
      const scope = { projectId: loop.projectId, editorSessionId: `workflow-route-${Math.random()}`, loopId: loop.id, loopRevision: loop.revision };
      const posts: any[] = [];
      try {
        await session.handle({ schemaVersion: 1, kind: "control-studio.engineer.tune", messageId: scope.editorSessionId,
          projectId: scope.projectId, editorSessionId: scope.editorSessionId, loopId: scope.loopId,
          expectedLoopRevision: scope.loopRevision, payload: { ...payload, ...values } }, scope,
        async value => { posts.push(value); });
        return posts[posts.length - 1];
      } finally { session.dispose(); }
    };

    const missingModel = await run(proportionalLoop(), undefined, {});
    assert.strictEqual(missingModel?.failure?.nextAction?.kind, "identify");

    const invalidSettings = await run(proportionalLoop(), model, { referenceChange: 0 });
    assert.strictEqual(invalidSettings?.failure?.nextAction?.kind, "tune");

    const missingPidAdapter = await run(pidLoop(), model, { family: "PI", applicationMode: "auto" });
    assert.strictEqual(missingPidAdapter?.failure?.nextAction?.kind, "choose-method");

    const constrained = proportionalLoop();
    constrained.parameterBinding.parameters[0].maximum = 0.1;
    const infeasibleTask = await run(constrained, model, {});
    assert.strictEqual(infeasibleTask?.failure?.nextAction?.kind, "open-requirements");
  });

  test("recommends P first for a typed PID adapter until the declared task proves an added term necessary", () => {
    const loop = pidLoop();
    const model: any = { kind: "transfer-function", numerator: [1], denominator: [1, 1], delaySeconds: 0,
      revision: "first-order", source: "provided", validation: "validated", evidence: ["bench"],
      normalizedError: 0.01, inputs: [channel("u", "V")], outputs: [channel("y", "m")] };
    const session = new EngineerWorkflowSession(loop, () => model, () => undefined);
    try {
      const result = session.modelChanged();
      assert.strictEqual(result.status, "model-ready");
      assert.match(String(result.reason), /Choose and tune a compatible controller/i);
      assert.match(String(result.reason), /PI, PD and PID|P, PI, PD, PID/i);
    } finally { session.dispose(); }
  });

  test("blocks unvalidated or channel-incompatible supplied models before recommendation", () => {
    const base: any = { kind: "transfer-function", numerator: [1], denominator: [1, 1], delaySeconds: 0,
      revision: "supplied", source: "provided", validation: "validated", evidence: ["bench"],
      inputs: [channel("u", "V")], outputs: [channel("y", "m")] };
    const rejected = (model: any) => {
      const session = new EngineerWorkflowSession(proportionalLoop(), () => model, () => undefined);
      try { return session.modelChanged(); } finally { session.dispose(); }
    };
    const unvalidated = rejected({ ...base, validation: "unvalidated", evidence: [] });
    assert.strictEqual(unvalidated.status, "unavailable");
    assert.strictEqual((unvalidated.failure as { nextAction?: { kind?: string } } | undefined)?.nextAction?.kind, "identify");
    assert.match(String(unvalidated.reason), /independent validation/i);
    assert.strictEqual((unvalidated.model as any)?.revision, "supplied", "The blocked model remains available for inspection");
    assert.strictEqual((unvalidated.modelAnalysis as any)?.model.revision, "supplied", "Model inspection is independent of controller qualification");

    const wrongOutputUnit = rejected({ ...base, revision: "wrong-output-unit", outputs: [channel("y", "°C")] });
    assert.strictEqual(wrongOutputUnit.status, "unavailable");
    assert.strictEqual((wrongOutputUnit.failure as { nextAction?: { kind?: string } } | undefined)?.nextAction?.kind, "identify");
    assert.match(String(wrongOutputUnit.reason), /output(?:\s+\w+)?\s+unit/i);

    const multivariable = rejected({ ...base, revision: "mimo", inputs: [channel("u1", "V"), channel("u2", "V")] });
    assert.strictEqual(multivariable.status, "model-ready");
    assert.strictEqual(multivariable.failure, undefined);
    assert.match(String(multivariable.reason), /multivariable|channel|compatible/i);
  });

  test("subjects a model returned by Find to the same compatibility gate as Supply", async () => {
    const loop = proportionalLoop();
    let selected: any;
    const search = require("../../controlStudio/runtimeSearchConnection");
    const connected = require("../../controlStudio/connectedIdentification");
    const oldSearch = search.connectRuntimeSearch;
    const oldRun = connected.ConnectedIdentification.prototype.run;
    search.connectRuntimeSearch = async () => ({ source: "simulation", request: async () => ({}) });
    connected.ConnectedIdentification.prototype.run = async function () { return { accepted: {
      model: { kind: "first_order_dead_time", gain: 1, timeConstantSeconds: 1, deadTimeSeconds: 0, stable: true, normalizedError: 0 },
      fit: { points: [], warnings: [], operatingRange: { inputMinimum: 0, inputMaximum: 1, outputMinimum: 0, outputMaximum: 1 } },
      evidence: "Identified from simulation data", review: {},
    }, runs: [] }; };
    const session = new EngineerWorkflowSession(loop, () => selected, () => {
      selected = { kind: "transfer-function", numerator: [1], denominator: [1, 1], delaySeconds: 0,
        revision: "unvalidated-find", source: "fitted", validation: "unvalidated", evidence: [],
        inputs: [channel("u", "V")], outputs: [channel("y", "m")] };
    }, undefined, undefined, undefined, undefined, undefined, undefined, liveAcquisitionPorts());
    const scope = { projectId: loop.projectId, editorSessionId: "find-compatibility", loopId: loop.id, loopRevision: loop.revision };
    const posts: any[] = [];
    try {
      await session.handle({ schemaVersion: 1, kind: "control-studio.engineer.find", messageId: "find-compatibility",
        projectId: scope.projectId, editorSessionId: scope.editorSessionId, loopId: scope.loopId,
        expectedLoopRevision: scope.loopRevision, payload: {} }, scope, async value => { posts.push(value); });
      const result = posts[posts.length - 1];
      assert.strictEqual(result?.status, "failed", String(result?.reason));
      assert.strictEqual(result?.failure?.nextAction?.kind, "identify");
      assert.match(String(result?.reason), /independent validation/i);
    } finally {
      session.dispose();
      search.connectRuntimeSearch = oldSearch;
      connected.ConnectedIdentification.prototype.run = oldRun;
    }
  });

  test("keeps a legal identified MIMO model on Identify while Tune stays gated for the scalar PID target", async () => {
    const loop = pidLoop();
    const mimoModel: any = {
      kind: "discrete-state-space", revision: "identified-mimo", source: "fitted", validation: "validated", evidence: ["fit", "independent"],
      inputs: [channel("u1", "V"), channel("u2", "V")], outputs: [channel("y1", "m"), channel("y2", "m")],
      states: [channel("x1", "m")], sampleTimeSeconds: loop.controller.taskPeriodMs / 1000, delaySeconds: 0,
      a: [[0.5]], b: [[0.2, 0.1]], c: [[1], [1]], d: [[0, 0], [0, 0]],
    };
    const search = require("../../controlStudio/runtimeSearchConnection");
    const connected = require("../../controlStudio/connectedIdentification");
    const oldSearch = search.connectRuntimeSearch;
    const oldRun = connected.ConnectedIdentification.prototype.run;
    let selected: any;
    search.connectRuntimeSearch = async () => ({ source: "simulation", request: async () => ({ project_revision: "project" }) });
    connected.ConnectedIdentification.prototype.run = async function () { return { model: mimoModel, runs: [] }; };
    const session = new EngineerWorkflowSession(loop, () => selected, () => assert.fail("MIMO must use the general-model handoff"),
      undefined, undefined, undefined, value => { selected = value; }, undefined, undefined, liveAcquisitionPorts());
    const scope = { projectId: loop.projectId, editorSessionId: "mimo-identify", loopId: loop.id, loopRevision: loop.revision };
    const posts: any[] = [];
    const message = (action: string) => ({ schemaVersion: 1, kind: `control-studio.engineer.${action}`, messageId: action,
      projectId: scope.projectId, editorSessionId: scope.editorSessionId, loopId: scope.loopId,
      expectedLoopRevision: scope.loopRevision, payload: {} });
    try {
      await session.handle(message("find"), scope, async value => { posts.push(value); });
      const result = posts[posts.length - 1];
      assert.strictEqual(result.status, "model-ready", "Identify must present the accepted model even when the current controller cannot tune it");
      assert.deepStrictEqual(result.model, mimoModel, "the accepted MIMO model must remain available for Identify review");
      assert.strictEqual(result.stageEvidence?.hasModel, false, "Tune must remain gated until a compatible declared controller target is selected");

      await session.handle(message("tune"), scope, async value => { posts.push(value); });
      assert.strictEqual(posts[posts.length - 1].status, "failed", "Tune must retain the controller/model compatibility boundary");
      assert.match(String(posts[posts.length - 1].reason), /multivariable|channel|compatible/i);
    } finally {
      session.dispose();
      search.connectRuntimeSearch = oldSearch;
      connected.ConnectedIdentification.prototype.run = oldRun;
    }
  });

  test("waits for a moving output to settle before exciting the connected plant", async function () {
    this.timeout(10000); // the connected acquisition observes one steady window and a baseline in real time
    const loop = proportionalLoop();
    loop.experiment = { execution: "control-studio", kind: "mv_step", amplitude: 1, durationSeconds: 10, seed: 7 };
    const search = require("../../controlStudio/runtimeSearchConnection");
    const recordingModule = require("../../controlStudio/runtimeRecording");
    const oldSearch = search.connectRuntimeSearch;
    const oldStart = recordingModule.RuntimeRecordingSession.start;
    const started = Date.now();
    // The previous experiment's response is still decaying: 5 m falling with a 0.25 s time constant.
    const moving = () => 5 * Math.exp(-(Date.now() - started) / 250);
    // The recording starts first and observes the baseline (B3.4 steps 1-2); the step is the experiment start, sent after the steady baseline.
    let atStep: number | undefined;
    search.connectRuntimeSearch = async () => ({ source: "simulation", request: async (operation: string) => {
      if (operation === "control.experiment.start") atStep = moving();
      return operation === "control.experiment.status" ? { status: "completed", stop_reason: "duration_complete", restoration: "restored" } : {};
    } });
    recordingModule.RuntimeRecordingSession.start = async () => ({ recoveryOwner: () => ({ schemaVersion: 1, recordingId: "settled-recording", projectId: loop.projectId, runtimeId: loop.runtimeId,
        loopId: loop.id, loopRevision: loop.revision, source: "simulation", bindings: [] }),
      // The campaign judges the steady baseline from the recording's follow pages on process time (B3.4 step 2).
      follow: async () => followPage((Date.now() - started) / 1000, moving()),
      stop: async () => ({ samples: [], evidence: { recordingId: "settled-recording", method: "runtime_scan_buffer", completeness: "incomplete",
        observedSampleCount: 0, missingScanRanges: [], droppedObservedSamples: 0, actualCycleTimingAvailable: false,
        freshnessAvailable: false, reason: "The runtime returned no samples." } }) });
    const sample = () => { const value = manualSample(loop); value.processVariable = { ...value.processVariable, value: moving() }; return value; };
    const session = new EngineerWorkflowSession(loop, () => undefined, () => undefined,
      undefined, undefined, undefined, undefined, undefined, undefined, liveAcquisitionPorts(sample));
    const scope = { projectId: loop.projectId, editorSessionId: "settle-first", loopId: loop.id, loopRevision: loop.revision };
    const posts: any[] = [];
    try {
      await session.handle({ schemaVersion: 1, kind: "control-studio.engineer.find", messageId: "settle-first",
        projectId: scope.projectId, editorSessionId: scope.editorSessionId, loopId: scope.loopId,
        expectedLoopRevision: scope.loopRevision, payload: {} }, scope, async value => { posts.push(value); });
      assert.ok(atStep !== undefined, "the experiment step was sent");
      assert.ok(Math.abs(atStep!) < 0.4, `the step starts from a steady output, not from ${atStep} m while it still moves`);
      assert.ok(posts.some(post => post.status === "running" && /settle/i.test(String(post.reason))), "the wait is shown as progress");
    } finally {
      session.dispose();
      search.connectRuntimeSearch = oldSearch;
      recordingModule.RuntimeRecordingSession.start = oldStart;
    }
  });

  test("returns a PRBS Find whose connected recording is unusable to Identify", async function () {
    this.timeout(10000); // the connected acquisition observes one steady window and a baseline in real time
    const loop = proportionalLoop();
    loop.experiment = { execution: "control-studio", kind: "prbs", amplitude: 1, durationSeconds: 10, seed: 7 };
    const search = require("../../controlStudio/runtimeSearchConnection");
    const recordingModule = require("../../controlStudio/runtimeRecording");
    const oldSearch = search.connectRuntimeSearch;
    const oldStart = recordingModule.RuntimeRecordingSession.start;
    let requests = 0;
    const started = Date.now();
    search.connectRuntimeSearch = async () => ({ source: "simulation", request: async (operation: string) => {
      requests += 1;
      return operation === "control.experiment.status" ? { status: "completed", stop_reason: "duration_complete", restoration: "restored" } : {};
    } });
    recordingModule.RuntimeRecordingSession.start = async () => ({
      recoveryOwner: () => ({ schemaVersion: 1, recordingId: "prbs-recording", projectId: loop.projectId, runtimeId: loop.runtimeId,
        loopId: loop.id, loopRevision: loop.revision, source: "simulation", bindings: [] }),
      // The output holds at 0 m, so the baseline is judged on its own window; the PRBS then runs and its recording is unusable.
      follow: async () => followPage((Date.now() - started) / 1000, 0),
      stop: async () => ({ samples: [], evidence: { recordingId: "prbs-recording", method: "runtime_scan_buffer", completeness: "incomplete",
        observedSampleCount: 0, missingScanRanges: [], droppedObservedSamples: 0, actualCycleTimingAvailable: false,
        freshnessAvailable: false, reason: "The runtime returned no samples." } }),
    });
    const session = new EngineerWorkflowSession(loop, () => undefined, () => undefined,
      undefined, undefined, undefined, undefined, undefined, undefined, liveAcquisitionPorts(() => manualSample(loop)));
    const scope = { projectId: loop.projectId, editorSessionId: "find-method", loopId: loop.id, loopRevision: loop.revision };
    const posts: any[] = [];
    try {
      await session.handle({ schemaVersion: 1, kind: "control-studio.engineer.find", messageId: "find-method",
        projectId: scope.projectId, editorSessionId: scope.editorSessionId, loopId: scope.loopId,
        expectedLoopRevision: scope.loopRevision, payload: {} }, scope, async value => { posts.push(value); });
      const result = posts[posts.length - 1];
      assert.strictEqual(result?.status, "failed", String(result?.reason));
      assert.strictEqual(result?.failure?.nextAction?.kind, "identify");
      assert.ok(requests > 0, "the implemented PRBS method may dispatch bounded acquisition before reporting retained-data failure");
    } finally {
      session.dispose();
      search.connectRuntimeSearch = oldSearch;
      recordingModule.RuntimeRecordingSession.start = oldStart;
    }
  });

  test("predicts the tuned controller against the running controller under the same conditions", async () => {
    const loop = pidLoop();
    loop.controller.pidInitialization = pidInitialization();
    const model: any = { kind: "transfer-function", numerator: [1], denominator: [2, 1], delaySeconds: 0,
      revision: "current-comparison", source: "provided", validation: "validated", evidence: ["bench"],
      inputs: [channel("u", "V")], outputs: [channel("y", "m")], operatingPoint: { inputs: [0], outputs: [0] } };
    const parameters = require("../../controlStudio/runtimeParameterConnection");
    const oldConnect = parameters.connectRuntimeParameters;
    const tune = async (running: boolean) => {
      parameters.connectRuntimeParameters = async () => {
        if (!running) throw new Error("Start the bound runtime before reviewing this operation.");
        return { source: "simulation", client: { status: async () => ({ values: { "Main.Kp": 0.5, "Main.Ki": 0.1, "Main.Kd": 0 } }) } };
      };
      const session = new EngineerWorkflowSession(loop, () => model, () => undefined);
      const scope = { projectId: loop.projectId, editorSessionId: `current-${running}`, loopId: loop.id, loopRevision: loop.revision };
      const posts: any[] = [];
      try {
        assert.strictEqual(session.modelChanged().status, "model-ready");
        await session.handle({ schemaVersion: 1, kind: "control-studio.engineer.tune", messageId: `current-${running}`, ...scope,
          expectedLoopRevision: scope.loopRevision, payload: { family: "PI", automaticRequirements: true } }, scope, async value => { posts.push(value); });
        return posts[posts.length - 1];
      } finally { session.dispose(); }
    };
    try {
      const compared = await tune(true);
      assert.strictEqual(compared.status, "candidate", String(compared.reason));
      const points = compared.plots?.timeResponse as { currentPv?: number; candidatePv: number }[];
      assert.ok(points?.length && points.every(point => typeof point.currentPv === "number"), "the prediction includes the running controller");
      assert.ok(points.some(point => Math.abs((point.currentPv ?? 0) - point.candidatePv) > 1e-6), "the two controllers are simulated separately");
      const alone = await tune(false);
      assert.strictEqual(alone.status, "candidate", String(alone.reason));
      assert.ok((alone.plots?.timeResponse as { currentPv?: number }[]).every(point => point.currentPv === undefined),
        "without a running controller only the tuned response is predicted");
    } finally { parameters.connectRuntimeParameters = oldConnect; }
  });

  test("tunes a supplied model on a loop that declares no identification experiment", async () => {
    const loop = pidLoop();
    loop.controller.pidInitialization = pidInitialization();
    delete loop.experiment;
    const model: any = { kind: "transfer-function", numerator: [1], denominator: [2, 1], delaySeconds: 0,
      revision: "supplied-without-experiment", source: "provided", validation: "validated", evidence: ["bench"],
      inputs: [channel("u", "V")], outputs: [channel("y", "m")], operatingPoint: { inputs: [0], outputs: [0] } };
    const session = new EngineerWorkflowSession(loop, () => model, () => undefined);
    const scope = { projectId: loop.projectId, editorSessionId: "supplied-no-experiment", loopId: loop.id, loopRevision: loop.revision };
    const posts: any[] = [];
    try {
      const ready = session.modelChanged();
      assert.strictEqual(ready.status, "model-ready", `a supplied model needs no experiment limits: ${ready.reason}`);
      await session.handle({ schemaVersion: 1, kind: "control-studio.engineer.tune", messageId: "supplied-no-experiment", ...scope,
        expectedLoopRevision: scope.loopRevision, payload: { family: "PI", automaticRequirements: true } }, scope, async value => { posts.push(value); });
      assert.strictEqual(posts[posts.length - 1].status, "candidate", String(posts[posts.length - 1].reason));
    } finally { session.dispose(); }
  });

  test("retains automatic P PI PD and PID attempts and chooses the simplest passing typed candidate", async () => {
    const loop = pidLoop();
    loop.controller.pidInitialization = pidInitialization();
    const model: any = { kind: "transfer-function", numerator: [1], denominator: [1, 1], delaySeconds: 0,
      revision: "first-order", source: "provided", validation: "validated", evidence: ["bench"],
      normalizedError: 0.01, inputs: [channel("u", "V")], outputs: [channel("y", "m")] };
    const session = new EngineerWorkflowSession(loop, () => model, () => undefined);
    const scope = { projectId: loop.projectId, editorSessionId: "pid-automatic", loopId: loop.id, loopRevision: loop.revision };
    const posts: any[] = [];
    try {
      await session.handle({ schemaVersion: 1, kind: "control-studio.engineer.tune", messageId: "automatic",
        projectId: scope.projectId, editorSessionId: scope.editorSessionId, loopId: scope.loopId, expectedLoopRevision: scope.loopRevision,
        payload: { family: "automatic", referenceChange: 1, maximumError: 0.1, settlingBand: 0.02,
          settlingTimeSeconds: 2, durationSeconds: 4, applicationMode: "auto" } }, scope, async value => { posts.push(value); });
      const result = posts[posts.length - 1];
      assert.strictEqual(result?.status, "candidate", String(result?.reason));
      assert.strictEqual(result?.design?.candidate?.structure, "P");
      assert.deepStrictEqual(result?.values, { "Main.Kp": 11.5, "Main.Ki": 0, "Main.Kd": 0 });
      assert.deepStrictEqual(result?.attempts.map((attempt: any) => [attempt.structure, attempt.status]), [
        ["P", "Feasible"], ["PI", "Feasible"], ["PD", "Rejected"], ["PID", "Feasible"],
      ]);
      assert.match(String(result?.reason), /simplest (?:model-screened|passing) candidate.*P/i);
    } finally { session.dispose(); }
  });

  test("automatically selects PI when the declared task proves P cannot meet the gain bounds", async () => {
    const loop = pidLoop();
    const model: any = { kind: "transfer-function", numerator: [1], denominator: [1, 1], delaySeconds: 0,
      revision: "first-order", source: "provided", validation: "validated", evidence: ["bench"],
      inputs: [channel("u", "V")], outputs: [channel("y", "m")] };
    const session = new EngineerWorkflowSession(loop, () => model, () => undefined);
    const scope = { projectId: loop.projectId, editorSessionId: "automatic-pi", loopId: loop.id, loopRevision: loop.revision };
    const posts: any[] = [];
    try {
      await session.handle({ schemaVersion: 1, kind: "control-studio.engineer.tune", messageId: "automatic-pi",
        projectId: scope.projectId, editorSessionId: scope.editorSessionId, loopId: scope.loopId,
        expectedLoopRevision: scope.loopRevision, payload: { family: "automatic", referenceChange: 1,
          maximumError: 0.005, settlingBand: 0.001, settlingTimeSeconds: 3, durationSeconds: 6,
          applicationMode: "manual" } }, scope, async value => { posts.push(value); });
      const result = posts[posts.length - 1];
      assert.strictEqual(result?.status, "candidate", String(result?.reason));
      assert.strictEqual(result?.design?.candidate?.structure, "PI");
      assert.deepStrictEqual(result?.values, { "Main.Kp": 3, "Main.Ki": 3, "Main.Kd": 0 });
      assert.deepStrictEqual(result?.attempts.map((attempt: any) => [attempt.structure, attempt.status]), [
        ["P", "Rejected"], ["PI", "Feasible"], ["PD", "Rejected"], ["PID", "Rejected"],
      ]);
      assert.match(result?.attempts[3].reason, /is up to .* m from the target after 3 s, where 0\.005 m is allowed/,
        `the derivative zero leaves a slow residual that PI's exact pole cancellation does not: ${result?.attempts[3].reason}`);
      assert.match(String(result?.reason), /PI is the simplest.*P/i);
    } finally { session.dispose(); }
  });

  test("rejects an automatic candidate that exceeds declared actuator output limits before Verify", async () => {
    const loop = proportionalLoop();
    loop.limits = { outputMinimum: -0.5, outputMaximum: 0.5 };
    const model: any = { kind: "transfer-function", numerator: [1], denominator: [1, 1], delaySeconds: 0,
      revision: "first-order", source: "provided", validation: "validated", evidence: ["bench"],
      inputs: [channel("u", "V")], outputs: [channel("y", "m")] };
    const session = new EngineerWorkflowSession(loop, () => model, () => undefined);
    const scope = { projectId: loop.projectId, editorSessionId: "automatic-output-limit", loopId: loop.id, loopRevision: loop.revision };
    const posts: any[] = [];
    try {
      await session.handle({ schemaVersion: 1, kind: "control-studio.engineer.tune", messageId: "automatic-output-limit",
        projectId: scope.projectId, editorSessionId: scope.editorSessionId, loopId: scope.loopId,
        expectedLoopRevision: scope.loopRevision, payload: { family: "automatic", referenceChange: 1,
          maximumError: 0.1, settlingBand: 0.02, settlingTimeSeconds: 2, durationSeconds: 4,
          applicationMode: "manual" } }, scope, async value => { posts.push(value); });
      const result = posts[posts.length - 1];
      assert.strictEqual(result?.status, "failed", String(result?.reason));
      assert.strictEqual(result?.failure?.nextAction?.kind, "open-requirements");
      assert.match(String(result?.reason), /output limit/i);
    } finally { session.dispose(); }
  });

  test("routes a valid unsupported model structure to controller selection rather than retuning", async () => {
    const loop = pidLoop();
    // Complex poles (ζ = 0.4): SIMC's PI rule has no half-rule reduction for them, so another configuration is chosen.
    const model: any = { kind: "transfer-function", numerator: [1], denominator: [1, 0.8, 1], delaySeconds: 0,
      revision: "second-order", source: "provided", validation: "validated", evidence: ["bench"],
      inputs: [channel("u", "V")], outputs: [channel("y", "m")] };
    const session = new EngineerWorkflowSession(loop, () => model, () => undefined);
    const scope = { projectId: loop.projectId, editorSessionId: "unsupported-structure", loopId: loop.id, loopRevision: loop.revision };
    const posts: any[] = [];
    try {
      await session.handle({ schemaVersion: 1, kind: "control-studio.engineer.tune", messageId: "unsupported-structure",
        projectId: scope.projectId, editorSessionId: scope.editorSessionId, loopId: scope.loopId,
        expectedLoopRevision: scope.loopRevision, payload: { family: "automatic", referenceChange: 1,
          maximumError: 0.1, settlingBand: 0.02, settlingTimeSeconds: 2, durationSeconds: 4,
          applicationMode: "manual" } }, scope, async value => { posts.push(value); });
      const result = posts[posts.length - 1];
      assert.strictEqual(result?.status, "candidate", String(result?.reason));
      assert.strictEqual(result?.design?.candidate?.structure, "PD");
      assert.match(String(result?.reason), /candidate|actual-ST/i);
    } finally { session.dispose(); }
  });

  test("permits Auto PI tuning only when the loop declares its typed initialization adapter", async () => {
    const loop = pidLoop();
    loop.controller.pidInitialization = {
      kind: "parallel-pid-v1",
      integralState: { symbol: "Main.Integral", iecType: "LREAL", unit: "V", minimum: -20, maximum: 20 },
      derivativeState: { symbol: "Main.PreviousError", iecType: "LREAL", unit: "m", minimum: -20, maximum: 20 },
      proportionalSetpointWeight: 1,
      derivativeSetpointWeight: 1,
      maximumSnapshotAgeMs: 100,
    };
    const model: any = { kind: "transfer-function", numerator: [1], denominator: [1, 1], delaySeconds: 0,
      revision: "first-order", source: "provided", validation: "validated", evidence: ["bench"],
      normalizedError: 0.01, inputs: [channel("u", "V")], outputs: [channel("y", "m")] };
    const scope = { projectId: loop.projectId, editorSessionId: "pid-auto", loopId: loop.id, loopRevision: loop.revision };
    const posts: any[] = [];
    const session = new EngineerWorkflowSession(loop, () => model, () => undefined);
    const tune = (value: any) => session.handle({ schemaVersion: 1, kind: "control-studio.engineer.tune", messageId: "pi-auto",
      projectId: scope.projectId, editorSessionId: scope.editorSessionId, loopId: scope.loopId, expectedLoopRevision: scope.loopRevision,
      payload: { family: "PI", referenceChange: 1, maximumError: 0.1, settlingBand: 0.02,
        settlingTimeSeconds: 2, durationSeconds: 4, applicationMode: "auto", ...value } }, scope,
    async value => { posts.push(value); });
    try {
      await tune({});
      assert.strictEqual(posts[posts.length - 1]?.status, "candidate", "the declared adapter makes an unapplied PI Auto candidate eligible for Verify");
      assert.match(String(posts[posts.length - 1]?.reason), /PI candidate ready|Auto initialization/i);

      const missing = pidLoop();
      const blocked = new EngineerWorkflowSession(missing, () => model, () => undefined);
      const blockedPosts: any[] = [];
      await blocked.handle({ schemaVersion: 1, kind: "control-studio.engineer.tune", messageId: "pi-auto-missing",
        projectId: scope.projectId, editorSessionId: scope.editorSessionId, loopId: missing.id, expectedLoopRevision: missing.revision,
        payload: { family: "PI", referenceChange: 1, maximumError: 0.1, settlingBand: 0.02,
          settlingTimeSeconds: 2, durationSeconds: 4, applicationMode: "auto" } },
      { ...scope, loopId: missing.id, loopRevision: missing.revision }, async value => { blockedPosts.push(value); });
      assert.strictEqual(blockedPosts[blockedPosts.length - 1]?.status, "failed");
      assert.match(String(blockedPosts[blockedPosts.length - 1]?.reason), /typed PI\/PID initialization adapter/i);
      blocked.dispose();
    } finally { session.dispose(); }
  });

  test("verifies the declared PI retained states and first output before Auto commissioning", async () => {
    const loop = pidLoop();
    const plants = drivingPlant(loop);
    loop.controller.pidInitialization = pidInitialization();
    const model: any = { kind: "transfer-function", numerator: [1], denominator: [1, 1], delaySeconds: 0,
      revision: "first-order", source: "provided", validation: "validated", evidence: ["bench"],
      normalizedError: 0.01, inputs: [channel("u", "V")], outputs: [channel("y", "m")] };
    const search = require("../../controlStudio/runtimeSearchConnection");
    const parameters = require("../../controlStudio/runtimeParameterConnection");
    const verification = require("../../controlStudio/verificationClient");
    const oldSearch = search.connectRuntimeSearch, oldParameters = parameters.connectRuntimeParameters;
    const oldVerify = verification.RuntimeVerificationClient.prototype.verify;
    const requests: any[] = [];
    search.connectRuntimeSearch = async () => ({ source: "simulation", request: async (operation: string) => {
      if (operation === "plant.read") return plants;
      throw new Error("Verify must use its existing client boundary");
    } });
    parameters.connectRuntimeParameters = async () => ({ source: "simulation", client: { status: async () => ({
      values: { "Main.Kp": 1, "Main.Ki": 0, "Main.Kd": 0 }, program_revision: "program", binding_revision: "binding",
    }) } });
    verification.RuntimeVerificationClient.prototype.verify = async function (request: any, requirements: any) {
      requests.push({ request, requirements });
      return { projectRevision: "project", request, run: {}, evaluation: { status: "Pass" } };
    };
    const session = new EngineerWorkflowSession(loop, () => model, () => undefined);
    const scope = { projectId: loop.projectId, editorSessionId: "pid-auto-verify", loopId: loop.id, loopRevision: loop.revision };
    const posts: any[] = [];
    const invoke = (action: string, payload: Record<string, unknown> = {}) => session.handle({ schemaVersion: 1,
      kind: `control-studio.engineer.${action}`, messageId: action, projectId: scope.projectId,
      editorSessionId: scope.editorSessionId, loopId: scope.loopId, expectedLoopRevision: scope.loopRevision, payload }, scope,
    async value => { posts.push(value); });
    try {
      await invoke("tune", { family: "PI", referenceChange: 1, maximumError: 0.1, settlingBand: 0.02,
        settlingTimeSeconds: 2, durationSeconds: 4, applicationMode: "auto" });
      await invoke("verify", { applicationMode: "auto" });
      const result = posts[posts.length - 1];
      assert.strictEqual(result?.status, "verified");
      const configuredPlantRequests = requests.filter(value => value.request.simulation_override === undefined);
      assert.strictEqual(configuredPlantRequests.length, 4, "Auto PI must execute baseline, candidate, a resting-noise probe for the supplied model and retained-state initialization records; prediction requests are separate");
      const probe = configuredPlantRequests[2];
      assert.deepStrictEqual(probe.request.events, [], "the probe holds the transfer trial's plant at rest in Manual");
      assert.strictEqual(probe.request.duration_ms, 1000);
      const initialization = configuredPlantRequests[3];
      assert.deepStrictEqual(initialization.request.initial_values, {
        "Main.Manual": 1, "Main.SP": 0, "Main.MV": 0, "Main.ManualCommand": 0,
        "Main.Integral": 0, "Main.PreviousError": 0,
      });
      assert.deepStrictEqual(initialization.request.events, [
        { at_ms: 60, symbol: "Main.SP", value: 0 },
        { at_ms: 60, symbol: "Main.Manual", value: 0 },
      ]);
      assert.ok(initialization.request.observations.some((value: any) => value.symbol === "Main.Integral"));
      assert.ok(initialization.request.observations.some((value: any) => value.symbol === "Main.PreviousError"));
      assert.ok(initialization.requirements.some((value: any) => value.id === "transfer-continuity"));
      assert.ok(initialization.requirements.some((value: any) => value.id === "integral-state"));
      assert.ok(initialization.requirements.some((value: any) => value.id === "derivative-state"));
    } finally {
      session.dispose(); search.connectRuntimeSearch = oldSearch; parameters.connectRuntimeParameters = oldParameters;
      verification.RuntimeVerificationClient.prototype.verify = oldVerify;
    }
  });

  test("routes a failed actual-ST verification back to Tune with its retained evidence", async () => {
    const loop = proportionalLoop();
    const plants = drivingPlant(loop);
    const model: any = { kind: "transfer-function", numerator: [1], denominator: [1, 1], delaySeconds: 0,
      revision: "first-order", source: "provided", validation: "validated", evidence: ["bench"],
      inputs: [channel("u", "V")], outputs: [channel("y", "m")] };
    const search = require("../../controlStudio/runtimeSearchConnection");
    const parameters = require("../../controlStudio/runtimeParameterConnection");
    const verification = require("../../controlStudio/verificationClient");
    const oldSearch = search.connectRuntimeSearch;
    const oldParameters = parameters.connectRuntimeParameters;
    const oldVerify = verification.RuntimeVerificationClient.prototype.verify;
    search.connectRuntimeSearch = async () => ({ source: "simulation", request: async (operation: string) => operation === "plant.read" ? plants : {} });
    parameters.connectRuntimeParameters = async () => ({ source: "simulation", client: { status: async () => ({
      values: { "Main.Kp": 1 }, program_revision: "program", binding_revision: "binding",
    }) } });
    const calls: any[] = [];
    verification.RuntimeVerificationClient.prototype.verify = async function (request: any, requirements: any[]) {
      calls.push(request);
      const configuredCall = calls.filter(value => value.simulation_override === undefined).length;
      return { projectRevision: "project", request, run: { id: `run-${calls.length}` }, evaluation: {
        status: configuredCall === 1 ? "Pass" : "Fail",
        requirements: requirements.map((requirement: any) => ({ id: requirement.id,
          status: configuredCall > 1 && ["error", "settling"].includes(requirement.kind) ? "Fail" : "Pass" })),
      } };
    };
    const session = new EngineerWorkflowSession(loop, () => model, () => undefined);
    const scope = { projectId: loop.projectId, editorSessionId: "verify-failure-route", loopId: loop.id, loopRevision: loop.revision };
    const posts: any[] = [];
    const invoke = (action: string, payload: Record<string, unknown> = {}) => session.handle({ schemaVersion: 1,
      kind: `control-studio.engineer.${action}`, messageId: action, projectId: scope.projectId, editorSessionId: scope.editorSessionId,
      loopId: scope.loopId, expectedLoopRevision: scope.loopRevision, payload }, scope, async value => { posts.push(value); });
    try {
      await invoke("tune", { family: "P", referenceChange: 1, maximumError: 0.1, settlingBand: 0.02,
        settlingTimeSeconds: 2, durationSeconds: 4, applicationMode: "manual" });
      await invoke("verify", { applicationMode: "manual" });
      const result = posts[posts.length - 1];
      assert.strictEqual(result?.status, "verification-failed");
      assert.strictEqual(result?.failure?.nextAction?.kind, "tune");
      assert.match(String(result?.reason), /Next: Tune controller/i);
      assert.strictEqual(calls.filter(request => request.simulation_override === undefined).length, 2,
        "the retained baseline and failed candidate must both execute actual-ST requests; prediction requests are separate");
    } finally {
      session.dispose();
      search.connectRuntimeSearch = oldSearch;
      parameters.connectRuntimeParameters = oldParameters;
      verification.RuntimeVerificationClient.prototype.verify = oldVerify;
    }
  });

  test("reviews Auto PI against the declared parallel PID state-transfer binding", async () => {
    const loop = pidLoop();
    loop.controller.pidInitialization = pidInitialization();
    const model: any = { kind: "transfer-function", numerator: [1], denominator: [1, 1], delaySeconds: 0,
      revision: "first-order", source: "provided", validation: "validated", evidence: ["bench"],
      normalizedError: 0.01, inputs: [channel("u", "V")], outputs: [channel("y", "m")] };
    const search = require("../../controlStudio/runtimeSearchConnection");
    const parameters = require("../../controlStudio/runtimeParameterConnection");
    const runtime = require("../../controlStudio/runtimeConnection");
    const verification = require("../../controlStudio/verificationClient");
    const oldSearch = search.connectRuntimeSearch, oldParameters = parameters.connectRuntimeParameters;
    const oldRuntime = runtime.bindRuntimeConnection, oldVerify = verification.RuntimeVerificationClient.prototype.verify;
    const observed = { values: { "Main.Kp": 1, "Main.Ki": 0, "Main.Kd": 0 }, program_revision: "program", binding_revision: "binding",
      active_revision: "active", status: "applied", previous_revision: null, previous_values: null, reason: null };
    const bindings: any[] = [];
    search.connectRuntimeSearch = async () => ({ source: "simulation", request: async (operation: string) => {
      if (operation === "control.simulation.run.status") return { ready: true, project_revision: "project", execution: "isolated-simulation", source_basis: "loaded-runtime-sources" };
      throw new Error(`unexpected search operation ${operation}`);
    } });
    parameters.connectRuntimeParameters = async () => ({ source: "simulation", client: {
      status: async () => observed,
      review: async (values: any) => ({ observed, values }),
    } });
    runtime.bindRuntimeConnection = async () => ({ source: "simulation", request: async (operation: string, payload: any) => {
      assert.strictEqual(operation, "control.transfer.status");
      bindings.push(payload.binding);
      return { program_revision: "program", binding_revision: "transfer", runtime_time_ns: 1_000_000_000, status: "idle", reason: null, transaction: null,
        supported_tracking: ["parallel-pid-v1"], values: { "Main.Manual": true, "Main.SP": 0, "Main.ManualCommand": 0,
          "Main.Integral": 0, "Main.PreviousError": 0, "Main.Kp": 1, "Main.Ki": 0, "Main.Kd": 0, "Main.PV": 0, "Main.MV": 0 } };
    } });
    verification.RuntimeVerificationClient.prototype.verify = async function (request: any) {
      return { projectRevision: "project", request, run: {}, evaluation: { status: "Pass" } };
    };
    const session = new EngineerWorkflowSession(loop, () => model, () => undefined);
    const scope = { projectId: loop.projectId, editorSessionId: "pid-auto-review", loopId: loop.id, loopRevision: loop.revision };
    const posts: any[] = [];
    const invoke = (action: string, payload: Record<string, unknown> = {}) => session.handle({ schemaVersion: 1,
      kind: `control-studio.engineer.${action}`, messageId: action, projectId: scope.projectId,
      editorSessionId: scope.editorSessionId, loopId: scope.loopId, expectedLoopRevision: scope.loopRevision, payload }, scope,
    async value => { posts.push(value); });
    try {
      const automatic = { applicationMode: "auto" as const };
      await invoke("tune", { family: "PI", referenceChange: 1, maximumError: 0.1, settlingBand: 0.02,
        settlingTimeSeconds: 2, durationSeconds: 4, ...automatic });
      await invoke("verify", automatic);
      await invoke("apply", automatic);
      assert.strictEqual(posts[posts.length - 1]?.status, "review", String(posts[posts.length - 1]?.reason));
      assert.deepStrictEqual(bindings[0].states.map((state: any) => state.symbol),
        ["Main.SP", "Main.ManualCommand", "Main.Integral", "Main.PreviousError"]);
      assert.deepStrictEqual(bindings[0].guards.map((guard: any) => guard.symbol),
        ["Main.Kp", "Main.Ki", "Main.Kd", "Main.PV"]);
    } finally {
      session.dispose(); search.connectRuntimeSearch = oldSearch; parameters.connectRuntimeParameters = oldParameters;
      runtime.bindRuntimeConnection = oldRuntime; verification.RuntimeVerificationClient.prototype.verify = oldVerify;
    }
  });

  test("verifies the full mapped PI record through the existing verification client", async () => {
    const loop = pidLoop();
    const model: any = { kind: "transfer-function", numerator: [1], denominator: [1, 1], delaySeconds: 0,
      revision: "first-order", source: "provided", validation: "validated", evidence: ["bench"],
      normalizedError: 0.01, inputs: [channel("u", "V")], outputs: [channel("y", "m")] };
    const search = require("../../controlStudio/runtimeSearchConnection");
    const parameters = require("../../controlStudio/runtimeParameterConnection");
    const verification = require("../../controlStudio/verificationClient");
    const oldSearch = search.connectRuntimeSearch, oldParameters = parameters.connectRuntimeParameters;
    const oldVerify = verification.RuntimeVerificationClient.prototype.verify;
    const requests: any[] = [];
    search.connectRuntimeSearch = async () => ({ source: "simulation", request: async () => { throw new Error("Verify must use its existing client boundary"); } });
    parameters.connectRuntimeParameters = async () => ({ source: "simulation", client: { status: async () => ({
      values: { "Main.Kp": 1, "Main.Ki": 0, "Main.Kd": 0 }, program_revision: "program", binding_revision: "binding",
    }) } });
    verification.RuntimeVerificationClient.prototype.verify = async function (request: any) {
      requests.push(request);
      return { projectRevision: "project", request, run: {}, evaluation: { status: "Pass" } };
    };
    const session = new EngineerWorkflowSession(loop, () => model, () => undefined);
    const scope = { projectId: loop.projectId, editorSessionId: "pid-verify", loopId: loop.id, loopRevision: loop.revision };
    const posts: any[] = [];
    const invoke = (action: string, payload: Record<string, unknown> = {}) => session.handle({ schemaVersion: 1,
      kind: `control-studio.engineer.${action}`, messageId: action, projectId: scope.projectId,
      editorSessionId: scope.editorSessionId, loopId: scope.loopId, expectedLoopRevision: scope.loopRevision, payload }, scope,
    async value => { posts.push(value); });
    try {
      await invoke("tune", { family: "PI", referenceChange: 1, maximumError: 0.1, settlingBand: 0.02,
        settlingTimeSeconds: 2, durationSeconds: 4, applicationMode: "manual" });
      await invoke("verify");
      assert.strictEqual(posts[posts.length - 1]?.status, "verified");
      assert.deepStrictEqual(requests[1].parameter_overrides.map((value: any) => [value.symbol, value.value]),
        [["Main.Kp", 3], ["Main.Ki", 3], ["Main.Kd", 0]]);
    } finally {
      session.dispose(); search.connectRuntimeSearch = oldSearch; parameters.connectRuntimeParameters = oldParameters;
      verification.RuntimeVerificationClient.prototype.verify = oldVerify;
    }
  });

  test("reviews and manually applies the full verified PI record", async () => {
    const loop = pidLoop();
    const model: any = { kind: "transfer-function", numerator: [1], denominator: [1, 1], delaySeconds: 0,
      revision: "first-order", source: "provided", validation: "validated", evidence: ["bench"], normalizedError: 0.01,
      inputs: [channel("u", "V")], outputs: [channel("y", "m")] };
    const search = require("../../controlStudio/runtimeSearchConnection");
    const parameters = require("../../controlStudio/runtimeParameterConnection");
    const verification = require("../../controlStudio/verificationClient");
    const oldSearch = search.connectRuntimeSearch, oldParameters = parameters.connectRuntimeParameters;
    const oldVerify = verification.RuntimeVerificationClient.prototype.verify;
    const observed = { values: { "Main.Kp": 1, "Main.Ki": 0, "Main.Kd": 0 }, program_revision: "program", binding_revision: "binding",
      active_revision: "active", status: "applied", previous_revision: null, previous_values: null, reason: null };
    const reviewed: any[] = []; const applied: any[] = []; const owners: any[] = [];
    search.connectRuntimeSearch = async () => ({ source: "simulation", request: async (operation: string) => {
      if (operation === "control.simulation.run.status") return { ready: true, project_revision: "project", execution: "isolated-simulation", source_basis: "loaded-runtime-sources" };
      throw new Error("existing client seam");
    } });
    parameters.connectRuntimeParameters = async () => ({ source: "simulation", client: {
      status: async () => observed,
      review: async (values: any) => { reviewed.push(values); return { observed, values }; },
      apply: async (review: any) => { applied.push(review.values); return { ...observed, values: review.values, active_revision: "pi", previous_revision: "active", previous_values: observed.values }; },
    } });
    verification.RuntimeVerificationClient.prototype.verify = async function (request: any) {
      return { projectRevision: "project", request, run: {}, evaluation: { status: "Pass" } };
    };
    const session = new EngineerWorkflowSession(loop, () => model, () => undefined, undefined,
      async owner => { owners.push(owner); });
    const scope = { projectId: loop.projectId, editorSessionId: "pid-apply", loopId: loop.id, loopRevision: loop.revision };
    const posts: any[] = [];
    const invoke = (action: string, payload: Record<string, unknown> = {}) => session.handle({ schemaVersion: 1,
      kind: `control-studio.engineer.${action}`, messageId: action, projectId: scope.projectId, editorSessionId: scope.editorSessionId,
      loopId: scope.loopId, expectedLoopRevision: scope.loopRevision, payload }, scope, async value => { posts.push(value); });
    try {
      await invoke("tune", { family: "PI", referenceChange: 1, maximumError: 0.1, settlingBand: 0.02, settlingTimeSeconds: 2, durationSeconds: 4, applicationMode: "manual" });
      await invoke("verify");
      await invoke("apply", { applicationMode: "manual" });
      await invoke("confirm");
      const expected = { "Main.Kp": 3, "Main.Ki": 3, "Main.Kd": 0 };
      assert.deepStrictEqual(reviewed, [expected]);
      assert.deepStrictEqual(applied, [expected]);
      assert.deepStrictEqual(owners.map(owner => owner.operation), ["apply"]);
      assert.strictEqual(posts[posts.length - 1]?.status, "applied");
    } finally {
      session.dispose(); search.connectRuntimeSearch = oldSearch; parameters.connectRuntimeParameters = oldParameters;
      verification.RuntimeVerificationClient.prototype.verify = oldVerify;
    }
  });

  test("restores a typed PID family as historical study evidence", () => {
    const loop = pidLoop();
    const model: any = { kind: "transfer-function", numerator: [1], denominator: [1, 1], delaySeconds: 0,
      revision: "first-order", source: "provided", validation: "validated", evidence: ["bench"],
      normalizedError: 0.01, inputs: [channel("u", "V")], outputs: [channel("y", "m")] };
    const session = new EngineerWorkflowSession(loop, () => model, () => undefined);
    try {
      session.restoreStudy({ selectedModel: model, family: "PID", applicationMode: "auto",
        task: { referenceChange: 1, maximumError: 0.1, settlingBand: 0.02, settlingTimeSeconds: 1, durationSeconds: 2 }, campaigns: [] });
      assert.strictEqual(session.snapshotStudy()?.family, "PID");
    } finally { session.dispose(); }
  });

  test("adopts inbound state design preferences before parsing the draft", async () => {
    const loop = stateDesignLoop();
    const model: any = stateDesignModel(loop);
    const session = new EngineerWorkflowSession(loop, () => model, () => undefined);
    const scope = { projectId: loop.projectId, editorSessionId: "state-draft-order", loopId: loop.id, loopRevision: loop.revision };
    const posts: any[] = [];
    const invoke = (payload: Record<string, unknown>) => session.handle({ schemaVersion: 1, kind: "control-studio.engineer.tune", messageId: `tune-${posts.length}`,
      projectId: scope.projectId, editorSessionId: scope.editorSessionId, loopId: scope.loopId, expectedLoopRevision: scope.loopRevision, payload }, scope,
    async value => { posts.push(value); });
    try {
      session.restoreStudy({ campaigns: [], tuningPreferences: { method: "state-design", stateDesignDraft: { stateWeights: "[[" } } } as any);
      await invoke({ family: "LQR", referenceChange: 1, maximumError: 0.1, settlingBand: 0.02, settlingTimeSeconds: 2, durationSeconds: 4, applicationMode: "manual",
        tuningPreferences: { method: "state-design", stateDesignDraft: { stateWeights: "[[1]]", inputWeights: "[[1]]", solverMode: "infinite" } } });
      assert.strictEqual(posts[posts.length - 1]?.status, "candidate", String(posts[posts.length - 1]?.reason));
      await invoke({ family: "LQR", referenceChange: 1, maximumError: 0.1, settlingBand: 0.02, settlingTimeSeconds: 2, durationSeconds: 4, applicationMode: "manual",
        tuningPreferences: { method: "state-design", stateDesignDraft: { stateWeights: "[[" } } });
      assert.strictEqual(posts[posts.length - 1]?.status, "failed", "A malformed raw state draft must be rejected before design");
    } finally { session.dispose(); }
  });
});

function stateDesignLoop(): any {
  const loop = pidLoop();
  loop.controller = { ...loop.controller, type: "State feedback", law: undefined, stateFeedback: {
    family: "LQR", convention: "sampled-deviation-negative-feedback", gainSymbols: [["Main.Kp"]], referenceGainSymbols: [["Main.N"]], stateWeights: [[1]], inputWeights: [[1]],
  } };
  loop.parameterBinding = { ...loop.parameterBinding, parameters: [
    { symbol: "Main.Kp", iec_type: "LREAL", unit: "V/m", convention: "state feedback gain", minimum: -100, maximum: 100 },
    { symbol: "Main.N", iec_type: "LREAL", unit: "V/m", convention: "reference gain", minimum: -100, maximum: 100 },
  ] };
  return loop;
}

function stateDesignModel(loop: any): any {
  const unit = loop.signals.processVariable.unit;
  return { kind: "continuous-state-space", revision: "state-draft-order-model", source: "provided", validation: "validated", evidence: ["independent"], delaySeconds: 0,
    inputs: [{ id: "u", unit: loop.signals.manipulatedVariable.unit, operatingRange: loop.signals.manipulatedVariable.range }],
    outputs: [{ id: "y", unit, operatingRange: loop.signals.processVariable.range }], states: [{ id: "x", unit, operatingRange: loop.signals.processVariable.range }],
    a: [[-1]], b: [[1]], c: [[1]], d: [[0]], operatingPoint: { inputs: [0], outputs: [0] } };
}

/**
 * One follow page of a running recording: a single good sample of the measured output at its process time (A3.2). The campaign
 * reads only these samples while it waits for a steady baseline (B3.4 step 2).
 */
function followPage(elapsedSeconds: number, processValue: number): Record<string, unknown> {
  return { state: "running", elapsedMs: Math.round(elapsedSeconds * 1_000), complete: false, reason: null,
    rawSamples: [{ elapsedSeconds, quality: "good", pv: processValue }] };
}

/** Find always runs through the panel's live acquisition service; these ports retain nothing. */
function liveAcquisitionPorts(sample: () => any = () => ({ availability: "unavailable", source: "Simulation", loopId: "none", reason: "not sampled" })): any {
  return { sample: async () => sample(), recordingOwner: () => undefined, persist: async () => undefined,
    clear: async () => undefined, retain: async () => undefined, mode: { read: () => undefined, write: async () => undefined },
    campaign: { read: () => undefined, write: async () => undefined } };
}

/**
 * A simulated plant that drives this loop's signals, so Validate runs the configured plant (Specification 35, B5.3). Without
 * one the qualified model is the plant and every actual-ST request keeps its model override.
 */
function drivingPlant(loop: any): { plants: { id: string; command_sources: string[]; feedback_targets: string[] }[] } {
  loop.plantId = "engineer-plant";
  return { plants: [{ id: loop.plantId, command_sources: [loop.signals.manipulatedVariable.address],
    feedback_targets: [loop.signals.processVariable.address] }] };
}

function manualSample(loop: any): any {
  const signal = (binding: any, value: number) => ({ symbol: binding.symbol, address: binding.address, value, unit: binding.unit, quality: "good" });
  return { availability: "available", source: "Simulation", loopId: loop.id, loopRevision: loop.revision,
    controller: { instance: loop.controller.instance, revision: loop.controller.revision, mode: "Manual" }, scan: 1, observedAtMs: Date.now(),
    setpoint: signal(loop.signals.setpoint, 0), processVariable: signal(loop.signals.processVariable, 0), manipulatedVariable: signal(loop.signals.manipulatedVariable, 0),
    outputLimits: { minimum: loop.limits.outputMinimum, maximum: loop.limits.outputMaximum }, rateLimits: {}, saturation: "none", recording: false };
}
