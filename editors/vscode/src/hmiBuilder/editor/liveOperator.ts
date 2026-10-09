// One editor's operating connection to its selected, deployed HMI (§11).
import type { SocketLike } from "../display/runtimeDisplay";
import type { HostToWebview, WebviewToHost } from "./messages";
import { LiveSession, LiveSessionDeps } from "./liveSession";
export interface LiveOperatorDeps {
  session: LiveSessionDeps;
  dirty(): boolean;
  bundle(): Promise<{ ok?: boolean; version?: string }>;
}
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
    try {
      if (message.type === "operateRequest") {
        if (this.deps.dirty()) {
          this.stop("The HMI changed. Save and deploy it before operating again.");
          return;
        }
        const answer = await current.session.request(message.method, message.path, message.body);
        if (this.current !== current) return;
        const served = answer.body as { bundle?: string } | null;
        if (message.method === "GET" && served?.bundle !== undefined && served.bundle !== current.bundle) {
          this.stop("The deployed HMI changed. Check and deploy the matching project before operating again.");
          return;
        }
        this.post({
          type: "operateReply",
          id: current.id,
          request: message.request,
          status: answer.status,
          body: answer.body,
        });
        if (answer.status === 409)
          this.stop(
            "The run target changed or disconnected. Any unfinished command has an unknown outcome; none will be resent.",
          );
      } else if (message.type === "operateSocket") {
        const socket = await current.session.openSocket(message.path);
        if (this.current !== current) {
          socket?.close();
          return;
        }
        if (!socket) {
          this.stop("The runtime stream is unavailable. Reopen Live mode to connect.");
          return;
        }
        current.sockets.get(message.socket)?.close();
        current.sockets.set(message.socket, socket);
        socket.onmessage = (event) => {
          if (this.current === current && typeof event.data === "string")
            this.post({ type: "operateStream", id: current.id, socket: message.socket, data: event.data });
        };
        socket.onclose = () => {
          if (this.current === current)
            this.stop("The runtime disconnected. Any unfinished command has an unknown outcome; none will be resent.");
        };
      }
    } catch {
      if (this.current === current)
        this.stop("The runtime cannot be reached. Any unfinished command has an unknown outcome; none will be resent.");
    }
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
      if (this.deps.dirty()) {
        refuse("Save the HMI before operating its deployed version.");
        return;
      }
      const bundle = await this.deps.bundle();
      if (this.current !== current) return;
      if (!bundle.ok || !bundle.version) {
        refuse("Check and deploy the HMI before operating it.");
        return;
      }
      const target = await this.deps.session.target();
      if (this.current !== current) return;
      if (!target) {
        refuse("The selected target has no HMI web server. Start a runtime with [runtime.web] enabled.");
        return;
      }
      const answer = await current.session.request("GET", `/api/hmi/page?name=${encodeURIComponent(page)}`);
      if (this.current !== current) return;
      const data = answer.body as { bundle?: string; page?: { name: string } } | null;
      if (
        answer.status !== 200 ||
        !data?.page ||
        data.page.name !== page ||
        data.bundle !== bundle.version.slice(0, 16)
      ) {
        refuse("Deploy this saved HMI to the selected runtime before operating it.");
        return;
      }
      if (this.deps.dirty()) {
        refuse("The HMI changed. Save and deploy it before operating again.");
        return;
      }
      current.bundle = data.bundle;
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
      if (!(await current.session.isCurrentTarget()) && this.current === current)
        this.stop("The selected run target changed. Reopen Live mode to connect.");
    } catch {
      if (this.current === current)
        this.stop("The selected run target cannot be reached. Reopen Live mode to connect.");
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
