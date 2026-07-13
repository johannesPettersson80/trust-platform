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
  localRuntimeTargetForAdsProbe,
  UNKNOWN_ADS_CONNECTION_PROBE_SAFETY_MESSAGE,
} from "../../networkCanvas/adsServiceProbeController";
import {
  classifyAdsServiceProbe,
  didAnyAdsServiceRespond,
  groupAdsServiceProbeResults,
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

suite("ADS service probe UX contracts", function () {
  test("ADS service errors keep socket details out of default cards", () => {
      const raw =
        "receiving UDP reply timed out (os error 10060); forcibly closed by local AMS router (os error 10054); ads-wire feature missing";
      const visible = adsServiceProbeVisibleError(raw);
      assert.strictEqual(visible, ADS_SERVICE_CHECK_FAILED_COPY);
      assert.doesNotMatch(visible, /UDP|router|10060|10054|ads-wire/i);

      const flow = source("networkCanvas/webview/AdsDiscoveryFlow.tsx");
      assert.ok(flow.includes("adsServiceProbeVisibleError(probe.error)"));
      assert.ok(flow.includes('return "Could not check this service"'));
      assert.ok(!flow.includes("Check failed — ${result.error?.message"));

      const deceptiveRaw =
        "active ADS connection: C:\\secret\\StaticRoutes.xml; forcibly closed (os error 10054)";
      assert.strictEqual(
        adsServiceProbeVisibleError(deceptiveRaw),
        ADS_SERVICE_CHECK_FAILED_COPY,
        "backend text containing a safe-message fragment must never bypass exact product-copy allowlisting"
      );
      assert.strictEqual(
        adsTechnicalDetail(deceptiveRaw),
        deceptiveRaw,
        "the raw detail remains available only to the collapsed Technical details disclosure"
      );
    });

  test("shows one primary ADS discovery action and reserves the wider Discover drawer", () => {
      assert.strictEqual(shouldShowScanSelected(["ads"]), false);
      assert.strictEqual(shouldShowScanSelected(["ads", "modbus_tcp"]), true);
      assert.strictEqual(activeDrawerWidth(false, false, false, true, undefined, false), 340);

      const pane = source("networkCanvas/webview/DiscoverPane.tsx");
      assert.ok(pane.includes("Scan ${selectedNonAdsScanRows.length} selected type"));
      assert.match(pane, /data-role="scan-selected"[\s\S]*className="trust-button"/);
      assert.strictEqual(
        discoveryOriginForMode("ads", "runtime:remote"),
        "this_host"
      );
      assert.strictEqual(
        discoveryOriginForMode("selected", "runtime:remote"),
        "runtime:remote"
      );
      const adsFlow = source("networkCanvas/webview/AdsDiscoveryFlow.tsx");
      assert.ok(adsFlow.includes('placeholder="Example: 192.168.50.42"'));
      assert.ok(adsFlow.includes('placeholder="Example: 5.23.91.12.1.1"'));
      assert.ok(adsFlow.includes('placeholder="Example: 9000, 9001"'));
      assert.ok(
        pane.includes('if (r.protocol === "ads")') &&
          pane.indexOf('if (r.protocol === "ads")') <
            pane.indexOf("if (selectedStoppedRuntimeReason)"),
        "a stopped hardware-scan origin must never disable the ordinary this-computer ADS action"
      );
    });

  test("automatically checks read-only ADS services and keeps recheck as recovery", () => {
      const pane = source("networkCanvas/webview/DiscoverPane.tsx");
      const flow = source("networkCanvas/webview/AdsDiscoveryFlow.tsx");

      assert.ok(pane.includes("autoAdsProbeCandidates"));
      assert.ok(pane.includes("onProbeAdsServices(candidate, snapshot.ports, snapshot.origin)"));
      assert.ok(!flow.includes('data-role="ads-probe-safety-confirmation"'));
      assert.ok(!flow.includes('data-role="ads-check-services"'));
      assert.ok(flow.includes('data-role="ads-recheck-services"'));
      assert.ok(flow.includes("Retry ADS service check"));
    });

  test("marks browsed ADS groups and variables for packaged UI proof", () => {
    const browse = source("networkCanvas/webview/BrowseTagsPanel.tsx");

    assert.ok(browse.includes('data-role="symbol-group"'));
    assert.ok(browse.includes("data-expanded={open}"));
    assert.ok(browse.includes('data-role="symbol-leaf"'));
  });

  test("checks every requested service even when one port has an isolated failure", async () => {
      const calls: number[] = [];
      const outcomes = new Map<
        number,
        "empty" | "unavailable" | "unsupported" | "check_failed"
      >([
        [851, "unavailable"],
        [852, "unsupported"],
        [853, "empty"],
        [854, "check_failed"],
        [9000, "empty"],
      ]);

      const results = await probeAdsServicesSequentially(
        [851, 852, 853, 854, 9000],
        async (port) => {
          calls.push(port);
          return response(outcomes.get(port) ?? "empty");
        }
      );

      assert.deepStrictEqual(calls, [851, 852, 853, 854, 9000]);
      assert.deepStrictEqual(
        results.map((result) => result.status),
        [
          "unavailable",
          "unsupported",
          "empty",
          "check_failed",
          "empty",
        ] satisfies AdsServiceProbeStatus[]
      );
    });

  test("never counts malformed or unexplained empty JSON as a responding ADS service", () => {
    const malformed: unknown[] = [
      {},
      { schema_version: 1, protocol: "opcua_client", kind: "symbols", tree: [] },
      { schema_version: 1, protocol: "ads", kind: "nodes", tree: [] },
      { schema_version: 1, protocol: "ads", kind: "symbols", tree: null },
      {
        schema_version: 1,
        protocol: "ads",
        kind: "symbols",
        tree: [{ id: "broken" }],
      },
      {
        schema_version: 1,
        protocol: "ads",
        kind: "symbols",
        tree: [{ id: "", name: "Value", path: "MAIN.Value" }],
      },
      {
        schema_version: 1,
        protocol: "ads",
        kind: "symbols",
        route: {},
        tree: [],
      },
      {
        schema_version: 1,
        protocol: "ads",
        kind: "symbols",
        route: { status: "connected-ish" },
        tree: [],
      },
      { schema_version: 1, protocol: "ads", kind: "symbols", tree: [] },
    ];

    for (const value of malformed) {
      const result = classifyAdsServiceProbe(
        301,
        value as BrowseSymbolsResponse
      );
      assert.strictEqual(result.status, "check_failed");
      assert.strictEqual(result.usable, false);
      assert.strictEqual(didAnyAdsServiceRespond([result]), false);
      assert.match(result.error?.code ?? "", /invalid|unexplained/);
    }

    const explicitEmpty = classifyAdsServiceProbe(501, response("empty"));
    assert.strictEqual(explicitEmpty.status, "empty");
    assert.strictEqual(didAnyAdsServiceRespond([explicitEmpty]), true);

    const available = classifyAdsServiceProbe(851, {
      schema_version: 1,
      protocol: "ads",
      kind: "symbols",
      tree: [{ id: "main-value", name: "Value", path: "MAIN.Value" }],
    });
    assert.strictEqual(available.status, "available");
    assert.strictEqual(available.symbolCount, 1);
    assert.strictEqual(available.usable, true);
  });

  test("keeps only responding ADS services on the normal card", () => {
    const results: AdsServiceProbeResult[] = [
      { port: 851, status: "available", symbolCount: 8, usable: true },
      { port: 852, status: "unavailable", symbolCount: 0, usable: false },
      { port: 301, status: "unsupported", symbolCount: 0, usable: false },
      { port: 501, status: "check_failed", symbolCount: 0, usable: false },
      { port: 9000, status: "empty", symbolCount: 0, usable: false },
      { port: 9001, status: "route_missing", symbolCount: 0, usable: false },
    ];

    const grouped = groupAdsServiceProbeResults(results);
    assert.deepStrictEqual(
      grouped.responding.map((result) => [result.port, result.status]),
      [
        [851, "available"],
        [301, "unsupported"],
        [9000, "empty"],
      ],
    );
    assert.deepStrictEqual(
      grouped.diagnostics.map((result) => [result.port, result.status]),
      [
        [852, "unavailable"],
        [501, "check_failed"],
        [9001, "route_missing"],
      ],
    );

    const flow = source("networkCanvas/webview/AdsDiscoveryFlow.tsx");
    assert.ok(flow.includes("Responding ADS services for {name}"));
    assert.ok(flow.includes('data-result-visibility="responding"'));
    assert.ok(flow.includes('data-result-visibility="technical"'));
    assert.ok(flow.includes('data-role="ads-service-diagnostics-summary"'));
    assert.ok(flow.includes('data-role="ads-no-service-response"'));
    assert.ok(flow.includes("See Technical details above."));
    assert.ok(!flow.includes("Technical details are collapsed above."));
    assert.ok(
      flow.indexOf('data-role="ads-browse-variables"') <
        flow.indexOf('data-role="ads-service-diagnostics-summary"'),
      "Browse must remain directly below responding choices, before diagnostic summary copy",
    );
  });

  test("preserves a failed automatic ADS path beside successful path results", () => {
      assert.strictEqual(discoveryProgressStatus("scanning"), "scanning");
      assert.strictEqual(discoveryProgressStatus("done"), "done");
      assert.strictEqual(discoveryProgressStatus("failed"), "failed");
      assert.strictEqual(discoveryProgressStatus("unexpected"), "scanning");

      const withPartialFailure = reduceDiscoverySessionState(
        {
          scanning: true,
          progress: [],
          results: [],
          sessionCurrent: true,
          terminal: false,
          adsServiceProbes: {},
        },
        {
          type: "progress",
          row: {
            protocol: "ads",
            label: "ADS on the discovery computer",
            status: discoveryProgressStatus("failed"),
          },
        }
      );
      const completed = reduceDiscoverySessionState(withPartialFailure, {
        type: "results",
        candidates: [
          {
            id: "ads:remote",
            protocol: "ads",
            label: "ADS device",
            source: "ads_broadcast",
            confidence: "observed",
            params: { ams_net_id: "1.2.3.4.1.1", host: "1.2.3.4" },
          },
        ],
      });

      assert.strictEqual(completed.progress[0]?.status, "failed");
      assert.strictEqual(completed.results.length, 1);
      assert.strictEqual(completed.error, undefined);
    });

  test("starting a second device check cannot leave the canceled card stuck probing", () => {
      const initial: DiscoverySessionState = {
        scanning: false,
        progress: [],
        results: [],
        sessionCurrent: true,
        terminal: true,
        adsServiceProbes: {
          deviceA: {
            probing: true,
            currentPort: 851,
            results: [],
            completed: false,
          },
        },
      };
      const switched = reduceDiscoverySessionState(initial, {
        type: "adsProbeStarted",
        candidateId: "deviceB",
      });

      assert.strictEqual(switched.adsServiceProbes.deviceA?.probing, false);
      assert.strictEqual(switched.adsServiceProbes.deviceA?.completed, true);
      assert.match(switched.adsServiceProbes.deviceA?.error ?? "", /canceled/i);
      assert.strictEqual(switched.adsServiceProbes.deviceB?.probing, true);

      const pane = source("networkCanvas/webview/DiscoverPane.tsx");
      assert.ok(
        pane.includes("adsProbeRunning && !adsServiceProbes[c.id]?.probing") &&
          pane.includes("Wait for the current ADS device check to finish."),
        "recheck and browse controls on other cards must stay disabled while one device is being probed"
      );
    });
});
