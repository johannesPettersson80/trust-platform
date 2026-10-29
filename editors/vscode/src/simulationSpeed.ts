import * as vscode from "vscode";

import { getSelectedRuntimeId } from "./selectedRuntime";
import { listManagedRuntimes } from "./localRuntime";
import { runtimeLifecycleService, type RuntimeLifecycleSnapshot } from "./runtimeLifecycle";
import {
  sendRuntimeControlRequest,
  type RuntimeControlRequestOptions,
} from "./runtimeControlClient";
import { projectRequest } from "./projectRequest";
import {
  resolveRunTargets,
  SIMULATOR_RUNTIME_ID,
  toModelSnapshot,
  type SelectedRuntime,
} from "./trustHomeModel";

// The project's speed, the speeds to offer, saving a chosen speed and whether the running simulator takes it at once are the
// language server's answers (`trust-lsp.simulationSpeed`, specification 46 §6.1 item 5). This module shows the chooser, sends
// the request the server names to the runtime, and reads the runtime's own speed words.

/** The status bar's refresh while a simulator runs: the Devices & Connections canvas cadence, never faster. */
export const SPEED_REFRESH_INTERVAL_MS = 1500;

const CONFIG_REQUEST_TIMEOUT_MS = 2000;
const SIMULATION_SPEED_COMMAND = "trust-lsp.simulationSpeed";

const speedChanged = new vscode.EventEmitter<void>();
/** Fires after the chosen speed is saved or applied, so the status bar and the sidebar show it at once. */
export const onDidChangeSimulationSpeed = speedChanged.event;

/** A simulation's speed: the configured whole-number speed and, once measured, the achieved speed (specification 46, §6.1). */
export interface SimulationSpeed {
  readonly configured: number;
  readonly achieved?: number;
}

/** What one status read of a running runtime reports: whether it simulates every attached I/O, its speeds, and the speed words
 * and the behind flag the runtime gives for them (specification 35, B9.2). */
export interface SimulatorStatusRead extends SimulationSpeed {
  readonly simulated: boolean;
  readonly speedText: string;
  readonly speedBehind: boolean;
  /** The runtime holds its scan for the debugger, at a breakpoint or a pause: its status `state` is `paused`. */
  readonly paused?: boolean;
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

export interface SpeedChoice {
  readonly speed: number;
  readonly label: string;
  readonly description?: string;
  readonly detail?: string;
}

/** The chooser's entries for the speeds the language server offers, with the current speed marked. */
export function speedChoices(current: number, speeds: readonly number[]): SpeedChoice[] {
  return speeds.map((speed) => ({
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

/** Whether the selected target is a simulator that is running: the status bar and the sidebar show its speed then. Where a
 * saved speed goes is the language server's decision (`trust-lsp.simulationSpeed`). */
export function isRunningSimulator(selected: Pick<SelectedRuntime, "kind" | "status">): boolean {
  return selected.kind === "simulator" && selected.status === "running";
}

/** The request the language server names for the running simulator after a save. */
export interface LiveSpeedRequest {
  readonly request: string;
  readonly params: unknown;
}

/** Sends the language server's request to the runtime; a refusal by the runtime throws its RuntimeControlError. */
export async function sendLiveSpeed(
  live: LiveSpeedRequest,
  endpoint: string,
  authToken: string | undefined,
  send: RuntimeRequestSender = sendRuntimeControlRequest
): Promise<void> {
  await send(endpoint, authToken, live.request, live.params, { timeoutMs: CONFIG_REQUEST_TIMEOUT_MS });
}

/** The speeds and simulation facts the runtime reports, or undefined when its status cannot be read: the language server
 * asks the runtime's `status` and reads it (`trust-lsp.runTargets`, action `speed`, specification 35 A2.4 and B9.2). */
export async function readRuntimeSpeed(
  endpoint: string,
  authToken: string | undefined
): Promise<SimulatorStatusRead | undefined> {
  const answer = await projectRequest<{ read: (SimulatorStatusRead & { achieved: number | null }) | null }>(
    "trust-lsp.runTargets",
    { action: "speed", endpoint, authToken },
    { wait: false }
  );
  if (!answer.ok || !answer.read) {
    return undefined;
  }
  const { achieved, ...read } = answer.read;
  return achieved === null ? read : { ...read, achieved };
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
  const targets = await resolveRunTargets({
    snapshot: toModelSnapshot(snapshot),
    managed,
    selectedId: getSelectedRuntimeId(),
  });
  return targets.selected;
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

/** The saved speed and the speeds to offer, or undefined without a project folder or a running language server. */
async function savedSpeeds(wait: boolean): Promise<{ speed: number; speeds: readonly number[] } | undefined> {
  const folder = runtimeLifecycleService.runtimeConfigTarget();
  if (!folder) {
    return undefined;
  }
  const answer = await projectRequest<{ speed: number; speeds: number[] }>(
    SIMULATION_SPEED_COMMAND,
    { action: "read", folder: folder.fsPath },
    { wait }
  );
  return answer.ok ? { speed: answer.speed, speeds: answer.speeds } : undefined;
}

async function savedSpeed(): Promise<number> {
  return (await savedSpeeds(false))?.speed ?? 1;
}

/** The "Simulation Speed…" command: saves the chosen speed and, when the language server says so, applies it live. */
export async function changeSimulationSpeed(context: vscode.ExtensionContext): Promise<void> {
  const folder = runtimeLifecycleService.runtimeConfigTarget();
  if (!folder) {
    void vscode.window.showWarningMessage("Open a truST project before choosing a simulation speed.");
    return;
  }
  const offered = await savedSpeeds(true);
  if (!offered) {
    void vscode.window.showErrorMessage("The simulation speed cannot be read: the truST language server is not running.");
    return;
  }
  const snapshot = await runtimeLifecycleService.snapshot();
  const selected = await selectedTarget(context, snapshot);
  const current = (await shownSimulatorSpeed(snapshot, selected)).configured;
  const pick = await vscode.window.showQuickPick(
    speedChoices(current, offered.speeds).map((choice) => ({
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
  const endpoint = snapshot.status.endpoint;
  const saved = await projectRequest<{ live: LiveSpeedRequest | null }>(SIMULATION_SPEED_COMMAND, {
    action: "save",
    folder: folder.fsPath,
    speed: pick.speed,
    target: { kind: selected.kind, status: selected.status, endpoint },
  });
  if (!saved.ok) {
    void vscode.window.showErrorMessage(`The simulation speed was not saved: ${saved.error}`);
    return;
  }
  announceSpeedChange();
  if (!saved.live) {
    return;
  }
  try {
    await sendLiveSpeed(saved.live, endpoint, await runtimeLifecycleService.controlAuthToken(endpoint));
    announceSpeedChange();
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
