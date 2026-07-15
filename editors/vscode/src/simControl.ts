import * as os from "os";
import * as path from "path";
import { createHash, randomBytes } from "crypto";

// Per-workspace local-simulator control credentials.
//
// The trust-debug adapter starts a full ControlServer on launch (serving comm.schema /
// comm.apply / fleet.topology). We pin that server to a known per-workspace Unix socket and a
// random per-session auth token, injected into the debug launch config. The Network Canvas then
// connects with the same endpoint + token so it can read the live sim AND apply device setup —
// a Unix connection with a matching token is granted write access by the runtime, whereas an
// unauthenticated Unix connection is Viewer-only.
//
// The value is cached per workspace so the launch-config injection (debug.ts) and the canvas
// panel resolve the *same* endpoint + token regardless of call order. Unix only: on Windows the
// adapter's default control endpoint is an auth-gated TCP port, so we return undefined and the
// canvas keeps its honest "select a runtime" copy there.

export interface SimControl {
  endpoint: string;
  authToken: string;
}

const cache = new Map<string, SimControl>();

export function localSimControl(workspacePath?: string): SimControl | undefined {
  if (process.platform === "win32") {
    return undefined;
  }
  const key = workspacePath ?? "default";
  const existing = cache.get(key);
  if (existing) {
    return existing;
  }
  const hash = createHash("sha1").update(key).digest("hex").slice(0, 8);
  const socketPath = path.join(os.tmpdir(), `trust-debug-${hash}.sock`);
  const creds: SimControl = {
    endpoint: `unix://${socketPath}`,
    authToken: randomBytes(18).toString("hex"),
  };
  cache.set(key, creds);
  return creds;
}
