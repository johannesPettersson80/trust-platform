// The extension host's HMI HTTP transport. Addresses are selected by the host; redirects never
// carry either its access token or the operator's named cookie to another endpoint (§11).
import type { SocketLike } from "../display/runtimeDisplay";
import type { LiveEndpoint, LiveReply } from "./liveSession";

export async function requestLive(
  target: LiveEndpoint,
  method: "GET" | "POST",
  path: string,
  body: unknown,
  cookie?: string,
): Promise<LiveReply> {
  const response = await fetch(target.url + path, {
    method,
    redirect: "manual",
    headers: {
      Accept: "application/json",
      ...(method === "POST" ? { "Content-Type": "application/json" } : {}),
      ...(target.token ? { "X-Trust-Token": target.token } : {}),
      ...(cookie ? { Cookie: cookie } : {}),
    },
    ...(method === "POST" ? { body: JSON.stringify(body) } : {}),
    signal: AbortSignal.timeout(15000),
  });
  if (response.status >= 300 && response.status < 400) {
    await response.body?.cancel();
    throw new Error("A Live connection cannot follow a redirect.");
  }
  return {
    status: response.status,
    body: await response.json().catch(() => null),
    cookie: response.headers.get("set-cookie") ?? undefined,
  };
}

interface NativeSocket {
  readyState: number;
  on(event: "message", callback: (data: { toString(): string }) => void): void;
  on(event: "close" | "error", callback: () => void): void;
  close(): void;
  terminate(): void;
}
const WebSocket = require("ws") as new (
  url: string,
  options: { headers: Record<string, string>; handshakeTimeout: number; followRedirects: boolean; maxPayload: number },
) => NativeSocket;

export function openLiveSocket(target: LiveEndpoint, path: string, cookie?: string): SocketLike {
  const url = target.url.replace(/^http/, "ws") + path;
  const socket = new WebSocket(url, {
    headers: { ...(target.token ? { "X-Trust-Token": target.token } : {}), ...(cookie ? { Cookie: cookie } : {}) },
    handshakeTimeout: 5000,
    followRedirects: false,
    maxPayload: 8 * 1024 * 1024,
  });
  let ended = false;
  const stream: SocketLike = {
    onmessage: null,
    onclose: null,
    close: () => {
      ended = true;
      if (socket.readyState === 1) socket.close();
      else if (socket.readyState === 0) socket.terminate();
    },
  };
  const closed = (): void => {
    if (!ended) {
      ended = true;
      stream.onclose?.();
    }
  };
  socket.on("message", (data) => {
    if (!ended) stream.onmessage?.({ data: data.toString() });
  });
  socket.on("close", closed);
  socket.on("error", closed);
  return stream;
}
