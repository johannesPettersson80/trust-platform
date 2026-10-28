// The shared runtime display's transport inside the editor (§11). No network credentials cross
// this boundary: requests go to the extension host's pinned named session.
import { assetUrl } from "../../display/frame";
import type { DisplayHost, SocketLike } from "../../display/runtimeDisplay";
import { KEEP_SETTINGS_PATH, type HostToWebview } from "../messages";
import type { VsCodeApi } from "./vscodeApi";

export class LiveDisplayHost implements DisplayHost {
  readonly location = { pathname: "/hmi", host: "live.invalid", protocol: "http:", search: "" };
  private sequence = 0;
  private readonly choices = new Map<string, string>();
  private ended = false;
  private readonly requests = new Map<
    number,
    {
      resolve: (answer: { status: number; body: unknown }) => void;
      reject: (error: Error) => void;
      timer: ReturnType<typeof setTimeout>;
    }
  >();
  private readonly sockets = new Map<number, SocketLike>();
  private readonly timers = new Set<ReturnType<typeof setTimeout>>();
  private readonly frames = new Set<number>();

  constructor(
    private readonly api: VsCodeApi,
    private readonly id: string,
    page: string,
    private readonly go: (path: string) => void,
    private readonly assetBase?: string,
  ) {
    this.location.pathname = `/hmi/${encodeURIComponent(page)}`;
  }

  receive(message: HostToWebview): void {
    if (this.ended || !("id" in message) || message.id !== this.id) return;
    if (message.type === "operateReply") {
      const pending = this.requests.get(message.request);
      if (!pending) return;
      clearTimeout(pending.timer);
      this.requests.delete(message.request);
      pending.resolve({ status: message.status, body: message.body });
    } else if (message.type === "operateStream") {
      const socket = this.sockets.get(message.socket);
      if (message.closed) {
        this.sockets.delete(message.socket);
        socket?.onclose?.();
      } else if (message.data !== undefined) socket?.onmessage?.({ data: message.data });
    }
  }

  fetchJson(path: string): Promise<{ status: number; body: unknown }> {
    return this.request("GET", path);
  }
  postJson(path: string, body: unknown): Promise<{ status: number; body: unknown }> {
    return this.request("POST", path, body);
  }
  keepSettings(
    instance: string,
    settings: Record<string, boolean | number | string | null>,
  ): Promise<{ status: number; body: unknown }> {
    return this.request("POST", KEEP_SETTINGS_PATH, { instance, settings });
  }
  private request(method: "GET" | "POST", path: string, body?: unknown): Promise<{ status: number; body: unknown }> {
    if (this.ended) return Promise.reject(new Error("Live connection ended."));
    const request = ++this.sequence;
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.requests.delete(request);
        reject(new Error("The runtime did not answer."));
      }, 15000);
      this.requests.set(request, { resolve, reject, timer });
      this.api.postMessage({
        type: "operateRequest",
        id: this.id,
        request,
        method,
        path,
        ...(body === undefined ? {} : { body }),
      });
    });
  }

  openSocket(url: string): SocketLike {
    const socket = ++this.sequence;
    const stream: SocketLike = {
      onmessage: null,
      onclose: null,
      close: () => {
        this.sockets.delete(socket);
        this.api.postMessage({ type: "operateSocketClose", id: this.id, socket });
      },
    };
    if (this.ended) return stream;
    this.sockets.set(socket, stream);
    const address = new URL(url);
    this.api.postMessage({ type: "operateSocket", id: this.id, socket, path: address.pathname + address.search });
    return stream;
  }

  asset = (path: string): string => (this.assetBase ? `${this.assetBase}/${path}` : assetUrl(path));

  remember(key: string, value: string): void {
    this.choices.set(key, value);
  }
  recall(key: string): string | undefined {
    return this.choices.get(key);
  }

  now(): number {
    return Date.now();
  }
  schedule(callback: () => void): void {
    if (this.ended) return;
    const frame = requestAnimationFrame(() => {
      this.frames.delete(frame);
      if (!this.ended) callback();
    });
    this.frames.add(frame);
  }
  later(milliseconds: number, callback: () => void): void {
    if (this.ended) return;
    const timer = setTimeout(() => {
      this.timers.delete(timer);
      if (!this.ended) callback();
    }, milliseconds);
    this.timers.add(timer);
  }
  reload(): void {
    this.go(this.location.pathname + this.location.search);
  }
  navigate(path: string): void {
    if (/^\/hmi(?:\/|\?|$)/.test(path)) this.go(path);
  }
  dispose(): void {
    this.ended = true;
    for (const request of this.requests.values()) {
      clearTimeout(request.timer);
      request.reject(new Error("Live connection ended."));
    }
    this.requests.clear();
    for (const socket of this.sockets.values()) socket.close();
    this.sockets.clear();
    this.timers.forEach(clearTimeout);
    this.frames.forEach(cancelAnimationFrame);
    this.timers.clear();
    this.frames.clear();
  }
}
