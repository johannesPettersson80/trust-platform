// Pure model for the host "Set up runtime…" wizard (§0.6.0). NO React/vscode, so it's unit-testable.
// The wizard offers ONLY what the backend can actually do; unbuilt options are surfaced under an explicit
// "coming soon" area (NOT sprayed as live buttons) — §0.6.12 omit-vs-grey rule.

export type SetUpRuntimeOptionId = "connect" | "local" | "install" | "docker";

export interface SetUpRuntimeOption {
  readonly id: SetUpRuntimeOptionId;
  readonly label: string;
  readonly detail: string;
  readonly available: boolean;
  readonly reason?: string; // why it's unavailable (shown when !available)
}

export interface SetUpRuntimeCaps {
  readonly connectExisting: boolean; // add a control endpoint — exists today
  readonly runLocal: boolean; // scaffold a runtime project on this host — exists today
  readonly installNative: boolean; // remote install over SSH — phase 11
  readonly docker: boolean; // run as a container — phase 12
}

// v1 capabilities: connect + add-local exist; native install + Docker are v2 (phases 11/12).
export const V1_SETUP_CAPS: SetUpRuntimeCaps = {
  connectExisting: true,
  runLocal: true,
  installNative: false,
  docker: false,
};

const LATER = "Available in a later release.";

export function setUpRuntimeOptions(
  caps: SetUpRuntimeCaps
): SetUpRuntimeOption[] {
  return [
    {
      id: "connect",
      label: "Connect existing runtime",
      detail: "Point at a runtime already running (this computer, a Pi, an IPC).",
      available: caps.connectExisting,
      reason: caps.connectExisting ? undefined : LATER,
    },
    {
      id: "local",
      label: "Run a runtime on this computer",
      detail: "Create a managed runtime project here, then start it from the Run target.",
      available: caps.runLocal,
      reason: caps.runLocal ? undefined : LATER,
    },
    {
      id: "install",
      label: "Install truST runtime",
      detail: "Install on a Raspberry Pi / IPC over SSH.",
      available: caps.installNative,
      reason: caps.installNative ? undefined : LATER,
    },
    {
      id: "docker",
      label: "Run in Docker",
      detail: "Run the runtime as a container.",
      available: caps.docker,
      reason: caps.docker ? undefined : LATER,
    },
  ];
}
