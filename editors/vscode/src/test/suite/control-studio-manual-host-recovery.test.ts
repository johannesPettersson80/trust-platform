import * as assert from "assert";

suite("Control Studio Manual host recovery", () => {
  test("waits for pending-owner storage before dispatching the native transaction", async () => {
    const persistence = require("../../controlStudio/panelPersistence");
    assert.strictEqual(typeof persistence.dispatchPendingManualApplication, "function",
      "Manual confirmation must await durable pending-owner storage before it can dispatch the native transaction.");
    const owner = { operation: "apply", origin: { program_revision: "program", binding_revision: "binding" } };
    const events: string[] = [];
    await persistence.dispatchPendingManualApplication(owner, async () => { events.push("stored"); }, async () => { events.push("native"); });
    assert.deepStrictEqual(events, ["stored", "native"]);
    await assert.rejects(() => persistence.dispatchPendingManualApplication(owner, async () => { throw new Error("storage unavailable"); },
      async () => { events.push("unexpected-native"); }), /storage unavailable/);
    assert.ok(!events.includes("unexpected-native"), "A failed owner write must reject the confirmation without dispatching native apply.");
  });

  test("persists a scoped pending Manual owner before terminal acknowledgement", async () => {
    const persistence = require("../../controlStudio/panelPersistence");
    assert.strictEqual(typeof persistence.storePendingManualApplicationRecovery, "function",
      "Closing a pending Manual transaction must synchronously request a host-lifetime owner before acknowledgement can settle.");
    const loop = { projectId: "project", runtimeId: "runtime", id: "loop", revision: "loop-revision" };
    const state = new Map<string, unknown>();
    const context = { workspaceState: {
      get: (key: string) => state.get(key),
      update: async (key: string, value: unknown) => { state.set(key, value); },
    } };
    const owner = {
      operation: "rollback" as const,
      origin: { program_revision: "program-revision", binding_revision: "binding-revision" },
    };

    await persistence.storePendingManualApplicationRecovery(context, loop, owner);
    const raw = [...state.values()][0];
    assert.deepStrictEqual(raw, {
      schemaVersion: 1,
      scope: { projectId: "project", runtimeId: "runtime", loopId: "loop", loopRevision: "loop-revision" },
      operation: "rollback",
      origin: { program_revision: "program-revision", binding_revision: "binding-revision" },
    });
    assert.doesNotMatch(JSON.stringify(raw), /review|token|client|credential|sample/i,
      "The host owner must not serialize a review capability, runtime client, credentials, or samples.");
    assert.deepStrictEqual(persistence.restoreManualApplicationRecovery(context, loop), owner);
    assert.strictEqual(persistence.restoreManualApplicationRecovery(context, { ...loop, revision: "changed" }), undefined,
      "A persisted owner cannot cross a loop revision boundary.");
    let reconciliations = 0;
    assert.strictEqual(await persistence.reconcileManualApplicationRecovery(context, loop, async (restored: typeof owner) => {
      reconciliations += 1;
      assert.deepStrictEqual(restored, owner);
      return false;
    }), false, "An unavailable or pending runtime must retain a retry-eligible owner.");
    assert.strictEqual(reconciliations, 1);
    assert.ok(persistence.restoreManualApplicationRecovery(context, loop),
      "An unresolved reconciliation retains its owner for the engineer's fresh review.");
    assert.strictEqual(await persistence.reconcileManualApplicationRecovery(context, loop, async () => true), true,
      "A terminal applied or rejected status completes reconciliation.");
    assert.strictEqual(persistence.restoreManualApplicationRecovery(context, loop), undefined);
  });
});
