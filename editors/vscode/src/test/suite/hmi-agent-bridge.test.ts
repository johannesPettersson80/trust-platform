import * as assert from "assert";
import * as http from "http";
import { startAgentBridge, AgentBridge } from "../../hmiBuilder/agent/bridge";

suite("HMI local MCP bridge", () => {
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
    body: object,
    auth = bridge.capability,
    origin?: string,
    client?: string,
    url = bridge.url,
  ): Promise<{ status: number; body: Record<string, unknown> }> {
    return new Promise((resolve, reject) => {
      const req = http.request(
        url,
        {
          method: "POST",
          headers: {
            Authorization: `Bearer ${auth}`,
            "Content-Type": "application/json",
            ...(origin ? { Origin: origin } : {}),
            ...(client ? { "X-Trust-Client": client } : {}),
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
  test("private capability authorizes initialization and tool discovery", async () => {
    assert.ok(bridge.capability.length >= 40, "unpredictable session capability");
    const init = await request({
      jsonrpc: "2.0",
      id: 1,
      method: "initialize",
      params: { protocolVersion: "2025-11-25", capabilities: {}, clientInfo: { name: "fixture", version: "1" } },
    });
    assert.strictEqual(init.status, 200);
    assert.ok(init.body.result);
    const tools = await request({ jsonrpc: "2.0", id: 2, method: "tools/list" });
    assert.deepStrictEqual(
      (tools.body.result as { tools: unknown[] }).tools.map((t: unknown) => (t as { name: string }).name),
      ["hmi_get_context"],
    );
    assert.ok(!JSON.stringify(tools.body).includes(bridge.capability));
  });
  test("wrong capabilities and browser-origin calls cannot reach tools", async () => {
    const call = {
      jsonrpc: "2.0",
      id: 3,
      method: "tools/call",
      params: { name: "hmi_get_context", arguments: { contractVersion: 1, input: {} } },
    };
    assert.strictEqual((await request(call, "wrong")).status, 401);
    assert.strictEqual((await request(call, bridge.capability, "https://untrusted.example")).status, 403);
    assert.strictEqual(calls.length, 0);
  });
  test("forwards only listed tools and pins workspace authority", async () => {
    const call = async (name: string, workspaceId?: string) =>
      request({
        jsonrpc: "2.0",
        id: 4,
        method: "tools/call",
        params: { name, arguments: { contractVersion: 1, input: {}, ...(workspaceId ? { workspaceId } : {}) } },
      });
    const result = await call("hmi_get_context");
    assert.strictEqual(result.status, 200);
    assert.ok(result.body.result);
    assert.deepStrictEqual(calls, [
      { name: "hmi_get_context", input: { contractVersion: 1, input: {}, workspaceId: "project-a" } },
    ]);
    assert.ok((await call("vscode.executeCommand")).body.error);
    assert.ok((await call("hmi_get_context", "another-project")).body.error);
    assert.strictEqual(calls.length, 1);
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
      const message = {
        jsonrpc: "2.0",
        id: 1,
        method: "tools/call",
        params: { name: "hmi_get_context", arguments: { contractVersion: 1, input: {} } },
      };
      assert.strictEqual((await request(message, bridge.capability, undefined, undefined, other.url)).status, 401);
      assert.strictEqual((await request(message, other.capability)).status, 401);
      assert.strictEqual(otherCalls, 0);
      assert.strictEqual(calls.length, 0);
      assert.strictEqual((await request(message, other.capability, undefined, undefined, other.url)).status, 200);
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
    assert.strictEqual((await request({ jsonrpc: "2.0", id: 1, method: "ping" }, previous.capability)).status, 401);
    assert.strictEqual((await request({ jsonrpc: "2.0", id: 1, method: "ping" })).status, 200);
  });
  test("cancellation is scoped by client even when request identifiers match", async () => {
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
    const message = {
      jsonrpc: "2.0",
      id: 7,
      method: "tools/call",
      params: { name: "hmi_get_context", arguments: { contractVersion: 1, input: {} } },
    };
    const first = request(message, bridge.capability, undefined, "first");
    let secondDone = false;
    const second = request(message, bridge.capability, undefined, "second").then((result) => {
      secondDone = true;
      return result;
    });
    await started;
    const cancel = { jsonrpc: "2.0", method: "notifications/cancelled", params: { requestId: 7 } };
    assert.strictEqual((await request(cancel, bridge.capability, undefined, "first")).status, 202);
    assert.strictEqual((await first).status, 200);
    assert.strictEqual(secondDone, false, "another client's request remains active");
    await request(cancel, bridge.capability, undefined, "second");
    assert.strictEqual((await second).status, 200);
  });
});
