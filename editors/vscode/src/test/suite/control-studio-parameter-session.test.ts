import * as assert from "assert";
import { ParameterApplicationClient } from "../../controlStudio/parameterApplication";
import type { ParameterBinding } from "../../controlStudio/parameterBinding";

suite("Control Studio generic parameter session", () => {
  test("retains the reviewed endpoint and invalidates review after an editor change", async () => {
    let api: any;
    try { api = require("../../controlStudio/parameterSession"); } catch { api = undefined; }
    const binding: ParameterBinding = { loop_id: "loop", task_name: "Task",
      parameters: [{ symbol: "P1.Gain", iec_type: "REAL", unit: "1", convention: "gain", minimum: 0, maximum: 20 }],
      mode: { symbol: "P1.Auto", manual_value: false } };
    const calls: string[] = [];
    const client = new ParameterApplicationClient(binding, "simulation", async operation => {
      calls.push(operation);
      return { program_revision: "program", binding_revision: "binding", active_revision: "active", values: { "P1.Gain": 1 },
        status: "idle", previous_revision: null, previous_values: null, reason: null };
    });
    const session = api ? new api.ParameterSession(binding, async () => ({ client, source: "simulation" })) : undefined;
    const scope = { projectId: "project", editorSessionId: "editor", loopId: "loop", loopRevision: "revision" };
    const message = (action: string, payload: object) => ({ schemaVersion: 1, kind: `control-studio.parameters.${action}`, messageId: "message",
      projectId: scope.projectId, editorSessionId: scope.editorSessionId, loopId: scope.loopId, expectedLoopRevision: scope.loopRevision, payload });
    const replies: any[] = [];
    const post = async (payload: unknown) => { replies.push(payload); };
    await session?.handle(message("review", { json: '{"P1.Gain":0.1}' }), scope, post);
    assert.strictEqual(replies[0]?.status, "reviewed", "the host must retain an exact parameter review");
    assert.deepStrictEqual(replies[0].values, { "P1.Gain": Math.fround(0.1) });
    const reviewId = replies[0].reviewId;
    await session.handle(message("invalidate", {}), scope, post);
    await session.handle(message("apply", { reviewId, physicalApproved: false }), scope, post);
    assert.strictEqual(replies[replies.length - 1].status, "failed");
    assert.deepStrictEqual(calls, ["control.parameters.status"], "invalidated reviews cannot write");
    const count = replies.length;
    await session.handle({ ...message("inspect", {}), expectedLoopRevision: "another" }, scope, post);
    assert.strictEqual(replies.length, count);
  });
});
