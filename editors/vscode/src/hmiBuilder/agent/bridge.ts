import * as http from "http";
import { randomBytes, timingSafeEqual } from "crypto";
import type { AddressInfo, Socket } from "net";

export interface BridgeTool {
  name: string;
  description: string;
  inputSchema: object;
  annotations?: object;
}
export interface BridgeOptions {
  workspaceId: string;
  tools: BridgeTool[];
  invoke: (name: string, input: unknown, signal: AbortSignal) => Promise<unknown>;
}
export interface AgentBridge {
  url: string;
  capability: string;
  close(): Promise<void>;
}
/** The largest request body the loopback reads. */
const MAX_BODY_BYTES = 8 * 1024 * 1024;
/** How long the loopback keeps a cancellation that came before its call: the MCP server sends a cancellation on a
 * thread of its own, so it can overtake its call's request (specification 43 §3.1). */
const EARLY_CANCEL_MS = 10_000;
/** The most early cancellations the loopback keeps; the oldest goes first. */
const MAX_EARLY_CANCELS = 64;
const object = (value: unknown): value is Record<string, unknown> =>
  value !== null && typeof value === "object" && !Array.isArray(value);

/** Private loopback of `trust-lsp hmi-agent`, the stdio MCP server that owns every MCP rule (specification 43, §3.1).
 * The capability scopes each request to this window's workspace. The loopback checks only the transport and runs what
 * the server decided: it lists the tools, calls a named tool with given arguments, or cancels a call by the id the
 * server gave it. It is never a general VS Code or LSP command proxy. */
export async function startAgentBridge(options: BridgeOptions): Promise<AgentBridge> {
  const capability = randomBytes(32).toString("hex");
  const expectedAuth = Buffer.from(`Bearer ${capability}`);
  const active = new Map<string, AbortController>();
  /** The cancellations that came before their calls, by call id: when each one expires. */
  const early = new Map<string, number>();
  const keepEarlyCancel = (id: string): void => {
    const now = Date.now();
    for (const [kept, expires] of early) if (expires <= now) early.delete(kept);
    if (early.size >= MAX_EARLY_CANCELS) early.delete(early.keys().next().value as string);
    early.set(id, now + EARLY_CANCEL_MS);
  };
  const takeEarlyCancel = (id: string): boolean => {
    const expires = early.get(id);
    early.delete(id);
    return expires !== undefined && expires > Date.now();
  };
  const sockets = new Set<Socket>();
  const server = http.createServer(async (req, res) => {
    const send = (status: number, body?: unknown): void => {
      if (res.writableEnded) return;
      const text = body === undefined ? "" : JSON.stringify(body);
      res.writeHead(status, {
        "Content-Type": "application/json",
        "Content-Length": Buffer.byteLength(text),
        "Cache-Control": "no-store",
      });
      res.end(text);
    };
    if (req.headers.origin !== undefined) return send(403);
    const supplied = Buffer.from(req.headers.authorization ?? "");
    if (supplied.length !== expectedAuth.length || !timingSafeEqual(supplied, expectedAuth)) return send(401);
    if (req.url !== "/mcp/tools" && req.url !== "/mcp/call" && req.url !== "/mcp/cancel") return send(404);
    if (req.method !== "POST") return send(405);
    if (!req.headers["content-type"]?.startsWith("application/json")) return send(415);
    let body: unknown;
    try {
      let bytes = 0;
      const chunks: Buffer[] = [];
      for await (const chunk of req) {
        bytes += chunk.length;
        if (bytes > MAX_BODY_BYTES) return send(413);
        chunks.push(Buffer.from(chunk));
      }
      body = JSON.parse(Buffer.concat(chunks).toString("utf8"));
    } catch {
      return send(400);
    }
    if (!object(body)) return send(400);
    if (req.url === "/mcp/tools") return send(200, { tools: options.tools });
    const id = body.id;
    if (typeof id !== "string") return send(400);
    if (req.url === "/mcp/cancel") {
      const running = active.get(id);
      if (running) running.abort();
      else keepEarlyCancel(id);
      return send(200, {});
    }
    if (typeof body.name !== "string" || !object(body.arguments)) return send(400);
    if (active.has(id)) return send(409);
    const controller = new AbortController();
    active.set(id, controller);
    // A cancellation that overtook this call cancels it from the start, and the tool answers its cancellation.
    if (takeEarlyCancel(id)) controller.abort();
    try {
      // The tool runs for this window's workspace, whatever the arguments name.
      const input = { ...body.arguments, workspaceId: options.workspaceId };
      send(200, await options.invoke(body.name, input, controller.signal));
    } catch {
      send(500);
    } finally {
      active.delete(id);
    }
  });
  server.on("connection", (socket) => {
    sockets.add(socket);
    socket.on("close", () => sockets.delete(socket));
  });
  await new Promise<void>((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolve);
  });
  return {
    url: `http://127.0.0.1:${(server.address() as AddressInfo).port}/mcp`,
    capability,
    close: () =>
      new Promise((resolve, reject) => {
        for (const controller of active.values()) controller.abort();
        server.close((error) => (error ? reject(error) : resolve()));
        for (const socket of sockets) socket.destroy();
      }),
  };
}
