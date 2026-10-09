import * as assert from "assert";
import * as fs from "fs/promises";
import * as os from "os";
import * as path from "path";
import { spawn } from "child_process";
import * as readline from "readline";
import { startAgentBridge } from "../../hmiBuilder/agent/bridge";

suite("HMI stdio MCP client", function () {
  this.timeout(10000);
  test("actual child process discovers tools and forwards image content without leaking capability", async () => {
    const bridge = await startAgentBridge({
      workspaceId: "project",
      tools: [{ name: "hmi_render_draft", description: "fixture", inputSchema: {} }],
      invoke: async () => ({ content: [{ type: "image", mimeType: "image/png", data: "fixture-image" }] }),
    });
    const directory = await fs.mkdtemp(path.join(os.tmpdir(), "trust-agent-proxy-"));
    const file = path.join(directory, "connection.json");
    await fs.writeFile(file, JSON.stringify({ url: bridge.url, capability: bridge.capability }), { mode: 0o600 });
    const child = spawn(process.execPath, [path.resolve(__dirname, "../../hmiBuilder/agent/mcpClient.js"), file], {
      env: { ...process.env, ELECTRON_RUN_AS_NODE: "1" },
      stdio: ["pipe", "pipe", "pipe"],
    });
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
        const timeout = setTimeout(() => reject(new Error("proxy did not respond")), 3000);
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
      assert.ok((listed.result as { tools: unknown[] }).tools.length);
      const image = await call("tools/call", {
        name: "hmi_render_draft",
        arguments: { contractVersion: 1, input: {} },
      });
      assert.deepStrictEqual(image.result, {
        content: [{ type: "image", mimeType: "image/png", data: "fixture-image" }],
      });
      assert.ok(!allOutput.includes(bridge.capability));
    } finally {
      child.kill();
      lines.close();
      await bridge.close();
      await fs.rm(directory, { recursive: true, force: true });
    }
  });
});
