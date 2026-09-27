import type { ControlLoopDefinition } from "./loopDefinition";
import { decodeControllerInitialization, type ControllerInitialization, type ControllerInitialValue } from "./controllerInitialState";
import { decodePidLaw } from "./controllerLaw";

/** The seven retained states of the shipped Control.PID, distinct from parallel PID history. */
export interface LibraryPidInitialization {
  kind: "control-pid-v1";
  states: ControllerInitialization["states"];
  applicationStates?: ControllerInitialization["states"];
  maximumSnapshotAgeMs: number;
}

export function decodeLibraryPidInitialization(value: unknown): LibraryPidInitialization | undefined {
  if (!value || typeof value !== "object" || Array.isArray(value)) return undefined;
  const entry = value as Record<string, unknown>;
  if (entry.kind !== "control-pid-v1" || !Object.keys(entry).every(key => ["kind", "maximumSnapshotAgeMs", "states", "applicationStates"].includes(key))
    || !Number.isSafeInteger(entry.maximumSnapshotAgeMs)) return undefined;
  const definition = decodeControllerInitialization({ kind: "declared-state-v1", states: entry.states });
  if (!definition || definition.states.length !== 7) return undefined;
  const application = entry.applicationStates === undefined ? undefined
    : decodeControllerInitialization({ kind: "declared-state-v1", states: entry.applicationStates });
  if (entry.applicationStates !== undefined && !application) return undefined;
  return { kind: "control-pid-v1", states: definition.states, maximumSnapshotAgeMs: entry.maximumSnapshotAgeMs as number,
    ...(application ? { applicationStates: application.states } : {}) };
}

/** Validate ownership and initial coordinates before sharing the generic state transaction. */
export function libraryPidInitialization(loop: ControlLoopDefinition): ControllerInitialization {
  const adapter = decodeLibraryPidInitialization(loop.controller.pidInitialization);
  const reference = loop.signals.setpoint, output = loop.signals.manipulatedVariable;
  if (!adapter || !reference || !decodePidLaw(loop.controller.law, loop.parameterBinding)
    || adapter.maximumSnapshotAgeMs < loop.controller.taskPeriodMs || adapter.maximumSnapshotAgeMs > 10000) {
    throw new Error("Declare all seven Control.PID states and compatible timing before Auto commissioning.");
  }
  assertLibraryPidStates(loop.controller.instance, adapter.states, reference.unit, output.unit);
  const states = [...adapter.states, ...(adapter.applicationStates ?? [])];
  const definition = decodeControllerInitialization({ kind: "declared-state-v1", states });
  const root = loop.controller.instance.split(".")[0].toUpperCase() + ".";
  if (!definition || new Set(states.map(state => state.symbol.toUpperCase())).size !== states.length
    || adapter.applicationStates?.some(state => !state.symbol.toUpperCase().startsWith(root))) {
    throw new Error("Declare unique application startup states in the controller's owning program.");
  }
  return definition;
}

/** The same retained-state contract applies to scalar and explicitly grouped library PIDs. */
export function assertLibraryPidStates(instance: string, states: ControllerInitialization["states"],
  outputUnit: string, inputUnit: string, inputIndex = 0, outputIndex = 0): void {
  const expected = new Map<string, { unit: string; initial: ControllerInitialValue }>([
    ["Setpoint", { unit: outputUnit, initial: { source: "model-output", index: outputIndex } }],
    ["SetpointRate", { unit: `${outputUnit}/s`, initial: 0 }],
    ["ProcessValue", { unit: outputUnit, initial: { source: "model-output", index: outputIndex } }],
    ["ProcessValueRate", { unit: `${outputUnit}/s`, initial: 0 }],
    ["FeedForward", { unit: inputUnit, initial: 0 }],
    ["NominalOutput", { unit: inputUnit, initial: { source: "model-input", index: inputIndex } }],
    ["SaturatedOutput", { unit: inputUnit, initial: { source: "model-input", index: inputIndex } }],
  ]);
  if (states.length !== expected.size) throw new Error(`Declare all seven retained states for ${instance}.`);
  for (const [member, contract] of expected) {
    const state = states.find(state => state.symbol === `${instance}.StateValue.${member}`);
    if (!state || state.iec_type !== "LREAL" || state.unit !== contract.unit || state.postScan !== "bounded"
      || state.minimum >= state.maximum || !sameCoordinate(state.initial, contract.initial)) {
      throw new Error(`Control.PID ${instance}.${member} must bind its actual retained state and declared operating-point coordinate.`);
    }
  }
}

function sameCoordinate(actual: ControllerInitialValue, expected: ControllerInitialValue): boolean {
  if (typeof actual === "number" || typeof expected === "number") return actual === expected;
  return actual.source === expected.source && "index" in actual && "index" in expected && actual.index === expected.index;
}
