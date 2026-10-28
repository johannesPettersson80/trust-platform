import * as fs from "fs";
import * as path from "path";

import { pidLoop } from "./control-studio-pid-fixtures";

export function mimoLoop(): any {
  const loop = pidLoop();
  loop.schemaVersion = 2;
  loop.signals = { ...loop.signals, additional: [
    { id: "mv-secondary", channel: "secondary", role: "MV", symbol: "Main.MV2", address: "%QL8", iecType: "LREAL", direction: "write", unit: "V", range: { minimum: -20, maximum: 20 } },
    { id: "pv-secondary", channel: "secondary", role: "PV", symbol: "Main.PV2", address: "%IL8", iecType: "LREAL", direction: "read", unit: "m", range: { minimum: -20, maximum: 20 } },
    { id: "sp-secondary", channel: "secondary", role: "SP", symbol: "Main.SP2", address: "%ML8", iecType: "LREAL", direction: "read", unit: "m", range: { minimum: -20, maximum: 20 } },
  ] };
  loop.channels = {
    inputs: [
      { modelChannelId: "u1", signalId: "manipulatedVariable", actuator: { kind: "continuous", minimum: -20, maximum: 20 } },
      { modelChannelId: "u2", signalId: "mv-secondary", actuator: { kind: "continuous", minimum: -20, maximum: 20 } },
    ],
    outputs: [
      { modelChannelId: "y1", signalId: "processVariable", setpointSignalId: "setpoint" },
      { modelChannelId: "y2", signalId: "pv-secondary", setpointSignalId: "sp-secondary" },
    ],
  };
  loop.id = "control-studio-pid-group";
  loop.revision = `${loop.revision}-group`;
  loop.parameterBinding.loop_id = loop.id;
  return loop;
}

export function member(instance: string, inputChannelId: string, outputChannelId: string): Record<string, unknown> {
  return {
    instance, type: "Control.PID", inputChannelId, outputChannelId,
    modeSymbol: `${instance}.Manual`,
    parameterSymbols: [`${instance}.Kp`, `${instance}.Ki`, `${instance}.Kd`],
    applicationStateSymbols: [`${instance}.Integral`, `${instance}.PreviousError`],
  };
}

export function declaredPidGroup(): Record<string, unknown> {
  return { members: [member("Main", "u1", "y1"), member("Main.Secondary", "u2", "y2")] };
}

function renameP1ToP2<T>(value: T): T {
  return JSON.parse(JSON.stringify(value).replace(/P1\./g, "P2.")) as T;
}

/** Shared declared group fixture for UI and native adapter tests. */
export function projectPidGroupLoop(): any {
  const loop = JSON.parse(fs.readFileSync(path.resolve(__dirname, "../../../../../examples/control-pid-commissioning/control-loop.json"), "utf8"));
  loop.id = `${loop.id}-pid-group`;
  loop.revision = `${loop.revision}-pid-group`;
  loop.parameterBinding.loop_id = loop.id;
  loop.parameterBinding.mode.symbol = "P1.AutoMode";
  loop.controller.modeSignal = "P1.AutoMode";
  const primaryInitialization = loop.controller.pidInitialization;
  const primaryStates = [...primaryInitialization.states, ...(primaryInitialization.applicationStates ?? [])];
  const secondaryStates = renameP1ToP2(primaryStates).map((state: any) => ({
    ...state,
    ...(state.initial && typeof state.initial === "object" && ["model-input", "model-output"].includes(state.initial.source)
      ? { initial: { ...state.initial, index: 1 } } : {}),
  }));
  const primaryParameters = structuredClone(loop.parameterBinding.parameters);
  const secondaryParameters = renameP1ToP2(primaryParameters);
  loop.parameterBinding.parameters = [...primaryParameters, ...secondaryParameters];
  loop.signals.additional = [
    { id: "mv-secondary", channel: "secondary", role: "MV", symbol: "P2.ManipulatedValue", address: "%QL8", iecType: "LREAL", direction: "write", unit: "%", range: { minimum: 0, maximum: 100 } },
    { id: "pv-secondary", channel: "secondary", role: "PV", symbol: "P2.ProcessValue", address: "%IL8", iecType: "LREAL", direction: "read", unit: "°C", range: { minimum: 0, maximum: 100 } },
    { id: "sp-secondary", channel: "secondary", role: "SP", symbol: "P2.Setpoint", address: "%ML8", iecType: "LREAL", direction: "read", unit: "°C", range: { minimum: 0, maximum: 100 } },
  ];
  loop.channels = {
    inputs: [
      { modelChannelId: "u1", signalId: "manipulatedVariable", actuator: { kind: "continuous", minimum: 0, maximum: 100 } },
      { modelChannelId: "u2", signalId: "mv-secondary", manualOutputSignal: "P2.ManualOutput", manualOutputType: "LREAL", actuator: { kind: "continuous", minimum: 0, maximum: 100 } },
    ],
    outputs: [
      { modelChannelId: "y1", signalId: "processVariable", setpointSignalId: "setpoint" },
      { modelChannelId: "y2", signalId: "pv-secondary", setpointSignalId: "sp-secondary" },
    ],
  };
  loop.controller.design = "project";
  delete loop.controller.law;
  delete loop.controller.pidInitialization;
  loop.controller.project = {
    family: "paired-PI", convention: "project-owned-native-controller",
    initialization: { kind: "declared-state-v1", states: [...primaryStates, ...secondaryStates] },
    axes: [], objective: { requirementId: "tracking-error", direction: "minimize" },
    maximumCandidates: 1, maximumCycles: 1000, wallBudgetMs: 1000,
  };
  loop.controller.group = {
    members: [
      { instance: "P1.Controller", type: "Control.PID", inputChannelId: "u1", outputChannelId: "y1", modeSymbol: "P1.AutoMode",
        parameterSymbols: primaryParameters.map((parameter: { symbol: string }) => parameter.symbol),
        applicationStateSymbols: primaryStates.map((state: { symbol: string }) => state.symbol) },
      { instance: "P2.Controller", type: "Control.PID", inputChannelId: "u2", outputChannelId: "y2", modeSymbol: "P1.AutoMode",
        parameterSymbols: secondaryParameters.map((parameter: { symbol: string }) => parameter.symbol),
        applicationStateSymbols: secondaryStates.map((state: { symbol: string }) => state.symbol) },
    ],
  };
  return JSON.parse(JSON.stringify(loop).replace(/P2\.(\w+)/g, (_match, name: string) => `P1.${name === "Controller" ? "Secondary" : `Secondary${name}`}`));
}
