import * as assert from "assert";

import { decodeCanvasMessage } from "../../networkCanvas/canvasMessageRouter";

suite("Network Canvas message routing", () => {
  test("accepts supported messages without rewriting their payload", () => {
    const input = { type: "search", query: "faulted" } as const;
    const decoded = decodeCanvasMessage(input);
    assert.strictEqual(decoded, input);
    assert.strictEqual(decoded?.query, "faulted");
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
