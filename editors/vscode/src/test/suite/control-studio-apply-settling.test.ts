import * as assert from "assert";

/** Loaded dynamically so that a missing wait fails at the behaviour assertion. */
function starting(): any {
  try { return require("../../controlStudio/commissioningStartingPoint"); } catch { return {}; }
}

const TASK = [{ name: "MainTask", intervalMs: 100 }];
/** A cold room validated from 12 °C with its compressor off. */
function coldRoomLoop() {
  const { onOffLoopDefinition } = require("../../controlStudio/loopSetup/pidLoopSetup");
  return onOffLoopDefinition({
    symbols: [
      { symbol: "P1.RoomC", type: "LREAL", address: "%IL0" }, { symbol: "P1.CompressorPct", type: "LREAL", address: "%QL0" },
      { symbol: "P1.SetpointC", type: "LREAL", address: "%ML0" }, { symbol: "P1.ManualPct", type: "LREAL", address: "%ML8" },
      { symbol: "P1.AutoMode", type: "BOOL" }, { symbol: "P1.CoolingOn", type: "BOOL" },
      ...["HysteresisC", "MinimumOnS", "MinimumOffS"].map(name => ({ symbol: `P1.${name}`, type: "LREAL" })),
    ],
    controllers: [{ instance: "P1", type: "Main", tasks: TASK }], links: [],
  }, { projectId: "project:apply", runtimeId: "runtime-1", name: "Cold room", action: "reverse", instance: "P1",
    processVariable: { symbol: "P1.RoomC", unit: "degC", minimum: -10, maximum: 20 }, setpoint: { symbol: "P1.SetpointC" },
    manipulatedVariable: { symbol: "P1.CompressorPct", unit: "%", minimum: 0, maximum: 100 }, mode: { symbol: "P1.AutoMode", whenTrue: "Auto" },
    manualOutput: "P1.ManualPct", hysteresis: "P1.HysteresisC", bandConvention: "half-band", minimumOn: "P1.MinimumOnS",
    minimumOff: "P1.MinimumOffS", latch: "P1.CoolingOn", experimentDurationSeconds: 120 });
}
const MODEL = { kind: "transfer-function", numerator: [-0.1], denominator: [60, 1], delaySeconds: 0, revision: "room", source: "fitted",
  validation: "validated", evidence: ["recording"], inputs: [{ id: "P1.CompressorPct", unit: "%", operatingRange: { minimum: 0, maximum: 100 } }],
  outputs: [{ id: "P1.RoomC", unit: "degC", operatingRange: { minimum: 2, maximum: 12 } }], operatingPoint: { inputs: [0], outputs: [12] } };
const REQUIREMENTS = { referenceChange: -7, maximumError: 2, settlingBand: 0.5, settlingTimeSeconds: 90, durationSeconds: 300 };

/** A runtime whose readings follow the given sequence, repeating the last one. */
function readings(values: Record<string, number>[]) {
  let index = 0;
  const read = async () => ({ values: values[Math.min(index++, values.length - 1)] });
  return { read, count: () => index };
}

suite("Control Studio Apply waits for a settled process", () => {
  test("waits for the measured output to settle at the validated point, showing its progress, then continues", async () => {
    const api = starting();
    assert.strictEqual(typeof api.awaitCommissioningStartingPoint, "function", "Apply waits for the process instead of asking to apply again");
    const runtime = readings([{ "P1.CompressorPct": 0, "P1.RoomC": 10.3 }, { "P1.CompressorPct": 0, "P1.RoomC": 11.2 }, { "P1.CompressorPct": 0, "P1.RoomC": 11.7 }]);
    const progress: string[] = [];
    const settled = await api.awaitCommissioningStartingPoint(coldRoomLoop(), MODEL, REQUIREMENTS, ["P1.CompressorPct"], ["P1.RoomC"], runtime.read,
      { progress: (text: string) => { progress.push(text); }, cancelled: () => false, pollMs: 1 });
    assert.strictEqual(settled.values["P1.RoomC"], 11.7, "the first reading inside the settling band starts the application");
    assert.strictEqual(progress[0], "Waiting for P1.RoomC to settle at 12 °C ± 0.5 °C (now 10.3 °C)…");
  });

  test("refuses at once when a plant input differs from the validated point, because the process cannot settle by itself", async () => {
    const runtime = readings([{ "P1.CompressorPct": 100, "P1.RoomC": 12 }]);
    await assert.rejects(() => starting().awaitCommissioningStartingPoint(coldRoomLoop(), MODEL, REQUIREMENTS, ["P1.CompressorPct"], ["P1.RoomC"], runtime.read,
      { progress: () => undefined, cancelled: () => false, pollMs: 1 }), /P1\.CompressorPct is 100 %, but the settings were validated from 0 %/);
    assert.strictEqual(runtime.count(), 1);
  });

  test("stops when cancelled, before any setting is written", async () => {
    const runtime = readings([{ "P1.CompressorPct": 0, "P1.RoomC": 9 }]);
    const api = starting();
    await assert.rejects(() => api.awaitCommissioningStartingPoint(coldRoomLoop(), MODEL, REQUIREMENTS, ["P1.CompressorPct"], ["P1.RoomC"], runtime.read,
      { progress: () => undefined, cancelled: () => true, pollMs: 1 }), (error: Error) => typeof api.StartingPointWaitCancelled === "function"
        && error instanceof api.StartingPointWaitCancelled && /cancelled.*nothing was written/i.test(error.message),
    "a cancelled wait is reported as the user's own cancellation, not as a failed Apply");
  });

  test("an integrating output rests wherever it is, so Apply starts from its present value", async () => {
    const api = starting();
    const tank = { ...MODEL, numerator: [0.01], denominator: [1, 0], revision: "integrating" };
    const runtime = readings([{ "P1.CompressorPct": 0, "P1.RoomC": 9 }]);
    const settled = await api.awaitCommissioningStartingPoint(coldRoomLoop(), tank, REQUIREMENTS, ["P1.CompressorPct"], ["P1.RoomC"], runtime.read,
      { progress: () => undefined, cancelled: () => false, pollMs: 1, timeoutMs: 20 });
    assert.strictEqual(runtime.count(), 1, "no wait: with the plant input at its validated value the output stays where it is");
    assert.strictEqual(typeof api.anchorIntegratingOutputs, "function");
    assert.deepStrictEqual(api.anchorIntegratingOutputs(tank, settled.values, ["P1.RoomC"]).operatingPoint.outputs, [9],
      "the validated step and the Auto entry start from the present value");
    assert.deepStrictEqual(api.anchorIntegratingOutputs(MODEL, settled.values, ["P1.RoomC"]).operatingPoint.outputs, [12],
      "a self-regulating output keeps its validated point");
  });

  test("reports an output that has not settled within the bound, with its current value", async () => {
    const runtime = readings([{ "P1.CompressorPct": 0, "P1.RoomC": 9 }]);
    await assert.rejects(() => starting().awaitCommissioningStartingPoint(coldRoomLoop(), MODEL, REQUIREMENTS, ["P1.CompressorPct"], ["P1.RoomC"], runtime.read,
      { progress: () => undefined, cancelled: () => false, pollMs: 1, timeoutMs: 20 }), /P1\.RoomC did not settle at 12 °C ± 0\.5 °C.*it is 9 °C\. Apply again once it has settled/);
  });
});
