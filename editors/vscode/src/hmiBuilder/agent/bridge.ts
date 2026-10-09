import * as http from "http";
import { randomBytes, timingSafeEqual } from "crypto";
import type { Socket } from "net";

export interface BridgeTool {
  name: string;
  description: string;
  inputSchema: object;
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
const VERSIONS = ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];
const object = (value: unknown): value is Record<string, unknown> =>
  value !== null && typeof value === "object" && !Array.isArray(value);

/** Private loopback transport for the stdio MCP proxy. The capability scopes every call to
 * this authoring session. It is never a general VS Code or LSP command proxy. */
export async function startAgentBridge(options: BridgeOptions): Promise<AgentBridge> {
  const capability = randomBytes(32).toString("hex");
  const expectedAuth = Buffer.from(`Bearer ${capability}`);
  const active = new Map<string, AbortController>();
  const sockets = new Set<Socket>();
  const server = http.createServer(async (req, res) => {
    const send = (status: number, body?: unknown): void => {
      if (res.writableEnded) return;
      res.writeHead(status, { "Content-Type": "application/json", "Cache-Control": "no-store" });
      res.end(body === undefined ? undefined : JSON.stringify(body));
    };
    if (req.headers.origin !== undefined) return send(403);
    const supplied = Buffer.from(req.headers.authorization ?? "");
    if (supplied.length !== expectedAuth.length || !timingSafeEqual(supplied, expectedAuth)) return send(401);
    if (req.url !== "/mcp") return send(404);
    if (req.method !== "POST") return send(405);
    if (!req.headers["content-type"]?.startsWith("application/json")) return send(415);
    const version = req.headers["mcp-protocol-version"];
    if (version && (typeof version !== "string" || !VERSIONS.includes(version))) return send(400);
    let bytes = 0;
    const chunks: Buffer[] = [];
    try {
      for await (const chunk of req) {
        bytes += chunk.length;
        if (bytes > 8 * 1024 * 1024) return send(413);
        chunks.push(Buffer.from(chunk));
      }
      const message: unknown = JSON.parse(Buffer.concat(chunks).toString("utf8"));
      if (!object(message) || message.jsonrpc !== "2.0" || typeof message.method !== "string") return send(400);
      const id = message.id;
      const client =
        typeof req.headers["x-trust-client"] === "string" ? req.headers["x-trust-client"].slice(0, 64) : "default";
      const params = object(message.params) ? message.params : {};
      const rpcError = (code: number, text: string): void =>
        send(200, { jsonrpc: "2.0", id: id ?? null, error: { code, message: text } });
      if (id === undefined) {
        if (message.method === "notifications/cancelled") active.get(`${client}:${String(params.requestId)}`)?.abort();
        return send(202);
      }
      if (typeof id !== "number" && typeof id !== "string") return rpcError(-32600, "Invalid request id.");
      let result: unknown;
      if (message.method === "initialize") {
        result = {
          protocolVersion:
            typeof params.protocolVersion === "string" && VERSIONS.includes(params.protocolVersion)
              ? params.protocolVersion
              : VERSIONS[0],
          capabilities: { tools: {} },
          serverInfo: { name: "trust-hmi-authoring", version: "1.0.0" },
          instructions:
            "Use the connected VS Code project's HMI tools. Inspect context and the visual library before editing. Render and inspect proposals, validate bindings, and review changes before applying. Authoring is separate from deployment and equipment operation.",
        };
      } else if (message.method === "ping") result = {};
      else if (message.method === "tools/list") result = { tools: options.tools };
      else if (message.method === "tools/call") {
        if (typeof params.name !== "string" || !options.tools.some((tool) => tool.name === params.name))
          return rpcError(-32602, "Unknown authoring tool.");
        if (!object(params.arguments)) return rpcError(-32602, "Tool arguments must be an object.");
        if (params.arguments.workspaceId !== undefined && params.arguments.workspaceId !== options.workspaceId)
          return rpcError(-32602, "The tool belongs to another workspace.");
        const key = `${client}:${String(id)}`;
        if (active.has(key) || active.size >= 8)
          return rpcError(-32600, "Duplicate request id or too many active requests.");
        const controller = new AbortController();
        active.set(key, controller);
        const timer = setTimeout(() => controller.abort(), 120000);
        try {
          result = await options.invoke(
            params.name,
            { ...params.arguments, workspaceId: options.workspaceId },
            controller.signal,
          );
        } finally {
          clearTimeout(timer);
          active.delete(key);
        }
      } else return rpcError(-32601, "Unknown method.");
      send(200, { jsonrpc: "2.0", id, result });
    } catch {
      send(400, {
        jsonrpc: "2.0",
        id: null,
        error: { code: -32600, message: "Invalid request or unavailable authoring operation." },
      });
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
    url: `http://127.0.0.1:${(server.address() as import("net").AddressInfo).port}/mcp`,
    capability,
    close: () =>
      new Promise((resolve, reject) => {
        for (const controller of active.values()) controller.abort();
        server.close((error) => (error ? reject(error) : resolve()));
        for (const socket of sockets) socket.destroy();
      }),
  };
}
