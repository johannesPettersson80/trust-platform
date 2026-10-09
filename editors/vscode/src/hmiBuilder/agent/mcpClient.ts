/** Stdio MCP entry point: forwards only to the private local authoring connection selected
 * by the customer. stdout contains protocol messages only; credentials never enter output. */
import * as fs from "fs/promises";
import * as http from "http";
import * as readline from "readline";
import { randomUUID } from "crypto";
import { StringDecoder } from "string_decoder";

/** One HTTP response body. Network chunks may end inside a multi-byte UTF-8 character, so the
 * bytes are decoded as one stream: a character split across two chunks arrives intact. The
 * limit counts the raw bytes received. */
export class ResponseBody {
  private readonly decoder = new StringDecoder("utf8");
  private text = "";
  private received = 0;
  constructor(private readonly limit = 64 * 1024 * 1024) {}
  /** False once the body exceeds its limit; the caller then abandons the response. */
  append(chunk: Buffer): boolean {
    this.received += chunk.length;
    if (this.received > this.limit) return false;
    this.text += this.decoder.write(chunk);
    return true;
  }
  /** The complete body, including a final character completed by the last chunk. */
  end(): string {
    return this.text + this.decoder.end();
  }
}

async function main(): Promise<void> {
  const file = process.argv[2];
  if (!file) throw new Error("Select an HMI authoring connection file.");
  const stat = await fs.stat(file);
  if (process.platform !== "win32" && (stat.mode & 0o077) !== 0)
    throw new Error("HMI connection file must be private to its owner.");
  const connection = JSON.parse(await fs.readFile(file, "utf8")) as { url: string; capability: string };
  const url = new URL(connection.url);
  if (
    url.protocol !== "http:" ||
    url.hostname !== "127.0.0.1" ||
    url.pathname !== "/mcp" ||
    url.username ||
    url.password ||
    url.search ||
    url.hash ||
    !/^[0-9a-f]{64}$/.test(connection.capability)
  ) {
    throw new Error("Invalid local HMI connection. Reconnect from VS Code.");
  }
  const clientId = randomUUID();
  const input = readline.createInterface({ input: process.stdin, crlfDelay: Infinity });
  const pending = new Set<Promise<void>>();
  const forward = async (line: string): Promise<void> => {
    let message: { id?: string | number; method?: string };
    try {
      message = JSON.parse(line);
    } catch {
      process.stdout.write(
        JSON.stringify({ jsonrpc: "2.0", id: null, error: { code: -32700, message: "Invalid JSON." } }) + "\n",
      );
      return;
    }
    try {
      if (Buffer.byteLength(line) > 8 * 1024 * 1024) throw new Error("Too large");
      const response = await new Promise<string>((resolve, reject) => {
        const req = http.request(
          url,
          {
            method: "POST",
            headers: {
              Authorization: `Bearer ${connection.capability}`,
              "Content-Type": "application/json",
              Accept: "application/json, text/event-stream",
              "X-Trust-Client": clientId,
            },
          },
          (res) => {
            const body = new ResponseBody();
            res.on("data", (chunk: Buffer) => {
              if (!body.append(chunk)) req.destroy(new Error("Response too large"));
            });
            res.on("end", () =>
              res.statusCode === 200 || res.statusCode === 202
                ? resolve(body.end())
                : reject(new Error("Bridge unavailable")),
            );
            res.on("error", reject);
          },
        );
        req.on("error", reject);
        req.setTimeout(125000, () => req.destroy(new Error("Timed out")));
        req.end(line);
      });
      if (response) process.stdout.write(JSON.stringify(JSON.parse(response)) + "\n");
    } catch {
      if (message?.id !== undefined)
        process.stdout.write(
          JSON.stringify({
            jsonrpc: "2.0",
            id: message.id,
            error: {
              code: -32000,
              message:
                "HMI authoring connection unavailable or request limit exceeded. Reconnect in the selected VS Code window.",
            },
          }) + "\n",
        );
    }
  };
  input.on("line", (line) => {
    if (pending.size >= 16) {
      process.stdout.write(
        JSON.stringify({
          jsonrpc: "2.0",
          id: null,
          error: { code: -32000, message: "Too many concurrent requests." },
        }) + "\n",
      );
      return;
    }
    const task = forward(line);
    pending.add(task);
    void task.finally(() => pending.delete(task));
  });
  await new Promise<void>((resolve) => input.on("close", resolve));
  await Promise.allSettled(pending);
}
if (require.main === module)
  void main().catch(() => {
    process.stderr.write("Unable to open the private HMI authoring connection. Connect from VS Code first.\n");
    process.exitCode = 1;
  });
