// Named HMI sessions held by the extension host (§11, §12.8).
import type { SocketLike } from "../display/runtimeDisplay";
export interface LiveEndpoint {
  simulator?: boolean;
  url: string;
  endpoint: string;
  token?: string;
  label: string;
}
export interface LiveReply {
  status: number;
  body: unknown;
  cookie?: string;
}
export interface LiveSessionDeps {
  target(): Promise<LiveEndpoint | undefined>;
  openSocket?(target: LiveEndpoint, path: string, cookie?: string): SocketLike;
  request(
    target: LiveEndpoint,
    method: "GET" | "POST",
    path: string,
    body: unknown,
    cookie?: string,
  ): Promise<LiveReply>;
}
const refused = (status: number, error: string): LiveReply => ({ status, body: { error } });
/** What Live mode forwards (§11, §21.3 item 7): the display's reads, the named session's login and
 * logout, and the writes that need that session: commands, alarm actions and settings (§21.4). */
const READS = /^\/api\/hmi\/(?:page|popup|session|alarms|history|remote)(?:\?[^#\r\n]*)?$/;
const WRITES = /^\/api\/hmi\/(?:login|logout|command|alarm|settings)$/;
const NEEDS_SESSION = new Set(["/api/hmi/command", "/api/hmi/alarm", "/api/hmi/settings"]);

/** One editor's session, pinned to the first selected target; never revived after stop. */
export class LiveSession {
  private cookie: string | undefined;
  private identity: string | undefined;
  private stopped = false;
  private readonly sockets = new Set<SocketLike>();
  constructor(private readonly deps: LiveSessionDeps) {}

  async request(method: "GET" | "POST", path: string, body?: unknown): Promise<LiveReply> {
    if (this.stopped) return refused(409, "Live connection ended. Reopen Live mode to connect.");
    const allowed = method === "GET" ? READS.test(path) : WRITES.test(path);
    if (!allowed) return refused(403, "This address is not part of Live operation.");
    const target = await this.target();
    if (!target)
      return refused(409, "The run target changed or the Live connection ended. Reopen Live mode to connect.");
    if (method === "POST" && NEEDS_SESSION.has(path) && !this.cookie) {
      return refused(401, "Log in with a named HMI user to operate.");
    }
    const answer = await this.deps.request(target, method, path, body, this.cookie);
    if (this.stopped) return refused(409, "Live connection ended.");
    if (path === "/api/hmi/login" && answer.status === 200 && !(body as { peer?: unknown } | undefined)?.peer) {
      this.cookie = /^trust_hmi_session=[^;\s]+/.exec(answer.cookie ?? "")?.[0];
    }
    if (path === "/api/hmi/logout") this.cookie = undefined;
    return { status: answer.status, body: answer.body };
  }

  async openSocket(path: string): Promise<SocketLike | undefined> {
    if (!/^\/ws\/hmi\?(?:page|popup|faceplate)=[^#\r\n]+$/.test(path)) return undefined;
    const target = await this.target();
    if (!target || !this.deps.openSocket) return undefined;
    const raw = this.deps.openSocket(target, path, this.cookie);
    const stream: SocketLike = {
      onmessage: null,
      onclose: null,
      close: () => {
        raw.onmessage = null;
        raw.onclose = null;
        raw.close();
        this.sockets.delete(stream);
      },
    };
    raw.onmessage = (event) => {
      if (!this.stopped) stream.onmessage?.(event);
    };
    raw.onclose = () => this.stop();
    this.sockets.add(stream);
    return stream;
  }

  async isCurrentTarget(): Promise<boolean> {
    return (await this.target()) !== undefined;
  }

  private async target(): Promise<LiveEndpoint | undefined> {
    if (this.stopped) return undefined;
    const target = await this.deps.target();
    if (this.stopped) return undefined;
    if (!target) {
      this.stop();
      return undefined;
    }
    const identity = JSON.stringify([target.endpoint, target.url, target.token]);
    if (this.identity !== undefined && this.identity !== identity) {
      this.stop();
      return undefined;
    }
    this.identity = identity;
    return target;
  }

  stop(): void {
    this.stopped = true;
    this.cookie = undefined;
    for (const socket of [...this.sockets]) {
      socket.close();
      socket.onclose?.();
    }
  }
}
