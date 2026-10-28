// The managed local runtimes (Phase 9): the fleet.toml runtime projects on THIS computer whose process the extension owns
// (Start/Stop via `trust-runtime fleet runtime …`), distinct from the debug simulator and from the runtimes it only connects
// to. Their states, labels and lifecycle outcomes are the language server's answers (`trust-lsp.runTargets`, actions
// `managed` and `managedOutcome`, specification 25 §5.1 and §6.5).

export type ManagedRuntimeState =
  | "running"
  | "stopped"
  | "starting"
  | "stopping"
  | "unavailable";

export interface ManagedRuntime {
  readonly name: string;
  /** The runtime's name in the sidebar and the canvas: `<name> (this computer)`. */
  readonly label: string;
  readonly controlEndpoint: string;
  readonly state: ManagedRuntimeState;
  readonly projectPath?: string;
  readonly logPath?: string;
}

export interface ManagedLifecycleResult {
  /** The Start reached `running`, or the Stop reached `stopped`. */
  readonly ok: boolean;
  readonly status?: string;
  readonly message?: string;
  readonly controlEndpoint?: string;
  readonly projectPath?: string;
  /** The runtime's name in the sidebar and the canvas, which its Live Values session takes. */
  readonly label?: string;
}
