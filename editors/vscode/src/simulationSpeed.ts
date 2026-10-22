import * as vscode from "vscode";

import { getTrustConfiguration } from "./configuration";
import { getSelectedRuntimeId } from "./selectedRuntime";
import { listManagedRuntimes } from "./localRuntime";
import { runtimeLifecycleService, type RuntimeLifecycleSnapshot } from "./runtimeLifecycle";
import {
  runtimeTargetFromStatus,
  simulationSpeed,
  type SimulationSpeed,
} from "./controlStudio/runtimeTarget";
import {
  sendRuntimeControlRequest,
  type RuntimeControlRequestOptions,
} from "./runtimeControlClient";
import {
  remoteLabelFromEndpoint,
  selectedRuntime,
  SIMULATOR_RUNTIME_ID,
  type RemoteRuntime,
  type RuntimeModelSnapshot,
  type SelectedRuntime,
} from "./trustHomeModel";

/** The speeds the chooser offers (specification 33, §6.1: a whole number from 1 to 100). */
export const SIMULATION_SPEEDS: readonly number[] = [1, 2, 5, 10, 20, 50, 100];

/** The status bar's refresh while a simulator runs: the Devices & Connections canvas cadence, never faster. */
export const SPEED_REFRESH_INTERVAL_MS = 1500;

const SIMULATION_FILE = "simulation.toml";
const STATUS_REQUEST_TIMEOUT_MS = 750;
const CONFIG_REQUEST_TIMEOUT_MS = 2000;

/** Any table header: `[name]` or `[[name]]`, optionally followed by a comment. */
const TABLE_HEADER = /^\s*\[\[?\s*[A-Za-z_][A-Za-z0-9_.-]*\s*\]\]?\s*(#.*)?$/;
const SIMULATION_HEADER = /^\s*\[simulation\]\s*(#.*)?$/;
const TIME_SCALE_KEY = /^\s*time_scale\s*=/;
const TIME_SCALE_VALUE = /^\s*time_scale\s*=\s*(\d+)\s*(#.*)?$/;

const speedChanged = new vscode.EventEmitter<void>();
/** Fires after the chosen speed is saved or applied, so the status bar and the sidebar show it at once. */
export const onDidChangeSimulationSpeed = speedChanged.event;

/** What one status read of a running runtime reports: whether it simulates every attached I/O, its speeds, and the speed words
 * and the behind flag the runtime gives for them (specification 35, B9.2). */
export interface SimulatorStatusRead extends SimulationSpeed {
  readonly simulated: boolean;
  readonly speedText: string;
  readonly speedBehind: boolean;
}

/** A status read as announced: the runtime it came from, then what it reported. */
export interface SimulationSpeedRead extends SimulatorStatusRead {
  readonly runtimeId: string;
}

const speedRead = new vscode.EventEmitter<SimulationSpeedRead>();
/** Fires each time the status-bar read gets a status (runtimeControls.ts). It has no timer of its own (specification 35, B9.2). */
export const onDidReadSimulationSpeed = speedRead.event;
/** The latest read of each runtime by its id, so the Devices & Connections canvas shows the simulator's at its own refresh. */
const latestReads = new Map<string, SimulationSpeedRead>();

/** Records one status read and announces it. Only the status-bar read calls this. */
export function announceSimulationSpeedRead(read: SimulationSpeedRead): void {
  latestReads.set(read.runtimeId, read);
  speedRead.fire(read);
}

/** The latest status read of the local simulator, or undefined before one has been read. */
export function latestSimulatorSpeedRead(): SimulationSpeedRead | undefined {
  return latestReads.get(SIMULATOR_RUNTIME_ID);
}

// ---- simulation.toml (pure: no VS Code API) ----

interface TableRange {
  /** The index of the `[simulation]` header line. */
  readonly start: number;
  /** The index of the first line after the table, or the line count. */
  readonly end: number;
}

function simulationTable(lines: readonly string[]): TableRange | undefined {
  const start = lines.findIndex((line) => SIMULATION_HEADER.test(line));
  if (start < 0) {
    return undefined;
  }
  const next = lines.findIndex((line, index) => index > start && TABLE_HEADER.test(line));
  return { start, end: next < 0 ? lines.length : next };
}

/**
 * The file text with `[simulation] time_scale = scale`. Only that value changes: comments, `enabled`, `[[plants]]`,
 * `[[couplings]]` and every other key stay as they were. A missing table is inserted before the first table header,
 * after any leading comments; a missing file becomes just the table.
 */
export function withSimulationTimeScale(text: string | undefined, scale: number): string {
  if (text === undefined || text.trim() === "") {
    return `[simulation]\ntime_scale = ${scale}\n`;
  }
  const eol = text.includes("\r\n") ? "\r\n" : "\n";
  const lines = text.split(/\r?\n/);
  const table = simulationTable(lines);
  if (table) {
    const key = lines.findIndex(
      (line, index) => index > table.start && index < table.end && TIME_SCALE_KEY.test(line)
    );
    if (key >= 0) {
      lines[key] = lines[key].replace(
        /^(\s*time_scale\s*=\s*)[^#]*?(\s*(#.*)?)$/,
        (_whole: string, head: string, tail: string) => `${head}${scale}${tail}`
      );
    } else {
      lines.splice(table.start + 1, 0, `time_scale = ${scale}`);
    }
    return lines.join(eol);
  }
  const firstTable = lines.findIndex((line) => TABLE_HEADER.test(line));
  if (firstTable < 0) {
    const base = text.endsWith("\n") ? text : `${text}${eol}`;
    return `${base}${eol}[simulation]${eol}time_scale = ${scale}${eol}`;
  }
  lines.splice(firstTable, 0, "[simulation]", `time_scale = ${scale}`, "");
  return lines.join(eol);
}

/** The speed the project saves in `[simulation] time_scale`: 1 when the file, the table or a valid whole number is missing. */
export function savedSimulationSpeed(text: string | undefined): number {
  if (text === undefined) {
    return 1;
  }
  const lines = text.split(/\r?\n/);
  const table = simulationTable(lines);
  if (!table) {
    return 1;
  }
  for (let index = table.start + 1; index < table.end; index += 1) {
    const match = TIME_SCALE_VALUE.exec(lines[index]);
    if (match) {
      const scale = Number(match[1]);
      return Number.isInteger(scale) && scale >= 1 && scale <= 100 ? scale : 1;
    }
  }
  return 1;
}

export interface SpeedChoice {
  readonly speed: number;
  readonly label: string;
  readonly description?: string;
  readonly detail?: string;
}

/** The chooser's entries, with the current speed marked. */
export function speedChoices(current: number): SpeedChoice[] {
  return SIMULATION_SPEEDS.map((speed) => ({
    speed,
    label: `${speed}×`,
    ...(speed === current ? { description: "current" } : {}),
    ...(speed === 1 ? { detail: "Real time" } : {}),
  }));
}

// ---- runtime requests ----

/** Sends one control request; the extension's control client in production. */
export type RuntimeRequestSender = (
  endpoint: string,
  authToken: string | undefined,
  requestType: string,
  params?: unknown,
  options?: RuntimeControlRequestOptions
) => Promise<unknown>;

/** Whether the selected target is a simulator that is running (A2.4: the selected target decides where a speed goes). */
export function isRunningSimulator(selected: Pick<SelectedRuntime, "kind" | "status">): boolean {
  return selected.kind === "simulator" && selected.status === "running";
}

/**
 * Applies a speed to the running simulator with `config.set` (specification 33, §6.1 item 5). Returns "saved-only" when
 * the selected target is not a running simulator. A refusal by the runtime throws its RuntimeControlError.
 */
export async function applyLiveSimulationSpeed(
  input: {
    readonly selected: Pick<SelectedRuntime, "kind" | "status">;
    readonly endpoint: string;
    readonly authToken: string | undefined;
    readonly speed: number;
  },
  send: RuntimeRequestSender = sendRuntimeControlRequest
): Promise<"applied" | "saved-only"> {
  if (!isRunningSimulator(input.selected) || input.endpoint.trim() === "") {
    return "saved-only";
  }
  await send(
    input.endpoint,
    input.authToken,
    "config.set",
    { "simulation.time_scale": input.speed },
    { timeoutMs: CONFIG_REQUEST_TIMEOUT_MS }
  );
  return "applied";
}

/** The speeds and simulation facts the runtime reports, or undefined when its status cannot be read. */
export async function readRuntimeSpeed(
  endpoint: string,
  authToken: string | undefined,
  send: RuntimeRequestSender = sendRuntimeControlRequest
): Promise<SimulatorStatusRead | undefined> {
  try {
    const report = await send(endpoint, authToken, "status", undefined, {
      timeoutMs: STATUS_REQUEST_TIMEOUT_MS,
    });
    const target = runtimeTargetFromStatus(report);
    return {
      simulated: target.simulated,
      ...simulationSpeed(target),
      speedText: target.speedText,
      speedBehind: target.speedBehind,
    };
  } catch {
    return undefined;
  }
}

/** The last status read from a runtime. Refreshes between the 1.5 s ticks reuse it, so they add no requests. */
let lastSpeedRead: { readonly at: number; readonly speed: SimulatorStatusRead | undefined } | undefined;
const SPEED_READ_REUSE_MS = 1000;

/** The speeds and simulation facts of the runtime the lifecycle snapshot describes, when it runs. */
export async function readSelectedSimulatorSpeed(
  snapshot: RuntimeLifecycleSnapshot
): Promise<SimulatorStatusRead | undefined> {
  if (lastSpeedRead && Date.now() - lastSpeedRead.at < SPEED_READ_REUSE_MS) {
    return lastSpeedRead.speed;
  }
  const endpoint = snapshot.status.endpoint.trim();
  if (endpoint === "") {
    return undefined;
  }
  const speed = await readRuntimeSpeed(endpoint, await runtimeLifecycleService.controlAuthToken(endpoint));
  lastSpeedRead = { at: Date.now(), speed };
  return speed;
}

/** Tells the status bar and the sidebar that the speed changed; the next read is fresh. */
function announceSpeedChange(): void {
  lastSpeedRead = undefined;
  speedChanged.fire();
}

/** The selected target, from the same selectedRuntime model the status bar and the sidebar use. */
export async function selectedTarget(
  context: vscode.ExtensionContext,
  snapshot: RuntimeLifecycleSnapshot
): Promise<SelectedRuntime> {
  const managed = await listManagedRuntimes(context);
  return selectedRuntime({
    snapshot: toModelSnapshot(snapshot),
    remotes: fleetRemotes(),
    managed,
    selectedId: getSelectedRuntimeId(),
  });
}

/** The speed the page shows: its configured speed and the words for it (specification 35, B9.2). */
export interface ShownSimulatorSpeed extends SimulationSpeed {
  readonly text: string;
}

/**
 * The speed to show for the simulator: the running runtime's own speeds and the words it gives for them, or the saved speed,
 * which applies at the next start when nothing runs. A saved speed has no runtime to word it, so it is shown as its unit.
 */
export async function shownSimulatorSpeed(
  snapshot: RuntimeLifecycleSnapshot,
  selected: Pick<SelectedRuntime, "kind" | "status">
): Promise<ShownSimulatorSpeed> {
  if (isRunningSimulator(selected)) {
    const live = await readSelectedSimulatorSpeed(snapshot);
    if (live) {
      return { ...live, text: live.speedText };
    }
  }
  const configured = await savedSpeed();
  return { configured, text: `×${configured}` };
}

// ---- project file and command ----

/** The project folder that holds runtime.toml, as the debug configuration finds it: the folder, else its bundle/ folder. */
async function projectDirectory(root: vscode.Uri): Promise<vscode.Uri> {
  if (await fileExists(vscode.Uri.joinPath(root, "runtime.toml"))) {
    return root;
  }
  const bundle = vscode.Uri.joinPath(root, "bundle");
  if (await fileExists(vscode.Uri.joinPath(bundle, "runtime.toml"))) {
    return bundle;
  }
  return root;
}

async function simulationFile(): Promise<vscode.Uri | undefined> {
  const root = runtimeLifecycleService.runtimeConfigTarget();
  if (!root) {
    return undefined;
  }
  return vscode.Uri.joinPath(await projectDirectory(root), SIMULATION_FILE);
}

async function fileExists(uri: vscode.Uri): Promise<boolean> {
  try {
    await vscode.workspace.fs.stat(uri);
    return true;
  } catch {
    return false;
  }
}

/** The file's text, or undefined when it does not exist. A file that exists but cannot be read is an error. */
async function readSimulationFile(uri: vscode.Uri): Promise<string | undefined> {
  try {
    return Buffer.from(await vscode.workspace.fs.readFile(uri)).toString("utf8");
  } catch (error) {
    if (await fileExists(uri)) {
      throw error;
    }
    return undefined;
  }
}

async function savedSpeed(): Promise<number> {
  const file = await simulationFile();
  if (!file) {
    return 1;
  }
  try {
    return savedSimulationSpeed(await readSimulationFile(file));
  } catch {
    return 1;
  }
}

/** The "Simulation Speed…" command: saves the chosen speed and, for a running selected simulator, applies it live. */
export async function changeSimulationSpeed(context: vscode.ExtensionContext): Promise<void> {
  const file = await simulationFile();
  if (!file) {
    void vscode.window.showWarningMessage("Open a truST project before choosing a simulation speed.");
    return;
  }
  const snapshot = await runtimeLifecycleService.snapshot();
  const selected = await selectedTarget(context, snapshot);
  const current = (await shownSimulatorSpeed(snapshot, selected)).configured;
  const pick = await vscode.window.showQuickPick(
    speedChoices(current).map((choice) => ({
      label: choice.label,
      description: choice.description,
      detail: choice.detail,
      speed: choice.speed,
    })),
    {
      title: "Simulation speed",
      placeHolder: "How fast the simulation runs against the wall clock. 1× is real time.",
    }
  );
  if (!pick) {
    return;
  }
  await vscode.workspace.fs.writeFile(
    file,
    Buffer.from(withSimulationTimeScale(await readSimulationFile(file), pick.speed), "utf8")
  );
  announceSpeedChange();
  const endpoint = snapshot.status.endpoint;
  try {
    const applied = await applyLiveSimulationSpeed({
      selected,
      endpoint,
      authToken: isRunningSimulator(selected) ? await runtimeLifecycleService.controlAuthToken(endpoint) : undefined,
      speed: pick.speed,
    });
    if (applied === "applied") {
      announceSpeedChange();
    }
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    void vscode.window.showErrorMessage(
      `The runtime did not accept ${pick.speed}×: ${message} The saved speed in simulation.toml is kept.`
    );
  }
}

export function registerSimulationSpeedCommand(context: vscode.ExtensionContext): vscode.Disposable {
  return vscode.Disposable.from(
    vscode.commands.registerCommand("trust-lsp.simulation.speed", () => changeSimulationSpeed(context)),
    speedChanged,
    speedRead
  );
}

// ---- helpers shared with the status bar ----

function toModelSnapshot(snapshot: RuntimeLifecycleSnapshot): RuntimeModelSnapshot {
  return {
    runtimeMode: snapshot.status.runtimeMode,
    runtimeState: snapshot.status.runtimeState,
    endpoint: snapshot.status.endpoint,
    endpointConfigured: snapshot.status.endpointConfigured,
    endpointReachable: snapshot.status.endpointReachable,
    starting: snapshot.starting,
  };
}

/** The fleet endpoints from settings, as the status bar reads them (runtime.fleetEndpoints). */
function fleetRemotes(): RemoteRuntime[] {
  const endpoints = getTrustConfiguration().get<string[]>("runtime.fleetEndpoints", []) ?? [];
  const seen = new Set<string>();
  const remotes: RemoteRuntime[] = [];
  for (const raw of endpoints) {
    const endpoint = (raw ?? "").trim();
    if (!endpoint || seen.has(endpoint)) {
      continue;
    }
    seen.add(endpoint);
    remotes.push({ id: endpoint, label: remoteLabelFromEndpoint(endpoint) });
  }
  return remotes;
}
