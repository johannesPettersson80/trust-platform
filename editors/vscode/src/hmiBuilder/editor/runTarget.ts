// The run target the HMI Builder talks to (specification 39, §11 item 6, §16.2 item 8): the
// simulator or runtime the extension has started or attached to, or the configured online runtime
// when it answers.

import type { RuntimeStatusPayload } from "../../io-panel/types";

export interface RunTarget {
  endpoint: string;
  label: string;
  simulator: boolean;
}

/** The selected run target from the runtime status, or none when nothing runs or answers. */
export function runTargetOf(
  status: Pick<RuntimeStatusPayload, "runtimeMode" | "runtimeState" | "targetLabel" | "endpoint" | "endpointReachable">,
): RunTarget | undefined {
  if (!status.endpoint) return undefined;
  const session = status.runtimeState !== "stopped";
  // An online runtime the editor does not debug is reported stopped, but it answers.
  const online = status.runtimeMode === "online" && status.endpointReachable;
  if (!session && !online) return undefined;
  return {
    endpoint: status.endpoint,
    label: status.targetLabel ?? (status.runtimeMode === "simulate" ? "Simulator" : status.endpoint),
    simulator: status.runtimeMode === "simulate",
  };
}
