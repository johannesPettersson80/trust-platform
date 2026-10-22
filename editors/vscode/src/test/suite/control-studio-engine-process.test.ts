import * as assert from "assert";
import { spawn } from "child_process";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import { createEngineClient } from "../../controlStudio/engine/engineClient";
import { startEngine, type EngineProcess } from "../../controlStudio/engine/engineProcess";
import type { OpenRequest } from "../../controlStudio/generated/OpenRequest";

/**
 * The engine binary the test runner names (`ST_CONTROL_STUDIO_TEST_BIN`), else the one the build left in the cargo
 * target folder (CARGO_TARGET_DIR, else `target`).
 */
function engineBinary(): string {
  const named = process.env.ST_CONTROL_STUDIO_TEST_BIN?.trim();
  if (named) {
    if (!fs.existsSync(named)) throw new Error(`ST_CONTROL_STUDIO_TEST_BIN not found at ${named}`);
    return named;
  }
  const repoRoot = process.env.TRUST_REPOSITORY_ROOT ?? path.resolve(__dirname, "../../../../..");
  const targetDir = process.env.CARGO_TARGET_DIR?.trim();
  const folder = targetDir ? path.resolve(repoRoot, targetDir) : path.join(repoRoot, "target");
  const name = process.platform === "win32" ? "trust-control-studio.exe" : "trust-control-studio";
  const binary = path.join(folder, "debug", name);
  if (!fs.existsSync(binary)) {
    throw new Error(`${name} is not built at ${binary}: run cargo build -p trust-control-studio first.`);
  }
  return binary;
}

function openRequest(folder: string): OpenRequest {
  return { projectRoot: folder, runtimeId: "test-runtime", storageFolder: folder };
}

/** A project without loops opens as an empty list of loops. */
const NO_LOOPS = { result: { kind: "loops", loops: [] } };

/** The process id of the engine's running child; the engine announces no start, so the test reads it from the process it supervises. */
function childProcessId(engine: EngineProcess): number | undefined {
  return (engine as unknown as { child?: { pid?: number } }).child?.pid;
}

/** Resolves once the engine runs a child other than the one with process id `stopped`, which is its restart. */
async function restartedAfter(engine: EngineProcess, stopped: number): Promise<void> {
  const deadline = Date.now() + 20_000;
  while (childProcessId(engine) === undefined || childProcessId(engine) === stopped) {
    if (Date.now() > deadline) {
      throw new Error("the engine was not started again");
    }
    await new Promise((resolve) => setTimeout(resolve, 20));
  }
}

function frame(body: string): Buffer {
  return Buffer.from(`Content-Length: ${Buffer.byteLength(body, "utf8")}\r\n\r\n${body}`, "utf8");
}

/** The first message on `stream`, read from its Content-Length frame. */
function readFrame(stream: NodeJS.ReadableStream): Promise<unknown> {
  return new Promise((resolve, reject) => {
    let received = Buffer.alloc(0);
    stream.on("error", reject);
    stream.on("data", (chunk: Buffer) => {
      received = Buffer.concat([received, chunk]);
      const headerEnd = received.indexOf("\r\n\r\n");
      if (headerEnd < 0) {
        return;
      }
      const header = received.subarray(0, headerEnd).toString("utf8");
      const match = /Content-Length:\s*(\d+)/i.exec(header);
      if (!match) {
        reject(new Error(`the engine answered without Content-Length: ${header}`));
        return;
      }
      const bodyStart = headerEnd + 4;
      const bodyEnd = bodyStart + Number(match[1]);
      if (received.length >= bodyEnd) {
        resolve(JSON.parse(received.subarray(bodyStart, bodyEnd).toString("utf8")));
      }
    });
  });
}

suite("Control Studio engine process", () => {
  let folder = "";
  let engine: EngineProcess | undefined;

  setup(() => {
    folder = fs.mkdtempSync(path.join(os.tmpdir(), "trust-control-studio-engine-"));
  });

  teardown(() => {
    engine?.dispose();
    engine = undefined;
    fs.rmSync(folder, { recursive: true, force: true });
  });

  test("starts the engine and answers controlStudio/open for a project without loops", async function () {
    this.timeout(30_000);
    engine = startEngine({ executable: engineBinary() });
    const answer = await createEngineClient(engine).open(openRequest(folder));
    assert.deepStrictEqual(answer, NO_LOOPS);
  });

  test("restarts the engine after it stops, and answers again", async function () {
    this.timeout(30_000);
    engine = startEngine({ executable: engineBinary() });
    const client = createEngineClient(engine);
    assert.deepStrictEqual(await client.open(openRequest(folder)), NO_LOOPS);
    const stopped = childProcessId(engine);
    assert.ok(stopped !== undefined, "the running engine has a process id");
    process.kill(stopped);
    await restartedAfter(engine, stopped);
    assert.deepStrictEqual(await client.open(openRequest(folder)), NO_LOOPS);
    assert.notStrictEqual(childProcessId(engine), stopped);
  });

  test("starts an engine that could not be started again on a call after the restart window, so a repaired engine needs no reload", async function () {
    // Wrong behaviour caught: a failed start stuck until the window was reloaded, although the engine was installed meanwhile.
    this.timeout(30_000);
    const executable = path.join(folder, process.platform === "win32" ? "engine.exe" : "engine");
    engine = startEngine({ executable });
    await assert.rejects(createEngineClient(engine).open(openRequest(folder)), /could not be started/);
    fs.copyFileSync(engineBinary(), executable);
    fs.chmodSync(executable, 0o755);
    const realNow = Date.now;
    try {
      // Inside the restart window the reason is kept, so a broken engine is not started on every call.
      await assert.rejects(createEngineClient(engine).open(openRequest(folder)), /could not be started/);
      Date.now = () => realNow() + 61_000;
      assert.deepStrictEqual(await createEngineClient(engine).open(openRequest(folder)), NO_LOOPS);
    } finally {
      Date.now = realNow;
    }
  });

  test("answers a malformed request with a JSON-RPC error", async function () {
    this.timeout(30_000);
    const child = spawn(engineBinary(), ["serve"], {
      stdio: ["pipe", "pipe", "pipe"],
      windowsHide: true,
    });
    try {
      // The params are neither an object nor an array, which JSON-RPC 2.0 section 4.2 refuses.
      const answer = readFrame(child.stdout);
      child.stdin.write(frame('{"jsonrpc":"2.0","id":7,"method":"controlStudio/open","params":42}'));
      const message = (await answer) as { id: unknown; error?: { code: number } };
      assert.strictEqual(message.id, 7);
      assert.strictEqual(message.error?.code, -32602);
    } finally {
      child.stdin.end();
      child.kill();
    }
  });
});
