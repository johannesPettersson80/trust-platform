import { validateToolInput } from "../../hmiBuilder/agent/schema";
import * as assert from "assert";
import { invokeAuthoring, AuthoringResult } from "../../hmiBuilder/agent/boundary";

suite("HMI agent request boundary", () => {
  const valid = { contractVersion: 1, workspaceId: "window-project-1", input: { page: "overview" } };
  const success: AuthoringResult = {
    ok: true,
    data: { title: "Overview" },
    revisions: { page: "hash-1" },
    warnings: [],
  };
  async function refused(request: unknown, code: string, aborted = false) {
    const controller = new AbortController();
    if (aborted) controller.abort();
    let calls = 0;
    const result = await invokeAuthoring(
      request,
      { workspaceId: valid.workspaceId, signal: controller.signal },
      async () => {
        calls++;
        return success;
      },
    );
    assert.strictEqual(result.ok, false, JSON.stringify(result));
    assert.ok(!result.ok && result.code === code, JSON.stringify(result));
    assert.strictEqual(calls, 0, "invalid request must not reach the authoring operation");
  }
  test("forwards only validated input with session authority", async () => {
    const session = { workspaceId: valid.workspaceId, signal: new AbortController().signal };
    const result = await invokeAuthoring(valid, session, async (input, received) => {
      assert.deepStrictEqual(input, valid.input);
      assert.strictEqual(received, session);
      return success;
    });
    assert.deepStrictEqual(result, success);
  });
  test("unsupported contract version is refused before an operation", async () =>
    refused({ ...valid, contractVersion: 2 }, "UNSUPPORTED_VERSION"));
  test("caller cannot choose another workspace", async () =>
    refused({ ...valid, workspaceId: "other-project" }, "PROJECT_UNAVAILABLE"));
  test("cancelled requests never reach an operation", async () => refused(valid, "CANCELLED", true));
  test("invalid envelopes and non-object inputs are rejected", async () => {
    for (const request of [null, [], {}, { ...valid, input: [] }, { ...valid, input: null }, { ...valid, extra: true }])
      await refused(request, "INVALID_ARGUMENT");
  });
  test("oversized requests are rejected before an operation", async () =>
    refused({ ...valid, input: { text: "x".repeat(8 * 1024 * 1024) } }, "LIMIT_EXCEEDED"));
  test("non-JSON inputs are rejected rather than partially serialized", async () => {
    const cycle: Record<string, unknown> = {};
    cycle.self = cycle;
    for (const input of [cycle, { value: Infinity }, { value: undefined }])
      await refused({ ...valid, input }, "INVALID_ARGUMENT");
  });
  test("malformed edits report the selected operation field without an unrelated schema dump", () => {
    assert.throws(
      () =>
        validateToolInput("hmi_edit_draft", {
          draftId: "fixture",
          revision: "1",
          key: "edit",
          operations: [{ op: "reroute", from: "a.outlet", to: "b.inlet", waypoints: "bad" }],
        }),
      (error: unknown) =>
        error instanceof Error &&
        error.message.length < 500 &&
        error.message.includes("waypoints") &&
        error.message.includes("array"),
    );
    assert.throws(
      () =>
        validateToolInput("hmi_edit_draft", {
          draftId: "fixture",
          revision: "1",
          key: "edit",
          operations: [{ op: "not-an-operation" }],
        }),
      (error: unknown) =>
        error instanceof Error && error.message.length < 500 && error.message.includes("not-an-operation"),
    );
  });
});
