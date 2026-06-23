// Pure model for managed local runtimes (Phase 9) — NO vscode import → unit-testable. A "managed local
// runtime" is a fleet.toml runtime project on THIS computer whose process the extension owns (Start/Stop
// via `trust-runtime fleet runtime …`), distinct from the ephemeral debug simulator and from remotes we
// only connect to.

export type ManagedRuntimeState = "running" | "stopped";

export interface ManagedRuntime {
  readonly name: string;
  readonly controlEndpoint: string;
  readonly state: ManagedRuntimeState;
  readonly logPath?: string;
}

// Raw shapes from the CLI (`fleet list --json`, `fleet runtime status --json`).
export interface FleetListEntry {
  readonly name: string;
  readonly control_endpoint?: string;
  readonly path?: string;
  readonly web_port?: number;
}
export interface FleetListResponse {
  readonly runtimes?: FleetListEntry[];
}
export interface FleetRuntimeStatusResponse {
  readonly name?: string;
  readonly status?: string;
  readonly control_endpoint?: string;
  readonly log_path?: string;
}

export function normalizeManagedState(raw: string | undefined): ManagedRuntimeState {
  return raw === "running" ? "running" : "stopped";
}

// "<name> (this computer)" — disambiguates managed locals from the Simulator + remotes in the dropdown.
export function managedRuntimeLabel(name: string): string {
  return `${name} (this computer)`;
}

// Merge `fleet list` with per-name status into the managed-runtime list the UI consumes.
export function toManagedRuntimes(
  list: FleetListResponse | undefined,
  statusByName: ReadonlyMap<string, FleetRuntimeStatusResponse>
): ManagedRuntime[] {
  return (list?.runtimes ?? [])
    .filter((entry) => !!entry && typeof entry.name === "string" && entry.name.length > 0)
    .map((entry) => {
      const status = statusByName.get(entry.name);
      return {
        name: entry.name,
        controlEndpoint: status?.control_endpoint ?? entry.control_endpoint ?? "",
        state: normalizeManagedState(status?.status),
        logPath: status?.log_path,
      };
    });
}
