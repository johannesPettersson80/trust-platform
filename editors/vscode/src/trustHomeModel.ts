// The sidebar's run targets and the one action of the selected target are the language server's answer
// (`trust-lsp.runTargets`, action `selection`, specification 25 §5.1 and §6.1): the Simulator, the managed runtimes, the
// configured runtimes and the runtime the extension is online with; Start and Stop for the processes the extension owns,
// Connect and Disconnect for the runtimes whose session it owns, a remote never stopped. The sidebar, the status bar and
// the speed chooser render this answer.

import { getTrustConfiguration } from "./configuration";
import type { ManagedRuntime } from "./localRuntimeModel";
import { projectRequest } from "./projectRequest";

export type RuntimeKind = "simulator" | "local" | "remote";

export type RuntimeStatus =
  | "stopped"
  | "starting"
  | "running"
  | "stopping"
  | "connected"
  | "disconnected"
  | "unreachable"
  | "unavailable";

// The single primary verb for the selected runtime's current state. `none` = no actionable button
// (e.g. mid-transition while starting/connecting).
export type RuntimeAction = "start" | "stop" | "connect" | "disconnect" | "none";

export const SIMULATOR_RUNTIME_ID = "simulator";

export interface RuntimeOption {
  readonly id: string;
  readonly label: string;
  readonly kind: RuntimeKind;
  /** The control endpoint that selecting the target points the runtime settings at; none for the Simulator and a managed
   * runtime that does not run. */
  readonly runEndpoint?: string;
}

export interface PrimaryAction {
  readonly action: RuntimeAction;
  // The button text — a literal verb, or a disabled progress label ("Starting…" | "Connecting…").
  readonly label: string;
  readonly enabled: boolean;
  // Why the action is disabled and what to do next, when there is something to explain.
  readonly hint?: string;
  /** Whether the compile gate can block the action: an enabled Start (specification 25 §6.6). */
  readonly gateable?: boolean;
}

export interface PrimaryActionGate {
  readonly reason: string;
}

export interface SelectedRuntime {
  readonly id: string;
  readonly label: string;
  readonly kind: RuntimeKind;
  readonly status: RuntimeStatus;
  readonly statusLabel: string;
  readonly primary: PrimaryAction;
  readonly runEndpoint?: string;
}

export interface RemoteRuntime {
  readonly id: string; // the control endpoint string — its identity
  readonly label: string; // friendly host name
}

// The slice of the lifecycle status the language server needs, plus the starting flag.
export interface RuntimeModelSnapshot {
  readonly runtimeMode: "simulate" | "online";
  readonly runtimeState: "running" | "connected" | "stopped";
  readonly endpoint: string;
  readonly endpointConfigured: boolean;
  readonly endpointReachable: boolean;
  readonly starting: boolean;
}

export interface RunTargets {
  readonly options: readonly RuntimeOption[];
  readonly remotes: readonly RemoteRuntime[];
  readonly selected: SelectedRuntime;
  /** Whether the stored selection names one of the options; when not, the Simulator is selected. */
  readonly selectionValid: boolean;
  /** The name the status bar gives the runtime the extension is connected to; none while it is connected to none. */
  readonly connectedLabel?: string;
}

export interface RunTargetsInput {
  readonly snapshot: RuntimeModelSnapshot;
  readonly managed: ReadonlyArray<ManagedRuntime>;
  readonly selectedId: string;
}

/** The run targets and the selected one, as the language server answers them. Without its answer the Simulator is shown
 * with no action and the reason. */
export async function resolveRunTargets(input: RunTargetsInput): Promise<RunTargets> {
  const answer = await projectRequest<RunTargets>("trust-lsp.runTargets", {
    action: "selection",
    snapshot: input.snapshot,
    fleetEndpoints: getTrustConfiguration().get<string[]>("runtime.fleetEndpoints", []) ?? [],
    managed: input.managed.map(({ name, controlEndpoint, state }) => ({ name, controlEndpoint, state })),
    selectedId: input.selectedId,
  });
  if (answer.ok) {
    return {
      options: answer.options.map(withoutNulls),
      remotes: answer.remotes,
      selected: withoutNulls({ ...answer.selected, primary: withoutNulls(answer.selected.primary) }),
      selectionValid: answer.selectionValid,
      ...(answer.connectedLabel ? { connectedLabel: answer.connectedLabel } : {}),
    };
  }
  const simulator: RuntimeOption = { id: SIMULATOR_RUNTIME_ID, label: "Simulator", kind: "simulator" };
  return {
    options: [simulator],
    remotes: [],
    selected: {
      ...simulator,
      status: "unavailable",
      statusLabel: "Status unavailable",
      primary: { action: "none", label: "Start", enabled: false, hint: answer.error },
    },
    selectionValid: input.selectedId === SIMULATOR_RUNTIME_ID,
  };
}

/** The selected target with the compile gate's verdict on its primary action: the language server says which action the
 * gate can block (`gateable`) and the gate gives the reason. */
export function withPrimaryActionGate(
  selected: SelectedRuntime,
  gate: PrimaryActionGate | undefined
): SelectedRuntime {
  if (!gate?.reason || !selected.primary.gateable) {
    return selected;
  }
  return { ...selected, primary: { ...selected.primary, enabled: false, hint: gate.reason } };
}

/** `value` without the members the language server sent as `null`. */
function withoutNulls<T extends object>(value: T): T {
  return Object.fromEntries(Object.entries(value).filter(([, member]) => member !== null)) as T;
}
