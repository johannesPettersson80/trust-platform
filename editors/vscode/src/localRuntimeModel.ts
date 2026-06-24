// Pure model for managed local runtimes (Phase 9) — NO vscode import → unit-testable. A "managed local
// runtime" is a fleet.toml runtime project on THIS computer whose process the extension owns (Start/Stop
// via `trust-runtime fleet runtime …`), distinct from the ephemeral debug simulator and from remotes we
// only connect to.

export type ManagedRuntimeState = "running" | "stopped";

export interface ManagedRuntime {
  readonly name: string;
  readonly controlEndpoint: string;
  readonly state: ManagedRuntimeState;
  readonly projectPath?: string;
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
  readonly status?: string; // "running" | "stopped" | "starting" | "stopping"
  readonly path?: string;
  readonly control_endpoint?: string;
  readonly log_path?: string;
  readonly message?: string;
}

export interface ManagedLifecycleResult {
  readonly ok: boolean;
  readonly status?: string;
  readonly message?: string;
  readonly controlEndpoint?: string;
  readonly projectPath?: string;
}

// A managed Start/Stop is only HONESTLY successful when the backend reports the *reached* state:
// start → "running" (NOT "starting": process up but control unreachable), stop → "stopped" (NOT
// "stopping"). Anything else is surfaced with the backend's message, not treated as success.
export function isManagedLifecycleSuccess(
  action: "start" | "stop",
  status: string | undefined
): boolean {
  return action === "start" ? status === "running" : status === "stopped";
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
        projectPath: status?.path ?? entry.path,
        logPath: status?.log_path,
      };
    });
}

export function parseRuntimeControlAuthToken(text: string): string | undefined {
  let section = "";
  for (const raw of text.split(/\r?\n/)) {
    const line = stripTomlInlineComment(raw).trim();
    if (!line) {
      continue;
    }
    if (line.startsWith("[") && line.endsWith("]")) {
      section = line.slice(1, -1).trim();
      continue;
    }
    if (section === "") {
      const dotted = line.match(/^runtime\.control\.auth_token\s*=\s*(.+)$/);
      if (dotted) {
        return parseTomlString(dotted[1]);
      }
    }
    if (section !== "runtime.control") {
      continue;
    }
    const match = line.match(/^auth_token\s*=\s*(.+)$/);
    if (!match) {
      continue;
    }
    return parseTomlString(match[1]);
  }
  return undefined;
}

function stripTomlInlineComment(line: string): string {
  let inSingle = false;
  let inDouble = false;
  for (let i = 0; i < line.length; i += 1) {
    const ch = line[i];
    if (ch === "'" && !inDouble) {
      inSingle = !inSingle;
    } else if (ch === '"' && !inSingle) {
      inDouble = !inDouble;
    } else if (ch === "#" && !inSingle && !inDouble) {
      return line.slice(0, i);
    }
  }
  return line;
}

function parseTomlString(value: string): string | undefined {
  const trimmed = value.trim();
  if (
    (trimmed.startsWith('"') && trimmed.endsWith('"')) ||
    (trimmed.startsWith("'") && trimmed.endsWith("'"))
  ) {
    return trimmed.slice(1, -1).trim() || undefined;
  }
  return undefined;
}
