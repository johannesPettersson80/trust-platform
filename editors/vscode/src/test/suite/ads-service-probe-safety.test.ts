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
  probeAdsServicesSequentially,
  resolveSelectedAdsServicePort,
  shouldShowAdsServiceCheckConfirmation,
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
  adsServiceProbeResultsNeedRecheck,
  applyAdsEmptyRecovery,
  discoveryProgressCopy,
  shouldShowScanSelected,
  validateAdsDiscoveryDraft,
  type AdsDiscoveryDraft,
  type AdsDiscoveryScanSnapshot,
} from "../../networkCanvas/webview/discoverPaneModel";
import { activeDrawerWidth } from "../../networkCanvas/webview/networkCanvasStyles";
import { sendRuntimeControlRequest } from "../../runtimeControlClient";
import {
  reduceDiscoverySessionState,
  type DiscoverySessionState,
} from "../../networkCanvas/webview/useDiscoverySession";

function response(
  status: "empty" | "unavailable" | "unsupported" | "check_failed"
): BrowseSymbolsResponse {
  if (status === "empty") {
    return { protocol: "ads", tree: [] };
  }
  const error =
    status === "unavailable"
      ? { code: "ads_port_unavailable", message: "target port not found" }
      : status === "unsupported"
        ? { code: "symbol_upload_unsupported", message: "not supported" }
        : { code: "control_request_failed", message: "authentication failed" };
  return { protocol: "ads", tree: [], error };
}

function source(relativePath: string): string {
  return fs.readFileSync(
    path.resolve(__dirname, "../../../src", relativePath),
    "utf8"
  );
}

suite("ADS service probe safety", () => {
  test("blocks remote service checks while the runtime owns ADS I/O", () => {
    const report = (state: string) => ({
      overall: "healthy",
      summary: "ADS status",
      connections: [
        {
          name: "production-plc",
          state,
          point_count: 4,
          degraded_points: 0,
          summary: state,
        },
      ],
    });

    for (const state of [
      "connected",
      "reconnecting",
      "not_ready",
      "faulted",
      "stale",
      "unknown",
    ]) {
      assert.strictEqual(
        hasActiveOrRecoveringAdsConnection(report(state)),
        true,
        `${state} must fail closed before opening a competing ADS connection.`
      );
    }
    assert.strictEqual(
      hasActiveOrRecoveringAdsConnection(report("disabled")),
      false
    );
    assert.strictEqual(
      hasActiveOrRecoveringAdsConnection({
        overall: "disabled",
        summary: "ADS is not configured.",
        connections: [],
      }),
      false
    );
    assert.strictEqual(
      adsStatusProbeSafetyMessage(report("connected")),
      ACTIVE_ADS_CONNECTION_PROBE_SAFETY_MESSAGE
    );
    assert.strictEqual(
      adsStatusProbeSafetyMessage({
        overall: "disabled",
        summary: "ADS is not configured.",
        connections: [],
      }),
      undefined
    );
    for (const malformed of [
      undefined,
      {},
      { overall: "disabled" },
      { overall: "healthy", connections: [] },
      { overall: "healthy", connections: [{}] },
    ]) {
      assert.strictEqual(
        adsStatusProbeSafetyMessage(malformed),
        UNKNOWN_ADS_CONNECTION_PROBE_SAFETY_MESSAGE
      );
    }
    assert.match(ACTIVE_ADS_CONNECTION_PROBE_SAFETY_MESSAGE, /protect live PLC I\/O/i);
    assert.match(UNKNOWN_ADS_CONNECTION_PROBE_SAFETY_MESSAGE, /could not verify.*protect PLC I\/O/i);

    const controller = source("networkCanvas/adsServiceProbeController.ts");
    const panel = source("networkCanvas/networkCanvasPanel.ts");
    assert.ok(controller.includes('"ads.status"'));
    assert.ok(controller.includes("return adsStatusProbeSafetyMessage(report);"));
    assert.ok(controller.includes("runtimeTargetOnDiscoveryComputer"));
    assert.ok(panel.includes("localRuntimeTargetForAdsProbe(activeRuntimeTarget)"));
    const localRuntime = {
      mode: "online",
      endpoint: "tcp://127.0.0.1:9901",
      endpointEnabled: true,
      reachable: true,
      status: "online_reachable",
      label: "Local runtime",
      credentialChannel: "trusted_same_host",
    } as const;
    assert.strictEqual(localRuntimeTargetForAdsProbe(localRuntime), localRuntime);
    for (const status of ["auth_failed", "online_unreachable"] as const) {
      const unverifiableLocalRuntime = {
        ...localRuntime,
        reachable: status === "auth_failed",
        status,
      };
      assert.strictEqual(
        localRuntimeTargetForAdsProbe(unverifiableLocalRuntime),
        unverifiableLocalRuntime,
        `${status} local runtimes must reach the fail-closed ADS preflight.`
      );
    }
    assert.strictEqual(
      localRuntimeTargetForAdsProbe({
        ...localRuntime,
        mode: "simulate",
        reachable: false,
        status: "simulate",
      }),
      undefined,
      "A stopped simulator with a configured local endpoint must not block offline checks."
    );
    assert.strictEqual(
      localRuntimeTargetForAdsProbe({
        ...localRuntime,
        endpoint: "tcp://192.168.77.12:9901",
        credentialChannel: "untrusted_remote_plain_tcp",
      }),
      undefined
    );
  });

  test("blocks local service checks when selected-runtime ADS activity is unverifiable", async () => {
    for (const status of ["auth_failed", "online_unreachable"] as const) {
      const posts: unknown[] = [];
      let statusRequests = 0;
      const panel = {
        visible: true,
        webview: {
          postMessage: async (message: unknown) => {
            posts.push(message);
            return true;
          },
        },
      } as unknown as vscode.WebviewPanel;
      const runtime = {
        mode: "online",
        endpoint: "tcp://127.0.0.1:9901",
        endpointEnabled: true,
        reachable: status === "auth_failed",
        status,
        label: "Local runtime",
        credentialChannel: "trusted_same_host",
      } as const;
      const controller = new AdsServiceProbeController({
        panel: () => panel,
        extensionContext: () => ({} as vscode.ExtensionContext),
        runtimeTargetForOrigin: () => undefined,
        runtimeTargetOnDiscoveryComputer: () =>
          localRuntimeTargetForAdsProbe(runtime),
        requestIsCurrent: () => true,
        runtimeControlRequest: (async () => {
          statusRequests += 1;
          throw new Error("unreachable");
        }) as typeof sendRuntimeControlRequest,
      });

      await controller.probe({
        sessionId: `session-${status}`,
        requestId: 1,
        origin: "this_host",
        candidate: {
          id: "plc-a",
          label: "TwinCAT computer",
          protocol: "ads",
          source: "ads_local_router",
          confidence: "observed",
          params: {
            host: "127.0.0.1",
            ams_net_id: "1.2.3.4.1.1",
          },
        },
        ports: [851],
      });

      assert.strictEqual(
        statusRequests,
        0,
        `${status} must be rejected before attempting an unverifiable status request.`
      );
      assert.deepStrictEqual(posts, [
        {
          type: "adsServiceProbeResults",
          sessionId: `session-${status}`,
          requestId: 1,
          candidateId: "plc-a",
          results: [],
          error: UNKNOWN_ADS_CONNECTION_PROBE_SAFETY_MESSAGE,
        },
      ]);
    }
  });

  test("turns an empty ADS scan into contextual recovery instead of naked 0 found", () => {
    const draft: AdsDiscoveryDraft = {
      location: "known_address",
      host: "192.168.77.11",
      amsNetId: "",
      customPorts: "",
      advanced: false,
    };
    const snapshot: AdsDiscoveryScanSnapshot = {
      origin: "this_host",
      location: "known_address",
      host: draft.host,
      ports: [851, 852, 853, 854, 301, 501],
    };

    assert.match(adsEmptyIdentityCopy(snapshot), /192\.168\.77\.11.*UDP Identify.*AMS Net ID/i);
    assert.deepStrictEqual(applyAdsEmptyRecovery(draft, snapshot), {
      ...draft,
      advanced: true,
    });
    assert.strictEqual(
      applyAdsEmptyRecovery(
        { ...draft, location: "local_network" },
        { ...snapshot, location: "local_network", host: undefined }
      ).location,
      "known_address"
    );
    assert.strictEqual(
      discoveryProgressCopy({
        protocol: "ads",
        label: "TwinCAT @ 192.168.77.11",
        status: "done",
        count: 0,
      }),
      "No TwinCAT computer found"
    );
    assert.strictEqual(
      discoveryProgressCopy({
        protocol: "ads",
        label: "TwinCAT @ 192.168.77.11",
        status: "done",
        count: 1,
      }),
      "TwinCAT @ 192.168.77.11 … 1 computer found"
    );
    assert.match(
      discoveryProgressCopy({
        protocol: "ads",
        label: "TwinCAT",
        status: "failed",
      }),
      /failed$/
    );
    assert.strictEqual(
      discoverLabel("ads", "127.0.0.1"),
      "TwinCAT on the discovery computer"
    );
    assert.strictEqual(
      discoverLabel("ads", "192.168.77.11"),
      "TwinCAT @ 192.168.77.11"
    );
  });

  test("shows one primary Find action and reserves the wider Discover drawer", () => {
    assert.strictEqual(shouldShowScanSelected(["ads"]), false);
    assert.strictEqual(shouldShowScanSelected(["ads", "modbus_tcp"]), true);
    assert.strictEqual(activeDrawerWidth(false, false, false, true, undefined, false), 340);

    const pane = source("networkCanvas/webview/DiscoverPane.tsx");
    assert.ok(pane.includes("Scan ${selectedScanRows.length} selected type"));
    assert.match(pane, /data-role="scan-selected"[\s\S]*className="trust-button"/);
  });

  test("surfaces invalid persisted Advanced values even while collapsed", () => {
    const validation = validateAdsDiscoveryDraft({
      location: "known_address",
      host: "192.168.77.11",
      amsNetId: "100.67.6.999.1.1",
      customPorts: "9000, nope",
      advanced: false,
    });

    assert.match(validation.amsNetIdError ?? "", /six decimal numbers.*0.*255/i);
    assert.match(validation.customPortError ?? "", /whole number/i);
    const flow = source("networkCanvas/webview/AdsDiscoveryFlow.tsx");
    assert.ok(flow.includes('data-role="ads-advanced-attention"'));
    assert.ok(flow.includes("Advanced settings need attention"));
  });

  test("does not preserve an automatic choice when a recheck finds two runtimes", () => {
    const available = (port: number): AdsServiceProbeResult => ({
      port,
      status: "available",
      symbolCount: 1,
      usable: true,
    });

    assert.strictEqual(resolveSelectedAdsServicePort([available(851)]), 851);
    assert.strictEqual(
      resolveSelectedAdsServicePort([available(851), available(852)]),
      undefined,
      "A formerly automatic 851 must not survive as an implicit choice after 852 appears."
    );
    assert.strictEqual(
      resolveSelectedAdsServicePort(
        [available(851), available(852)],
        852
      ),
      852,
      "A deliberate radio selection may be preserved while it remains usable."
    );
    assert.strictEqual(
      resolveSelectedAdsServicePort([available(9000)], 9000, false),
      undefined,
      "Results from a previous port plan must never keep a removed ADS service browseable."
    );
    assert.strictEqual(
      adsServiceProbeResultsNeedRecheck(
        "851,852,853,854,301,501,9000",
        "851,852,853,854,301,501",
        "Each logical ADS service port must be a whole number."
      ),
      true,
      "Invalid edited port text must stale earlier results even when parsing falls back to the preset plan."
    );
  });

  test("every failed or deliberate repeated service check requires fresh confirmation", () => {
    const available: AdsServiceProbeResult = {
      port: 851,
      status: "available",
      symbolCount: 1,
      usable: true,
    };
    const unavailable: AdsServiceProbeResult = {
      port: 852,
      status: "unavailable",
      symbolCount: 0,
      usable: false,
    };
    const failed: AdsServiceProbeResult = {
      port: 853,
      status: "check_failed",
      symbolCount: 0,
      usable: false,
    };

    assert.strictEqual(
      shouldShowAdsServiceCheckConfirmation(
        { probing: false, completed: true, results: [available] },
        false,
        false
      ),
      false,
      "A completed usable result keeps Browse primary and offers a separate recheck action."
    );
    assert.strictEqual(
      shouldShowAdsServiceCheckConfirmation(
        { probing: false, completed: true, results: [available] },
        false,
        true
      ),
      true
    );
    for (const results of [[unavailable], [failed], []]) {
      assert.strictEqual(
        shouldShowAdsServiceCheckConfirmation(
          { probing: false, completed: true, results },
          false,
          false
        ),
        true,
        "Completed non-usable outcomes must expose an in-place confirmed retry."
      );
    }
  });

  test("a later service check cancels an earlier status preflight before ADS browse starts", async () => {
    const statusCalls: Array<{
      token: vscode.CancellationToken | undefined;
      resolve: (value: unknown) => void;
    }> = [];
    const posts: unknown[] = [];
    const panel = {
      visible: true,
      webview: {
        postMessage: async (message: unknown) => {
          posts.push(message);
          return true;
        },
      },
    } as unknown as vscode.WebviewPanel;
    const runtime = {
      mode: "online",
      endpoint: "tcp://127.0.0.1:9901",
      endpointEnabled: true,
      reachable: true,
      status: "online_reachable",
      label: "Local runtime",
      credentialChannel: "trusted_same_host",
    } as const;
    const runtimeControlRequest = (<T>(
      _endpoint: string,
      _authToken: string | undefined,
      _requestType: string,
      _params?: unknown,
      options?: { cancellationToken?: vscode.CancellationToken }
    ) =>
      new Promise<T>((resolve) => {
        statusCalls.push({
          token: options?.cancellationToken,
          resolve: resolve as (value: unknown) => void,
        });
      })) as typeof sendRuntimeControlRequest;
    const controller = new AdsServiceProbeController({
      panel: () => panel,
      extensionContext: () => ({} as vscode.ExtensionContext),
      runtimeTargetForOrigin: () => undefined,
      runtimeTargetOnDiscoveryComputer: () => runtime,
      requestIsCurrent: () => true,
      runtimeControlRequest,
    });
    const request = (requestId: number): Record<string, unknown> => ({
      sessionId: "session-a",
      requestId,
      origin: "this_host",
      candidate: {
        id: "plc-a",
        label: "TwinCAT computer",
        protocol: "ads",
        source: "ads_local_router",
        confidence: "observed",
        params: {
          host: "127.0.0.1",
          ams_net_id: "1.2.3.4.1.1",
        },
      },
      ports: [851],
    });

    const first = controller.probe(request(1));
    assert.strictEqual(statusCalls.length, 1);
    assert.ok(statusCalls[0].token, "The status preflight must be cancellable.");

    const second = controller.probe(request(2));
    assert.strictEqual(statusCalls.length, 2);
    assert.strictEqual(statusCalls[0].token?.isCancellationRequested, true);
    controller.cancel();
    assert.strictEqual(statusCalls[1].token?.isCancellationRequested, true);

    const stoppedReport = {
      overall: "disabled",
      summary: "ADS is not configured.",
      connections: [],
    };
    statusCalls[0].resolve(stoppedReport);
    statusCalls[1].resolve(stoppedReport);
    await Promise.all([first, second]);
    assert.deepStrictEqual(
      posts,
      [],
      "Canceled preflights must not progress into a service probe or publish stale results."
    );
  });

  test("checks cancellation before every logical ADS service", async () => {
    const calls: number[] = [];
    let active = true;

    const results = await probeAdsServicesSequentially(
      [851, 852, 853],
      async (port) => {
        calls.push(port);
        active = false;
        return response("empty");
      },
      { isActive: () => active }
    );

    assert.deepStrictEqual(calls, [851]);
    assert.deepStrictEqual(results.map((result) => result.port), [851]);
  });

  test("cancellation terminates the local ADS command instead of leaving network I/O running", async function () {
    this.timeout(5_000);
    const markerRoot = fs.mkdtempSync(
      path.join(os.tmpdir(), "trust-ads-cancel-")
    );
    const marker = path.join(markerRoot, "still-running.txt");
    const cancellation = new vscode.CancellationTokenSource();
    try {
      const started = Date.now();
      const pending = runJsonCommand<unknown>(
        process.execPath,
        [
          "-e",
          `setTimeout(() => require("fs").writeFileSync(${JSON.stringify(marker)}, "late"), 1000); setTimeout(() => process.stdout.write("{}"), 4000);`,
        ],
        undefined,
        cancellation.token
      );
      setTimeout(() => cancellation.cancel(), 100);
      const result = await pending;

      assert.strictEqual(result.ok, false);
      assert.strictEqual(result.message, "Command cancelled.");
      assert.ok(Date.now() - started < 2_000, "Cancellation must be prompt.");
      await new Promise((resolve) => setTimeout(resolve, 1_100));
      assert.strictEqual(
        fs.existsSync(marker),
        false,
        "The canceled process must not continue its delayed work."
      );
    } finally {
      cancellation.dispose();
      fs.rmSync(markerRoot, { recursive: true, force: true });
    }
  });

  test("reports each port before its network request", async () => {
    const events: string[] = [];
    await probeAdsServicesSequentially(
      [851, 852],
      async (port) => {
        events.push(`probe:${port}`);
        return response("empty");
      },
      {
        onBeforeProbe: (port) => {
          events.push(`progress:${port}`);
        },
      }
    );

    assert.deepStrictEqual(events, [
      "progress:851",
      "probe:851",
      "progress:852",
      "probe:852",
    ]);
  });

  test("continues ordinary service outcomes but stops on infrastructure failure", async () => {
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

    assert.deepStrictEqual(calls, [851, 852, 853, 854]);
    assert.deepStrictEqual(
      results.map((result) => result.status),
      ["unavailable", "unsupported", "empty", "check_failed"] satisfies AdsServiceProbeStatus[]
    );
  });

  test("terminal results reject late discovery and probe progress", () => {
    const initial: DiscoverySessionState = {
      scanning: true,
      progress: [],
      results: [],
      sessionCurrent: true,
      terminal: false,
      adsServiceProbes: {},
    };
    const terminal = reduceDiscoverySessionState(initial, {
      type: "results",
      candidates: [],
    });
    assert.strictEqual(
      reduceDiscoverySessionState(terminal, {
        type: "progress",
        row: { protocol: "ads", label: "TwinCAT", status: "scanning" },
      }),
      terminal
    );

    const probing = reduceDiscoverySessionState(terminal, {
      type: "adsProbeStarted",
      candidateId: "plc-a",
    });
    const completed = reduceDiscoverySessionState(probing, {
      type: "adsProbeResults",
      candidateId: "plc-a",
      results: [],
    });
    assert.strictEqual(
      reduceDiscoverySessionState(completed, {
        type: "adsProbeProgress",
        candidateId: "plc-a",
        port: 852,
      }),
      completed
    );
  });

  test("keeps UDP Identify no-reply in the error state while enabling manual identity recovery", () => {
    const runtimeError = discoveryTypedFailureMessage(
      "ads_udp_identify_blocked"
    );
    const failed = reduceDiscoverySessionState(
      {
        scanning: true,
        progress: [
          {
            protocol: "ads",
            label: "TwinCAT @ 192.168.77.11",
            status: "scanning",
          },
        ],
        results: [],
        sessionCurrent: true,
        terminal: false,
        adsServiceProbes: {},
      },
      {
        type: "results",
        candidates: [],
        error: runtimeError,
        errorCode: "ads_udp_identify_blocked",
      }
    );

    assert.strictEqual(failed.scanning, false);
    assert.strictEqual(failed.terminal, true);
    assert.strictEqual(failed.error, runtimeError);
    assert.strictEqual(failed.errorCode, "ads_udp_identify_blocked");
    assert.strictEqual(failed.progress[0]?.status, "failed");
    assert.deepStrictEqual(failed.results, []);
    assert.strictEqual(
      runtimeError,
      "TwinCAT identity did not answer UDP discovery. Enter the target AMS Net ID to continue manually."
    );
    assert.ok(!runtimeError.includes("UdpIdentifyBlocked"));
    assert.strictEqual(
      offersAdsManualIdentityRecovery(failed.errorCode),
      true
    );

    const pane = source("networkCanvas/webview/DiscoverPane.tsx");
    const app = source("networkCanvas/webview/NetworkCanvasApp.tsx");
    assert.ok(pane.includes('data-state={adsIdentityRecoveryError ? "error" : "empty"}'));
    assert.ok(pane.includes("showAdsIdentityRecovery && adsScanSnapshot.current"));
    assert.ok(app.includes("errorCode={discoverErrorCode}"));
  });

  test("compare-and-release keeps a newer remote Browse lease alive", () => {
    const leases = new DiscoveryBrowseLeaseStore();
    leases.begin("runtime:a", "lease-a", "webview-1");
    assert.strictEqual(
      leases.bindAndValidate("runtime:a", "lease-a", "webview-1", "browse-a"),
      true
    );
    leases.begin("runtime:b", "lease-b", "webview-1");
    assert.strictEqual(
      leases.bindAndValidate("runtime:b", "lease-b", "webview-1", "browse-b"),
      true
    );
    assert.strictEqual(leases.release("runtime:a", "lease-a", "browse-a"), false);
    assert.strictEqual(leases.current()?.originId, "runtime:b");
    assert.strictEqual(leases.release("runtime:b", "lease-b", "browse-b"), true);
  });

  test("only a host-registered discovery endpoint can be resolved", () => {
    const endpoints = new Map([["runtime:a", "tcp://runtime-a:9901"]]);
    assert.strictEqual(
      resolveRegisteredDiscoveryOriginEndpoint(
        endpoints,
        "runtime:a",
        "tcp://runtime-a:9901"
      ),
      "tcp://runtime-a:9901"
    );
    assert.strictEqual(
      resolveRegisteredDiscoveryOriginEndpoint(
        endpoints,
        "runtime:a",
        "tcp://attacker:9901"
      ),
      undefined
    );
    assert.strictEqual(
      resolveRegisteredDiscoveryOriginEndpoint(endpoints, "runtime:missing"),
      undefined
    );
  });

  test("ADS discovery hands off before Browse and releases only its scoped lease", () => {
    const discover = source("networkCanvas/webview/useDiscoverPane.ts");
    const session = source("networkCanvas/webview/useDiscoverySession.ts");
    const browse = source("networkCanvas/webview/useBrowseSession.ts");
    const host = source("networkCanvas/networkCanvasPanel.ts");
    const originContext = source("networkCanvas/discoveryOriginContext.ts");
    const actions = source("networkCanvas/protocolActions.ts");

    assert.ok(
      discover.indexOf("handoffToBrowse(candidate)") <
        discover.indexOf("openBrowse(")
    );
    assert.ok(discover.includes('candidate.protocol === "ads"'));
    assert.ok(session.includes('type: "handoffDiscoveryToBrowse"'));
    assert.ok(session.includes("discovery_origin_lease_id: leaseId"));
    assert.ok(browse.includes('type: "releaseDiscoveryOrigin"'));
    assert.ok(browse.includes("closePanel(!discoveryOriginConsumedByAdd)"));
    assert.ok(host.includes("discoveryOriginContext.releaseBrowse("));
    assert.ok(originContext.includes("resolveRegisteredDiscoveryOriginEndpoint("));
    assert.ok(actions.includes("discovery_origin_lease_id"));
    assert.ok(actions.includes("Remote discovery is read-only in this release"));
  });

  test("ADS Browse uses variables vocabulary, one route recovery, and visible fresh results", () => {
    const actions = source("networkCanvas/webview/browseActions.ts");
    const browse = source("networkCanvas/webview/BrowseTagsPanel.tsx");
    const controls = source("networkCanvas/webview/AdsBrowseTargetControls.tsx");
    const flow = source("networkCanvas/webview/AdsDiscoveryFlow.tsx");

    assert.ok(actions.includes('label: "Browse variables"'));
    assert.ok(actions.includes('actionLabel: "Add variables"'));
    assert.ok(browse.includes('isAds ? "Browse variables" : "Browse tags"'));
    assert.ok(browse.includes('isAds ? "Search variables" : "Search symbols"'));
    assert.ok(browse.includes("lastAutoExpandedTreeRef"));
    assert.ok(browse.includes("setExpanded((previous)"));
    assert.ok(browse.includes("setSelected(new Set())"));
    assert.ok(browse.includes("setAllowWritesChecked(false)"));
    assert.ok(controls.includes('"Browse variables"'));
    assert.ok(flow.includes("Set up the route, then check and browse variables."));
    assert.ok(flow.includes("!routeMissing && ("));
    assert.ok(flow.includes("Additional task 1"));
    assert.ok(flow.includes("NC SAF service"));
  });

  test("keeps dedicated discovery, updated-port, progress, and recovery UI states", () => {
    const pane = source("networkCanvas/webview/DiscoverPane.tsx");
    const flow = source("networkCanvas/webview/AdsDiscoveryFlow.tsx");
    const panel = source("networkCanvas/networkCanvasPanel.ts");

    assert.ok(flow.includes('data-role="ads-find-twincat"'));
    assert.ok(pane.includes('startScan([adsRow], "ads")'));
    assert.ok(pane.includes('data-role="scan-selected"'));
    assert.ok(flow.includes('data-role="ads-probe-safety"'));
    assert.ok(flow.includes('data-role="ads-probe-safety-confirmation"'));
    assert.ok(flow.includes('data-role="ads-check-services"'));
    assert.ok(flow.includes("const servicePortPlanKey = servicePorts.join"));
    assert.match(
      flow,
      /setConnectionSafetyConfirmed\(false\);\r?\n\s*}, \[servicePortPlanKey\]\);/
    );
    assert.ok(!pane.includes("requestedAdsProbeIds"));
    assert.ok(pane.includes("const clearStaleIdentityResults"));
    assert.ok(pane.includes("const identityChanged"));
    assert.ok(pane.includes("onReset();"));
    assert.ok(
      pane.indexOf("clearStaleIdentityResults();") <
        pane.indexOf("setOrigin(event.target.value)")
    );
    assert.ok(flow.includes('data-state={serviceResultsStale ? "ports-changed" : "ready"}'));
    assert.ok(flow.includes('data-role="ads-results-stale"'));
    assert.ok(flow.includes('data-role="ads-recheck-services"'));
    assert.ok(flow.includes("!resultsAreCurrent || Boolean(disabledReason)"));
    assert.ok(
      flow.includes(
        "ADS service settings changed. Check the updated services before browsing variables."
      )
    );
    assert.ok(pane.includes("adsServiceProbeResultsNeedRecheck("));
    assert.ok(pane.includes("sessionDisabledReason ?? adsCustomPortError"));
    assert.ok(flow.includes('candidate.source === "ads_local_router"'));
    assert.ok(flow.includes('" · On the discovery computer"'));
    assert.ok(!flow.includes('data-role="ads-check-updated-ports"'));
    assert.ok(pane.includes("onProbeAdsServices(c, currentPorts, snapshot.origin)"));
    assert.ok(flow.includes('data-role="ads-probe-progress"'));
    assert.ok(flow.includes('data-role="ads-route-setup"'));
    assert.ok(flow.includes('data-role="ads-runtime-choice-required"'));
    assert.ok(flow.includes('data-role="ads-validation-error"'));
    assert.ok(flow.includes("Available —"));
    assert.match(
      flow,
      /probe\?\.error \|\| terminalFailure[\s\S]*\? "check-failed"/,
      "A terminal structured probe failure must set the card state to check-failed."
    );

    const cancelCase = panel.slice(
      panel.indexOf('case "cancelDiscover"'),
      panel.indexOf('case "browseSymbols"')
    );
    assert.ok(
      cancelCase.includes("clearDiscoveryOriginContext()"),
      "Explicit cancel must release the pinned origin target and credentials."
    );
  });
});
