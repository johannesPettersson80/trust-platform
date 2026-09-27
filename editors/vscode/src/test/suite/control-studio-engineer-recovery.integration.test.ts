import * as assert from "assert";
import * as vscode from "vscode";
import * as fs from "fs";
import * as path from "path";
import { runtimeLifecycleService } from "../../runtimeLifecycle";
import { attachControlStudio } from "./control-studio-rendered-harness";
import { connectCdp, devToolsPort, evaluateInWebview, httpJson, pollValue, requiredEnvironment } from "./plant-rendered-harness";

suite("Control Studio engineer recovery", () => {
  test("retains a pending Manual rollback recovery ticket without reusing its closed review", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_ENGINEER_WORKFLOW !== "1") this.skip();
    const { EngineerWorkflowSession } = require("../../controlStudio/engineerWorkflowSession");
    const parameters = require("../../controlStudio/runtimeParameterConnection");
    const previous = parameters.connectRuntimeParameters;
    const loop = JSON.parse(fs.readFileSync(path.join(vscode.workspace.workspaceFolders![0].uri.fsPath, "control-loop.json"), "utf8"));
    let status: { program_revision: string; binding_revision: string; active_revision: string; values: Record<string, number>;
      status: "applied"; previous_revision: string | null; previous_values: Record<string, number> | null; reason: string | null } = {
      program_revision: "program", binding_revision: "binding", active_revision: "candidate",
      values: { "Main.Kp": 20 }, status: "applied", previous_revision: "original", previous_values: { "Main.Kp": 1 }, reason: null };
    let release!: () => void; const gate = new Promise<void>(resolve => { release = resolve; }); let rollbacks = 0; let statusCalls = 0;
    parameters.connectRuntimeParameters = async () => ({ source: "simulation", client: {
      status: async () => { statusCalls += 1; return status; },
      reviewRollback: async () => ({ observed: status, values: status.previous_values! }),
      rollback: async () => { rollbacks += 1; await gate; return status = { ...status, active_revision: "original", values: { "Main.Kp": 1 }, previous_revision: null, previous_values: null }; },
    } });
    const session = new EngineerWorkflowSession(loop, () => undefined, () => undefined);
    const scope = { projectId: loop.projectId, editorSessionId: "manual-close", loopId: loop.id, loopRevision: loop.revision };
    const posts: Record<string, unknown>[] = [];
    const call = (action: string) => session.handle({ schemaVersion: 1, kind: `control-studio.engineer.${action}`, messageId: action,
      projectId: scope.projectId, editorSessionId: scope.editorSessionId, loopId: scope.loopId, expectedLoopRevision: scope.loopRevision, payload: {} }, scope,
    async (payload: Record<string, unknown>) => { posts.push(payload); });
    try {
      await call("rollback"); const pending = call("confirm");
      await new Promise(resolve => setTimeout(resolve, 0));
      assert.strictEqual(session.hasPendingManualApplication(), true);
      const closing = session.finishPendingManualApplicationForClose();
      release();
      const recovery = await closing as unknown as { operation?: unknown; origin?: unknown; observed?: unknown } | undefined;
      await pending;
      assert.strictEqual(rollbacks, 1);
      assert.deepStrictEqual(recovery, {
        operation: "rollback",
        origin: { program_revision: "program", binding_revision: "binding" },
        observed: { ...status, active_revision: "original", values: { "Main.Kp": 1 }, previous_revision: null, previous_values: null },
      }, "Closing must retain only the operation and binding identity with native observed status; it must not retain a review token or client.");
      await call("confirm");
      assert.match(String(posts[posts.length - 1]?.reason), /Review the exact application or rollback first/,
        "A reopened or closed panel cannot confirm the consumed rollback review a second time.");
      statusCalls = 0;
      const reopened = new EngineerWorkflowSession(loop, () => undefined, () => undefined);
      const reopenedPosts: Record<string, unknown>[] = [];
      try {
        await (reopened as unknown as {
          reconcileManualApplicationRecovery(value: NonNullable<typeof recovery>, post: (payload: Record<string, unknown>) => Promise<void>): Promise<void>;
        }).reconcileManualApplicationRecovery(recovery!, async payload => { reopenedPosts.push(payload); });
      } finally { reopened.dispose(); }
      assert.strictEqual(statusCalls, 1, "Reopen must inspect the current runtime status exactly once instead of replaying the rollback request.");
      assert.strictEqual(rollbacks, 1, "Reopen must not issue a second rollback.");
      assert.deepStrictEqual(reopenedPosts, [{ status: "applied", observed: recovery!.observed,
        reason: "Closed-panel rollback is confirmed by the current runtime status." }]);
      const changedPosts: Record<string, unknown>[] = [];
      const changed = new EngineerWorkflowSession(loop, () => undefined, () => undefined);
      status = { ...status, binding_revision: "changed-binding" };
      try {
        await (changed as unknown as {
          reconcileManualApplicationRecovery(value: NonNullable<typeof recovery>, post: (payload: Record<string, unknown>) => Promise<void>): Promise<void>;
        }).reconcileManualApplicationRecovery(recovery!, async payload => { changedPosts.push(payload); });
      } finally { changed.dispose(); }
      assert.strictEqual(changedPosts[0]?.status, "recovery-required");
      assert.match(String(changedPosts[0]?.reason), /binding changed.*fresh review/i);
      const pendingPosts: Record<string, unknown>[] = [];
      const pendingRecovery = new EngineerWorkflowSession(loop, () => undefined, () => undefined);
      status = { ...status, binding_revision: "binding", status: "pending" } as any;
      try {
        await (pendingRecovery as unknown as {
          reconcileManualApplicationRecovery(value: NonNullable<typeof recovery>, post: (payload: Record<string, unknown>) => Promise<void>): Promise<void>;
        }).reconcileManualApplicationRecovery(recovery!, async payload => { pendingPosts.push(payload); });
      } finally { pendingRecovery.dispose(); }
      assert.strictEqual(pendingPosts[0]?.status, "recovery-required");
      assert.match(String(pendingPosts[0]?.reason), /did not provide a terminal/i);
    } finally { release?.(); session.dispose(); parameters.connectRuntimeParameters = previous; }
  });

  test("preserves a compatible gain pin through task edits and clears it after model changes", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_ENGINEER_WORKFLOW !== "1") this.skip();
    const { EngineerWorkflowSession } = require("../../controlStudio/engineerWorkflowSession");
    const loop = JSON.parse(fs.readFileSync(path.join(vscode.workspace.workspaceFolders![0].uri.fsPath, "control-loop.json"), "utf8"));
    const channel = (id: string, unit: string) => ({ id, unit, operatingRange: { minimum: -20, maximum: 20 } });
    let model = { kind: "transfer-function", numerator: [1], denominator: [1, 1], delaySeconds: 0,
      source: "provided", validation: "validated", evidence: ["bench"], revision: "sha256:pin-model",
      inputs: [channel("drive", "V")], outputs: [channel("position", "m")] };
    const session = new EngineerWorkflowSession(loop, () => model, () => undefined);
    const scope = { projectId: loop.projectId, editorSessionId: "pin", loopId: loop.id, loopRevision: loop.revision };
    const payload = { family: "P", referenceChange: 1, maximumError: 0.1, settlingBand: 0.02, settlingTimeSeconds: 2, durationSeconds: 4 };
    const posts: any[] = [];
    const invoke = (action: string, values = {}) => session.handle({ schemaVersion: 1, kind: `control-studio.engineer.${action}`,
      messageId: action, projectId: scope.projectId, editorSessionId: scope.editorSessionId,
      loopId: scope.loopId, expectedLoopRevision: scope.loopRevision, payload: values }, scope, async (p: any) => { posts.push(p); });
    try {
      await invoke("tune", { ...payload, gainMode: "manual", proportionalGain: 20 });
      assert.strictEqual(posts[posts.length - 1].status, "candidate");
      const saved = session.snapshotStudy();
      assert.strictEqual(saved.gainChoice.pin.value, 20);
      await invoke("invalidate");
      assert.strictEqual(session.snapshotStudy().gainChoice.pin.value, 20);
      await invoke("tune", { ...payload, gainMode: "automatic" });
      assert.notStrictEqual(posts[posts.length - 1].design.candidate.proportionalGain, 20);
      assert.deepStrictEqual(session.snapshotStudy().gainChoice, { mode: "automatic", pin: saved.gainChoice.pin });
      for (const pin of [{ ...saved.gainChoice.pin, unit: "wrong" }, { ...saved.gainChoice.pin, loopRevision: "old" },
        { ...saved.gainChoice.pin, parameterSymbol: "Other.Kp" }, { ...saved.gainChoice.pin, value: 101 }]) {
        assert.throws(() => session.restoreStudy({ ...saved, gainChoice: { mode: "manual", pin } }), /does not match/);
        assert.strictEqual(session.snapshotStudy().gainChoice.mode, "automatic", "Rejected restore must not mutate the current study");
      }
      model = { ...model, numerator: [2] };
      session.modelChanged();
      assert.strictEqual(session.snapshotStudy().gainChoice.pin, undefined,
        "Changing coefficients under a reused supplied revision invalidates the old pin");
      await invoke("tune", { ...payload, gainMode: "manual", proportionalGain: 20 });
      model = { ...model, operatingPoint: { inputs: [2], outputs: [10] } } as typeof model;
      session.modelChanged();
      assert.strictEqual(session.snapshotStudy().gainChoice.pin, undefined,
        "Changing operating coordinates under the same revision invalidates the old pin");
      await invoke("tune", { ...payload, gainMode: "manual", proportionalGain: 20 });
      model = { ...model, revision: "sha256:new-model" };
      session.modelChanged();
      assert.strictEqual(session.snapshotStudy().gainChoice.pin, undefined);
      assert.strictEqual(session.snapshotStudy().gainChoice.mode, "automatic");
    } finally { session.dispose(); }
  });

  test("waits for Verify cancellation confirmation and retains its terminal outcome", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_ENGINEER_WORKFLOW !== "1") this.skip();
    this.timeout(15000);
    const { EngineerWorkflowSession } = require("../../controlStudio/engineerWorkflowSession");
    const search = require("../../controlStudio/runtimeSearchConnection");
    const parameters = require("../../controlStudio/runtimeParameterConnection");
    const oldSearch = search.connectRuntimeSearch, oldParameters = parameters.connectRuntimeParameters;
    const loop = JSON.parse(fs.readFileSync(path.join(vscode.workspace.workspaceFolders![0].uri.fsPath, "control-loop.json"), "utf8"));
    const channel = (id: string, unit: string) => ({ id, unit, operatingRange: { minimum: -20, maximum: 20 } });
    const model = { kind: "transfer-function", numerator: [1], denominator: [1, 1], delaySeconds: 0,
      source: "provided", validation: "validated", evidence: ["bench"], revision: "sha256:cancel-model",
      inputs: [channel("drive", "V")], outputs: [channel("position", "m")] };
    let started!: () => void; const starting = new Promise<void>(resolve => { started = resolve; });
    let releaseStart!: (value: unknown) => void;
    let releaseTerminal!: (value: unknown) => void; const terminal = new Promise(resolve => { releaseTerminal = resolve; });
    const completed = { job_id: "pending-verify", project_revision: "project", status: "cancelled", reason: "Cancelled by engineer.",
      source_basis: "loaded-runtime-sources", io_provenance: "isolated-simulation-no-physical-drivers" };
    search.connectRuntimeSearch = async () => ({ source: "simulation", request: async (operation: string, params: any) => {
      if (operation.endsWith(".status") && !params.job_id) return { ready: true, project_revision: "project", execution: "isolated-simulation", source_basis: "loaded-runtime-sources" };
      if (operation.endsWith(".start")) { started(); return new Promise(resolve => { releaseStart = resolve; }); }
      if (operation.endsWith(".cancel")) return { ...completed, status: "running", cancel_requested: true };
      if (operation.endsWith(".status")) return terminal;
      throw new Error("Unexpected verification operation");
    } });
    parameters.connectRuntimeParameters = async () => ({ source: "simulation", client: { status: async () => ({ values: { "Main.Kp": 1 } }) } });
    const scope = { projectId: loop.projectId, editorSessionId: "verify-cancel", loopId: loop.id, loopRevision: loop.revision };
    const posts: any[] = [];
    const session = new EngineerWorkflowSession(loop, () => model, () => undefined);
    const invoke = (action: string, payload = {}) => session.handle({ schemaVersion: 1, kind: `control-studio.engineer.${action}`, messageId: action,
      projectId: scope.projectId, editorSessionId: scope.editorSessionId, loopId: scope.loopId, expectedLoopRevision: scope.loopRevision, payload }, scope, async (p: any) => { posts.push(p); });
    let pending: Promise<unknown> | undefined;
    try {
      await invoke("tune", { family: "P", referenceChange: 1, maximumError: 0.1, settlingBand: 0.02, settlingTimeSeconds: 2, durationSeconds: 4 });
      pending = invoke("verify"); await starting;
      let stopped = false; const stopping = invoke("cancel").then(() => { stopped = true; });
      await new Promise(resolve => setTimeout(resolve, 0));
      assert.strictEqual(stopped, false, "Cancel cannot finish before a pending Verify start and runtime confirmation settle");
      releaseStart({ job_id: "pending-verify", project_revision: "project" });
      await new Promise(resolve => setTimeout(resolve, 0));
      assert.strictEqual(stopped, false, "A running cancel acknowledgement is not a terminal stop");
      releaseTerminal(completed); await Promise.all([pending, stopping]);
      assert.strictEqual(posts[posts.length - 1].status, "cancelled");
      assert.strictEqual(session.snapshotStudy().verificationAttempts[0].jobId, "pending-verify");
      assert.strictEqual(session.snapshotStudy().verificationAttempts[0].status, "cancelled");
    } finally {
      releaseStart?.({ job_id: "pending-verify", project_revision: "project" }); releaseTerminal(completed);
      await pending?.catch(() => undefined); session.dispose(); search.connectRuntimeSearch = oldSearch; parameters.connectRuntimeParameters = oldParameters;
    }
  });

  test("cancels pending Find and reports retained terminal campaign before retry", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_ENGINEER_WORKFLOW !== "1") this.skip();
    this.timeout(15000);
    const { EngineerWorkflowSession } = require("../../controlStudio/engineerWorkflowSession");
    const connector = require("../../controlStudio/runtimeSearchConnection");
    const previous = connector.connectRuntimeSearch;
    const loop = JSON.parse(fs.readFileSync(path.join(vscode.workspace.workspaceFolders![0].uri.fsPath, "control-loop.json"), "utf8"));
    let started!: () => void; const starting = new Promise<void>(resolve => { started = resolve; });
    let release!: (value: unknown) => void;
    const cancelled: string[] = [];
    connector.connectRuntimeSearch = async () => ({ source: "simulation", request: async (operation: string, params: Record<string, unknown>) => {
      if (operation === "control.signals.inspect") return { available: true, loop_revision: params.loop_revision,
        program_revision: "program", bindings: params.bindings, scan: 1, runtime_time_ns: 20000000,
        captured_at_ms: Date.now(), values: [0, 0, 1], forced: [false, false, false], quality: ["good", "good", "good"] };
      if (operation === "control.simulation.run.status" && !params.job_id) return { ready: true, project_revision: "project", execution: "isolated-simulation", source_basis: "loaded-runtime-sources" };
      if (operation === "control.simulation.run.start") { started(); return new Promise(resolve => { release = resolve; }); }
      if (operation === "control.simulation.run.cancel") { cancelled.push(String(params.job_id)); return { status: "running", cancel_requested: true, job_id: params.job_id, project_revision: "project" }; }
      if (operation === "control.simulation.run.status" && params.job_id) return { status: "cancelled", job_id: params.job_id,
        project_revision: "project", source_basis: "loaded-runtime-sources", io_provenance: "isolated-simulation-no-physical-drivers" };
      throw new Error("Unexpected request after cancellation");
    } });
    const scope = { projectId: loop.projectId, editorSessionId: "recovery", loopId: loop.id, loopRevision: loop.revision };
    const message = (action: string) => ({ schemaVersion: 1, kind: `control-studio.engineer.${action}`, messageId: action,
      projectId: scope.projectId, editorSessionId: scope.editorSessionId, loopId: scope.loopId, expectedLoopRevision: scope.loopRevision, payload: {} });
    const posts: Record<string, any>[] = [];
    const session = new EngineerWorkflowSession(loop, () => undefined, () => { throw new Error("Cancelled Find cannot select a model"); });
    const post = async (payload: Record<string, unknown>) => { posts.push(payload); };
    try {
      const pending = session.handle(message("find"), scope, post);
      await starting;
      assert.throws(() => session.restoreStudy({ campaigns: [] }), /cancel|finish|running/i,
        "Opening a saved study must reject an active Find before replacing its history");
      const stopping = session.handle(message("cancel"), scope, post);
      release({ job_id: "pending-find", project_revision: "project" });
      await Promise.all([pending, stopping]);
      assert.deepStrictEqual(cancelled, ["pending-find"]);
      assert.strictEqual(posts[posts.length - 1].status, "cancelled", "Cancel must wait for the campaign outcome and publish terminal retained evidence");
      assert.strictEqual(posts[posts.length - 1].campaigns[0].status, "cancelled");
      assert.strictEqual(posts[posts.length - 1].campaigns[0].runs.length, 0);
      assert.strictEqual(posts.some(payload => payload.status === "model-ready"), false);
    } finally { session.dispose(); connector.connectRuntimeSearch = previous; }
  });

  test("retains failed actual identification records across retry", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_ENGINEER_WORKFLOW !== "1") this.skip();
    this.timeout(60000);
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    const project = vscode.workspace.workspaceFolders![0].uri.fsPath;
    const loop = JSON.parse(fs.readFileSync(path.join(project, "control-loop.json"), "utf8"));
    loop.signals.processVariable.range.maximum = 0.1;
    const start = await runtimeLifecycleService.startLocalSimulator();
    assert.strictEqual(start.ok, true, JSON.stringify(start));
    await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop",
      scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop });
    const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15000);
    const connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
    try {
      let session = await attachControlStudio(connection, port);
      const inner = (body: string) => evaluateInWebview(connection, session, body);
      await pollValue("engineer handlers", () => inner("return d.querySelector('[data-engineer-workflow]').dataset.ready;"), s => s === "true", 10000);
      await inner("d.defaultView.__campaignMessages=[];d.defaultView.addEventListener('trust-control-studio-engineer',e=>d.defaultView.__campaignMessages.push(e.detail));d.querySelector('[data-engineer-find]').click();return 'find';");
      await pollValue("failed bounded trial", () => inner("return d.querySelector('[data-engineer-status]').textContent;"), s => s.includes("find failed"), 15000);
      const state = JSON.parse(await inner("return JSON.stringify(d.defaultView.__campaignMessages.at(-1));"));
      assert.strictEqual(state.campaigns?.length, 1, "Find failure must return retained campaign evidence to the actual webview");
      assert.strictEqual(state.campaigns[0].status, "failed");
      assert.strictEqual(state.campaigns[0].runs[0].run.evidence, "ST-executed");
      assert.strictEqual(state.campaigns[0].runs[0].evaluation.status, "Fail");
      assert.ok(state.campaigns[0].runs[0].run.samples.length > 2);
      assert.strictEqual(await inner("return String(Boolean(d.querySelector('[data-engineer-campaign-history]')));"), "true",
        "Failed identification records must be inspectable in the rendered workflow");
      assert.match(await inner("return d.querySelector('[data-engineer-campaign-history]').textContent;"), /failed.*ST-executed/s);
      assert.strictEqual(await inner("const r=d.querySelector('[data-engineer-campaign-history]').getBoundingClientRect();return String(r.width>0 && r.height>0);"), "true", "Failure history must be visible in the active workflow stage");
      assert.ok(Number(await inner("return String(d.querySelectorAll('[data-engineer-campaign-history] tbody tr').length);")) > 2,
        "Raw failed trial values must remain available");
      await inner("d.querySelector('[data-engineer-find]').click();return 'retry';");
      await pollValue("retained retry", () => inner("return String(d.defaultView.__campaignMessages.at(-1)?.campaigns?.length ?? 0);"), s => s === "2", 15000);
      const retried = JSON.parse(await inner("return JSON.stringify(d.defaultView.__campaignMessages.at(-1));"));
      assert.notStrictEqual(retried.campaigns[0].id, retried.campaigns[1].id);
      assert.strictEqual(retried.campaigns[0].runs[0].run.id, state.campaigns[0].runs[0].run.id);
      assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-apply]').disabled);"), "true");
      assert.strictEqual(await inner("return String(d.querySelector('[data-result-save]').disabled);"), "false",
        "A failed Find study must be saveable without a selected model or candidate");
      await inner("[...d.querySelectorAll('[data-control-studio-mode]')].find(b=>b.textContent.trim()==='Validate').click();d.querySelector('[data-result-save]').click();return 'save';");
      await pollValue("saved failed study", () => inner("return d.querySelector('[data-result-state]').textContent;"), text => text.includes("Saved revision"), 10000);
      // Hold a real runtime request at the transport boundary to make Open/Find ordering deterministic.
      const connector = require("../../controlStudio/runtimeSearchConnection");
      const originalConnect = connector.connectRuntimeSearch;
      let entered!: () => void; const entering = new Promise<void>(resolve => { entered = resolve; });
      let release!: () => void; const released = new Promise<void>(resolve => { release = resolve; });
      const interruptCancellationStatus = process.env.TRUST_ENGINEER_CANCEL_INTERRUPT === "1";
      let statusInterruptionReleased = !interruptCancellationStatus;
      let startCalls = 0;
      connector.connectRuntimeSearch = async (...args: unknown[]) => {
        const connection = await originalConnect(...args);
        return { ...connection, request: async (operation: string, params: any) => {
          if (operation === "control.simulation.run.start") { startCalls += 1; entered(); await released; }
          if (!statusInterruptionReleased && operation === "control.simulation.run.status" && params?.job_id) {
            throw new Error("deliberate job-status interruption after cancellation acknowledgement");
          }
          return connection.request(operation, params);
        } };
      };
      try {
        await inner("d.querySelector('[data-engineer-find]').click();return 'find';");
        await entering;
        await inner("const h=d.querySelector('[data-result-history]');h.value=h.options[h.options.length-1].value;h.dispatchEvent(new d.defaultView.Event('change'));d.querySelector('[data-result-open]').click();return 'open';");
        await pollValue("Open rejected during active Find", () => inner("return d.querySelector('[data-result-state]').textContent;"),
          text => text.includes("retry Open"), 10000);
        assert.match(await inner("return d.querySelector('[data-result-state]').textContent;"), /cancel.*retained evidence/i);
        await inner("d.querySelector('[data-engineer-cancel]').click();return 'cancel';");
        release();
        if (interruptCancellationStatus) {
          await pollValue("unconfirmed cancellation", () => inner("return String(d.defaultView.__campaignMessages.at(-1)?.verificationAttempts?.[0]?.status ?? '');"),
            text => text === "cancellation-unconfirmed", 10000);
          const unconfirmed = JSON.parse(await inner("return JSON.stringify(d.defaultView.__campaignMessages.at(-1));"));
          const outcome = unconfirmed.verificationAttempts?.[0];
          assert.ok(outcome?.jobId, "Interrupted confirmation must retain the owned runtime job identity");
          assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-find]').disabled);"), "true",
            "Find stays disabled while cancellation confirmation is unconfirmed");
          await inner("[...d.querySelectorAll('[data-control-studio-mode]')].find(b=>b.textContent.trim()==='Validate').click();return 'inspect unconfirmed cancellation';");
          assert.match(await inner("return d.querySelector('[data-engineer-verification-history]')?.textContent??'';"), new RegExp(outcome.jobId),
            "Unconfirmed cancellation must remain inspectable before retry");
          await inner("d.querySelector('[data-result-save]').click();return 'save unconfirmed outcome';");
          await pollValue("saved unconfirmed outcome", () => inner("return d.querySelector('[data-result-state]').textContent;"), text => text.includes("Saved revision 2"), 10000);
          statusInterruptionReleased = true;
          await inner("d.querySelector('[data-engineer-cancel]').click();return 'retry cancellation confirmation';");
          await pollValue("confirmed cancellation", () => inner("return String(d.defaultView.__campaignMessages.at(-1)?.verificationAttempts?.[0]?.status ?? '');"),
            text => text === "cancelled" || text === "completed", 10000);
          const confirmed = JSON.parse(await inner("return JSON.stringify(d.defaultView.__campaignMessages.at(-1));"));
          assert.strictEqual(confirmed.verificationAttempts?.length, 1, "Confirmation retry must update the owned outcome without duplication");
          assert.strictEqual(confirmed.verificationAttempts?.[0]?.jobId, outcome.jobId, "Confirmation retry must retain the same runtime job");
          assert.strictEqual(startCalls, 1, "Confirmation retry must not start another actual runtime run");
          await inner("d.querySelector('[data-result-save]').click();return 'save confirmed outcome';");
          await pollValue("saved confirmed outcome", () => inner("return d.querySelector('[data-result-state]').textContent;"), text => text.includes("Saved revision 3"), 10000);
          await inner("const h=d.querySelector('[data-result-history]');h.value=h.options[h.options.length-1].value;h.dispatchEvent(new d.defaultView.Event('change'));return 'select confirmed outcome';");
          await inner("d.querySelector('[data-result-open]').click();return 'reopen confirmed outcome';");
          await pollValue("reopen confirmed historical study", () => inner("return d.querySelector('[data-engineer-status]').textContent;"),
            text => /historical|restored/i.test(text), 10000);
        } else {
        await pollValue("terminal cancelled campaign", () => inner("return String(d.defaultView.__campaignMessages.at(-1)?.status);"),
          text => text === "cancelled", 10000);
        const cancelled = JSON.parse(await inner("return JSON.stringify(d.defaultView.__campaignMessages.at(-1));"));
        assert.strictEqual(cancelled.campaigns.length, 3, "Rejected Open must preserve the active study and its cancelled campaign");
        const outcome = cancelled.verificationAttempts?.[0];
        assert.ok(outcome?.jobId, "Cancellation must retain the real runtime job identity");
        await inner("[...d.querySelectorAll('[data-control-studio-mode]')].find(b=>b.textContent.trim()==='Validate').click();return 'inspect retained cancellation';");
        assert.match(await inner("return d.querySelector('[data-engineer-verification-history]')?.textContent??'';"), new RegExp(outcome.jobId),
          "Cancelled runtime outcomes must be inspectable in Control Studio");
        await inner("d.querySelector('[data-result-save]').click();return 'save cancelled outcome';");
        await pollValue("saved cancelled outcome", () => inner("return d.querySelector('[data-result-state]').textContent;"), text => text.includes("Saved revision 2"), 10000);
        await inner("const h=d.querySelector('[data-result-history]');h.value='2';h.dispatchEvent(new d.defaultView.Event('change'));return 'select retained outcome';");

        await inner("d.querySelector('[data-result-open]').click();return 'retry open';");
        await pollValue("retry Open restores historical study", () => inner("return d.querySelector('[data-engineer-status]').textContent;"),
          text => /historical|restored/i.test(text), 10000);
        }
      } finally { release(); connector.connectRuntimeSearch = originalConnect; }
      const originalId = state.campaigns[0].runs[0].run.id;
      await vscode.commands.executeCommand("workbench.action.closeActiveEditor");
      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop",
        scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop });
      session = await attachControlStudio(connection, port);
      await pollValue("reopened failed evidence", () => inner("return d.querySelector('[data-engineer-campaign-history]')?.textContent ?? '';"),
        text => text.includes(originalId), 10000);
      assert.match(await inner("return d.querySelector('[data-engineer-status]').textContent;"), /historical|restored/i);
      await inner("[...d.querySelectorAll('[data-control-studio-mode]')].find(b=>b.textContent.trim()==='Validate').click();return 'review reopened outcomes';");
      assert.match(await inner("return d.querySelector('[data-engineer-verification-history]')?.textContent??'';"), /cancelled|completed|failed/,
        "Saved cancelled outcomes must survive panel close and reopening");
      assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-apply]').disabled);"), "true",
        "Reopening historical evidence cannot authorize Apply");

    } finally { connection.close(); await runtimeLifecycleService.stopRuntime(); }
  });
});
