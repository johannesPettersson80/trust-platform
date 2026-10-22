import type { RuntimeConnection } from "../generated/IdentifyRequest";
import { runtimeLifecycleService, type RuntimeLifecycleSnapshot } from "../../runtimeLifecycle";
import { getSelectedRuntimeId } from "../../selectedRuntime";

/**
 * The runtime connection a live engine request carries (E11.1): the endpoint of the running runtime, its control token, and the
 * runtime the window has selected. The extension only reports these facts; the engine decides the target. A live request whose loop
 * is bound to another runtime is refused by the engine (A2.7, `refuse_other_runtime` in `commands.rs`). The token goes to the engine
 * only, never to the page. Undefined when no runtime is running, so the engine answers the request with its own runtime failure.
 */
export async function studyRuntimeConnection(): Promise<RuntimeConnection | undefined> {
  return runtimeConnectionOf(await runtimeLifecycleService.snapshot());
}

/**
 * The same connection, read from a lifecycle snapshot the caller already holds: one snapshot then gives the connection and the
 * debug session it belongs to (`sessionId`), which the monitor needs together.
 */
export async function runtimeConnectionOf(lifecycle: RuntimeLifecycleSnapshot): Promise<RuntimeConnection | undefined> {
  if (!lifecycle.status.running) return undefined;
  const endpoint = lifecycle.status.endpoint;
  const token = await runtimeLifecycleService.controlAuthToken(endpoint);
  return { endpoint, runtimeId: getSelectedRuntimeId(), ...(token ? { token } : {}) };
}
