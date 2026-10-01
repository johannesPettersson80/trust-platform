import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import { validateControlLoopDefinition } from "../../controlStudio/loopDefinition";

/** Loaded dynamically so that a missing save owner fails at the behaviour assertion. */
function saving(): any {
  try { return require("../../controlStudio/loopSetup/loopSetupSave"); } catch { return {}; }
}

const TASK = [{ name: "MainTask", intervalMs: 100 }];
const PARAMETER_FIELDS = ["ProportionalGain", "IntegralGain", "DerivativeGain", "ProportionalSetpointWeight", "DerivativeSetpointWeight",
  "Bias", "OutputMinimum", "OutputMaximum", "RateMinimum", "RateMaximum", "TrackingTime"];
const STATE_FIELDS = ["Setpoint", "SetpointRate", "ProcessValue", "ProcessValueRate", "FeedForward", "NominalOutput", "SaturatedOutput"];
const catalog = {
  symbols: [
    { symbol: "P1.VesselC", type: "LREAL", address: "%IL0" }, { symbol: "P1.JacketC", type: "LREAL", address: "%IL8" },
    { symbol: "P1.VesselSetpointC", type: "LREAL", address: "%ML0" }, { symbol: "P1.JacketSetpointC", type: "LREAL", address: "%ML8" },
    { symbol: "P1.HeaterPct", type: "LREAL", address: "%QL0" }, { symbol: "P1.OuterAuto", type: "BOOL" }, { symbol: "P1.InnerAuto", type: "BOOL" },
    { symbol: "P1.OuterManual", type: "LREAL", address: "%ML16" }, { symbol: "P1.InnerManual", type: "LREAL", address: "%ML24" },
    ...["Outer", "Inner"].flatMap(name => [...PARAMETER_FIELDS.map(field => ({ symbol: `P1.${name}Parameters.${field}`, type: "LREAL" })),
      ...STATE_FIELDS.map(field => ({ symbol: `P1.${name}.StateValue.${field}`, type: "LREAL" }))]),
  ],
  controllers: [{ instance: "P1", type: "Main", tasks: TASK }, { instance: "P1.Inner", type: "Control.PID", tasks: TASK }, { instance: "P1.Outer", type: "Control.PID", tasks: TASK }],
  links: [{ instance: "P1.Outer", method: "Update", argument: "Input", record: "P1.OuterInput", output: "P1.JacketSetpointC", fields: { Setpoint: ["P1.VesselSetpointC"] } },
    { instance: "P1.Inner", method: "Update", argument: "Input", record: "P1.InnerInput", output: "P1.HeaterPct", fields: { Setpoint: ["P1.JacketSetpointC"] } }],
};
const scope = { projectId: "project:save", runtimeId: "runtime-1" };
const signal = (symbol: string, unit: string, minimum: number, maximum: number) => ({ symbol, unit, minimum, maximum });
const controller = (name: string) => ({ instance: `P1.${name}`, parameterRecord: `P1.${name}Parameters`,
  mode: { symbol: `P1.${name}Auto`, whenTrue: "Auto" }, manualOutput: `P1.${name}Manual` });

suite("Control Studio loop setup saving", () => {
  test("writes a cascade as one loop file for both controllers (Specification 35 §17.11)", async () => {
    const api = saving();
    assert.strictEqual(typeof api.saveLoopSetup, "function", "the setup form must save through one owner");
    const root = fs.mkdtempSync(path.join(os.tmpdir(), "trust-loop-setup-"));
    const draft = { structure: "cascade", name: "Vessel temperature", action: "direct", experimentDurationSeconds: 30,
      outer: { ...controller("Outer"), processVariable: signal("P1.VesselC", "degC", 0, 90), setpoint: { symbol: "P1.VesselSetpointC" } },
      inner: { ...controller("Inner"), processVariable: signal("P1.JacketC", "degC", 0, 90), manipulatedVariable: signal("P1.HeaterPct", "%", 0, 100) } };
    try {
      const requests = await api.saveLoopSetup(root, scope, catalog, draft);
      assert.deepStrictEqual(fs.readdirSync(path.join(root, "control-loops")), ["vessel-temperature.json"]);
      const loop = JSON.parse(fs.readFileSync(path.join(root, "control-loops", "vessel-temperature.json"), "utf8"));
      assert.strictEqual(validateControlLoopDefinition(loop, scope).kind, "accepted");
      assert.deepStrictEqual([loop.controller.project.family, requests.length, requests[0].scope], ["cascade", 1, scope]);
      await assert.rejects(() => api.saveLoopSetup(root, scope, catalog, draft), /already exists/, "an existing loop file is never overwritten");
    } finally { fs.rmSync(root, { recursive: true, force: true }); }
  });

  test("writes an identify-only loop that records the chosen plant input and output", async () => {
    const api = saving();
    assert.strictEqual(typeof api.saveLoopSetup, "function");
    const root = fs.mkdtempSync(path.join(os.tmpdir(), "trust-loop-setup-"));
    try {
      const [request] = await api.saveLoopSetup(root, scope, catalog, { structure: "identify", name: "Jacket response",
        manipulatedVariable: signal("P1.HeaterPct", "%", 0, 100), processVariable: signal("P1.JacketC", "degC", 0, 90), experimentDurationSeconds: 120 });
      const loop = request.loopDefinition;
      assert.strictEqual(validateControlLoopDefinition(loop, scope).kind, "accepted");
      assert.deepStrictEqual([loop.topology, loop.controller.instance, loop.controller.type, loop.signals.setpoint], ["open-loop", "P1", "Main", undefined]);
      assert.deepStrictEqual([loop.signals.processVariable.address, loop.signals.manipulatedVariable.address], ["%IL8", "%QL0"]);
    } finally { fs.rmSync(root, { recursive: true, force: true }); }
  });

  test("writes an ON/OFF loop for the program's own switching logic", async () => {
    const api = saving();
    const coldRoom = { symbols: [{ symbol: "P1.RoomC", type: "LREAL", address: "%IL0" }, { symbol: "P1.CompressorPct", type: "LREAL", address: "%QL0" },
      { symbol: "P1.SetpointC", type: "LREAL", address: "%ML0" }, { symbol: "P1.ManualPct", type: "LREAL", address: "%ML8" },
      { symbol: "P1.AutoMode", type: "BOOL" }, { symbol: "P1.CoolingOn", type: "BOOL" },
      ...["HysteresisC", "MinimumOnS", "MinimumOffS"].map(name => ({ symbol: `P1.${name}`, type: "LREAL" }))],
      controllers: [{ instance: "P1", type: "Main", tasks: TASK }], links: [] };
    const root = fs.mkdtempSync(path.join(os.tmpdir(), "trust-loop-setup-"));
    try {
      const [request] = await api.saveLoopSetup(root, scope, coldRoom, { structure: "onoff", name: "Cold room", action: "reverse", instance: "P1",
        processVariable: signal("P1.RoomC", "degC", -10, 20), setpoint: { symbol: "P1.SetpointC" }, manipulatedVariable: signal("P1.CompressorPct", "%", 0, 100),
        mode: { symbol: "P1.AutoMode", whenTrue: "Auto" }, manualOutput: "P1.ManualPct", hysteresis: "P1.HysteresisC", bandConvention: "half-band",
        minimumOn: "P1.MinimumOnS", minimumOff: "P1.MinimumOffS", latch: "P1.CoolingOn", experimentDurationSeconds: 120 });
      assert.deepStrictEqual(fs.readdirSync(path.join(root, "control-loops")), ["cold-room.json"]);
      assert.deepStrictEqual([request.loopDefinition.controller.onOff?.hysteresisSymbol, request.loopDefinition.controller.onOff?.highOutput],
        ["P1.HysteresisC", 100], "the switching controller is saved, not a PID");
    } finally { fs.rmSync(root, { recursive: true, force: true }); }
  });

  test("writes a state-feedback loop for the program's own law", async () => {
    const api = saving();
    const tanks = { symbols: [{ symbol: "P1.Tank1M", type: "LREAL", address: "%IL0" }, { symbol: "P1.Tank2M", type: "LREAL", address: "%IL8" },
      { symbol: "P1.PumpM3PerS", type: "LREAL", address: "%QL0" }, { symbol: "P1.Tank2SetpointM", type: "LREAL", address: "%ML0" },
      { symbol: "P1.ManualM3PerS", type: "LREAL", address: "%ML8" }, { symbol: "P1.FeedbackEnabled", type: "BOOL" },
      ...["K1", "K2", "N"].map(name => ({ symbol: `P1.${name}`, type: "LREAL" }))],
      controllers: [{ instance: "P1", type: "Main", tasks: [{ name: "MainTask", intervalMs: 1000 }] }], links: [] };
    const root = fs.mkdtempSync(path.join(os.tmpdir(), "trust-loop-setup-"));
    try {
      const [request] = await api.saveLoopSetup(root, scope, tanks, { structure: "state-feedback", name: "Tank 2 level", action: "direct", family: "LQR",
        instance: "P1", manipulatedVariable: signal("P1.PumpM3PerS", "m3/s", 0, 0.02), processVariable: signal("P1.Tank2M", "m", 0, 0.5),
        setpoint: { symbol: "P1.Tank2SetpointM" }, gain: "P1.K2", referenceGain: "P1.N", states: [{ signal: signal("P1.Tank1M", "m", 0, 0.5), gain: "P1.K1" }],
        mode: { symbol: "P1.FeedbackEnabled", whenTrue: "Auto" }, manualOutput: "P1.ManualM3PerS", experimentStep: 0.002, experimentDurationSeconds: 300 });
      assert.deepStrictEqual(fs.readdirSync(path.join(root, "control-loops")), ["tank-2-level.json"]);
      assert.deepStrictEqual(request.loopDefinition.controller.stateFeedback?.gainSymbols, [["P1.K2", "P1.K1"]], "the state-feedback law is saved, not a PID");
    } finally { fs.rmSync(root, { recursive: true, force: true }); }
  });
});
