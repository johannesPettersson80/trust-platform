import * as fs from "fs";
import * as net from "net";
import * as path from "path";
import { execFile, spawn, type ChildProcess } from "child_process";
import { runtimeLifecycleService } from "../../runtimeLifecycle";
import { sendRuntimeControlRequest } from "../../runtimeControlClient";
import { setSelectedRuntimeId } from "../../selectedRuntime";
import type { ControlLoopDefinition } from "../../controlStudio/loopDefinition";
import { loopInspectionBindings } from "../../controlStudio/loopInspectionBindings";
import { inspectSymbolicSignals, type SymbolicInspectionSample } from "../../controlStudio/symbolicInspection";
import { sleep } from "./plant-rendered-harness";

/** A prepared project copy listens on this socket inside its own folder, so runs never share an endpoint. */
const SOCKET_NAME = ".trust-control.sock";
const MAX_SOCKET_PATH_BYTES = 100;
const STATUS_TIMEOUT_MS = 30_000;
const STOP_TIMEOUT_MS = 10_000;
const LOG_TAIL_LINES = 40;

export interface AttachedRuntime {
  readonly endpoint: string;
  readonly child: ChildProcess;
  /** Detaches Control Studio, then stops the process this test started. It never kills a runtime it did not start. */
  stop(): Promise<void>;
}

/** The trust-runtime binary the test runner built and passed in ST_RUNTIME_TEST_BIN. */
export function runtimeBinary(): string {
  const binary = process.env.ST_RUNTIME_TEST_BIN?.trim();
  if (!binary) throw new Error("ST_RUNTIME_TEST_BIN is not set: the test runner passes the trust-runtime binary.");
  if (!fs.existsSync(binary)) throw new Error(`ST_RUNTIME_TEST_BIN names a missing file: ${binary}`);
  return binary;
}

/** The control endpoint of a project copy, spelled as the runtime and the extension both read it. */
export function attachedEndpoint(projectRoot: string): string {
  const socketPath = path.join(projectRoot, SOCKET_NAME);
  const bytes = Buffer.byteLength(socketPath);
  if (bytes > MAX_SOCKET_PATH_BYTES) {
    throw new Error(`The control socket path is ${bytes} bytes (${socketPath}); the limit is ${MAX_SOCKET_PATH_BYTES}. Use a shorter project path.`);
  }
  return `unix://${socketPath}`;
}

/** A free loopback TCP port, for a project copy whose web server must answer (an operator's HMI route). */
export function reserveLoopbackPort(): Promise<number> {
  return new Promise((resolve, reject) => {
    const server = net.createServer();
    server.once("error", reject);
    server.listen(0, "127.0.0.1", () => {
      const address = server.address();
      const port = typeof address === "object" && address !== null ? address.port : 0;
      server.close(() => (port > 0 ? resolve(port) : reject(new Error("No loopback port could be reserved."))));
    });
  });
}

/**
 * Makes a copy of a project safe to run beside other work: its control endpoint is a unix socket in the copy, the web
 * server is off (or on the given loopback port), no auth token is configured (a unix client is then Admin, auth.rs), and
 * every loop names this endpoint as its runtime. Returns the endpoint.
 */
export function prepareAttachedProject(projectRoot: string, options: { webPort?: number } = {}): string {
  const endpoint = attachedEndpoint(projectRoot);
  const runtimePath = path.join(projectRoot, "runtime.toml");
  let runtimeText = fs.readFileSync(runtimePath, "utf8");
  runtimeText = setTomlKeys(runtimeText, "runtime.control", { endpoint: JSON.stringify(endpoint) });
  runtimeText = removeTomlKey(runtimeText, "runtime.control", "auth_token");
  runtimeText = setTomlKeys(runtimeText, "runtime.web", options.webPort === undefined
    ? { enabled: "false" }
    : { enabled: "true", listen: JSON.stringify(`127.0.0.1:${options.webPort}`) });
  fs.writeFileSync(runtimePath, runtimeText);
  for (const loopPath of loopFiles(projectRoot)) {
    const loop = JSON.parse(fs.readFileSync(loopPath, "utf8"));
    loop.runtimeId = endpoint;
    fs.writeFileSync(loopPath, `${JSON.stringify(loop, null, 2)}\n`);
  }
  return endpoint;
}

/**
 * Builds the project, starts `trust-runtime run --simulation` on it, waits until it answers status, and attaches to it
 * the way the Devices & Connections canvas does: connectRemote, then the selected runtime is this endpoint.
 */
export async function startAttachedRuntime(projectRoot: string, options: { timeScale: number; label: string }): Promise<AttachedRuntime> {
  const binary = runtimeBinary();
  const endpoint = attachedEndpoint(projectRoot);
  await buildProject(binary, projectRoot);
  // A socket left by an earlier run would only make the new runtime fail to bind.
  fs.rmSync(path.join(projectRoot, SOCKET_NAME), { force: true });
  const child = spawnRuntime(binary, projectRoot, options.timeScale);
  const launch: { error?: Error } = {};
  child.once("error", (error) => { launch.error = error; });
  await waitForStatus(endpoint, child, launch, path.join(projectRoot, "runtime.log"));
  const attached = await runtimeLifecycleService.connectRemote(endpoint, options.label);
  if (!attached.ok) {
    await terminate(child);
    throw new Error(`Control Studio could not attach to ${endpoint}: ${attached.failure.message}`);
  }
  await setSelectedRuntimeId(endpoint);
  let stopping: Promise<void> | undefined;
  return {
    endpoint,
    child,
    stop: () => {
      if (!stopping) stopping = detachAndTerminate(child);
      return stopping;
    },
  };
}

/** One coherent sample of the loop's bindings from the runtime (control.signals.inspect), or undefined while it has none. */
export async function inspectAttachedLoop(endpoint: string, loop: ControlLoopDefinition): Promise<SymbolicInspectionSample | undefined> {
  const result = await inspectSymbolicSignals(
    (operation, params) => sendRuntimeControlRequest(endpoint, undefined, operation, params, { timeoutMs: 5000 }),
    { loopRevision: loop.revision },
    loopInspectionBindings(loop),
  );
  return result.kind === "available" ? result.sample : undefined;
}

/** The last lines of a runtime log, for a failure message. */
export function runtimeLogTail(logPath: string): string {
  try {
    return fs.readFileSync(logPath, "utf8").split(/\r?\n/).slice(-LOG_TAIL_LINES).join("\n");
  } catch {
    return "(runtime.log is missing)";
  }
}

function loopFiles(projectRoot: string): string[] {
  const files = [path.join(projectRoot, "control-loop.json")].filter((file) => fs.existsSync(file));
  const folder = path.join(projectRoot, "control-loops");
  if (fs.existsSync(folder)) {
    files.push(...fs.readdirSync(folder).filter((name) => name.endsWith(".json")).sort().map((name) => path.join(folder, name)));
  }
  return files;
}

/** The line range of one TOML table: from its header to the next table header, or to the end of the text. */
function tableRange(lines: string[], table: string): { header: number; end: number } | undefined {
  const header = lines.findIndex((line) => line.trim() === `[${table}]`);
  if (header < 0) return undefined;
  let end = header + 1;
  while (end < lines.length && !lines[end].trimStart().startsWith("[")) end += 1;
  return { header, end };
}

/** Sets keys in one table of a TOML text, replacing their lines, and adds the table when it is missing. */
function setTomlKeys(text: string, table: string, values: Record<string, string>): string {
  const lines = text.split("\n");
  const range = tableRange(lines, table);
  if (!range) {
    const block = Object.entries(values).map(([key, value]) => `${key} = ${value}`);
    return [text.replace(/\s*$/, ""), "", `[${table}]`, ...block, ""].join("\n");
  }
  const missing = new Map(Object.entries(values));
  for (let index = range.header + 1; index < range.end; index += 1) {
    const match = /^(\s*)([A-Za-z0-9_-]+)\s*=/.exec(lines[index]);
    const value = match ? missing.get(match[2]) : undefined;
    if (match && value !== undefined) {
      lines[index] = `${match[1]}${match[2]} = ${value}`;
      missing.delete(match[2]);
    }
  }
  lines.splice(range.header + 1, 0, ...[...missing].map(([key, value]) => `${key} = ${value}`));
  return lines.join("\n");
}

/** Removes one key from one table of a TOML text. */
function removeTomlKey(text: string, table: string, key: string): string {
  const lines = text.split("\n");
  const range = tableRange(lines, table);
  if (!range) return text;
  const pattern = new RegExp(`^\\s*${key}\\s*=`);
  return lines.filter((line, index) => index <= range.header || index >= range.end || !pattern.test(line)).join("\n");
}

function buildProject(binary: string, projectRoot: string): Promise<void> {
  return new Promise((resolve, reject) => {
    execFile(binary, ["build", "--project", projectRoot], { maxBuffer: 16 * 1024 * 1024 }, (error, _stdout, stderr) => {
      if (error) reject(new Error(`trust-runtime build --project ${projectRoot} failed: ${error.message}\n${stderr}`));
      else resolve();
    });
  });
}

/** Starts the runtime with its stdout and stderr appended to the project's runtime.log. */
function spawnRuntime(binary: string, projectRoot: string, timeScale: number): ChildProcess {
  const logFile = fs.openSync(path.join(projectRoot, "runtime.log"), "a");
  try {
    return spawn(binary, ["run", "--project", projectRoot, "--simulation", "--time-scale", String(timeScale)],
      { stdio: ["ignore", logFile, logFile] });
  } finally {
    fs.closeSync(logFile);
  }
}

async function waitForStatus(endpoint: string, child: ChildProcess, launch: { error?: Error }, logPath: string): Promise<void> {
  const deadline = Date.now() + STATUS_TIMEOUT_MS;
  for (;;) {
    if (launch.error || hasExited(child)) {
      const reason = launch.error?.message ?? `exit code ${child.exitCode ?? "none"}, signal ${child.signalCode ?? "none"}`;
      throw new Error(`trust-runtime stopped before it answered status (${reason}). Last lines of runtime.log:\n${runtimeLogTail(logPath)}`);
    }
    if (await answersStatus(endpoint)) return;
    if (Date.now() >= deadline) {
      await terminate(child);
      throw new Error(`trust-runtime did not answer status at ${endpoint} within ${STATUS_TIMEOUT_MS / 1000} s. Last lines of runtime.log:\n${runtimeLogTail(logPath)}`);
    }
    await sleep(200);
  }
}

async function answersStatus(endpoint: string): Promise<boolean> {
  try {
    await sendRuntimeControlRequest(endpoint, undefined, "status", {}, { timeoutMs: 2000 });
    return true;
  } catch {
    return false;
  }
}

async function detachAndTerminate(child: ChildProcess): Promise<void> {
  try {
    await runtimeLifecycleService.stopRuntime();
  } finally {
    await terminate(child);
  }
}

/** Sends SIGTERM to the process by its PID, then SIGKILL when it has not exited within the stop timeout. */
async function terminate(child: ChildProcess): Promise<void> {
  if (child.pid === undefined || hasExited(child)) return;
  const exited = new Promise<void>((resolve) => child.once("exit", () => resolve()));
  child.kill("SIGTERM");
  const timedOut = await Promise.race([exited.then(() => false), sleep(STOP_TIMEOUT_MS).then(() => true)]);
  if (timedOut) child.kill("SIGKILL");
  await exited;
}

function hasExited(child: ChildProcess): boolean {
  return child.exitCode !== null || child.signalCode !== null;
}
