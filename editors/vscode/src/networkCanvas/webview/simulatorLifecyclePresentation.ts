/**
 * A Simulator is a process that this extension starts and stops. Its live
 * state is therefore a lifecycle state (Running), not a connection state
 * (Online/Connected). Other runtimes keep connection vocabulary because the
 * extension may only be attached to them.
 */
export function simulatorLifecycleLabel(
  health: string,
  mode: string
): string | undefined {
  if (mode.trim().toLowerCase() !== "simulate") {
    return undefined;
  }
  return health === "connected" ? "Running" : undefined;
}
