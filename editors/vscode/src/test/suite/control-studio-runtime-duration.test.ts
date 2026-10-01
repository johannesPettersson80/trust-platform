import * as assert from "assert";

/** Loaded dynamically so that a missing owner fails at the behaviour assertion. */
function duration(): any {
  try { return require("../../controlStudio/runtimeDuration"); } catch { return {}; }
}

/** A runtime clock in nanoseconds that advances `stepMs` of runtime time on every reading. */
function clock(stepMs: number) {
  let readings = 0;
  return { read: async () => (readings++) * stepMs * 1e6, readings: () => readings };
}

suite("Control Studio runtime-time observation", () => {
  test("waits until the runtime has executed the whole duration, also when it runs behind real time", async () => {
    const api = duration();
    assert.strictEqual(typeof api.awaitRuntimeDuration, "function", "Apply observes the test duration in the runtime's own time");
    const runtime = clock(50);
    await api.awaitRuntimeDuration(runtime.read, 500, { checkCancelled: () => undefined, boundMs: 5000, pollMs: 1 });
    assert.ok(runtime.readings() >= 11, `500 ms of runtime time at 50 ms per reading needs at least 11 readings: ${runtime.readings()}`);
  });

  test("reports a runtime that falls further behind than the wall-clock bound", async () => {
    const stopped = { read: async () => 7e9 };
    await assert.rejects(() => duration().awaitRuntimeDuration(stopped.read, 500, { checkCancelled: () => undefined, boundMs: 20, pollMs: 1 }),
      /executed 0 s of the reviewed 0\.5 s/);
  });

  test("stops when cancelled", async () => {
    const runtime = clock(1);
    let checks = 0;
    await assert.rejects(() => duration().awaitRuntimeDuration(runtime.read, 10_000, {
      checkCancelled: () => { if (++checks > 3) throw new Error("Controller commissioning cancelled."); }, boundMs: 5000, pollMs: 1 }), /cancelled/);
  });
});
