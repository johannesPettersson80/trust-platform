import * as assert from "assert";
import { decodeReviewedStudioApplicationCommand, ReviewedStudioApplicationCoordinator } from "../../controlStudio/reviewedStudioApplication";

const scope = { projectId: "project", editorSessionId: "editor", loopId: "loop", loopRevision: "revision" };
const base = { schemaVersion: 1, projectId: "project", editorSessionId: "editor", loopId: "loop", expectedLoopRevision: "revision" };

suite("Control Studio reviewed application orchestration", () => {
  test("strictly decodes scoped single-click Apply and Restore commands", () => {
    assert.deepStrictEqual(decodeReviewedStudioApplicationCommand({ ...base, kind: "control-studio.application.apply", messageId: "apply", payload: {} }, scope),
      { action: "apply", messageId: "apply" });
    assert.deepStrictEqual(decodeReviewedStudioApplicationCommand({ ...base, kind: "control-studio.application.restore", messageId: "restore", payload: {} }, scope),
      { action: "restore", messageId: "restore" });
    assert.strictEqual(decodeReviewedStudioApplicationCommand({ ...base, kind: "control-studio.application.apply", messageId: "wrong", payload: { extra: true } }, scope), undefined);
    assert.strictEqual(decodeReviewedStudioApplicationCommand({ ...base, kind: "control-studio.application.apply", messageId: "wrong", projectId: "other", payload: {} }, scope), undefined);
  });

  test("reviews and confirms exactly one Apply request, marking both normal payloads", async () => {
    const calls: string[] = [];
    const posts: Record<string, unknown>[] = [];
    const engineer = { handle: async (message: any, _scope: any, post: (payload: Record<string, unknown>) => Promise<void>) => {
      calls.push(message.kind);
      await post(message.kind.endsWith("apply") ? { status: "review", rollback: false } : { status: "applied", observed: { activeRevision: "new" } });
      return true;
    } } as any;
    const coordinator = new ReviewedStudioApplicationCoordinator();
    assert.strictEqual(await coordinator.handle({ ...base, kind: "control-studio.application.apply", messageId: "request", payload: {} }, { engineer, scope, post: async payload => { posts.push(payload); } }), true);
    assert.deepStrictEqual(calls, ["control-studio.engineer.apply", "control-studio.engineer.confirm"]);
    assert.deepStrictEqual(posts, [
      { status: "review", rollback: false, applicationAction: "apply" },
      { status: "applied", observed: { activeRevision: "new" }, applicationAction: "apply" },
    ]);
  });

  test("does not confirm a Restore request when the returned review is for Apply", async () => {
    const calls: string[] = [];
    const posts: Record<string, unknown>[] = [];
    const engineer = { handle: async (message: any, _scope: any, post: (payload: Record<string, unknown>) => Promise<void>) => {
      calls.push(message.kind);
      await post({ status: "review", rollback: false });
      return true;
    } } as any;
    const coordinator = new ReviewedStudioApplicationCoordinator();
    await coordinator.handle({ ...base, kind: "control-studio.application.restore", messageId: "restore", payload: {} }, { engineer, scope, post: async payload => { posts.push(payload); } });
    assert.deepStrictEqual(calls, ["control-studio.engineer.rollback"]);
    assert.deepStrictEqual(posts, [
      { status: "review", rollback: false, applicationAction: "restore" },
      { status: "failed", reason: "The restore review was not available.", applicationAction: "restore" },
    ]);
  });

  test("deduplicates a completed request and suppresses late confirmation after disposal", async () => {
    const calls: string[] = [];
    const posts: Record<string, unknown>[] = [];
    let release!: () => void;
    const engineer = { handle: async (message: any, _scope: any, post: (payload: Record<string, unknown>) => Promise<void>) => {
      calls.push(message.kind);
      if (message.kind.endsWith("apply")) await post({ status: "review", rollback: false });
      else await new Promise<void>(resolve => { release = resolve; });
      return true;
    } } as any;
    const coordinator = new ReviewedStudioApplicationCoordinator();
    const pending = coordinator.handle({ ...base, kind: "control-studio.application.apply", messageId: "late", payload: {} }, { engineer, scope, post: async payload => { posts.push(payload); } });
    await new Promise(resolve => setImmediate(resolve));
    coordinator.dispose(); release(); await pending;
    assert.deepStrictEqual(calls, ["control-studio.engineer.apply", "control-studio.engineer.confirm"]);
    assert.deepStrictEqual(posts, [{ status: "review", rollback: false, applicationAction: "apply" }]);
    assert.strictEqual(await coordinator.handle({ ...base, kind: "control-studio.application.apply", messageId: "late", payload: {} }, { engineer, scope, post: async () => {} }), true);
    assert.strictEqual(calls.length, 2, "a disposed request cannot repeat the runtime operation");
  });

  test("blocks a distinct request while the first request is awaiting confirmation", async () => {
    let release!: () => void;
    const calls: string[] = [];
    const posts: Record<string, unknown>[] = [];
    const engineer = { handle: async (message: any, _scope: any, post: (payload: Record<string, unknown>) => Promise<void>) => {
      calls.push(message.messageId);
      if (message.kind.endsWith("apply")) await post({ status: "review", rollback: false });
      else await new Promise<void>(resolve => { release = resolve; });
      return true;
    } } as any;
    const coordinator = new ReviewedStudioApplicationCoordinator();
    const owner = { engineer, scope, post: async (payload: Record<string, unknown>) => { posts.push(payload); } };
    const first = coordinator.handle({ ...base, kind: "control-studio.application.apply", messageId: "first", payload: {} }, owner);
    await new Promise(resolve => setImmediate(resolve));
    await coordinator.handle({ ...base, kind: "control-studio.application.restore", messageId: "second", payload: {} }, owner);
    assert.deepStrictEqual(posts[posts.length - 1], { status: "busy", reason: "Another application request is awaiting confirmation.", applicationAction: "restore" });
    release(); await first;
    assert.deepStrictEqual(calls, ["first:review", "first:confirm"]);
  });

  test("preserves an underlying recovery-required review outcome", async () => {
    const posts: Record<string, unknown>[] = [];
    const engineer = { handle: async (_message: any, _scope: any, post: (payload: Record<string, unknown>) => Promise<void>) => {
      await post({ status: "recovery-required", reason: "The runtime acknowledgement is unresolved." });
      return true;
    } } as any;
    const coordinator = new ReviewedStudioApplicationCoordinator();
    await coordinator.handle({ ...base, kind: "control-studio.application.apply", messageId: "recovery", payload: {} }, {
      engineer, scope, post: async payload => { posts.push(payload); },
    });
    assert.deepStrictEqual(posts, [{ status: "recovery-required", reason: "The runtime acknowledgement is unresolved.", applicationAction: "apply" }]);
  });

  test("the existing StudioAnalysisSessions router does not yet claim the new application boundary", async () => {
    const { StudioAnalysisSessions } = require("../../controlStudio/studioAnalysisSessions");
    const { pidLoop } = require("./control-studio-pid-fixtures");
    const sessions = new StudioAnalysisSessions(pidLoop());
    const handled = await sessions.handle({ ...base, kind: "control-studio.application.apply", messageId: "router", payload: {} }, scope,
      async () => {});
    assert.strictEqual(handled, true, "the application boundary must be routed through the shared session owner");
  });
});
