import { pidSymbols, scope, TASK } from "./control-studio-cascade-fixtures";

/** A water heater held by one library PID whose program adds a lead/lag feedforward from the measured heat extraction. */
export const heater = {
  symbols: [
    { symbol: "P1.WaterC", type: "LREAL", address: "%IL0" },
    { symbol: "P1.SetpointC", type: "LREAL", address: "%ML0" },
    { symbol: "P1.HeaterPct", type: "LREAL", address: "%QL0" },
    { symbol: "P1.LoadKw", type: "LREAL", address: "%ML8" },
    { symbol: "P1.Auto", type: "BOOL", address: "%MX40.0" },
    { symbol: "P1.ManualPct", type: "LREAL", address: "%ML16" },
    { symbol: "P1.FeedforwardGain", type: "LREAL" },
    { symbol: "P1.LeadS", type: "LREAL" },
    { symbol: "P1.LagS", type: "LREAL" },
    { symbol: "P1.FilteredLoadKw", type: "LREAL" },
    ...pidSymbols("P1.Loop", "P1.Parameters"),
  ],
  controllers: [{ instance: "P1", type: "Main", tasks: TASK }, { instance: "P1.Loop", type: "Control.PID", tasks: TASK }],
  links: [
    { instance: "P1.Loop", method: "Configure", argument: "Parameters", record: "P1.Parameters", output: null, fields: {} },
    { instance: "P1.Loop", method: "Update", argument: "Input", record: "P1.Input", output: "P1.HeaterPct",
      fields: { ManualOutput: ["P1.ManualPct"], ProcessValue: ["P1.WaterC"], Setpoint: ["P1.SetpointC"] } },
  ],
};

/** The setup form's Feedforward structure for that heater (Specification 35 §17.12). */
export function feedforwardDraft(withLeadLag = true) {
  return {
    ...scope, name: "Water temperature", instance: "P1.Loop", action: "direct", parameterRecord: "P1.Parameters",
    processVariable: { symbol: "P1.WaterC", unit: "degC", minimum: 0, maximum: 80 },
    manipulatedVariable: { symbol: "P1.HeaterPct", unit: "%", minimum: 0, maximum: 100 },
    setpoint: { symbol: "P1.SetpointC" }, mode: { symbol: "P1.Auto", whenTrue: "Auto" }, manualOutput: "P1.ManualPct",
    disturbance: { symbol: "P1.LoadKw", unit: "kW", minimum: 0, maximum: 50 }, disturbanceCommand: "P1.LoadKw", disturbanceStep: 10,
    feedforward: { gain: "P1.FeedforwardGain", ...(withLeadLag ? { lead: "P1.LeadS", lag: "P1.LagS", filtered: "P1.FilteredLoadKw" } : {}) },
    experimentStep: 10, experimentDurationSeconds: 120,
  };
}

/**
 * The heater's model: 0.2 °C per % through an 8 s lag, −0.1 °C per kW of extraction through a 3 s lag, at rest at
 * 35 °C with 80 % and 10 kW. The exact feedforward is 0.5 %/kW with an 8 s lead and a 3 s lag.
 */
export const heaterModel: any = { kind: "continuous-state-space", revision: "heater", source: "fitted", validation: "validated", evidence: ["recording"],
  delaySeconds: 0, a: [[-1 / 8, 0], [0, -1 / 3]], b: [[0.2 / 8, 0], [0, -0.1 / 3]], c: [[1, 1]], d: [[0, 0]],
  states: [{ id: "heater", unit: "degC", operatingRange: { minimum: -80, maximum: 80 } }, { id: "load", unit: "degC", operatingRange: { minimum: -80, maximum: 80 } }],
  inputs: [{ id: "P1.HeaterPct", unit: "%", operatingRange: { minimum: 0, maximum: 100 } }, { id: "P1.LoadKw", unit: "kW", operatingRange: { minimum: 0, maximum: 50 } }],
  outputs: [{ id: "P1.WaterC", unit: "degC", operatingRange: { minimum: 0, maximum: 80 } }],
  operatingPoint: { inputs: [80, 10], outputs: [35] } };

export const heaterTask = { referenceChange: 1, maximumError: 0.3, settlingBand: 0.1, settlingTimeSeconds: 40, durationSeconds: 80 };
