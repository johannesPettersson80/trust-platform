import * as assert from "assert";

suite("Control Studio runtime start action", () => {
  test("starts only the scoped stopped simulator and shares a pending start", async () => {
    let starts = 0;
    let finish: ((value: { ok: true; message: string }) => void) | undefined;
    const api = hostApi();
    const handler = api.createControlStudioRuntimeStartHandler({
      snapshot: async () => ({ status: { running: false, runtimeMode: "simulate" }, starting: false }),
      startLocalSimulator: async () => {
        starts++;
        return new Promise(resolve => { finish = resolve; });
      },
    });
    const request = scope();
    const first = handler(request, request);
    const second = handler(request, request);
    await new Promise(resolve => setImmediate(resolve));
    assert.strictEqual(starts, 1);
    finish!({ ok: true, message: "Simulator started." });
    assert.deepStrictEqual(await Promise.all([first, second]), [
      { ok: true, message: "Simulator started." },
      { ok: true, message: "Simulator started." },
    ]);
  });

  test("rejects stale scope and physical or online targets before starting", async () => {
    let starts = 0;
    const api = hostApi();
    const handler = api.createControlStudioRuntimeStartHandler({
      snapshot: async () => ({ status: { running: false, runtimeMode: "online" }, starting: false }),
      startLocalSimulator: async () => { starts++; return { ok: true, message: "started" }; },
    });
    const request = scope();
    const stale = await handler({ ...request, loopRevision: "new" }, request);
    assert.strictEqual(stale.ok, false);
    assert.match(stale.failure.message, /scope|revision/i);
    const online = await handler(request, request);
    assert.strictEqual(online.ok, false);
    assert.match(online.failure.message, /simulator|physical|online/i);
    assert.strictEqual(starts, 0);
  });

  test("returns an actionable lifecycle failure and does not remain locked", async () => {
    let starts = 0;
    const api = hostApi();
    const handler = api.createControlStudioRuntimeStartHandler({
      snapshot: async () => ({ status: { running: false, runtimeMode: "simulate" }, starting: false }),
      startLocalSimulator: async () => (++starts === 1
        ? { ok: false, failure: { message: "Compile failed; inspect Problems." } }
        : { ok: true, message: "Simulator started." }),
    });
    const request = scope();
    const failed = await handler(request, request);
    assert.strictEqual(failed.ok, false);
    assert.strictEqual(failed.failure.message, "Compile failed; inspect Problems.");
    assert.strictEqual((await handler(request, request)).ok, true);
    assert.strictEqual(starts, 2);
  });

});

function scope(): any {
  return { schemaVersion: 1, projectId: "project", editorSessionId: "editor", loopId: "loop", loopRevision: "revision", runtimeId: "simulator", runtimeMode: "simulate" };
}

function hostApi(): any {
  try { return require("../../controlStudio/runtimeStartAction"); } catch { return {}; }
}
