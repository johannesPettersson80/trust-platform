import type { AdsStatusSummary } from "../adsStatusSummary";

export type IoEntry = {
  name?: string;
  address: string;
  source?: string;
  valueType?: string;
  value: string;
  forced?: boolean;
  quality?: "good" | "uncertain" | "stale" | "bad";
  writable?: boolean;
};

export type IoState = {
  scan?: number;
  inputs: IoEntry[];
  outputs: IoEntry[];
  memory: IoEntry[];
  ads: IoEntry[];
};

export type RuntimeStatusPayload = {
  running: boolean;
  inlineValuesEnabled: boolean;
  runtimeMode: "simulate" | "online";
  /** The running target is simulated (Specification 35, A2.4): the runtime reports simulation enabled and every attached
   * I/O driver simulated. Editor settings and the debug configuration never set it. */
  simulated?: boolean;
  /** The resource cycle the runtime reports, when it reports one (A7 connection-loss timeout). */
  cycleIntervalMs?: number;
  runtimeState: "running" | "connected" | "stopped";
  targetLabel?: string;
  endpoint: string;
  endpointConfigured: boolean;
  endpointEnabled: boolean;
  endpointReachable: boolean;
  access?: RuntimeAccessPayload;
  /** A Force on this target must be armed first (specification 25 §4.2), as the language server decides it. */
  forceRequiresArm?: boolean;
  /** What each section's rows may do in each row state (specification 25 §4.1, §4.2), as the language server decides
   * it. */
  rows?: Record<"inputs" | "outputs" | "memory" | "ads", Record<"live" | "forced" | "readOnly", RowOperations>>;
  ads?: AdsStatusSummary;
  /** The online runtime answered Connect but keeps debugging disabled, as the lifecycle reported it (specification 25,
   * section 4.4): the panel names that state instead of "Not connected". */
  debugDisabled?: boolean;
};

export type RowOperations = {
  write: boolean;
  force: boolean;
  release: boolean;
  /** Why an operation is not allowed, `null` while it is. */
  writeReason: string | null;
  forceReason: string | null;
  releaseReason: string | null;
};

export type RuntimeAccessPayload = {
  role?: string;
  allowWrite: boolean;
  allowForce: boolean;
  allowRelease: boolean;
  reason?: string;
};

export type SettingsPayload = {
  serverPath?: string;
  traceServer?: string;
  debugAdapterPath?: string;
  debugAdapterArgs?: string[];
  debugAdapterEnv?: Record<string, string>;
  runtimeControlEndpoint?: string;
  runtimeControlAuthToken?: string;
  runtimeIncludeGlobs?: string[];
  runtimeExcludeGlobs?: string[];
  runtimeIgnorePragmas?: string[];
  runtimeInlineValuesEnabled?: boolean;
};
