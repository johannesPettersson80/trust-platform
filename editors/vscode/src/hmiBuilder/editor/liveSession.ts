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
  /** The answer's `Set-Cookie` fields, one each. */
  cookies?: string[];
}
/** What the language server decides about the session's traffic, from facts only: never the password or the session
 * cookie, which a language client's `verbose` trace would log (`trust-lsp.hmiLiveBridge`, §11 item 6). */
export interface LiveBridge {
  /** Whether a request goes out, or how it is refused, and whether its answer's status comes back. */
  request(facts: {
    method: "GET" | "POST";
    path: string;
    session: boolean;
    peer: boolean;
  }): Promise<{ bridged: true; reply: boolean } | { bridged: false; status: number; message: string }>;
  /** What the named session becomes after the answer to a login or a logout. */
  reply(facts: {
    path: string;
    status: number;
    peer: boolean;
  }): Promise<{ session: "keep"; cookie: string } | { session: "drop" | "unchanged" }>;
  /** Whether a value stream opens. */
  stream(facts: { path: string }): Promise<boolean>;
}
export interface LiveSessionDeps {
  target(): Promise<LiveEndpoint | undefined>;
  bridge: LiveBridge;
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
/** How long the language server may take to give a verdict (specification 39, §11 item 6). */
const VERDICT_WAIT_MS = 5_000;

/** `verdict`, or a rejection when the language server does not give it within 5 seconds. */
async function decided<T>(verdict: Promise<T>): Promise<T> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  const late = new Promise<never>((_, reject) => {
    timer = setTimeout(
      () => reject(new Error(`trust-lsp.hmiLiveBridge did not answer within ${VERDICT_WAIT_MS / 1000} seconds`)),
      VERDICT_WAIT_MS,
    );
  });
  try {
    return await Promise.race([verdict, late]);
  } finally {
    clearTimeout(timer);
  }
}

/** The cookie-pair `name=value` that the answer's `Set-Cookie` fields set for `name` (RFC 6265 §5.2): a field's
 * name-value pair up to its first `;`, name and value trimmed, the last field that sets the name with a value winning;
 * undefined when none does. The host keeps it in memory only (§11 item 6). */
function cookiePair(fields: readonly string[] | undefined, name: string): string | undefined {
  let pair: string | undefined;
  for (const field of fields ?? []) {
    const nameValue = field.split(";", 1)[0];
    const equals = nameValue.indexOf("=");
    if (equals < 0) continue;
    const value = nameValue.slice(equals + 1).trim();
    if (nameValue.slice(0, equals).trim() === name && value !== "") pair = `${name}=${value}`;
  }
  return pair;
}

const reasonOf = (error: unknown): string => (error instanceof Error ? error.message : String(error));

/** One editor's session, pinned to the first selected target; never revived after stop. */
export class LiveSession {
  private cookie: string | undefined;
  private identity: string | undefined;
  private stopped = false;
  /** Why the session stopped itself, when it did: its run target changed, or the runtime closed its stream. */
  private endedBy: "targetChanged" | "streamClosed" | undefined;
  private readonly sockets = new Set<SocketLike>();
  constructor(private readonly deps: LiveSessionDeps) {}

  async request(method: "GET" | "POST", path: string, body?: unknown): Promise<LiveReply> {
    if (this.stopped) return refused(409, "Live connection ended. Reopen Live mode to connect.");
    // Facts only: whether a named session is held and whether a login names a peer, never their secrets.
    const peer = Boolean((body as { peer?: unknown } | undefined)?.peer);
    let verdict: Awaited<ReturnType<LiveBridge["request"]>>;
    let target: LiveEndpoint | undefined;
    let cookie: string | undefined;
    // A verdict holds for the session it was asked for: a login or a logout that ended meanwhile asks again.
    do {
      cookie = this.cookie;
      try {
        verdict = await decided(this.deps.bridge.request({ method, path, session: cookie !== undefined, peer }));
      } catch (error) {
        // No verdict: no request, and no session.
        this.cookie = undefined;
        return refused(503, reasonOf(error));
      }
      if (!verdict.bridged) return refused(verdict.status, verdict.message);
      target = await this.target();
      if (!target)
        return refused(409, "The run target changed or the Live connection ended. Reopen Live mode to connect.");
    } while (this.cookie !== cookie);
    const answer = await this.deps.request(target, method, path, body, cookie);
    if (this.stopped) return refused(409, "Live connection ended.");
    if (verdict.reply) {
      let after: Awaited<ReturnType<LiveBridge["reply"]>>;
      try {
        after = await decided(this.deps.bridge.reply({ path, status: answer.status, peer }));
      } catch (error) {
        // No verdict on the answer: no session, and the caller learns that the request did not hold.
        this.cookie = undefined;
        return this.stopped ? refused(409, "Live connection ended.") : refused(503, reasonOf(error));
      }
      if (this.stopped) return refused(409, "Live connection ended.");
      if (after.session === "keep") this.cookie = cookiePair(answer.cookies, after.cookie);
      else if (after.session === "drop") this.cookie = undefined;
    }
    return { status: answer.status, body: answer.body };
  }

  async openSocket(path: string): Promise<SocketLike | undefined> {
    if (!(await decided(this.deps.bridge.stream({ path })).catch(() => false))) return undefined;
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
    raw.onclose = () => this.stop("streamClosed");
    this.sockets.add(stream);
    return stream;
  }

  async isCurrentTarget(): Promise<boolean> {
    return (await this.target()) !== undefined;
  }

  /** The named session's cookie, which an HMI agent simulator journey hands to the language server
   * with the journey (§11 item 6, specification 43 AT-08); none once the session ended. */
  namedCookie(): string | undefined {
    return this.stopped ? undefined : this.cookie;
  }

  private async target(): Promise<LiveEndpoint | undefined> {
    if (this.stopped) return undefined;
    const target = await this.deps.target();
    if (this.stopped) return undefined;
    if (!target) {
      this.stop("targetChanged");
      return undefined;
    }
    const identity = JSON.stringify([target.endpoint, target.url, target.token]);
    if (this.identity !== undefined && this.identity !== identity) {
      this.stop("targetChanged");
      return undefined;
    }
    this.identity = identity;
    return target;
  }

  /** Why the session stopped itself, if it did. */
  stoppedBy(): "targetChanged" | "streamClosed" | undefined {
    return this.endedBy;
  }

  stop(reason?: "targetChanged" | "streamClosed"): void {
    if (!this.stopped) this.endedBy = reason;
    this.stopped = true;
    this.cookie = undefined;
    for (const socket of [...this.sockets]) {
      socket.close();
      socket.onclose?.();
    }
  }
}
