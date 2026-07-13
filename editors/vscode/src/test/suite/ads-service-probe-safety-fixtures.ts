import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";

import { hasActiveOrRecoveringAdsConnection } from "../../adsStatusSummary";
import {
  ACTIVE_ADS_CONNECTION_PROBE_SAFETY_MESSAGE,
  AdsServiceProbeController,
  adsStatusProbeSafetyMessage,
  failedBrowseResponse,
  localRuntimeTargetForAdsProbe,
  UNKNOWN_ADS_CONNECTION_PROBE_SAFETY_MESSAGE,
} from "../../networkCanvas/adsServiceProbeController";
import {
  classifyAdsServiceProbe,
  probeAdsServicesSequentially,
  resolveSelectedAdsServicePort,
  type AdsServiceProbeResult,
  type AdsServiceProbeStatus,
} from "../../networkCanvas/adsServiceProbeModel";
import {
  runJsonCommand,
  type BrowseSymbolsResponse,
} from "../../networkCanvas/offlineComm";
import { DiscoveryBrowseLeaseStore } from "../../networkCanvas/discoveryBrowseLease";
import { discoverLabel } from "../../networkCanvas/discoveryController";
import { resolveRegisteredDiscoveryOriginEndpoint } from "../../networkCanvas/discoveryOriginTargets";
import {
  discoveryTypedFailureMessage,
  offersAdsManualIdentityRecovery,
} from "../../networkCanvas/discoveryErrors";
import {
  adsEmptyIdentityCopy,
  adsEmptyRecoveryFocusRole,
  adsServiceProbeResultsNeedRecheck,
  applyAdsEmptyRecovery,
  discoveryOriginForMode,
  discoveryProgressCopy,
  shouldShowScanSelected,
  validateAdsDiscoveryDraft,
  type AdsDiscoveryDraft,
  type AdsDiscoveryScanSnapshot,
} from "../../networkCanvas/webview/discoverPaneModel";
import {
  ADS_SERVICE_CHECK_FAILED_COPY,
  adsServiceProbeVisibleError,
  adsTechnicalDetail,
} from "../../networkCanvas/webview/adsErrorPresentation";
import { activeDrawerWidth } from "../../networkCanvas/webview/networkCanvasStyles";
import { sendRuntimeControlRequest } from "../../runtimeControlClient";
import {
  discoveryProgressStatus,
  reduceDiscoverySessionState,
  type DiscoverySessionState,
} from "../../networkCanvas/webview/useDiscoverySession";

function response(
  status: "empty" | "unavailable" | "unsupported" | "check_failed"
): BrowseSymbolsResponse {
  const error =
    status === "empty"
      ? { code: "empty_symbol_table", message: "no symbols" }
      : status === "unavailable"
      ? { code: "ads_port_unavailable", message: "target port not found" }
      : status === "unsupported"
        ? { code: "symbol_upload_unsupported", message: "not supported" }
        : { code: "control_request_failed", message: "authentication failed" };
  return {
    schema_version: 1,
    protocol: "ads",
    kind: "symbols",
    tree: [],
    error,
  };
}

function source(relativePath: string): string {
  return fs.readFileSync(
    path.resolve(__dirname, "../../../src", relativePath),
    "utf8"
  );
}

export {
  assert,
  fs,
  os,
  path,
  vscode,
  hasActiveOrRecoveringAdsConnection,
  ACTIVE_ADS_CONNECTION_PROBE_SAFETY_MESSAGE,
  AdsServiceProbeController,
  adsStatusProbeSafetyMessage,
  failedBrowseResponse,
  localRuntimeTargetForAdsProbe,
  UNKNOWN_ADS_CONNECTION_PROBE_SAFETY_MESSAGE,
  classifyAdsServiceProbe,
  probeAdsServicesSequentially,
  resolveSelectedAdsServicePort,
  runJsonCommand,
  DiscoveryBrowseLeaseStore,
  discoverLabel,
  resolveRegisteredDiscoveryOriginEndpoint,
  discoveryTypedFailureMessage,
  offersAdsManualIdentityRecovery,
  adsEmptyIdentityCopy,
  adsEmptyRecoveryFocusRole,
  adsServiceProbeResultsNeedRecheck,
  applyAdsEmptyRecovery,
  discoveryOriginForMode,
  discoveryProgressCopy,
  shouldShowScanSelected,
  validateAdsDiscoveryDraft,
  ADS_SERVICE_CHECK_FAILED_COPY,
  adsServiceProbeVisibleError,
  adsTechnicalDetail,
  activeDrawerWidth,
  sendRuntimeControlRequest,
  discoveryProgressStatus,
  reduceDiscoverySessionState,
  response,
  source,
};
export type {
  AdsServiceProbeResult,
  AdsServiceProbeStatus,
  BrowseSymbolsResponse,
  AdsDiscoveryDraft,
  AdsDiscoveryScanSnapshot,
  DiscoverySessionState,
};
