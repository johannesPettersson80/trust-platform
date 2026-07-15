import type { RuntimeTarget } from "../runtimeTarget";
import {
  summarizeAdsStatus,
  type AdsStatusReport,
} from "../adsStatusSummary";
import {
  COMMUNICATION_PROTOCOLS,
  type CommunicationProtocolDescriptor,
} from "./communicationProtocols";

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

export interface CommunicationCardModel {
  protocol: CommunicationProtocolDescriptor;
  status: CommunicationStatusId;
  detail: string;
  nextStep: string;
  capability?: RuntimeCapabilityStatus;
}

export function buildCommunicationCards(
  runtime: RuntimeTarget,
  capabilities?: CommCapabilitiesResponse,
  error?: string,
  adsStatus?: AdsStatusReport
): CommunicationCardModel[] {
  const byId = new Map(
    (capabilities?.capabilities ?? []).map((capability) => [
      capability.id,
      capability,
    ])
  );
  return COMMUNICATION_PROTOCOLS.map((protocol) => {
    if (protocol.id === "enterprise") {
      return {
        protocol,
        status: "configured_policy",
        detail: "Guidance only. Pick OPC UA, MQTT, ADS, or OpenOT based on the consumer.",
        nextStep: "Choose the protocol that matches the consuming system.",
      };
    }
    if (runtime.status !== "online_reachable") {
      return {
        protocol,
        status: runtime.status === "simulate" ? "simulate" : "runtime_unreachable",
        detail: runtimeBlockedDetail(runtime),
        nextStep: "Open Runtime pane",
      };
    }
    if (protocol.id === "ads") {
      const client = byId.get("ads");
      const server = byId.get("ads_server");
      if (client || server || adsStatus) {
        const summary = adsStatus ? summarizeAdsStatus(adsStatus) : undefined;
        const status = combinedAdsStatus(client, server, summary?.overall);
        const details = [
          summary?.text,
          `Client: ${client?.detail ?? "not reported"} Server: ${server?.detail ?? "not reported"}`,
        ].filter(Boolean);
        return {
          protocol,
          status,
          detail: details.join(" "),
          nextStep: nextActionLabel(client ?? server, status),
          capability: client ?? server,
        };
      }
    }
    const capability = byId.get(protocol.id);
    if (capability) {
      return {
        protocol,
        status: capability.health,
        detail: capability.detail,
        nextStep: nextActionLabel(capability, capability.health),
        capability,
      };
    }
    return {
      protocol,
      status: error ? "degraded" : "not_configured",
      detail: error
        ? `Runtime capabilities are unavailable: ${error}`
        : "This runtime does not report Communication capabilities yet.",
      nextStep: error ? "Retry or open Runtime pane" : "Update runtime or use docs",
    };
  });
}

function combinedAdsStatus(
  client: RuntimeCapabilityStatus | undefined,
  server: RuntimeCapabilityStatus | undefined,
  adsOverall?: string
): CommunicationStatusId {
  const statuses = [client?.health, server?.health].filter(
    Boolean
  ) as CommunicationStatusId[];
  if (statuses.includes("error")) return "error";
  if (statuses.includes("connected")) return "connected";
  if (statuses.includes("degraded")) return "degraded";
  if (statuses.some((status) => status !== "not_in_build")) {
    return "not_configured";
  }
  const fromAdsStatus = communicationStatusFromAdsOverall(adsOverall);
  if (fromAdsStatus) return fromAdsStatus;
  return "not_in_build";
}

function communicationStatusFromAdsOverall(
  overall: string | undefined
): CommunicationStatusId | undefined {
  switch ((overall ?? "").toLowerCase()) {
    case "connected":
    case "healthy":
    case "ok":
      return "connected";
    case "degraded":
    case "stale":
      return "degraded";
    case "error":
    case "faulted":
    case "failed":
      return "error";
    case "disabled":
    case "not_configured":
    case "none":
      return "not_configured";
    default:
      return undefined;
  }
}

export function statusLabel(status: CommunicationStatusId): string {
  switch (status) {
    case "not_in_build":
      return "Not in this build";
    case "not_configured":
      return "Not configured";
    case "simulate":
      return "Simulate mode";
    case "runtime_unreachable":
      return "Runtime unreachable";
    case "connected":
      return "Connected";
    case "degraded":
      return "Degraded";
    case "error":
      return "Error";
    case "configured_policy":
      return "Configured policy";
  }
}

function runtimeBlockedDetail(runtime: RuntimeTarget): string {
  switch (runtime.status) {
    case "simulate":
      return "Select an online runtime before configuring production communication.";
    case "missing_endpoint":
      return "No online runtime control endpoint is selected.";
    case "auth_failed":
      return runtime.authFailureKind === "missing"
        ? "No auth token is configured for the selected runtime."
        : "The selected runtime rejected the configured control credentials.";
    case "online_unreachable":
      return "The selected runtime is not reachable.";
    default:
      return "Runtime must be reachable before Communication status can be proven.";
  }
}

function nextActionLabel(
  capability: RuntimeCapabilityStatus | undefined,
  status: CommunicationStatusId
): string {
  if (capability?.next_action?.label) {
    return capability.next_action.label;
  }
  switch (status) {
    case "connected":
      return "Status";
    case "not_in_build":
      return "Get a build with this feature";
    case "simulate":
    case "runtime_unreachable":
      return "Open Runtime pane";
    case "configured_policy":
      return "Review policy";
    case "not_configured":
      return "Set up";
    case "degraded":
    case "error":
      return "Review setup";
  }
}
