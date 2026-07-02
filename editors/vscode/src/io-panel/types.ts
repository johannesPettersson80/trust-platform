import type { AdsStatusSummary } from "../adsStatusSummary";

export type IoEntry = {
  name?: string;
  address: string;
  value: string;
  forced?: boolean;
};

export type IoState = {
  inputs: IoEntry[];
  outputs: IoEntry[];
  memory: IoEntry[];
};

export type CompileIssue = {
  file: string;
  line: number;
  column: number;
  severity: "error" | "warning";
  message: string;
  code?: string;
  source?: string;
};

export type CompileResult = {
  target: string;
  dirty: boolean;
  errors: number;
  warnings: number;
  issues: CompileIssue[];
  runtimeStatus: "ok" | "error" | "skipped";
  runtimeMessage?: string;
};

export type RuntimeStatusPayload = {
  running: boolean;
  inlineValuesEnabled: boolean;
  runtimeMode: "simulate" | "online";
  runtimeState: "running" | "connected" | "stopped";
  targetLabel?: string;
  endpoint: string;
  endpointConfigured: boolean;
  endpointEnabled: boolean;
  endpointReachable: boolean;
  access?: RuntimeAccessPayload;
  ads?: AdsStatusSummary;
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

export type CompileOptions = {
  startDebugAfter?: boolean;
};
