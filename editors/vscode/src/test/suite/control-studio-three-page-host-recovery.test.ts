import * as assert from "assert";
import { EngineerWorkflowSession } from "../../controlStudio/engineerWorkflowSession";
import { pidLoop } from "./control-studio-pid-fixtures";

type Payload = Record<string, unknown>;

const parameterValues = (value: number) => ({ "Main.Kp": value, "Main.Ki": 0, "Main.Kd": 0 });

function status(overrides: Partial<{
  status: "idle" | "pending" | "applied" | "rejected";
  transaction_id: string | null;
  active_revision: string;
  values: Record<string, number>;
  previous_revision: string | null;
  previous_values: Record<string, number> | null;
  reason: string | null;
}> = {}) {
  return {
    program_revision: "program-revision", binding_revision: "binding-revision",
    active_revision: "active-revision", values: parameterValues(1), status: "applied" as const,
    previous_revision: null, previous_values: null, reason: null, transaction_id: "transaction", ...overrides,
  };
}

function owner(operation: "apply" | "rollback", transaction_id = "transaction") {
  return {
    operation,
    origin: {
      program_revision: "program-revision", binding_revision: "binding-revision",
      review: {
        transaction_id, active_revision: "active-revision", previous_revision: "previous-revision",
        values: parameterValues(1), requested: parameterValues(operation === "rollback" ? 1 : 2),
      },
    },
  } as any;
}

async function reconcile(recovery: any, current: any, cancelOwner?: () => Promise<string>) {
  const parameters = require("../../controlStudio/runtimeParameterConnection");
  const previous = parameters.connectRuntimeParameters;
  parameters.connectRuntimeParameters = async () => ({ source: "simulation", client: {
    status: async () => current,
    ...(cancelOwner ? { cancelOwner } : {}),
  } });
  const session = new EngineerWorkflowSession(pidLoop(), () => undefined, () => undefined);
  const posts: Payload[] = [];
  try {
    const resolved = await session.reconcileManualApplicationRecovery(recovery, async payload => { posts.push(payload); });
    return { resolved, posts };
  } finally {
    session.dispose();
    parameters.connectRuntimeParameters = previous;
  }
}

suite("Control Studio typed host recovery", () => {
  test("marks observed applied and rejected results resolved with their application action", async () => {
    const applied = await reconcile(owner("apply"), status({
      active_revision: "candidate-revision", values: parameterValues(2),
      previous_revision: "active-revision", previous_values: parameterValues(1),
    }));
    assert.strictEqual(applied.resolved, true);
    assert.strictEqual(applied.posts[0].recoveryResolved, true);
    assert.strictEqual(applied.posts[0].applicationAction, "apply");

    const rejected = await reconcile(owner("apply"), status({ status: "rejected", reason: "rejected by runtime" }));
    assert.strictEqual(rejected.resolved, true);
    assert.strictEqual(rejected.posts[0].recoveryResolved, true);
    assert.strictEqual(rejected.posts[0].applicationAction, "apply");
  });

  test("marks an observed rollback resolved as Restore and resolves cancellation before dispatch", async () => {
    const restored = await reconcile(owner("rollback"), status({
      active_revision: "previous-revision", values: parameterValues(1),
    }));
    assert.strictEqual(restored.resolved, true);
    assert.strictEqual(restored.posts[0].recoveryResolved, true);
    assert.strictEqual(restored.posts[0].applicationAction, "restore");

    const cancelled = await reconcile(owner("rollback"), status({ status: "pending", transaction_id: "transaction" }),
      async () => "before-dispatch");
    assert.strictEqual(cancelled.resolved, true);
    assert.strictEqual(cancelled.posts[0].recoveryResolved, true);
    assert.strictEqual(cancelled.posts[0].applicationAction, "restore");
  });

  test("keeps an unresolved terminal record retryable without replaying it", async () => {
    const unresolved = await reconcile(owner("apply"), status({
      active_revision: "different-revision", values: parameterValues(99),
    }));
    assert.strictEqual(unresolved.resolved, false);
    assert.strictEqual(unresolved.posts[0].status, "recovery-required");
    assert.strictEqual(unresolved.posts[0].retryManualRecovery, true);
  });

  test("marks a changed runtime epoch as an explicitly typed replacement review", async () => {
    const parameters = require("../../controlStudio/runtimeParameterConnection");
    const previous = parameters.connectRuntimeParameters;
    parameters.connectRuntimeParameters = async () => ({ source: "simulation", client: {
      status: async () => status({ active_revision: "replacement-active" }),
    } });
    const loop = pidLoop();
    const session = new EngineerWorkflowSession(loop, () => undefined, () => undefined);
    (session as any).state.interruptedCheckpoint = {
      id: "checkpoint", loopRevision: loop.revision, stage: "parameters",
      origin: { ...status(), program_revision: "old-program" },
      model: {}, task: {}, candidate: parameterValues(2), parameterOperation: {
        id: "operation", program: "old-program", binding: "binding-revision",
      }, references: {}, binding: {}, recoveryTracking: {}, tolerance: 0.1,
    };
    const scope = { projectId: loop.projectId, editorSessionId: "recovery", loopId: loop.id, loopRevision: loop.revision };
    const posts: Payload[] = [];
    try {
      const handled = await session.handle({ schemaVersion: 1,
        kind: "control-studio.engineer.review-runtime", messageId: "review-runtime",
        projectId: scope.projectId, editorSessionId: scope.editorSessionId, loopId: scope.loopId,
        expectedLoopRevision: scope.loopRevision, payload: {} }, scope,
      async payload => { posts.push(payload); });
      assert.strictEqual(handled, true);
      assert.strictEqual(posts[0].status, "review");
      assert.strictEqual(posts[0].runtimeReplacementReview, true);
      assert.strictEqual(posts[0].replacementRuntimeAvailable, true);
    } finally {
      session.dispose();
      parameters.connectRuntimeParameters = previous;
    }
  });
});
