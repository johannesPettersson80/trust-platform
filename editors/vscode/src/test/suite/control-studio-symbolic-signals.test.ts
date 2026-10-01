import * as assert from "assert";
import { validateControlLoopDefinition } from "../../controlStudio/loopDefinition";

/** Loaded dynamically so that a missing capability fails at the behaviour assertion. */
function setup(): any {
  try { return require("../../controlStudio/loopSetup/pidLoopSetup"); } catch { return {}; }
}

const TASK = [{ name: "MainTask", intervalMs: 100 }];
const PARAMETERS = ["ProportionalGain", "IntegralGain", "DerivativeGain", "ProportionalSetpointWeight", "DerivativeSetpointWeight",
  "Bias", "OutputMinimum", "OutputMaximum", "RateMinimum", "RateMaximum", "TrackingTime"];
const STATES = ["Setpoint", "SetpointRate", "ProcessValue", "ProcessValueRate", "FeedForward", "NominalOutput", "SaturatedOutput"];
const scope = { projectId: "project:symbolic", runtimeId: "runtime-1" };

/**
 * A heater whose PID output is an internal variable that the program then limits onto the physical output, and whose
 * controller measurement is computed by the program from the addressed temperature.
 */
const catalog = {
  symbols: [
    { symbol: "P1.TemperatureC", type: "LREAL", address: "%IL8" },
    { symbol: "P1.FilteredC", type: "LREAL" },
    { symbol: "P1.OperatorC", type: "LREAL" },
    { symbol: "P1.ControllerPct", type: "LREAL" },
    { symbol: "P1.HeaterPct", type: "LREAL", address: "%QL0" },
    { symbol: "P1.SetpointC", type: "LREAL" },
    { symbol: "P1.ManualOutput", type: "LREAL", address: "%ML72" },
    { symbol: "P1.AutoMode", type: "BOOL", address: "%MX96.0" },
    ...PARAMETERS.map(field => ({ symbol: `P1.Parameters.${field}`, type: "LREAL" })),
    ...STATES.map(field => ({ symbol: `P1.Controller.StateValue.${field}`, type: "LREAL" })),
  ],
  controllers: [{ instance: "P1", type: "Main", tasks: TASK }, { instance: "P1.Controller", type: "Control.PID", tasks: TASK }],
  links: [],
  // The program assigns the PID output, the limited heater command and the filtered measurement.
  writes: ["P1.ControllerPct", "P1.HeaterPct", "P1.FilteredC"],
};
const draft = (processVariable: string) => ({ ...scope, name: "Heater", action: "direct", instance: "P1.Controller", parameterRecord: "P1.Parameters",
  processVariable: { symbol: processVariable, unit: "degC", minimum: 0, maximum: 100 },
  manipulatedVariable: { symbol: "P1.ControllerPct", unit: "%", minimum: 0, maximum: 100 },
  setpoint: { symbol: "P1.SetpointC" }, mode: { symbol: "P1.AutoMode", whenTrue: "Auto" }, manualOutput: "P1.ManualOutput",
  experimentStep: 10, experimentDurationSeconds: 20 });

suite("Control Studio symbolic signals", () => {
  test("sets up a loop on a controller output and setpoint that have no I/O address (Specification 35 §17.1)", () => {
    const api = setup();
    let loop: any;
    assert.doesNotThrow(() => { loop = api.pidLoopDefinition(catalog, draft("P1.TemperatureC")); });
    assert.strictEqual(validateControlLoopDefinition(loop, scope).kind, "accepted");
    assert.deepStrictEqual([loop.signals.manipulatedVariable.symbol, loop.signals.manipulatedVariable.address, loop.signals.setpoint.address],
      ["P1.ControllerPct", undefined, undefined]);
  });

  test("refuses a measurement the program computes, which a simulated measurement could not reach", () => {
    assert.throws(() => setup().pidLoopDefinition(catalog, draft("P1.FilteredC")),
      /P1\.FilteredC is computed by the program.*overwritten.*measurement it is computed from/);
  });

  test("accepts a measurement without an address that the program only reads", () => {
    const loop = setup().pidLoopDefinition(catalog, draft("P1.OperatorC"));
    assert.strictEqual(validateControlLoopDefinition(loop, scope).kind, "accepted");
    assert.strictEqual(loop.signals.processVariable.address, undefined);
  });

  test("validates on the model through the program variables themselves and records them by symbol", () => {
    const loop = setup().pidLoopDefinition(catalog, draft("P1.OperatorC"));
    const { engineerVerificationPlan } = require("../../controlStudio/engineerVerificationPlan");
    const channel = (id: string, unit: string) => ({ id, unit, operatingRange: { minimum: 0, maximum: 100 } });
    const model = { kind: "transfer-function", numerator: [0.5], denominator: [10, 1], delaySeconds: 0, revision: "heater", source: "fitted",
      validation: "validated", evidence: ["recording"], inputs: [channel("P1.ControllerPct", "%")], outputs: [channel("P1.OperatorC", "degC")],
      operatingPoint: { inputs: [0], outputs: [20] } };
    const values = Object.fromEntries(loop.parameterBinding.parameters.map((parameter: any) => [parameter.symbol,
      Math.min(parameter.maximum, Math.max(parameter.minimum, parameter.symbol.endsWith("ProportionalGain") ? 1 : 0))]));
    const plan = engineerVerificationPlan(loop, model, { referenceChange: 5, maximumError: 1, settlingBand: 0.5, settlingTimeSeconds: 60, durationSeconds: 120 }, values);
    const plant = String(plan.request.simulation_override.simulation_toml);
    assert.match(plant, /command_sources = \["P1\.ControllerPct"\]/, `the model reads the controller output by its symbol: ${plant}`);
    assert.match(plant, /feedback_targets = \["P1\.OperatorC"\]/, "and writes the measurement by its symbol");
    const { loopInspectionBindings } = require("../../controlStudio/loopInspectionBindings");
    const recorded = loopInspectionBindings(loop).filter((binding: any) => ["processVariable", "manipulatedVariable", "setpoint"].includes(binding.id));
    assert.deepStrictEqual(recorded.map((binding: any) => [binding.id, binding.symbol, binding.address]),
      [["setpoint", "P1.SetpointC", undefined], ["processVariable", "P1.OperatorC", undefined], ["manipulatedVariable", "P1.ControllerPct", undefined]]);
  });
});
