import type { ControlLoopDefinition } from "./loopDefinition";
import type { ControllerInitialization } from "./controllerInitialState";

/** The measurement a library PID holds (Specification 35 §17.1): `<instance>.StateValue.ProcessValue`, what its last Update
 * received, whatever the program computed it from (a 4–20 mA input, a sensor block, a filter). */
export function pidOwnMeasurement(instance: string): string {
  return `${instance}.StateValue.ProcessValue`;
}

/** The loop's library PID whose own measurement `symbol` is, if it is one. */
export function measuredPid(loop: ControlLoopDefinition, symbol: string): string | undefined {
  if (loop.controller.type !== "Control.PID") return undefined;
  const instances = [loop.controller.instance, ...(loop.controller.group?.members.map(member => member.instance) ?? [])];
  return instances.find(instance => pidOwnMeasurement(instance).toUpperCase() === symbol.toUpperCase());
}

/** Where a simulated plant writes a measurement (Specification 33 §13.2): its address; a library PID's own measurement is given
 * to that PID as what its Update receives (`pid:<instance>`); any other program variable is written itself. */
export function plantFeedbackTarget(loop: ControlLoopDefinition, signal: { symbol: string; address?: string }): string {
  if (signal.address) return signal.address;
  const pid = measuredPid(loop, signal.symbol);
  return pid ? `pid:${pid}` : signal.symbol;
}

/** The controller states a transfer initializes: a state the loop measures through, its PID's own measurement when the loop
 * observes that (§17.1), is read at the task boundary like every measurement, never written. A loop that measures another
 * signal, such as the lessons' input, initializes all of its states. */
export function initializedStates(loop: ControlLoopDefinition, definition: ControllerInitialization): ControllerInitialization["states"] {
  const measured = new Set([loop.signals.processVariable, ...(loop.signals.additional ?? []).filter(signal => signal.role === "PV")]
    .map(signal => signal.symbol.toUpperCase()));
  return definition.states.filter(state => !measured.has(state.symbol.toUpperCase()));
}
