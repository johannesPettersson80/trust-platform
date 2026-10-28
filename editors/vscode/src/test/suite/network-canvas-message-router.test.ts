import * as assert from "assert";

import { decodeCanvasMessage } from "../../networkCanvas/canvasMessageRouter";

suite("Network Canvas message routing", () => {
  test("accepts supported messages without rewriting their payload", () => {
    const input = { type: "search", query: "faulted" } as const;
    const decoded = decodeCanvasMessage(input);
    assert.strictEqual(decoded, input);
    assert.strictEqual(decoded?.query, "faulted");
    // The browse pane asks the server to check a typed ADS port (specification 25 §5.4).
    const port = { type: "checkAdsPort", requestId: 3, text: "301" } as const;
    assert.strictEqual(decodeCanvasMessage(port), port);
    // A runtime node's Identify (specification 25 §5.12).
    const identify = { type: "identifyRuntime", name: "line", endpoint: "tcp://10.0.0.9:5510" } as const;
    assert.strictEqual(decodeCanvasMessage(identify), identify);
    // The retired schema form's client-side field errors are no longer a message of the canvas.
    assert.strictEqual(decodeCanvasMessage({ type: "commApplyClientError" }), undefined);
  });

  test("routes saved plant removal without rewriting its payload", () => {
    const input = {
      type: "plantRemove",
      runtimeId: "runtime:local",
      plantId: "tank",
      expectedRevision: "sha256:saved",
    } as const;
    assert.strictEqual(decodeCanvasMessage(input), input);
  });

  test("rejects malformed and unsupported host messages", () => {
    assert.strictEqual(decodeCanvasMessage(undefined), undefined);
    assert.strictEqual(decodeCanvasMessage({}), undefined);
    assert.strictEqual(decodeCanvasMessage({ type: 42 }), undefined);
    assert.strictEqual(decodeCanvasMessage({ type: "not-a-canvas-command" }), undefined);
  });

  test("accepts only identity and revision fields for a plant Control Studio request", () => {
    let decodeOpenControlStudioMessage: ((value: unknown) => unknown) | undefined;
    try {
      decodeOpenControlStudioMessage = require("../../networkCanvas/canvasMessageRouter")
        .decodeOpenControlStudioMessage;
    } catch {
      decodeOpenControlStudioMessage = undefined;
    }
    const request = {
      type: "openControlStudio",
      projectId: "project:sha256:abc",
      runtimeId: "runtime:local",
      plantId: "tank",
      savedConfigurationRevision: "sha256:saved",
      activeConfigurationDigest: "sha256:active",
    } as const;
    assert.deepStrictEqual(decodeOpenControlStudioMessage?.(request), request);
    assert.strictEqual(decodeOpenControlStudioMessage?.({ ...request, liveValues: [1, 2] }), undefined);
    assert.strictEqual(decodeOpenControlStudioMessage?.({ ...request, savedConfigurationRevision: "stale" }), undefined);
  });
});
