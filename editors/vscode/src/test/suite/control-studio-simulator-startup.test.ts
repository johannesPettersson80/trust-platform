import * as assert from "assert";
suite("Control Studio simulator startup", () => {
  test("shares startup, respects target selection and cancellation, and permits retry", async () => {
    let api: any = {};
    try { api = require("../../controlStudio/studySimulatorStartup"); } catch { /* expected missing behavior */ }
    assert.strictEqual(typeof api.StudySimulatorStartup, "function");
    let selected = "simulator", running = false, mode = "simulate", starts = 0;
    let finish: (() => void) | undefined;
    const owner = new api.StudySimulatorStartup({
      snapshot: async () => ({ status: { running, runtimeMode: mode } }),
      startLocalSimulator: async () => { starts++; await new Promise<void>(resolve => { finish = resolve; }); running = true; return { ok: true }; },
    }, () => selected);
    const first = owner.ensure("simulator", () => false);
    const second = owner.ensure("simulator", () => false);
    await new Promise(resolve => setImmediate(resolve));
    assert.strictEqual(starts, 1);
    finish!(); await Promise.all([first, second]);
    await owner.ensure("simulator", () => false);
    assert.strictEqual(starts, 1);
    running = false; mode = "online";
    await assert.rejects(owner.ensure("simulator", () => false), /local simulator/i);
    mode = "simulate";
    await assert.rejects(owner.ensure("simulator", () => true), /cancel/i);
    assert.strictEqual(starts, 1);
    let cancelled = false;
    const changing = owner.ensure("simulator", () => cancelled);
    await new Promise(resolve => setImmediate(resolve));
    cancelled = true; finish!();
    await assert.rejects(changing, /cancel/i);
    running = false;
    const targetChange = owner.ensure("simulator", () => false);
    await new Promise(resolve => setImmediate(resolve));
    selected = "other"; finish!();
    await assert.rejects(targetChange, /selected|target/i);
    let attempts = 0;
    const retry = new api.StudySimulatorStartup({ snapshot: async () => ({ status: { running: false, runtimeMode: "simulate" } }),
      startLocalSimulator: async () => (++attempts === 1 ? { ok: false, failure: { message: "Compile failed" } } : { ok: true }),
    }, () => "simulator");
    await assert.rejects(retry.ensure("simulator", () => false), /Compile failed/);
    await retry.ensure("simulator", () => false);
    assert.strictEqual(attempts, 2);
  });

  test("accepts a connected runtime that reports its own simulation mode and starts nothing", async () => {
    const { StudySimulatorStartup } = require("../../controlStudio/studySimulatorStartup");
    const { simulationTarget } = require("../../controlStudio/simulationTarget");
    let simulated = true, starts = 0;
    const owner = new StudySimulatorStartup({
      snapshot: async () => ({ status: { running: true, runtimeMode: "online", simulated } }),
      startLocalSimulator: async () => { starts++; return { ok: true }; },
    }, () => "unix:///tmp/trust-runtime.sock");
    await owner.ensure("unix:///tmp/trust-runtime.sock", () => false);
    assert.strictEqual(starts, 0, "a runtime in simulation mode needs no local simulator");
    simulated = false;
    await assert.rejects(owner.ensure("unix:///tmp/trust-runtime.sock", () => false), /simulation mode/i,
      "a runtime that does not report simulation is physical");
    assert.strictEqual(simulationTarget({ runtimeMode: "online", simulated: true }), true);
    assert.strictEqual(simulationTarget({ runtimeMode: "online", simulated: false }), false);
    assert.strictEqual(simulationTarget({ runtimeMode: "simulate" }), false, "a simulate setting alone never makes a target simulated (A2.4)");
    assert.strictEqual(simulationTarget({ runtimeMode: "simulate", simulated: true }), true, "the runtime's own report decides");
  });
});
