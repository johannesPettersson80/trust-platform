import {
  isStructuralRuntimeLifecycleChange,
  type RuntimeLifecycleChange,
} from "../runtimeLifecycle";
import type { LifecyclePhase } from "../lifecycleEntryFailure";

/**
 * Network Canvas performs slow schema/topology reads and already polls at a
 * bounded interval. Per-scan I/O events must not invalidate those reads or a
 * continuous PLC scan can starve the Starting -> Running render forever.
 */
export function shouldRefreshNetworkCanvasForLifecycleChange(
  change: RuntimeLifecycleChange
): boolean {
  return isStructuralRuntimeLifecycleChange(change);
}

export interface ImmediateSimulatorLifecycleProjection {
  readonly running: boolean;
  readonly starting: boolean;
  readonly stopped: boolean;
}

/** Remote attach state is topology-owned; every local Simulator phase posts immediately. */
export function immediateSimulatorLifecycleProjection(
  phase: LifecyclePhase
): ImmediateSimulatorLifecycleProjection | undefined {
  if (phase === "connected") {
    return undefined;
  }
  return {
    running: phase === "running",
    starting: phase === "starting",
    stopped: phase === "stopped",
  };
}
