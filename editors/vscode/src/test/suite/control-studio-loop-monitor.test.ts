import * as assert from "assert";

// The host builds the page's loop monitor message (the messages module's loopMonitorEnvelope) from the engine's sample. The rules that
// decide the sample are Rust tests (crates/trust-control/src/loop_monitor_tests.rs).
suite("Control Studio live loop monitor messages", () => {
  test("builds the loop monitor message with its session scope and the sample unchanged", () => {
    const messages = require("../../controlStudio/messages") as any;
    const expected = {
      projectId: "project:sha256:abc",
      editorSessionId: "session-1",
      loopId: "loop:tank",
      loopRevision: "sha256:loop-1",
    };
    const sample = tankSample();
    const envelope = messages.loopMonitorEnvelope(expected, "message-1", sample);
    assert.deepStrictEqual(envelope, {
      schemaVersion: 1,
      kind: "loop.monitor.snapshot",
      messageId: "message-1",
      projectId: expected.projectId,
      editorSessionId: expected.editorSessionId,
      loopId: expected.loopId,
      expectedLoopRevision: expected.loopRevision,
      payload: sample,
    });
  });
});

// The page's messages to the host carry an envelope of the panel's scope. The panel routes a message to itself by that scope and reads the
// page's ready message and its two host commands from it; it decides nothing about the loop.
suite("Control Studio page message envelope", () => {
  const scope = { projectId: "project:sha256:abc", editorSessionId: "session-1", loopId: "loop:tank", loopRevision: "sha256:loop-1" };
  const messages = () => require("../../controlStudio/messages") as any;
  /** A page message of the panel's scope: exactly the envelope's keys, with the fields of `overrides` replaced. */
  const envelope = (kind: string, payload: Record<string, unknown>, overrides: Record<string, unknown> = {}) => ({
    schemaVersion: 1, kind, messageId: "message-1", projectId: scope.projectId, editorSessionId: scope.editorSessionId,
    loopId: scope.loopId, expectedLoopRevision: scope.loopRevision, payload, ...overrides,
  });

  test("the page's ready message is an envelope of the panel's scope with an empty payload", () => {
    const { decodeLoopReady } = messages();
    assert.strictEqual(decodeLoopReady(envelope("control-studio.ready", {}), scope), true);
    assert.strictEqual(decodeLoopReady(envelope("control-studio.ready", { extra: 1 }), scope), false, "a ready message carries no payload");
    assert.strictEqual(decodeLoopReady(envelope("control-studio.ready", {}, { loopId: "loop:other" }), scope), false, "a message of another loop is not this panel's");
    assert.strictEqual(decodeLoopReady(envelope("control-studio.ready", {}, { expectedLoopRevision: "sha256:other" }), scope), false);
    assert.strictEqual(decodeLoopReady({ ...envelope("control-studio.ready", {}), extra: true }, scope), false, "the envelope has exactly its keys");
    assert.strictEqual(decodeLoopReady(envelope("control-studio.flow.start", {}), scope), false, "another kind is not the ready message");
    assert.strictEqual(decodeLoopReady(envelope("control-studio.ready", {}, { messageId: "" }), scope), false, "a message names itself");
    assert.strictEqual(decodeLoopReady(undefined, scope), false);
  });

  test("a host command names one of the two commands the page may ask the extension for, and nothing else", () => {
    const { decodeControlStudioHostCommand } = messages();
    const command = (value: unknown, overrides: Record<string, unknown> = {}) =>
      decodeControlStudioHostCommand(envelope("control-studio.host-command", { command: value }, overrides), scope);
    assert.strictEqual(command("connect-runtime"), "connect-runtime");
    assert.strictEqual(command("open-loop-setup"), "open-loop-setup");
    assert.strictEqual(command("format-disk"), undefined);
    assert.strictEqual(command("connect-runtime", { projectId: "project:other" }), undefined, "a message of another project is not this panel's");
    assert.strictEqual(decodeControlStudioHostCommand(envelope("control-studio.host-command", { command: "connect-runtime", extra: 1 }), scope), undefined,
      "the payload is the command alone");
  });
});

/** One available sample of the tank loop, with the fields the engine answers (an absent field is left out, not null). */
function tankSample(overrides: Record<string, unknown> = {}): any {
  return {
    availability: "available",
    source: "Simulation",
    loopId: "loop:tank",
    loopRevision: "sha256:loop-1",
    controller: { instance: "TankController", revision: "sha256:controller-1", mode: "Auto" },
    scan: 41,
    observedAtMs: 1_000,
    setpoint: signal("LevelSP", "%ML0", 50),
    processVariable: signal("LevelPV", "%IL0", 21.5),
    manipulatedVariable: signal("ValveMV", "%QL0", 36),
    outputLimits: { minimum: 0, maximum: 100 },
    rateLimits: { upPerSecond: 20, downPerSecond: 25 },
    saturation: "none",
    recording: false,
    ...overrides,
  };
}

function signal(symbol: string, address: string, value: number) {
  return { symbol, address, value, unit: "%", quality: "good" };
}
