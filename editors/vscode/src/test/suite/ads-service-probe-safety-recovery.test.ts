import {
  assert,
  vscode,
  DiscoveryBrowseLeaseStore,
  resolveRegisteredDiscoveryOriginEndpoint,
  discoveryTypedFailureMessage,
  offersAdsManualIdentityRecovery,
  adsServiceProbeResultsNeedRecheck,
  discoveryOriginForMode,
  reduceDiscoverySessionState,
  source,
} from "./ads-service-probe-safety-fixtures";

suite("ADS service probe safety", () => {
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
            label: "TwinCAT @ 192.168.50.42",
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
      "No ADS device answered. Make sure it is running and that your firewall allows truST on this network. Try again, or use Advanced if you know its address."
    );
    assert.doesNotMatch(runtimeError, /UdpIdentifyBlocked|UDP|router|10060|10054|ads-wire/i);
    assert.doesNotMatch(runtimeError, /private networks/i);
    assert.strictEqual(
      offersAdsManualIdentityRecovery(failed.errorCode),
      true
    );

    const pane = source("networkCanvas/webview/DiscoverPane.tsx");
    const app = source("networkCanvas/webview/NetworkCanvasApp.tsx");
    assert.ok(pane.includes('data-state={adsIdentityRecoveryError ? "error" : "empty"}'));
    assert.ok(pane.includes("showAdsIdentityRecovery && adsScanSnapshot.current"));
    assert.ok(pane.includes("error && !adsIdentityRecoveryError"));
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
    const confirmedStart = controls.indexOf("if (confirmedByDiscovery)");
    const confirmedBrowse = controls.slice(
      confirmedStart,
      controls.indexOf("\n  return (", confirmedStart)
    );
    assert.ok(
      confirmedBrowse.includes("Selected ADS service") &&
        !confirmedBrowse.includes("AMS Net ID"),
      "normal Discover to Browse must show the selected ADS service without exposing the recovery-only AMS identity"
    );
    assert.ok(
      flow.includes(
        "This remote ADS device needs a route before its services can be checked."
      )
    );
    assert.ok(flow.includes("!routeMissing && usableCount > 0 && ("));
    assert.ok(flow.includes("Automatic checks include ADS 851–854, 301, and 501"));
    assert.ok(!flow.includes("Additional task 1"));
    assert.ok(!flow.includes("NC SAF service"));
    assert.ok(browse.includes('className="trust-inspector"'));
    for (const themedSource of [browse, controls]) {
      assert.ok(
        !themedSource.includes("--vscode-") &&
          !/#[0-9a-fA-F]{3,8}|rgba\(/.test(themedSource),
        "ADS discovery click-through must use the shared truST theme instead of a second raw VS Code token system"
      );
    }
  });
  test("keeps dedicated discovery, updated-port, progress, and recovery UI states", () => {
    const pane = source("networkCanvas/webview/DiscoverPane.tsx");
    const flow = source("networkCanvas/webview/AdsDiscoveryFlow.tsx");
    const panel = source("networkCanvas/networkCanvasPanel.ts");

    assert.ok(flow.includes('data-role="ads-discover"'));
    assert.ok(pane.includes('startScan([ADS], "ads")'));
    assert.ok(pane.includes('data-role="scan-selected"'));
    assert.ok(!flow.includes('data-role="ads-probe-safety"'));
    assert.ok(!flow.includes('data-role="ads-probe-safety-confirmation"'));
    assert.ok(!flow.includes('data-role="ads-check-services"'));
    assert.ok(pane.includes("autoAdsProbeCandidates"));
    assert.ok(!pane.includes("requestedAdsProbeIds"));
    assert.ok(pane.includes("const clearStaleIdentityResults"));
    assert.ok(pane.includes("const identityChanged"));
    assert.ok(pane.includes("onReset();"));
    assert.ok(pane.includes("discoveryOriginForMode(mode, hardwareOrigin)"));
    assert.ok(!pane.includes('data-role="ads-scan-origin"'));
    assert.ok(flow.includes('data-role="ads-results-stale"'));
    assert.ok(flow.includes('data-role="ads-recheck-services"'));
    assert.ok(flow.includes("!resultsAreCurrent || Boolean(disabledReason)"));
    assert.ok(
      flow.includes(
        "ADS service settings changed. Check the updated services before browsing variables."
      )
    );
    assert.ok(pane.includes("adsServiceProbeResultsNeedRecheck("));
    assert.ok(pane.includes("sessionDisabledReason ??") && pane.includes("adsCustomPortError ??"));
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
