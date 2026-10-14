import type { RuntimeConnection } from "../generated/IdentifyRequest";
import type { ControlLoopDefinition } from "../loopDefinition";
import { resolveStudyRuntimeTarget } from "../panelWorkflowSupport";
import { runtimeLifecycleService } from "../../runtimeLifecycle";

/**
 * The runtime connection a live engine request carries (E11.1): the study's pinned endpoint (A2.7) and its control token.
 * The token goes to the engine only, never to the page. Undefined when no runtime is running, so the engine answers the
 * request with its own runtime failure.
 */
export async function studyRuntimeConnection(loop: ControlLoopDefinition): Promise<RuntimeConnection | undefined> {
  const target = await resolveStudyRuntimeTarget(loop).catch(() => undefined);
  if (!target) return undefined;
  const token = await runtimeLifecycleService.controlAuthToken(target.endpoint);
  return token ? { endpoint: target.endpoint, token } : { endpoint: target.endpoint };
}
