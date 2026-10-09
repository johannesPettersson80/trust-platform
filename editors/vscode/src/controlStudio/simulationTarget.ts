import type { RuntimeStatusPayload } from "../io-panel/types";

/** Whether a runtime status describes a simulated target (Specification 35, A2.4). The flag is set from the runtime's own
 * report (simulation enabled, all attached I/O simulated) by io-panel/status.ts; no editor setting is read here. */
export function simulationTarget(status: Pick<RuntimeStatusPayload, "simulated">): boolean {
  return status.simulated === true;
}
