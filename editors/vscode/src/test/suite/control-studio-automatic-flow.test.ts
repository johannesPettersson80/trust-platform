import * as assert from "assert";
import { StudioAnalysisSessions } from "../../controlStudio/studioAnalysisSessions";

const scope = {
  projectId: "project:flow",
  editorSessionId: "editor:flow",
  loopId: "loop:flow",
  loopRevision: "sha256:flow",
};

suite("Control Studio automatic three-page flow", () => {
  function fixture() {
    const sessions = new StudioAnalysisSessions();
    const messages: any[] = [];
    let releaseFind: (() => void) | undefined;
    let findBlocked = false;
    const engineer = {
      dispose() {},
      snapshotStudy() { return { task }; },
      async handle(message: any, _scope: any, post: (payload: any) => Promise<void>) {
        messages.push(message);
        const action = String(message.kind).split(".").pop();
        if (action === "find" && findBlocked) await new Promise<void>(resolve => { releaseFind = resolve; });
        if (action === "find" || action === "recording") await post({ status: "model-ready", model: { revision: "model-1" } });
        if (action === "tune") await post({ status: "candidate", candidate: { applied: false } });
        if (action === "verify") await post({ status: "verified", candidate: { evaluation: { status: "Pass" } } });
        if (action === "cancel" && releaseFind) releaseFind();
        return true;
      },
    };
    (sessions as any).engineer = engineer;
    return { sessions, messages, blockFind() { findBlocked = true; } };
  }

  const task = { referenceChange: 3, maximumError: .2, settlingBand: .05, settlingTimeSeconds: 5, durationSeconds: 30 };

  test("handles a scoped start and pauses with a specific missing-controller stage", async () => {
    const sessions = new StudioAnalysisSessions();
    const posted: Array<{ stage?: string; status?: string; reason?: string }> = [];
    try {
      const handled = await sessions.handle({
        schemaVersion: 1,
        kind: "control-studio.flow.start",
        messageId: "flow-start-1",
        projectId: scope.projectId,
        editorSessionId: scope.editorSessionId,
        loopId: scope.loopId,
        expectedLoopRevision: scope.loopRevision,
        payload: {},
      }, scope, async (stage, payload) => { posted.push({ stage, status: String(payload.status ?? ""), reason: String(payload.reason ?? "") }); });

      assert.strictEqual(handled, true);
      assert.deepStrictEqual(posted, [
        { stage: "engineer", status: "running", reason: "Starting Identify." },
        { stage: "flow", status: "paused", reason: "No configured analysis session is available." },
      ]);
    } finally {
      sessions.dispose();
    }
  });

  test("identifies and tunes separately; validation requires its own explicit request", async () => {
    const f = fixture(); const posted: any[] = [];
    try {
      const handled = await f.sessions.handle({ schemaVersion: 1, kind: "control-studio.flow.start", messageId: "sequence-1",
        ...scope, expectedLoopRevision: scope.loopRevision, payload: { task, family: "PI", applicationMode: "manual" } }, scope,
      async (stage, payload) => { posted.push({ stage, ...payload }); });
      assert.strictEqual(handled, true);
      assert.deepStrictEqual(f.messages.map(message => message.kind), ["control-studio.engineer.find"], "Identification must stop for model review even with saved requirements");
      assert.strictEqual(posted[posted.length - 1].flowStage, "Identify");
      assert.strictEqual(posted[posted.length - 1].state, "completed");
      await f.sessions.handle({ schemaVersion: 1, kind: "control-studio.flow.start", messageId: "sequence-tune",
        ...scope, expectedLoopRevision: scope.loopRevision, payload: { from: "tune", task, family: "PI", applicationMode: "manual" } }, scope,
      async (stage, payload) => { posted.push({ stage, ...payload }); });
      assert.deepStrictEqual(f.messages.map(message => message.kind), [
        "control-studio.engineer.find", "control-studio.engineer.tune",
      ]);
      assert.strictEqual(f.messages.some(message => message.kind.endsWith("apply")), false);
      assert.strictEqual(posted[posted.length - 1].state, "completed");
      assert.strictEqual(posted[posted.length - 1].flowStage, "Tune");
      await f.sessions.handle({ schemaVersion: 1, kind: "control-studio.flow.start", messageId: "sequence-validate",
        ...scope, expectedLoopRevision: scope.loopRevision, payload: { from: "verify" } }, scope,
      async (stage, payload) => { posted.push({ stage, ...payload }); });
      assert.strictEqual(f.messages[f.messages.length - 1].kind, "control-studio.engineer.verify");
      assert.strictEqual(posted.filter(value => value.stage === "engineer" && value.status === "model-ready").length, 1, "Forward the actual model result to the webview once");
      assert.strictEqual(posted.filter(value => value.stage === "general-model" && value.status === "imported").length, 1, "Use the existing selected-model handoff");
      assert.strictEqual(posted.filter(value => value.stage === "engineer" && value.status === "candidate").length, 1);
    } finally { f.sessions.dispose(); }
  });

  test("leaves identification on its result without asking for Tune requirements", async () => {
    const f = fixture(); const posted: Record<string, unknown>[] = [];
    try {
      await f.sessions.handle({ schemaVersion: 1, kind: "control-studio.flow.start", messageId: "identify-without-task", ...scope,
        expectedLoopRevision: scope.loopRevision, payload: { from: "find", task: {} } }, scope, async (stage, payload) => { posted.push({ stage, ...payload }); });
      assert.deepStrictEqual(f.messages.map(message => message.kind), ["control-studio.engineer.find"]);
      assert.strictEqual(posted[posted.length - 1].flowStage, "Identify");
      assert.strictEqual(posted[posted.length - 1].state, "completed");
      assert.strictEqual(posted.some(value => value.missing), false);
    } finally { f.sessions.dispose(); }
  });

  for (const compatibleControllerRequired of [false, true]) test(`recording stops for review with compatibleControllerRequired=${compatibleControllerRequired}`, async () => {
    const f = fixture(); const posted: any[] = [];
    (f.sessions as any).engineer.handle = async (message: any, _scope: any, post: (payload: any) => Promise<void>) => {
      f.messages.push(message);
      await post({ status: "model-ready", model: { revision: "recorded-model" }, compatibleControllerRequired });
      return true;
    };
    try {
      await f.sessions.handle({ schemaVersion: 1, kind: "control-studio.flow.start", messageId: "recording-review", ...scope,
        expectedLoopRevision: scope.loopRevision, payload: { from: "recording", json: "{}", task } }, scope,
      async (stage, payload) => { posted.push({ stage, ...payload }); });
      assert.deepStrictEqual(f.messages.map(message => message.kind), ["control-studio.engineer.recording"]);
      assert.strictEqual(posted[posted.length - 1].flowStage, "Identify");
      assert.strictEqual(posted[posted.length - 1].state, "completed");
      assert.strictEqual(posted.some(value => value.missing), false, "Controller selection belongs to the user's next visit to Tune");
    } finally { f.sessions.dispose(); }
  });

  test("pauses at the failed stage and does not continue", async () => {
    const f = fixture(); const posted: any[] = [];
    (f.sessions as any).engineer.handle = async (message: any, _scope: any, post: (payload: any) => Promise<void>) => {
      f.messages.push(message); await post({ status: "failed", reason: "requirements rejected" }); return true;
    };
    try {
      await f.sessions.handle({ schemaVersion: 1, kind: "control-studio.flow.start", messageId: "failure-1", ...scope,
        expectedLoopRevision: scope.loopRevision, payload: { task } }, scope, async (stage, payload) => { posted.push({ stage, ...payload }); });
      assert.deepStrictEqual(f.messages.map(message => message.kind), ["control-studio.engineer.find"]);
      assert.strictEqual(posted[posted.length - 1].state, "paused");
      assert.match(posted[posted.length - 1].reason, /requirements rejected/);
    } finally { f.sessions.dispose(); }
  });

  test("cancels a running generation and suppresses its stale continuation", async () => {
    const f = fixture(); const posted: any[] = [];
    f.blockFind();
    const start = f.sessions.handle({ schemaVersion: 1, kind: "control-studio.flow.start", messageId: "cancel-1", ...scope,
      expectedLoopRevision: scope.loopRevision, payload: { task } }, scope, async (stage, payload) => { posted.push({ stage, ...payload }); });
    await new Promise(resolve => setTimeout(resolve, 0));
    await f.sessions.handle({ schemaVersion: 1, kind: "control-studio.flow.cancel", messageId: "cancel-2", ...scope,
      expectedLoopRevision: scope.loopRevision, payload: {} }, scope, async (stage, payload) => { posted.push({ stage, ...payload }); });
    await start;
    assert.strictEqual(f.messages.some(message => message.kind === "control-studio.engineer.tune"), false);
    assert.strictEqual(posted.filter(value => value.flowStage === "Tune").length, 0);
    assert.strictEqual(posted[posted.length - 1].state, "paused");
    f.sessions.dispose();
  });

  test("rejects malformed start stages, reports duplicate requests as busy, and reuses task on restart", async () => {
    const f = fixture(); const posted: any[] = [];
    try {
      await f.sessions.handle({ schemaVersion: 1, kind: "control-studio.flow.start", messageId: "bad-stage", ...scope,
        expectedLoopRevision: scope.loopRevision, payload: { from: "unknown", task } }, scope, async (stage, payload) => { posted.push({ stage, ...payload }); });
      assert.strictEqual(posted[posted.length - 1].state, "paused");
      const duplicate = f.sessions.handle({ schemaVersion: 1, kind: "control-studio.flow.start", messageId: "bad-stage", ...scope,
        expectedLoopRevision: scope.loopRevision, payload: { task } }, scope, async (stage, payload) => { posted.push({ stage, ...payload }); });
      await duplicate;
      assert.strictEqual(posted[posted.length - 1].status, "busy");
      await f.sessions.handle({ schemaVersion: 1, kind: "control-studio.flow.start", messageId: "task-seed", ...scope,
        expectedLoopRevision: scope.loopRevision, payload: { from: "find", task } }, scope, async (stage, payload) => { posted.push({ stage, ...payload }); });
      await f.sessions.handle({ schemaVersion: 1, kind: "control-studio.flow.start", messageId: "task-reuse", ...scope,
        expectedLoopRevision: scope.loopRevision, payload: { from: "tune" } }, scope, async (stage, payload) => { posted.push({ stage, ...payload }); });
      assert.strictEqual(f.messages.some(message => message.kind === "control-studio.engineer.tune"), true);
      const tuneCount = f.messages.filter(message => message.kind.endsWith(".tune")).length;
      await f.sessions.handle({ schemaVersion: 1, kind: "control-studio.flow.start", messageId: "invalid-task", ...scope,
        expectedLoopRevision: scope.loopRevision, payload: { from: "tune", task: {} } }, scope, async (stage, payload) => { posted.push({ stage, ...payload }); });
      assert.strictEqual(f.messages.filter(message => message.kind.endsWith(".tune")).length, tuneCount + 1);
      assert.strictEqual(f.messages[f.messages.length - 1].payload.automaticRequirements, true);
      assert.strictEqual(posted[posted.length - 1].state, "completed");
    } finally { f.sessions.dispose(); }
  });

  test("does not restart an active generation", async () => {
    const f = fixture(); const posted: any[] = [];
    f.blockFind();
    try {
      const first = f.sessions.handle({ schemaVersion: 1, kind: "control-studio.flow.start", messageId: "active-1", ...scope,
        expectedLoopRevision: scope.loopRevision, payload: { task } }, scope, async (stage, payload) => { posted.push({ stage, ...payload }); });
      await new Promise(resolve => setTimeout(resolve, 0));
      await f.sessions.handle({ schemaVersion: 1, kind: "control-studio.flow.start", messageId: "active-2", ...scope,
        expectedLoopRevision: scope.loopRevision, payload: { task } }, scope, async (stage, payload) => { posted.push({ stage, ...payload }); });
      assert.strictEqual(posted[posted.length - 1].status, "busy");
      await f.sessions.handle({ schemaVersion: 1, kind: "control-studio.flow.cancel", messageId: "active-cancel", ...scope,
        expectedLoopRevision: scope.loopRevision, payload: {} }, scope, async (stage, payload) => { posted.push({ stage, ...payload }); });
      await first;
    } finally { f.sessions.dispose(); }
  });

  test("retains recovery-required cancellation without announcing abort", async () => {
    const f = fixture(); const posted: any[] = [];
    let release: (() => void) | undefined;
    (f.sessions as any).engineer.handle = async (message: any, _scope: any, post: (payload: any) => Promise<void>) => {
      f.messages.push(message);
      if (message.kind.endsWith(".find")) {
        await new Promise<void>(resolve => { release = resolve; });
        await post({ status: "model-ready" });
      }
      if (message.kind.endsWith(".cancel")) {
        await post({ status: "recovery-required", reason: "cancel confirmation unavailable" });
        release?.();
      }
      return true;
    };
    try {
      const first = f.sessions.handle({ schemaVersion: 1, kind: "control-studio.flow.start", messageId: "recovery-1", ...scope,
        expectedLoopRevision: scope.loopRevision, payload: { task } }, scope, async (stage, payload) => { posted.push({ stage, ...payload }); });
      await new Promise(resolve => setTimeout(resolve, 0));
      await f.sessions.handle({ schemaVersion: 1, kind: "control-studio.flow.cancel", messageId: "recovery-cancel", ...scope,
        expectedLoopRevision: scope.loopRevision, payload: {} }, scope, async (stage, payload) => { posted.push({ stage, ...payload }); });
      await first;
      assert.match(String(posted[posted.length - 1].reason), /recovery|confirmation/i);
      assert.strictEqual(posted.some(value => value.reason === "Automatic flow aborted."), false);
    } finally { f.sessions.dispose(); }
  });
});
