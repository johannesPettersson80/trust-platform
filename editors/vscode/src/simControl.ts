import * as os from "os";
import * as path from "path";
import { createHash, randomBytes } from "crypto";

// Per-workspace local-simulator control credentials.
//
// The trust-debug adapter starts a full ControlServer on launch (serving comm.schema /
// comm.apply / fleet.topology). We pin that server to a known per-workspace Unix socket and a
// random in-memory workspace auth token, injected into the debug launch config. The Network Canvas then
// connects with the same endpoint + token so it can read the live sim AND apply device setup.
//
// The value is cached per workspace so the launch-config injection (debug.ts) and the canvas
// panel resolve the *same* endpoint + token regardless of call order. Unix uses a per-workspace
// socket; Windows uses a deterministic per-workspace loopback TCP port. Both keep one random token
// per workspace only in memory and inject it into launch configurations without logging it.

export interface SimControl {
  endpoint: string;
  authToken: string;
}

/**
 * Returns the exact control channel injected into an active simulator launch.
 *
 * The debug session is the authority here: deriving the credentials again from
 * a workspace path can select the wrong folder in a multi-root workspace and
 * can leave Devices & Connections probing an unrelated configured runtime.
 */
export function simulatorControlFromDebugConfiguration(
  configuration: unknown
): SimControl | undefined {
  if (!isRecord(configuration) || configuration.request !== "launch") {
    return undefined;
  }
  const endpoint = normalizedString(configuration.controlEndpoint);
  const authToken = normalizedString(configuration.controlAuthToken);
  return endpoint && authToken ? { endpoint, authToken } : undefined;
}

const cache = new Map<string, SimControl>();

export function localSimControl(workspacePath?: string): SimControl | undefined {
  const rawKey = workspacePath ?? "default";
  const key = process.platform === "win32" ? rawKey.toLowerCase() : rawKey;
  const existing = cache.get(key);
  if (existing) {
    return existing;
  }
  const hash = createHash("sha1").update(key).digest("hex").slice(0, 8);
  const endpoint =
    process.platform === "win32"
      ? `tcp://127.0.0.1:${windowsWorkspacePort(hash)}`
      : `unix://${path.join(os.tmpdir(), `trust-debug-${hash}.sock`)}`;
  const creds: SimControl = {
    endpoint,
    authToken: randomBytes(18).toString("hex"),
  };
  cache.set(key, creds);
  return creds;
}

function windowsWorkspacePort(hash: string): number {
  const base = Number.parseInt(hash, 16);
  return 20_000 + (base % 20_000);
}

function normalizedString(value: unknown): string | undefined {
  return typeof value === "string" && value.trim().length > 0
    ? value.trim()
    : undefined;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}
