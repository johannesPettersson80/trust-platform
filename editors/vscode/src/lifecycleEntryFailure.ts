export type LifecycleRuntimeState = "running" | "connected" | "stopped";
export type LifecyclePhase = LifecycleRuntimeState | "starting";
export type LifecycleAction =
  | "start"
  | "connect"
  | "stop"
  | "disconnect"
  | "other";

export function runtimeLifecyclePhase(
  starting: boolean,
  sessionRequest: unknown,
  sessionAccepted: boolean,
): LifecyclePhase {
  if (starting) {
    return "starting";
  }
  if (!sessionAccepted) {
    return "stopped";
  }
  return sessionRequest === "attach" ? "connected" : "running";
}

/**
 * Reconciles an entry-point-local error with the shared lifecycle authority.
 * A later successful/starting transition from another surface supersedes an
 * old local error; a current lifecycle failure always wins.
 */
export function effectiveLifecycleEntryFailure<T>(
  localFailure: T | undefined,
  lifecycleFailure: T | undefined,
  action: LifecycleAction | undefined,
  phase: LifecyclePhase
): T | undefined {
  if (lifecycleFailure !== undefined) {
    return lifecycleFailure;
  }
  return lifecycleActionSucceeded(action, phase)
    ? undefined
    : localFailure;
}

export function lifecycleActionSucceeded(
  action: LifecycleAction | undefined,
  phase: LifecyclePhase
): boolean {
  switch (action) {
    case "start":
      return phase === "starting" || phase === "running";
    case "connect":
      return phase === "starting" || phase === "connected";
    case "stop":
    case "disconnect":
      return phase === "stopped";
    case "other":
      // Canvas-only I/O actions (for example, adding the simulated device
      // while the Simulator is stopped) can leave a local recovery message.
      // A later structural Simulator transition proves that message is stale.
      return phase === "starting" || phase === "running";
    default:
      return false;
  }
}
