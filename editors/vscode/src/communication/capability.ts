// The `comm.capabilities` answer of a runtime (specification 11): each protocol's build, setup and
// health facts as the runtime reports them.

export type CommunicationStatusId =
  | "not_in_build"
  | "not_configured"
  | "simulate"
  | "runtime_unreachable"
  | "connected"
  | "degraded"
  | "error"
  | "configured_policy";

export interface RuntimeCapabilityStatus {
  id: string;
  built: boolean;
  configured: boolean;
  operational: boolean;
  platform?: string;
  health: CommunicationStatusId;
  detail: string;
  next_action?: {
    kind: string;
    label: string;
  };
}

export interface CommCapabilitiesResponse {
  schema_version: number;
  capabilities: RuntimeCapabilityStatus[];
}
