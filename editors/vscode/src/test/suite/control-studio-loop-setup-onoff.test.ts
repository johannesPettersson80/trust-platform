import * as assert from "assert";
import { validateControlLoopDefinition } from "../../controlStudio/loopDefinition";

/** Loaded dynamically so that a missing ON/OFF generator fails at the behaviour assertion. */
function setup(): any {
  try { return require("../../controlStudio/loopSetup/pidLoopSetup"); } catch { return {}; }
}

const TASK = [{ name: "MainTask", intervalMs: 100 }];
const scope = { projectId: "project:setup", runtimeId: "runtime-1" };
/** A cold room whose program switches a compressor between 0 and 100 %. */
const coldRoom = {
  symbols: [
    { symbol: "P1.RoomC", type: "LREAL", address: "%IL0" }, { symbol: "P1.CompressorPct", type: "LREAL", address: "%QL0" },
    { symbol: "P1.SetpointC", type: "LREAL", address: "%ML0" }, { symbol: "P1.ManualPct", type: "LREAL", address: "%ML8" },
    { symbol: "P1.AutoMode", type: "BOOL" }, { symbol: "P1.CoolingOn", type: "BOOL" }, { symbol: "P1.DwellS", type: "LREAL" },
    { symbol: "P1.HysteresisC", type: "LREAL" }, { symbol: "P1.MinimumOnS", type: "LREAL" }, { symbol: "P1.MinimumOffS", type: "LREAL" },
  ],
  controllers: [{ instance: "P1", type: "Main", tasks: TASK }],
  links: [],
  writes: ["P1.CompressorPct", "P1.CoolingOn", "P1.DwellS"],
};
const draft = { ...scope, name: "Cold room", action: "reverse", instance: "P1",
  processVariable: { symbol: "P1.RoomC", unit: "degC", minimum: -10, maximum: 20 },
  manipulatedVariable: { symbol: "P1.CompressorPct", unit: "%", minimum: 0, maximum: 100 },
  setpoint: { symbol: "P1.SetpointC" }, mode: { symbol: "P1.AutoMode", whenTrue: "Auto" }, manualOutput: "P1.ManualPct",
  hysteresis: "P1.HysteresisC", bandConvention: "half-band", minimumOn: "P1.MinimumOnS", minimumOff: "P1.MinimumOffS",
  latch: "P1.CoolingOn", dwellTimer: "P1.DwellS", experimentDurationSeconds: 120 };

suite("Control Studio ON/OFF setup", () => {
  test("offers the instances that can hold switching logic and every REAL setting", () => {
    const api = setup();
    const withPid = { ...coldRoom, controllers: [...coldRoom.controllers, { instance: "P1.Backup", type: "Control.PID", tasks: TASK },
      { instance: "P1.Unscheduled", type: "Switch", tasks: [] }] };
    const model = api.setupFormModel(withPid);
    assert.deepStrictEqual(model.instances, ["P1"], "a library PID is set up as a PID; switching logic needs one cyclic task");
    assert.deepStrictEqual(model.reals, ["P1.CompressorPct", "P1.DwellS", "P1.HysteresisC", "P1.ManualPct", "P1.MinimumOffS", "P1.MinimumOnS",
      "P1.RoomC", "P1.SetpointC"], "settings are written by Studio directly, so they need no I/O address");
  });

  test("writes an ON/OFF loop that switches between the output's limits and tunes its band and dwell times", () => {
    const api = setup();
    assert.strictEqual(typeof api.onOffLoopDefinition, "function", "a switching controller needs its own setup structure");
    const loop = api.onOffLoopDefinition(coldRoom, draft);
    assert.strictEqual(validateControlLoopDefinition(loop, scope).kind, "accepted");
    const { decodeOnOffController, designOnOffController } = require("../../controlStudio/onOffController");
    const onOff = decodeOnOffController(loop.controller.onOff, loop.parameterBinding);
    assert.ok(onOff, "the existing ON/OFF contract accepts the generated controller");
    assert.deepStrictEqual([onOff.lowOutput, onOff.highOutput, onOff.bandConvention], [0, 100, "half-band"]);
    assert.deepStrictEqual(loop.parameterBinding.parameters.map((parameter: any) => [parameter.symbol, parameter.unit]),
      [["P1.HysteresisC", "degC"], ["P1.MinimumOnS", "s"], ["P1.MinimumOffS", "s"]]);
    assert.deepStrictEqual(onOff.initialization.states.map((state: any) => [state.symbol, state.iec_type, state.initial]),
      [["P1.CoolingOn", "BOOL", { source: "model-input-is", index: 0, value: 100 }], ["P1.DwellS", "LREAL", 0]],
      "the latch starts ON exactly when the output starts high; the dwell timer starts at zero");
    assert.deepStrictEqual([loop.experiment.amplitude, loop.experiment.durationSeconds], [100, 120], "identification switches between both levels");
    const model = { kind: "transfer-function", numerator: [-0.1], denominator: [60, 1], delaySeconds: 0, revision: "room", source: "fitted",
      validation: "validated", evidence: ["recording"], inputs: [{ id: "u", unit: "%", operatingRange: { minimum: 0, maximum: 100 } }],
      outputs: [{ id: "y", unit: "degC", operatingRange: { minimum: -10, maximum: 20 } }], operatingPoint: { inputs: [0], outputs: [12] } };
    const design = designOnOffController(loop, model, { referenceChange: -2, maximumError: 1.5, settlingBand: 0.5, settlingTimeSeconds: 120, durationSeconds: 300 });
    assert.strictEqual(design.candidate.parameters["P1.HysteresisC"], 0.5, "Tune sets the half band from the settling band");
  });

  test("validates each switching condition once when the equipment's limits equal the controller's", () => {
    const api = setup();
    const loop = api.onOffLoopDefinition(coldRoom, { ...draft, equipmentMinimumOnSeconds: 20, equipmentMinimumOffSeconds: 20 });
    const model = { kind: "transfer-function", numerator: [-0.1], denominator: [60, 1], delaySeconds: 0, revision: "room", source: "fitted",
      validation: "validated", evidence: ["recording"], inputs: [{ id: "P1.CompressorPct", unit: "%", operatingRange: { minimum: 0, maximum: 100 } }],
      outputs: [{ id: "P1.RoomC", unit: "degC", operatingRange: { minimum: 2, maximum: 12 } }], operatingPoint: { inputs: [0], outputs: [12] } };
    const { engineerVerificationPlan } = require("../../controlStudio/engineerVerificationPlan");
    const plan = engineerVerificationPlan(loop, model, { referenceChange: -7, maximumError: 2, settlingBand: 0.5, settlingTimeSeconds: 90, durationSeconds: 300 },
      { "P1.HysteresisC": 0.5, "P1.MinimumOnS": 20, "P1.MinimumOffS": 20 });
    const switching = plan.requirements.filter((requirement: any) => ["levels", "dwell"].includes(requirement.kind))
      .map((requirement: any) => [requirement.kind, requirement.channel, requirement.state ?? "", requirement.minimumSeconds ?? ""]);
    assert.deepStrictEqual(switching, [["levels", "P1.CompressorPct", "", ""], ["dwell", "P1.CompressorPct", "high", 20], ["dwell", "P1.CompressorPct", "low", 20]],
      "the controller's switching checks and its on-off output's checks are the same conditions, listed once");
  });

  test("keeps the switched equipment's own minimum ON and OFF times as the floor of the designed times", () => {
    const api = setup();
    const loop = api.onOffLoopDefinition(coldRoom, { ...draft, equipmentMinimumOnSeconds: 20, equipmentMinimumOffSeconds: 30 });
    assert.strictEqual(validateControlLoopDefinition(loop, scope).kind, "accepted");
    assert.deepStrictEqual(loop.channels?.inputs.map((input: any) => [input.modelChannelId, input.signalId, input.actuator]),
      [["P1.CompressorPct", "manipulatedVariable", { kind: "on-off", minimum: 0, maximum: 100, minimumOnSeconds: 20, minimumOffSeconds: 30 }]]);
    assert.deepStrictEqual(loop.channels?.outputs.map((output: any) => [output.modelChannelId, output.signalId, output.setpointSignalId]),
      [["P1.RoomC", "processVariable", "setpoint"]], "the identified model's channels are named after their signals");
    const { designOnOffController } = require("../../controlStudio/onOffController");
    const model = { kind: "transfer-function", numerator: [-0.1], denominator: [60, 1], delaySeconds: 0, revision: "room", source: "fitted",
      validation: "validated", evidence: ["recording"], inputs: [{ id: "P1.CompressorPct", unit: "%", operatingRange: { minimum: 0, maximum: 100 } }],
      outputs: [{ id: "P1.RoomC", unit: "degC", operatingRange: { minimum: -10, maximum: 20 } }], operatingPoint: { inputs: [0], outputs: [12] } };
    const design = designOnOffController(loop, model, { referenceChange: -2, maximumError: 1.5, settlingBand: 0.5, settlingTimeSeconds: 120, durationSeconds: 300 });
    assert.deepStrictEqual([design.candidate.parameters["P1.MinimumOnS"], design.candidate.parameters["P1.MinimumOffS"]], [20, 30],
      "a compressor's restart protection is never shortened by tuning");
  });
});
