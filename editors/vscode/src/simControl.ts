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
