// Deploy from the editor (specification 39, §16.2 item 8): checks the project's bundle, finds the
// selected runtime's web server from the endpoints it reports, and sends the bundle there with the
// runtime's administrator access. The outside calls are passed in.

/** A problem with a file and its line. */
export interface BundleProblem {
  file?: string;
  line?: number;
  message: string;
}

export interface DeployDeps {
  /** The editor check: `trust-lsp.hmiBundle` for the project. */
  bundle(root: string): Promise<{
    ok?: boolean;
    bundle?: unknown;
    version?: string;
    problems?: BundleProblem[];
    errors?: BundleProblem[];
  }>;
  /** The run target the extension has selected and started. */
  target(): Promise<{ endpoint: string; token?: string; label: string; simulator: boolean } | undefined>;
  /** A control request to the target. */
  control(endpoint: string, token: string | undefined, operation: string, params: unknown): Promise<unknown>;
  /** A JSON POST with the administrator access. */
  post(url: string, token: string | undefined, body: unknown): Promise<{ status: number; body: unknown }>;
}

export type DeployOutcome =
  { ok: true; version: string; target: string } | { ok: false; reason: string; problems?: BundleProblem[] };

/**
 * The web server's address: the listen port on the control endpoint's host, or the listen address
 * itself when it names a host of its own (not 0.0.0.0 or ::).
 */
export function webAddress(controlEndpoint: string, listen: string, tls: boolean): string | undefined {
  const match = /^(?:\[([^\]]+)\]|([^:]+)):(\d+)$/.exec(listen.trim());
  if (!match) return undefined;
  const listenHost = match[1] ?? match[2];
  const port = match[3];
  const anyHost = ["0.0.0.0", "::", ""].includes(listenHost);
  let host = anyHost ? controlHost(controlEndpoint) : listenHost;
  if (host.includes(":")) host = `[${host}]`;
  return `${tls ? "https" : "http"}://${host}:${port}`;
}

/** The control endpoint's host; a local socket is this machine. */
function controlHost(endpoint: string): string {
  const tcp = /^tcp:\/\/(?:\[([^\]]+)\]|([^:/]+))/.exec(endpoint.trim());
  return tcp ? (tcp[1] ?? tcp[2]) : "127.0.0.1";
}

/** The web server the runtime reports for itself in `fleet.topology`. */
export function ownWebServer(topology: unknown): { listen: string; tls: boolean } | undefined {
  const hosts = (topology as { hosts?: unknown[] } | undefined)?.hosts ?? [];
  for (const host of hosts as Array<{ runtimes?: unknown[]; containers?: Array<{ runtimes?: unknown[] }> }>) {
    const runtimes = [
      ...(host.runtimes ?? []),
      ...(host.containers ?? []).flatMap((container) => container.runtimes ?? []),
    ];
    for (const runtime of runtimes as Array<{ endpoints?: unknown[] }>) {
      for (const endpoint of (runtime.endpoints ?? []) as Array<{
        protocol?: string;
        source?: string;
        params?: { listen?: unknown; tls?: unknown };
      }>) {
        if (endpoint.protocol === "web" && endpoint.source === "self" && typeof endpoint.params?.listen === "string") {
          return { listen: endpoint.params.listen, tls: endpoint.params.tls === true };
        }
      }
    }
  }
  return undefined;
}

/** The selected runtime's web server, or why there is none to deploy to. */
async function webServer(
  deps: DeployDeps,
): Promise<{ url: string; token?: string; label: string } | { reason: string }> {
  const target = await deps.target();
  if (!target) return { reason: "No runtime is running: start a runtime to deploy to." };
  if (target.simulator) {
    return {
      reason: "The simulator has no web server: use Live mode to see the page, or start a runtime to deploy to.",
    };
  }
  let topology: unknown;
  try {
    topology = await deps.control(target.endpoint, target.token, "fleet.topology", {});
  } catch (error) {
    return { reason: `${target.label} does not answer: ${error instanceof Error ? error.message : String(error)}` };
  }
  const web = ownWebServer(topology);
  const url = web ? webAddress(target.endpoint, web.listen, web.tls) : undefined;
  if (!url) return { reason: `${target.label} has no web server: enable [runtime.web] in its runtime.toml.` };
  return { url, token: target.token, label: target.label };
}

/** Sends a request to the web server and reads its answer as an outcome. */
async function send(deps: DeployDeps, path: string, body: unknown): Promise<DeployOutcome> {
  const server = await webServer(deps);
  if ("reason" in server) return { ok: false, reason: server.reason };
  let answer: { status: number; body: unknown };
  try {
    answer = await deps.post(`${server.url}${path}`, server.token, body);
  } catch (error) {
    return {
      ok: false,
      reason: `${server.label} cannot be reached: ${error instanceof Error ? error.message : String(error)}`,
    };
  }
  // The runtime answers with the versions now active and kept (§16.4), or why it refused.
  const reply = answer.body as {
    ok?: boolean;
    active?: string | null;
    error?: string;
    problems?: BundleProblem[];
  } | null;
  if (answer.status === 200 && reply?.ok) return { ok: true, version: reply.active ?? "", target: server.label };
  const problems = reply?.problems ?? [];
  const reason =
    reply?.error ??
    (problems.length > 0
      ? `${server.label} found ${problems.length} problem${problems.length === 1 ? "" : "s"} in the bundle.`
      : `${server.label} answered ${answer.status}`);
  return { ok: false, reason, ...(problems.length > 0 ? { problems } : {}) };
}

/** Checks the project's bundle and deploys it to the selected runtime (§16.2). */
export async function deployBundle(root: string, deps: DeployDeps): Promise<DeployOutcome> {
  const checked = await deps.bundle(root);
  const problems = [...(checked.problems ?? []), ...(checked.ok ? [] : (checked.errors ?? []))];
  if (problems.length > 0) {
    return {
      ok: false,
      reason: `The editor check found ${problems.length} problem${problems.length === 1 ? "" : "s"}.`,
      problems,
    };
  }
  if (!checked.bundle) return { ok: false, reason: "The bundle could not be built." };
  return send(deps, "/api/hmi/deploy", checked.bundle);
}

/** Makes the selected runtime's previous bundle active again (§16.2, item 5). */
export async function rollbackBundle(deps: DeployDeps): Promise<DeployOutcome> {
  return send(deps, "/api/hmi/rollback", {});
}
