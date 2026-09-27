import * as assert from "assert";
import { postEngineeringResult } from "../../controlStudio/panelWorkflowSupport";

suite("Control Studio stage error routing", () => {
  test("routes manual and imported tuning errors to the visible Tune status", async () => {
    let message: any;
    await postEngineeringResult({ webview: { postMessage: async (value: unknown) => { message = value; return true; } } } as any,
      { projectId: "project", id: "loop", revision: "rev" } as any, "editor", "tune-settings",
      { status: "failed", reason: "Controller instance is missing." });
    assert.strictEqual(message.stage, "tune", "Manual tuning errors must leave the Generating state and reach the Tune view");
    assert.strictEqual(message.payload.reason, "Controller instance is missing.");
  });
});
