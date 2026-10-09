import { runtimeLifecycleService } from "../runtimeLifecycle";
import { sendRuntimeControlRequest } from "../runtimeControlClient";
import { getSelectedRuntimeId } from "../selectedRuntime";
import { simulationTarget } from "./simulationTarget";

/** Captures the reviewed endpoint and token once; later selections cannot reroute it. */
export async function bindRuntimeConnection(runtimeId: string) {
  if (getSelectedRuntimeId() !== runtimeId) throw new Error("The selected runtime does not match this loop.");
  const lifecycle = await runtimeLifecycleService.snapshot();
  if (!lifecycle.status.running) throw new Error("Start the bound runtime before reviewing this operation.");
  const endpoint = lifecycle.status.endpoint;
  const token = await runtimeLifecycleService.controlAuthToken(endpoint);
  if (getSelectedRuntimeId() !== runtimeId) throw new Error("The selected runtime changed before review.");
  const source = simulationTarget(lifecycle.status) ? "simulation" as const : "physical" as const;
  return { source, endpoint, request: (operation: string, params: Record<string, unknown>): Promise<unknown> =>
    sendRuntimeControlRequest(endpoint, token, operation, params, { timeoutMs: 5000 }) };
}
