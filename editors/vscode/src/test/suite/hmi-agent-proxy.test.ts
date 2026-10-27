import * as assert from "assert";
import * as fs from "fs/promises";
import * as os from "os";
import * as path from "path";
import { spawn } from "child_process";
import * as readline from "readline";
import { startAgentBridge } from "../../hmiBuilder/agent/bridge";

// Specification 43, §3.1: the agent starts `trust-lsp hmi-agent <connection file>`. This runs the language server binary
// under test against the extension's bridge and proves the wiring between them: the routes, the workspace and the result.
// The MCP rules themselves are proven by the Rust tests in crates/trust-lsp/src/hmi_agent_mcp/tests/.

suite("HMI stdio MCP server (trust-lsp hmi-agent)", function () {
  this.timeout(20000);
  test("the actual server lists and calls the bridge's tools and forwards images without leaking the capability", async () => {
    const server = process.env.ST_LSP_TEST_SERVER?.trim();
    assert.ok(server, "the test runner names the trust-lsp binary under test");
    const inputs: unknown[] = [];
    const bridge = await startAgentBridge({
      workspaceId: "project",
      tools: [{ name: "hmi_render_draft", description: "fixture", inputSchema: {} }],
      invoke: async (_name, input) => {
        inputs.push(input);
        return { content: [{ type: "image", mimeType: "image/png", data: "fixture-image" }] };
      },
    });
    const directory = await fs.mkdtemp(path.join(os.tmpdir(), "trust-agent-proxy-"));
    const file = path.join(directory, "connection.json");
    await fs.writeFile(
      file,
      JSON.stringify({ url: bridge.url, capability: bridge.capability, workspaceId: "project" }),
      { mode: 0o600 },
    );
    const child = spawn(server, ["hmi-agent", file], { stdio: ["pipe", "pipe", "pipe"] });
    const pending = new Map<number, (value: Record<string, unknown>) => void>();
    let allOutput = "";
    child.stdout.on("data", (chunk) => (allOutput += chunk.toString()));
    child.stderr.on("data", (chunk) => (allOutput += chunk.toString()));
    const lines = readline.createInterface({ input: child.stdout });
    lines.on("line", (line) => {
      const result = JSON.parse(line);
      pending.get(result.id)?.(result);
    });
    let id = 0;
    function call(method: string, params?: object): Promise<Record<string, unknown>> {
      const requestId = ++id;
      return new Promise((resolve, reject) => {
        const timeout = setTimeout(() => reject(new Error("the server did not respond")), 5000);
        pending.set(requestId, (result) => {
          clearTimeout(timeout);
          resolve(result);
        });
        child.stdin.write(JSON.stringify({ jsonrpc: "2.0", id: requestId, method, params }) + "\n");
      });
    }
    try {
      const init = await call("initialize", {
        protocolVersion: "2025-11-25",
        capabilities: {},
        clientInfo: { name: "fixture", version: "1" },
      });
      assert.strictEqual((init.result as { serverInfo?: { name: string } }).serverInfo?.name, "trust-hmi-authoring");
      const listed = await call("tools/list");
      assert.deepStrictEqual(
        (listed.result as { tools: Array<{ name: string }> }).tools.map((tool) => tool.name),
        ["hmi_render_draft"],
      );
      const image = await call("tools/call", {
        name: "hmi_render_draft",
        arguments: { contractVersion: 1, input: {} },
      });
      assert.deepStrictEqual(image.result, {
        content: [{ type: "image", mimeType: "image/png", data: "fixture-image" }],
      });
      assert.deepStrictEqual(inputs, [{ contractVersion: 1, input: {}, workspaceId: "project" }]);
      assert.ok(!allOutput.includes(bridge.capability));
    } finally {
      child.kill();
      lines.close();
      await bridge.close();
      await fs.rm(directory, { recursive: true, force: true });
    }
  });
});
