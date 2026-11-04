// One editor's operating connection to its selected, deployed HMI (§11).
import type { SocketLike } from "../display/runtimeDisplay";
import type { HostToWebview, WebviewToHost } from "./messages";
import { LiveSession, LiveSessionDeps } from "./liveSession";
/** What Live mode's operating gate decides from (`trust-lsp.hmiLiveGate`, §11 item 6). */
export interface LiveGateFacts {
  dirty: boolean;
  bundle: { ok?: boolean; version?: string };
  webServer: boolean;
  runTarget?: { label: string; simulator: boolean };
  page: string;
  /** The web server's answer to the page read, once the gate asked for it. */
  served?: { status: number; body: unknown };
}

/** The gate's verdict: read the page, operate the deployed bundle, or stay read-only and say why. */
export type LiveGateVerdict =
  | { kind: "read_page" }
  | { kind: "operate"; bundle: string }
  | { kind: "read_only"; reason: string };

/** What happened while the operating view runs (`trust-lsp.hmiLiveOperation`, §11 item 6). */
export type LiveOperationEvent =
  | { event: "request"; dirty: boolean }
  | { event: "reply"; method: string; status: number; body: unknown; bundle: string }
  | { event: "streamUnavailable" | "streamClosed" | "unreachable" | "targetChanged" | "targetUnreachable" };

/** Whether what an event is about goes on, and why operation then ends, as the language server decides it. */
export interface LiveOperationVerdict {
  proceed: boolean;
  end: string | null;
}

export interface LiveOperatorDeps {
  session: LiveSessionDeps;
  /** The selected run target, also when it has no HMI web server: the read-only explanation names
   * it (§11, item 6). */
  runTarget(): Promise<{ label: string; simulator: boolean } | undefined>;
  dirty(): boolean;
  bundle(): Promise<{ ok?: boolean; version?: string }>;
  /** Whether the operating view opens, as the language server decides it (§11 item 6). */
  liveGate(facts: LiveGateFacts): Promise<LiveGateVerdict>;
  /** Whether it ends, and why, as the language server decides it (§11 item 6). */
  liveOperation(event: LiveOperationEvent): Promise<LiveOperationVerdict>;
}
/** An ending the language server gave no reason for, against its contract. */
const NO_REASON = "trust-lsp.hmiLiveOperation gave no reason";

interface Connection {
  id: string;
  bundle?: string;
  session: LiveSession;
  sockets: Map<number, SocketLike>;
}

export class LiveOperator {
  private current: Connection | undefined;
  private checking = false;
  constructor(
    private readonly deps: LiveOperatorDeps,
    private readonly post: (message: HostToWebview) => void,
  ) {}

  async handle(message: WebviewToHost): Promise<void> {
    if (message.type === "operateOpen") {
      await this.open(message.id, message.page);
      return;
    }
    const current = this.current;
    if (!current || !("id" in message) || message.id !== current.id) return;
    if (message.type === "operateClose") {
      this.stop();
      return;
    }
    if (!current.bundle) return;
    if (message.type === "operateSocketClose") {
      current.sockets.get(message.socket)?.close();
      current.sockets.delete(message.socket);
      return;
    }
    const bundle = current.bundle;
    try {
      if (message.type === "operateRequest") {
        // The request goes out only once the language server lets it.
        const before = await this.verdict({ event: "request", dirty: this.deps.dirty() });
        if (this.current !== current) return;
        if (!before.proceed) {
          this.stop(before.end ?? NO_REASON);
          return;
        }
        const answer = await current.session.request(message.method, message.path, message.body);
        if (this.current !== current) return;
        const after = await this.verdict({
          event: "reply",
          method: message.method,
          status: answer.status,
          body: answer.body,
          bundle,
        });
        if (this.current !== current) return;
        if (after.proceed)
          this.post({
            type: "operateReply",
            id: current.id,
            request: message.request,
            status: answer.status,
            body: answer.body,
          });
        if (after.end) this.stop(after.end);
      } else if (message.type === "operateSocket") {
        const socket = await current.session.openSocket(message.path);
        if (this.current !== current) {
          socket?.close();
          return;
        }
        if (!socket) {
          await this.endAt(current, { event: "streamUnavailable" });
          return;
        }
        current.sockets.get(message.socket)?.close();
        current.sockets.set(message.socket, socket);
        socket.onmessage = (event) => {
          if (this.current === current && typeof event.data === "string")
            this.post({ type: "operateStream", id: current.id, socket: message.socket, data: event.data });
        };
        socket.onclose = () => {
          void this.endAt(current, { event: "streamClosed" });
        };
      }
    } catch {
      await this.endAt(current, { event: "unreachable" });
    }
  }

  /** The language server's verdict on `event`; without its answer, operation ends with its error. */
  private async verdict(event: LiveOperationEvent): Promise<LiveOperationVerdict> {
    try {
      return await this.deps.liveOperation(event);
    } catch (error) {
      return { proceed: false, end: error instanceof Error ? error.message : String(error) };
    }
  }

  /** Ends operation at once, so that nothing goes out after `event`, then says why, as the language server words it. */
  private async endAt(current: Connection, event: LiveOperationEvent): Promise<void> {
    if (this.current !== current) return;
    this.stop();
    const verdict = await this.verdict(event);
    this.post({ type: "operateEnded", id: current.id, reason: verdict.end ?? NO_REASON });
  }

  private async open(id: string, page: string): Promise<void> {
    this.stop();
    const current: Connection = { id, session: new LiveSession(this.deps.session), sockets: new Map() };
    this.current = current;
    const refuse = (reason: string): void => {
      if (this.current !== current) return;
      this.post({ type: "operateReady", id, reason });
      this.stop();
    };
    try {
      // The language server decides whether the operating view opens (§11 item 6): once from the
      // saved state, the bundle and the run target, and once more from the page the web server
      // serves. The editor gathers those and reads the page.
      const bundle = await this.deps.bundle();
      if (this.current !== current) return;
      const target = await this.deps.session.target();
      if (this.current !== current) return;
      const selected = target ? undefined : await this.deps.runTarget();
      if (this.current !== current) return;
      const facts = {
        bundle: { ok: bundle.ok, version: bundle.version },
        webServer: target !== undefined,
        runTarget: selected && { label: selected.label, simulator: selected.simulator },
        page,
      };
      const before = await this.deps.liveGate({ ...facts, dirty: this.deps.dirty() });
      if (this.current !== current) return;
      if (before.kind === "read_only") {
        refuse(before.reason);
        return;
      }
      if (!target) throw new Error("trust-lsp.hmiLiveGate opened Live mode without an HMI web server");
      const answer = await current.session.request("GET", `/api/hmi/page?name=${encodeURIComponent(page)}`);
      if (this.current !== current) return;
      const after = await this.deps.liveGate({
        ...facts,
        dirty: this.deps.dirty(),
        served: { status: answer.status, body: answer.body },
      });
      if (this.current !== current) return;
      if (after.kind === "read_only") {
        refuse(after.reason);
        return;
      }
      if (after.kind !== "operate") throw new Error("trust-lsp.hmiLiveGate gave no verdict on the page");
      current.bundle = after.bundle;
      this.post({ type: "operateReady", id, target: target.label });
    } catch {
      refuse("The runtime cannot be reached. Check its connection and HMI web server.");
    }
  }

  async checkTarget(): Promise<void> {
    const current = this.current;
    if (!current || this.checking) return;
    this.checking = true;
    try {
      if (!(await current.session.isCurrentTarget())) await this.endAt(current, { event: "targetChanged" });
    } catch {
      await this.endAt(current, { event: "targetUnreachable" });
    } finally {
      this.checking = false;
    }
  }

  stop(reason?: string): void {
    const current = this.current;
    this.current = undefined;
    if (!current) return;
    current.session.stop();
    current.sockets.clear();
    if (reason) this.post({ type: "operateEnded", id: current.id, reason });
  }
}
