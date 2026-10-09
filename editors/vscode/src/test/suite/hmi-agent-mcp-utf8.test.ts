import * as assert from "assert";
import { spawn } from "child_process";
import { randomBytes } from "crypto";
import * as fs from "fs/promises";
import * as http from "http";
import type { AddressInfo } from "net";
import * as os from "os";
import * as path from "path";
import * as readline from "readline";
import { ResponseBody } from "../../hmiBuilder/agent/mcpClient";

// Specification 43, "Repairs — 7 October 2026", item 3: the stdio MCP client decodes a response
// as one byte stream, so a character split across network chunks reaches the agent intact.

const TEXT = "Pumpe Ø 50 – Füllstand";
const MESSAGE = JSON.stringify({
  jsonrpc: "2.0",
  id: 1,
  result: { content: [{ type: "text", text: `${TEXT} · 🌊` }] },
});

suite("HMI MCP client UTF-8", function () {
  this.timeout(15000);

  test("a message split into two chunks at every byte offset decodes to the original", () => {
    for (const message of [TEXT, MESSAGE]) {
      const bytes = Buffer.from(message, "utf8");
      assert.ok(bytes.length > message.length, "fixture: the message has multi-byte characters");
      for (let offset = 0; offset <= bytes.length; offset++) {
        const body = new ResponseBody();
        assert.strictEqual(body.append(bytes.subarray(0, offset)), true);
        assert.strictEqual(body.append(bytes.subarray(offset)), true);
        assert.strictEqual(body.end(), message, `split at byte ${offset} of ${JSON.stringify(message)}`);
      }
    }
  });

  test("the response limit counts raw bytes", () => {
    const body = new ResponseBody(4);
    assert.strictEqual(body.append(Buffer.from("Ø", "utf8")), true);
    assert.strictEqual(body.append(Buffer.from("Ø", "utf8")), true);
    assert.strictEqual(body.append(Buffer.from("x", "utf8")), false, "five bytes exceed a four-byte limit");
  });

  test("the stdio client forwards a response whose character arrives in two network chunks", async () => {
    const response = Buffer.from(MESSAGE, "utf8");
    const split = response.indexOf(Buffer.from("Ø", "utf8")) + 1;
    const server = http.createServer((request, reply) => {
      request.resume();
      request.on("end", () => {
        reply.writeHead(200, { "Content-Type": "application/json" });
        reply.flushHeaders();
        // The first chunk ends inside the two-byte character.
        reply.write(response.subarray(0, split));
        setTimeout(() => reply.end(response.subarray(split)), 100);
      });
    });
    await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));
    const directory = await fs.mkdtemp(path.join(os.tmpdir(), "trust-agent-utf8-"));
    const file = path.join(directory, "connection.json");
    const url = `http://127.0.0.1:${(server.address() as AddressInfo).port}/mcp`;
    await fs.writeFile(file, JSON.stringify({ url, capability: randomBytes(32).toString("hex") }), { mode: 0o600 });
    const child = spawn(process.execPath, [path.resolve(__dirname, "../../hmiBuilder/agent/mcpClient.js"), file], {
      env: { ...process.env, ELECTRON_RUN_AS_NODE: "1" },
      stdio: ["pipe", "pipe", "pipe"],
    });
    const lines = readline.createInterface({ input: child.stdout });
    try {
      const reply = await new Promise<unknown>((resolve, reject) => {
        const timer = setTimeout(() => reject(new Error("the stdio client did not respond")), 8000);
        lines.once("line", (line) => {
          clearTimeout(timer);
          resolve(JSON.parse(line));
        });
        child.stdin.write(JSON.stringify({ jsonrpc: "2.0", id: 1, method: "tools/list" }) + "\n");
      });
      assert.deepStrictEqual(reply, JSON.parse(MESSAGE));
    } finally {
      child.kill();
      lines.close();
      server.closeAllConnections();
      server.close();
      await fs.rm(directory, { recursive: true, force: true });
    }
  });
});
