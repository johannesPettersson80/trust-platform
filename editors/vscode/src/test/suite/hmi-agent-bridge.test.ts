import * as assert from "assert";
import * as http from "http";
import { startAgentBridge, AgentBridge } from "../../hmiBuilder/agent/bridge";

// Specification 43, §3.1: the window's loopback checks the transport and runs what `trust-lsp hmi-agent` decided. The
// MCP rules are proven by the Rust tests in crates/trust-lsp/src/hmi_agent_mcp/tests/; these tests prove the wiring.

interface RequestOptions {
  auth?: string;
  origin?: string;
  url?: string;
  method?: string;
  contentType?: string;
}

suite("HMI agent loopback bridge", () => {
  let bridge: AgentBridge;
  let calls: Array<{ name: string; input: unknown }>;
  setup(async () => {
    calls = [];
    bridge = await startAgentBridge({
      workspaceId: "project-a",
      tools: [{ name: "hmi_get_context", description: "Read project", inputSchema: { type: "object" } }],
      invoke: async (name, input) => {
        calls.push({ name, input });
        return { content: [{ type: "text", text: "current unsaved page" }] };
      },
    });
  });
  teardown(async () => bridge.close());
  function request(
    route: string,
    body: object,
    options: RequestOptions = {},
  ): Promise<{ status: number; body: Record<string, unknown> }> {
    return new Promise((resolve, reject) => {
      const req = http.request(
        `${options.url ?? bridge.url}${route}`,
        {
          method: options.method ?? "POST",
          headers: {
            Authorization: `Bearer ${options.auth ?? bridge.capability}`,
            "Content-Type": options.contentType ?? "application/json",
            ...(options.origin ? { Origin: options.origin } : {}),
          },
        },
        (res) => {
          let text = "";
          res.on("data", (chunk) => (text += chunk));
          res.on("end", () => resolve({ status: res.statusCode!, body: text ? JSON.parse(text) : {} }));
        },
      );
      req.on("error", reject);
      req.end(JSON.stringify(body));
    });
  }
  const call = { id: "server-1", name: "hmi_get_context", arguments: { contractVersion: 1, input: {} } };
  test("the private capability lists the tools without revealing itself", async () => {
    assert.ok(bridge.capability.length >= 40, "unpredictable session capability");
    const tools = await request("/tools", {});
    assert.strictEqual(tools.status, 200);
    assert.deepStrictEqual(
      (tools.body.tools as Array<{ name: string }>).map((tool) => tool.name),
      ["hmi_get_context"],
    );
    assert.ok(!JSON.stringify(tools.body).includes(bridge.capability));
  });
  test("wrong capabilities, browser origins, other paths and other forms cannot reach tools", async () => {
    assert.strictEqual((await request("/call", call, { auth: "wrong" })).status, 401);
    assert.strictEqual((await request("/call", call, { origin: "https://untrusted.example" })).status, 403);
    assert.strictEqual((await request("", call)).status, 404, "the MCP path itself is no route");
    assert.strictEqual((await request("/execute", call)).status, 404);
    assert.strictEqual((await request("/call", call, { method: "PUT" })).status, 405);
    assert.strictEqual((await request("/call", call, { contentType: "text/plain" })).status, 415);
    assert.strictEqual((await request("/call", { ...call, id: 7 })).status, 400, "the server's call id is a string");
    assert.strictEqual((await request("/call", { ...call, arguments: [] })).status, 400);
    assert.strictEqual(calls.length, 0);
  });
  test("a call runs the named tool for the bridge's workspace and returns its result", async () => {
    const answer = await request("/call", { ...call, arguments: { ...call.arguments, workspaceId: "project-b" } });
    assert.strictEqual(answer.status, 200);
    assert.deepStrictEqual(answer.body, { content: [{ type: "text", text: "current unsaved page" }] });
    assert.deepStrictEqual(calls, [
      { name: "hmi_get_context", input: { contractVersion: 1, input: {}, workspaceId: "project-a" } },
    ]);
  });
  test("separate editor windows cannot reuse each other's capabilities", async () => {
    let otherCalls = 0;
    const other = await startAgentBridge({
      workspaceId: "project-b",
      tools: [{ name: "hmi_get_context", description: "Read", inputSchema: {} }],
      invoke: async () => {
        otherCalls++;
        return { content: [] };
      },
    });
    try {
      assert.strictEqual((await request("/call", call, { url: other.url })).status, 401);
      assert.strictEqual((await request("/call", call, { auth: other.capability })).status, 401);
      assert.strictEqual(otherCalls, 0);
      assert.strictEqual(calls.length, 0);
      assert.strictEqual((await request("/call", call, { url: other.url, auth: other.capability })).status, 200);
      assert.strictEqual(otherCalls, 1);
      assert.strictEqual(calls.length, 0);
    } finally {
      await other.close();
    }
  });
  test("closing a window expires its capability and a new connection uses a new one", async () => {
    const previous = bridge;
    await bridge.close();
    bridge = await startAgentBridge({ workspaceId: "project-a", tools: [], invoke: async () => ({ content: [] }) });
    assert.notStrictEqual(bridge.capability, previous.capability);
    assert.strictEqual((await request("/tools", {}, { auth: previous.capability })).status, 401);
    assert.strictEqual((await request("/tools", {})).status, 200);
  });
  test("cancel ends only the call with the id the server gave it", async () => {
    await bridge.close();
    let count = 0;
    let ready!: () => void;
    const started = new Promise<void>((resolve) => {
      ready = resolve;
    });
    bridge = await startAgentBridge({
      workspaceId: "project-a",
      tools: [{ name: "hmi_get_context", description: "Read", inputSchema: {} }],
      invoke: async (_name, _input, signal) => {
        count++;
        if (count === 2) ready();
        return new Promise((resolve) =>
          signal.addEventListener("abort", () => resolve({ content: [], isError: true }), { once: true }),
        );
      },
    });
    const first = request("/call", { ...call, id: "server-1" });
    let secondDone = false;
    const second = request("/call", { ...call, id: "server-2" }).then((result) => {
      secondDone = true;
      return result;
    });
    await started;
    assert.strictEqual((await request("/call", { ...call, id: "server-1" })).status, 409, "one call per id");
    assert.strictEqual((await request("/cancel", { id: "server-1" })).status, 200);
    assert.deepStrictEqual((await first).body, { content: [], isError: true });
    assert.strictEqual(secondDone, false, "the other call is still running");
    await request("/cancel", { id: "server-2" });
    assert.strictEqual((await second).status, 200);
  });
});
