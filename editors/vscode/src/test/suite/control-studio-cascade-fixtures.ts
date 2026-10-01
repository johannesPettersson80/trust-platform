/** Shared compiled facts of a two-PID cascade: the outer output feeds the inner setpoint. */
export const TASK = [{ name: "MainTask", intervalMs: 100 }];
export const PARAMETER_FIELDS = ["ProportionalGain", "IntegralGain", "DerivativeGain", "ProportionalSetpointWeight", "DerivativeSetpointWeight",
  "Bias", "OutputMinimum", "OutputMaximum", "RateMinimum", "RateMaximum", "TrackingTime"];
export const STATE_FIELDS = ["Setpoint", "SetpointRate", "ProcessValue", "ProcessValueRate", "FeedForward", "NominalOutput", "SaturatedOutput"];
export const scope = { projectId: "project:setup", runtimeId: "runtime-1" };

export function pidSymbols(instance: string, record: string) {
  return [
    ...PARAMETER_FIELDS.map(field => ({ symbol: `${record}.${field}`, type: "LREAL" })),
    ...STATE_FIELDS.map(field => ({ symbol: `${instance}.StateValue.${field}`, type: "LREAL" })),
  ];
}

/** Two library PIDs where the outer call's output feeds the inner setpoint through a limit. */
export const cascade = {
  symbols: [
    { symbol: "P1.VesselC", type: "LREAL", address: "%IL0" },
    { symbol: "P1.JacketC", type: "LREAL", address: "%IL8" },
    { symbol: "P1.VesselSetpointC", type: "LREAL", address: "%ML0" },
    { symbol: "P1.JacketSetpointC", type: "LREAL", address: "%ML8" },
    { symbol: "P1.HeaterPct", type: "LREAL", address: "%QL0" },
    { symbol: "P1.OuterAuto", type: "BOOL", address: "%MX40.0" },
    { symbol: "P1.InnerAuto", type: "BOOL", address: "%MX40.1" },
    { symbol: "P1.OuterManual", type: "LREAL", address: "%ML16" },
    { symbol: "P1.InnerManual", type: "LREAL", address: "%ML24" },
    ...pidSymbols("P1.Outer", "P1.OuterParameters"),
    ...pidSymbols("P1.Inner", "P1.InnerParameters"),
  ],
  controllers: [{ instance: "P1", type: "Main", tasks: TASK }, { instance: "P1.Inner", type: "Control.PID", tasks: TASK },
    { instance: "P1.Outer", type: "Control.PID", tasks: TASK }],
  links: [
    { instance: "P1.Outer", method: "Configure", argument: "Parameters", record: "P1.OuterParameters", output: null, fields: {} },
    { instance: "P1.Inner", method: "Configure", argument: "Parameters", record: "P1.InnerParameters", output: null, fields: {} },
    { instance: "P1.Outer", method: "Update", argument: "Input", record: "P1.OuterInput", output: "P1.JacketSetpointC",
      fields: { ManualOutput: ["P1.OuterManual"], ProcessValue: ["P1.VesselC"], Setpoint: ["P1.VesselSetpointC"] } },
    { instance: "P1.Inner", method: "Update", argument: "Input", record: "P1.InnerInput", output: "P1.HeaterPct",
      fields: { ManualOutput: ["P1.InnerManual"], ProcessValue: ["P1.JacketC"], Setpoint: ["P1.JacketSetpointC"] } },
  ],
};

export const cascadeDraft = {
  ...scope, name: "Vessel temperature", action: "direct",
  outer: { instance: "P1.Outer", parameterRecord: "P1.OuterParameters", mode: { symbol: "P1.OuterAuto", whenTrue: "Auto" }, manualOutput: "P1.OuterManual",
    processVariable: { symbol: "P1.VesselC", unit: "degC", minimum: 0, maximum: 90 }, setpoint: { symbol: "P1.VesselSetpointC" }, experimentDurationSeconds: 30 },
  inner: { instance: "P1.Inner", parameterRecord: "P1.InnerParameters", mode: { symbol: "P1.InnerAuto", whenTrue: "Auto" }, manualOutput: "P1.InnerManual",
    processVariable: { symbol: "P1.JacketC", unit: "degC", minimum: 0, maximum: 90 },
    manipulatedVariable: { symbol: "P1.HeaterPct", unit: "%", minimum: 0, maximum: 100 }, experimentDurationSeconds: 3 },
};

/** The same cascade tuned together: one identification step and duration for the plant input (Specification 35 §17.11). */
export function cascadeTogetherDraft() {
  const { experimentDurationSeconds: _outer, ...outer } = cascadeDraft.outer;
  const { experimentDurationSeconds: _inner, ...inner } = cascadeDraft.inner;
  return { ...scope, name: cascadeDraft.name, action: cascadeDraft.action, outer, inner, experimentStep: 10, experimentDurationSeconds: 120 };
}
