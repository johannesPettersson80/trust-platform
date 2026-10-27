import { projectRequest } from "./projectRequest";

// The control endpoint and token of the local simulator (specification 25 §6.8). The debug adapter
// serves the simulator's control API (comm.schema, comm.apply, fleet.topology) on the endpoint that
// the launch configuration names, with the token it carries; Devices & Connections reaches the
// running simulator with the same endpoint and token, which a Unix connection needs for write access.
// The language server names them (`trust-lsp.simulatorControl`); this module keeps its answer per
// workspace folder for the VS Code session, so the launch (debug.ts) and the canvas use the same
// pair whatever their order. On Windows the server names none, and the adapter takes its default.

export interface SimControl {
  endpoint: string;
  authToken: string;
}

const cache = new Map<string, Promise<SimControl | undefined>>();

async function requestSimControl(workspacePath: string | undefined): Promise<SimControl | undefined> {
  const answer = await projectRequest<{ endpoint: string | null; authToken: string | null }>(
    "trust-lsp.simulatorControl",
    { action: "create", workspace: workspacePath ?? null }
  );
  if (!answer.ok) {
    throw new Error(answer.error);
  }
  return answer.endpoint && answer.authToken
    ? { endpoint: answer.endpoint, authToken: answer.authToken }
    : undefined;
}

/** The simulator's endpoint and token for `workspacePath` in this session; none when the server names none or does
 * not answer (it is asked again next time). */
export async function localSimControl(workspacePath?: string): Promise<SimControl | undefined> {
  const key = workspacePath ?? "";
  let pending = cache.get(key);
  if (!pending) {
    pending = requestSimControl(workspacePath);
    cache.set(key, pending);
  }
  try {
    return await pending;
  } catch {
    if (cache.get(key) === pending) {
      cache.delete(key);
    }
    return undefined;
  }
}

/** Why a launch may not bind `endpoint` (a TCP port that a program already holds), as the language server checks it;
 * none when it may, or when the server does not answer. */
export async function launchEndpointProblem(endpoint: unknown): Promise<string | undefined> {
  if (typeof endpoint !== "string" || !endpoint.trim()) {
    return undefined;
  }
  const answer = await projectRequest<{ problem: string | null }>("trust-lsp.simulatorControl", {
    action: "check",
    endpoint,
  });
  return answer.ok ? answer.problem ?? undefined : undefined;
}
