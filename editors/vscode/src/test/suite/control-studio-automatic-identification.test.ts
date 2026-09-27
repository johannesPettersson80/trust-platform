import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import { readIdentificationBaseline } from "../../controlStudio/identificationBaseline";
import { AutomaticIdentification } from "../../controlStudio/automaticIdentification";
import type { ControlLoopDefinition } from "../../controlStudio/loopDefinition";
import { loopInspectionBindings } from "../../controlStudio/loopInspectionBindings";
import { ControlLoopMonitor } from "../../controlStudio/loopMonitor";
import { SymbolicLoopObservation } from "../../controlStudio/symbolicLoopObservation";
import type { RuntimeLifecycleSnapshot } from "../../runtimeLifecycle";
import { evaluateRecordedRun, type RecordedRun } from "../../controlStudio/runValidation";
import type { RuntimeVerificationClient, RuntimeVerificationResult } from "../../controlStudio/verificationClient";

const loop = JSON.parse(fs.readFileSync(path.resolve(__dirname, "../../../src/test/fixtures/control-studio-engineer/control-loop.json"), "utf8")) as ControlLoopDefinition;
function trial(id: string, value: number): RuntimeVerificationResult {
  const run: RecordedRun = { id, evidence: "ST-executed", parameters: {},
    conditions: { initialState: {}, events: [], durationSeconds: 2, sampleTimesSeconds: [0, 0.05, 0.1, 1, 2], seed: 7,
      modelRevision: "plant", programRevision: "program", parameterSchemaRevision: "parameters" },
    channels: [{ id: "Main.PV", unit: "m" }, { id: "Main.MV", unit: "V" }],
    samples: [0, 0.05, 0.1, 1, 2].map((timeSeconds, sequence) => ({ timeSeconds, sequence,
      values: { "Main.PV": { value, quality: "good" }, "Main.MV": { value: 0, quality: "good" } } })),
  };
  return { projectRevision: "project", request: {}, run, evaluation: evaluateRecordedRun(run,
    [{ id: "bounds", kind: "bounds", channel: "Main.PV", minimum: -20, maximum: 20, window: { start: 0, end: 2 } }]) };
}
type Snapshot = { status: string; reason?: string; runs: RuntimeVerificationResult[] };
function snapshot(campaign: AutomaticIdentification): Snapshot | undefined {
  return (campaign as unknown as { snapshot?: () => Snapshot }).snapshot?.();
}
function selectorVector(bindings: readonly { id: string; address?: string; symbol?: string; iec_type: string }[]): string[][] {
  return bindings.map(binding => [binding.id, binding.address ?? "", binding.symbol ?? "", binding.iec_type]);
}
function observationLifecycle(): RuntimeLifecycleSnapshot {
  return { sessionId: "identification-contention", status: { running: true, inlineValuesEnabled: false,
    runtimeMode: "simulate", runtimeState: "running", endpoint: "unix:///identification-contention",
    endpointConfigured: true, endpointEnabled: true, endpointReachable: true },
    ioState: { inputs: [], outputs: [], memory: [] }, starting: false };
}
function baselineReadings(bindings: readonly { address?: string; symbol?: string; iec_type: string }[]) {
  return { values: bindings.map(binding => binding.iec_type === "BOOL" ? 1 : binding.address === "%IL0" ? 10 : 2),
    forced: bindings.map(() => false), quality: bindings.map(() => "good") };
}
suite("Control Studio automatic identification retention", () => {
  test("waits for the first coherent scan after registering inspection bindings", async () => {
    let count = 0;
    const point = await readIdentificationBaseline(loop, async (_method, request: any) => {
      count++;
      const identity = { loop_revision: request.loop_revision, program_revision: "same-program", bindings: request.bindings };
      if (count === 1) return { ...identity, available: false, reason: "Awaiting scan" };
      assert.strictEqual(request.expected_program_revision, "same-program");
      return { ...identity, available: true, scan: 4, runtime_time_ns: 80000000, captured_at_ms: Date.now(),
        ...baselineReadings(request.bindings) };
    });
    assert.deepStrictEqual(point, { input: 2, output: 10 });
    assert.strictEqual(count, 2);
  });
  test("cancels pending baseline acquisition before another inspection", async () => {
    let count = 0;
    let cancelled = false;
    await assert.rejects((readIdentificationBaseline as any)(loop, async (_method: string, request: any) => {
      count++; cancelled = true;
      return { loop_revision: request.loop_revision, program_revision: "program", bindings: request.bindings,
        available: false, reason: "Awaiting scan" };
    }, () => cancelled), /cancelled/i);
    assert.strictEqual(count, 1);
  });
  test("shares the complete ordered watch with concurrent symbolic observation", async function () {
    this.timeout(10000);
    const connection = require("../../controlStudio/runtimeConnection");
    const previous = connection.bindRuntimeConnection;
    const expected = selectorVector(loopInspectionBindings(loop));
    const calls: string[][][] = [];
    let selected = "";
    let selectionChanges = 0;
    let scan = 0;
    const request = async (operation: string, params: any) => {
      assert.strictEqual(operation, "control.signals.inspect");
      const vector = selectorVector(params.bindings);
      calls.push(vector);
      const key = JSON.stringify(vector);
      if (selected !== key) {
        selected = key;
        selectionChanges++;
        return { available: false, program_revision: "program", loop_revision: params.loop_revision,
          bindings: params.bindings, reason: "Waiting for the next coherent runtime scan." };
      }
      if (params.expected_program_revision !== undefined) assert.strictEqual(params.expected_program_revision, "program");
      const values = params.bindings.map((binding: { address?: string; symbol?: string; iec_type: string }) => {
        if (binding.iec_type === "BOOL") return 1;
        if (binding.address === "%ML0") return 35;
        if (binding.address === "%IL0") return 10;
        return 2;
      });
      return { available: true, program_revision: "program", loop_revision: params.loop_revision,
        bindings: params.bindings, scan: ++scan, runtime_time_ns: scan * 20_000_000, captured_at_ms: Date.now(),
        values, forced: params.bindings.map(() => false), quality: params.bindings.map(() => "good") };
    };
    connection.bindRuntimeConnection = async () => ({ source: "simulation", endpoint: "unix:///identification-contention", request });
    try {
      const watch = new SymbolicLoopObservation();
      const monitor = new ControlLoopMonitor({ staleAfterMs: 1500, recordingCapacity: 32 });
      const baselinePromise = readIdentificationBaseline(loop, request);
      const observedPromise = watch.observe(monitor, loop, observationLifecycle(), Date.now());
      const [baseline, observed] = await Promise.allSettled([baselinePromise, observedPromise]);
      assert.strictEqual(baseline.status, "fulfilled", baseline.status === "rejected" ? String(baseline.reason) : "baseline result");
      assert.deepStrictEqual(baseline.status === "fulfilled" ? baseline.value : undefined, { input: 2, output: 10 });
      assert.strictEqual(observed.status, "fulfilled", observed.status === "rejected" ? String(observed.reason) : "observation result");
      assert.strictEqual(observed.status === "fulfilled" ? observed.value.availability : "unavailable", "available");
      assert.ok(calls.length >= 2, "both owners must use the native inspection protocol");
      assert.ok(calls.every(vector => vector.length === expected.length), "every owner must request the complete loop selection");
      assert.ok(calls.every(vector => JSON.stringify(vector) === JSON.stringify(expected)), "owners must use equal ordered selectors regardless of object property order");
      assert.strictEqual(selectionChanges, 1, "the native single watch must not be replaced by a competing baseline selection");
    } finally {
      connection.bindRuntimeConnection = previous;
    }
  });
  test("retains the declared acquisition mode when controller mode differs or is absent", async () => {
    for (const modeSignal of ["Main.AutoMode", undefined]) {
      const distinct: ControlLoopDefinition = { ...loop, controller: { ...loop.controller, modeSignal } };
      const point = await readIdentificationBaseline(distinct, async (_method, request: any) => {
        assert.deepStrictEqual(selectorVector(request.bindings), selectorVector(loopInspectionBindings(distinct)),
          "baseline acquisition must not install a different watch even for a separate activation mode");
        assert.ok(request.bindings.some((binding: any) => binding.symbol === loop.parameterBinding!.mode.symbol),
          "preserve the declared acquisition-mode observation");
        return { loop_revision: request.loop_revision, program_revision: "program", bindings: request.bindings,
          available: true, scan: 4, runtime_time_ns: 80000000, captured_at_ms: Date.now(), ...baselineReadings(request.bindings) };
      });
      assert.deepStrictEqual(point, { input: 2, output: 10 });
    }
  });
  test("rejects stale forced or bad baseline observations", async () => {
    for (const mutation of ["stale", "forced", "bad"]) {
      await assert.rejects(readIdentificationBaseline(loop, async (_method, request: any) => {
        const readings = baselineReadings(request.bindings);
        const pv = request.bindings.findIndex((binding: any) => binding.address === loop.signals.processVariable.address);
        if (mutation === "forced") readings.forced[pv] = true;
        if (mutation === "bad") readings.quality[pv] = "bad";
        return {
        loop_revision: request.loop_revision, program_revision: "program", bindings: request.bindings,
        available: true, scan: 4, runtime_time_ns: 80000000, captured_at_ms: Date.now() - (mutation === "stale" ? 10000 : 0),
        ...readings,
      }; }), /fresh.*unforced/);
    }
  });
  test("starts a trial from the measured command rather than an invented zero", async () => {
    let initial: any;
    const runtime = { verify: async (request: any) => { initial = request.initial_values; throw new Error("captured request"); },
      dispose: () => undefined } as unknown as RuntimeVerificationClient;
    const campaign = new (AutomaticIdentification as any)(runtime, async () => ({ input: 2, output: 10 }));
    await assert.rejects(campaign.run(loop, async () => undefined), /captured request/);
    assert.strictEqual(initial["Main.ManualCommand"], 2);
    assert.strictEqual(initial["Main.MV"], 2);
  });
  test("uses a declared bounded pulse and restores it before the identification run ends", async () => {
    const pulseLoop = JSON.parse(JSON.stringify(loop)) as ControlLoopDefinition;
    pulseLoop.experiment = { execution: "control-studio", kind: "mv_pulse", amplitude: 1, durationSeconds: 10, seed: 7 };
    let request: any;
    const runtime = { verify: async (value: any) => { request = value; throw new Error("captured pulse request"); },
      dispose: () => undefined } as unknown as RuntimeVerificationClient;
    const campaign = new (AutomaticIdentification as any)(runtime, async () => ({ input: 2, output: 10 }));
    await assert.rejects(campaign.run(pulseLoop, async () => undefined), /captured pulse request/);
    assert.deepStrictEqual(request.events, [
      { at_ms: 1000, symbol: "Main.ManualCommand", value: 3 },
      { at_ms: 5500, symbol: "Main.ManualCommand", value: 2 },
    ]);
  });
  test("runs declared PRBS and relay identification methods through their own acquisition paths", async () => {
    for (const kind of ["prbs", "relay"] as const) {
      const requestedLoop = JSON.parse(JSON.stringify(loop)) as ControlLoopDefinition;
      requestedLoop.experiment = { execution: "control-studio", kind, amplitude: 1, durationSeconds: 10, seed: 7,
        ...(kind === "relay" ? { relayDirection: "positive" as const } : {}) };
      let calls = 0;
      const runtime = { verify: async () => { calls += 1; throw new Error(`${kind} acquisition stopped after dispatch`); },
        dispose: () => undefined } as unknown as RuntimeVerificationClient;
      const campaign = new (AutomaticIdentification as any)(runtime, async () => ({ input: 2, output: 10 }));
      await assert.rejects(campaign.run(requestedLoop, async () => undefined), new RegExp(`${kind}.*acquisition|stopped after dispatch`, "i"));
      assert.ok(calls > 0, `${kind} must dispatch its declared acquisition rather than substitute a step`);
    }
  });
  test("runs independent multisine trials and returns their validated fitted model", async () => {
    const multisineLoop = JSON.parse(JSON.stringify(loop)) as ControlLoopDefinition;
    multisineLoop.experiment = { execution: "control-studio", kind: "multisine", amplitude: 1, durationSeconds: 10, seed: 7 };
    const requests: any[] = [];
    const runtime = { verify: async (request: any) => {
      requests.push(request);
      return multisineTrial(`multisine-${requests.length}`, request);
    }, dispose: () => undefined } as unknown as RuntimeVerificationClient;
    const campaign = new AutomaticIdentification(runtime, async () => ({ input: 2, output: 10 }));
    const result = await campaign.run(multisineLoop, async () => undefined);
    assert.strictEqual(requests.length, 2, "frequency identification requires independent estimation and validation trials");
    assert.ok(requests.every(request => request.events.length > 32), "each trial must contain a real multisine trajectory");
    assert.notDeepStrictEqual(requests[0].events, requests[1].events, "validation must use an independent excitation record");
    const model = (result as any).model;
    assert.ok(model, "multisine must return a selected fitted model rather than a step-only review");
    if (model) {
      assert.strictEqual(model.kind, "transfer-function");
      assert.strictEqual(model.validation, "validated");
      assert.strictEqual(model.source, "fitted");
      assert.strictEqual(model.denominator.length, 2);
      assert.ok(model.evidence.length >= 2, "both frequency records must remain in model provenance");
    }
  });
  test("runs independent PRBS trials through the frequency-response fit path", async () => {
    const prbsLoop = JSON.parse(JSON.stringify(loop)) as ControlLoopDefinition;
    prbsLoop.experiment = { execution: "control-studio", kind: "prbs", amplitude: 1, durationSeconds: 10, seed: 7 };
    const requests: any[] = [];
    const runtime = { verify: async (request: any) => {
      requests.push(request);
      return multisineTrial(`prbs-${requests.length}`, request);
    }, dispose: () => undefined } as unknown as RuntimeVerificationClient;
    const campaign = new AutomaticIdentification(runtime, async () => ({ input: 2, output: 10 }));
    const result = await campaign.run(prbsLoop, async () => undefined);
    assert.strictEqual(requests.length, 2);
    assert.ok(requests.every(request => request.events.length > 32));
    assert.notDeepStrictEqual(requests[0].events, requests[1].events, "PRBS validation must have an independent sequence");
    const model = (result as any).model;
    assert.ok(["transfer-function", "discrete-state-space"].includes(model?.kind));
    assert.strictEqual(model?.validation, "validated");
  });
  test("rejects unavailable measured coordinates before starting a trial", async () => {
    let count = 0;
    const runtime = { verify: async () => { count++; throw new Error("trial must not start"); },
      dispose: () => undefined } as unknown as RuntimeVerificationClient;
    const campaign = new (AutomaticIdentification as any)(runtime, async () => { throw new Error("Operating point unavailable"); });
    await assert.rejects(campaign.run(loop, async () => undefined), /Operating point unavailable/);
    assert.strictEqual(count, 0);
  });
  test("retains and rejects a configured plant baseline that disagrees with observation", async () => {
    let count = 0;
    const runtime = { verify: async () => { count++; return trial("wrong-initial-state", 2); },
      dispose: () => undefined } as unknown as RuntimeVerificationClient;
    const campaign = new AutomaticIdentification(runtime, async () => ({ input: 0, output: 1 }));
    await assert.rejects(campaign.run(loop, async () => undefined), /baseline.*incompatible/i);
    assert.strictEqual(count, 1, "No second excitation follows a rejected baseline");
    assert.strictEqual(snapshot(campaign)?.runs[0]?.run.id, "wrong-initial-state");
  });
  test("retains the raw failed bounds trial before rejecting model selection", async () => {
    const failed = trial("outside-bounds", 30);
    const runtime = { verify: async () => failed, dispose: () => undefined } as unknown as RuntimeVerificationClient;
    const campaign = new AutomaticIdentification(runtime, async () => ({ input: 0, output: 1 }));
    await assert.rejects(campaign.run(loop, async () => undefined), /bounds|missing evidence/);
    const retained = snapshot(campaign);
    assert.strictEqual(retained?.status, "failed", "Failed identification must retain its outcome and raw trial");
    assert.strictEqual(retained!.runs[0].run.id, "outside-bounds");
    assert.strictEqual(retained!.runs[0].evaluation.status, "Fail");
    retained!.runs.length = 0;
    assert.strictEqual(snapshot(campaign)!.runs.length, 1, "Readers must not mutate retained evidence");
  });
  test("cancellation during progress retains the first record and does not start another trial", async () => {
    let count = 0;
    const runtime = { verify: async () => { count++; return trial("estimation", 1); },
      dispose: () => undefined } as unknown as RuntimeVerificationClient;
    const campaign = new AutomaticIdentification(runtime, async () => ({ input: 0, output: 1 }));
    await assert.rejects(campaign.run(loop, async text => {
      if (text.startsWith("Independent validation trial")) campaign.dispose();
    }), /cancelled/);
    assert.strictEqual(count, 1, "Cancellation must prevent a trial from starting after an awaited progress callback");
    assert.strictEqual(snapshot(campaign)?.status, "cancelled");
    assert.strictEqual(snapshot(campaign)?.runs[0]?.run.id, "estimation");
  });
  test("retains the first trial when the independent trial fails", async () => {
    let count = 0;
    const runtime = { verify: async () => { if (count++) throw new Error("Independent trial disconnected"); return trial("estimation", 1); },
      dispose: () => undefined } as unknown as RuntimeVerificationClient;
    const campaign = new AutomaticIdentification(runtime, async () => ({ input: 0, output: 1 }));
    await assert.rejects(campaign.run(loop, async () => undefined), /disconnected/);
    assert.strictEqual(snapshot(campaign)?.runs[0]?.run.id, "estimation", "A later failure must not erase the estimation record");
    assert.match(snapshot(campaign)?.reason ?? "", /disconnected/);
  });
});

function multisineTrial(id: string, request: any): RuntimeVerificationResult {
  const stepSeconds = request.step_ms / 1000;
  const events = request.events as Array<{ at_ms: number; symbol: string; value: number }>;
  const initial = request.initial_values as Record<string, number>;
  let manual = initial["Main.ManualCommand"];
  let pv = initial["Main.PV"] ?? 10;
  let event = 0;
  const samples = Array.from({ length: request.max_total_cycles }, (_, sequence) => {
    const timeSeconds = sequence * stepSeconds;
    while (event < events.length && events[event].at_ms <= Math.round(timeSeconds * 1000)) {
      manual = events[event].value; event += 1;
    }
    const target = 10 + (manual - 2);
    pv += stepSeconds / 0.8 * (target - pv);
    return { timeSeconds, sequence, values: {
      "Main.MV": { value: manual, quality: "good" as const },
      "Main.PV": { value: pv, quality: "good" as const },
    } };
  });
  return { projectRevision: "project", request, evaluation: { status: "Pass" } as any, run: {
    id, evidence: "ST-executed", parameters: {}, conditions: {
      initialState: request.initial_values, events: request.events, durationSeconds: request.duration_ms / 1000,
      sampleTimesSeconds: samples.map((sample: any) => sample.timeSeconds), seed: request.seed,
      modelRevision: "plant", programRevision: "project", parameterSchemaRevision: "parameters",
    },
    channels: [{ id: "Main.MV", unit: "V" }, { id: "Main.PV", unit: "m" }], samples,
  } as any };
}
