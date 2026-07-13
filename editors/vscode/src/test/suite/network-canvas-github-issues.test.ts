import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";

import {
  DEFAULT_ADS_PORT,
  adsPortDraftIsStale,
  adsTargetNetId,
  parseAdsPortInput,
  withAdsTargetPort,
  withCandidateAdsPort,
} from "../../networkCanvas/webview/adsTargetPort";
import {
  DiscoveryRequestTracker,
  candidateDisabledReason,
  isActiveWebviewSession,
} from "../../networkCanvas/discoverySession";
import { discoveryRuntimeFailureMessage } from "../../networkCanvas/discoveryErrors";
import { LatestRefreshCoordinator } from "../../networkCanvas/refreshCoordinator";
import { becameVisible } from "../../networkCanvas/panelVisibility";
import { NetworkCanvasPolling } from "../../networkCanvas/panelPolling";
import {
  immediateSimulatorLifecycleProjection,
  shouldRefreshNetworkCanvasForLifecycleChange,
} from "../../networkCanvas/lifecycleRefreshPolicy";
import {
  OPEN_RUN_ACTION,
  adsImportFailurePrompt,
} from "../../networkCanvas/adsImportUx";
import { classifyBrowseError } from "../../networkCanvas/webview/browseErrorModel";
import { adsConnectionIdentityParts } from "../../networkCanvas/webview/adsConnectionSummary";
import {
  buildOfflineAdsImportArgs,
  buildOfflineBrowseSymbolsArgs,
  classifyAdsBrowseCommandFailure,
} from "../../networkCanvas/adsBrowseContract";

function readSrc(relativePath: string): string {
  return fs.readFileSync(
    path.join(__dirname, "..", "..", "..", "src", relativePath),
    "utf8",
  );
}

suite("Network Canvas GitHub issues #94-#97", function () {
  test("#94 symbol selection preserves native pointer and keyboard activation", () => {
    const source = readSrc("networkCanvas/webview/SymbolSelectionCheckbox.tsx");

    assert.ok(
      source.includes('type="checkbox"'),
      "selection must remain a native checkbox",
    );
    assert.ok(
      source.includes("onChange"),
      "native change must own the controlled state update",
    );
    assert.ok(
      !source.includes("onPointer"),
      "pointer overrides can double-toggle native input",
    );
    assert.ok(
      !source.includes("onKeyDown"),
      "Space must retain native checkbox semantics",
    );
    assert.ok(
      !source.includes("onClick"),
      "native click must not be reimplemented",
    );
    assert.ok(
      source.includes("aria-label"),
      "each symbol checkbox needs an accessible name",
    );
  });

  test("#95 refresh coordinator is single-flight and latest-wins", async () => {
    const coordinator = new LatestRefreshCoordinator();
    let releaseFirst: (() => void) | undefined;
    const firstGate = new Promise<void>((resolve) => {
      releaseFirst = resolve;
    });
    let active = 0;
    let maxActive = 0;
    const committed: number[] = [];

    const first = coordinator.request(async (context) => {
      active += 1;
      maxActive = Math.max(maxActive, active);
      await firstGate;
      if (context.isCurrent()) {
        committed.push(1);
      }
      active -= 1;
    });
    const second = coordinator.request(async (context) => {
      active += 1;
      maxActive = Math.max(maxActive, active);
      if (context.isCurrent()) {
        committed.push(2);
      }
      active -= 1;
    });
    const third = coordinator.request(async (context) => {
      active += 1;
      maxActive = Math.max(maxActive, active);
      if (context.isCurrent()) {
        committed.push(3);
      }
      active -= 1;
    });

    releaseFirst?.();
    await Promise.all([first, second, third]);

    assert.strictEqual(maxActive, 1, "refresh work must never overlap");
    assert.deepStrictEqual(
      committed,
      [3],
      "only the newest queued snapshot may commit",
    );
  });

  test("#95 refresh requested at drain completion is not stranded", async () => {
    const coordinator = new LatestRefreshCoordinator();
    let release!: () => void;
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    const ran: string[] = [];

    void coordinator.request(async () => {
      await gate;
      ran.push("first");
    });
    // Register after request(): this lands a request between the old drain
    // promise resolving and its external finalizer clearing the running marker.
    void gate.then(() =>
      queueMicrotask(() => {
        void coordinator.request(async () => {
          ran.push("second");
        });
      })
    );

    release();
    await new Promise<void>((resolve) => setTimeout(resolve, 0));

    assert.deepStrictEqual(ran, ["first", "second"]);
  });

  test("#95 periodic polling waits for a slow refresh before starting another", async function () {
    this.timeout(2_000);
    let releaseFirst!: () => void;
    const firstGate = new Promise<void>((resolve) => {
      releaseFirst = resolve;
    });
    let firstStarted!: () => void;
    const firstStartedGate = new Promise<void>((resolve) => {
      firstStarted = resolve;
    });
    let calls = 0;
    let active = 0;
    let maxActive = 0;
    let secondStarted!: () => void;
    const secondStartedGate = new Promise<void>((resolve) => {
      secondStarted = resolve;
    });
    const polling = new NetworkCanvasPolling(async () => {
      calls += 1;
      active += 1;
      maxActive = Math.max(maxActive, active);
      if (calls === 1) {
        firstStarted();
        await firstGate;
      } else if (calls === 2) {
        secondStarted();
      }
      active -= 1;
    }, 10);

    try {
      polling.start();
      await firstStartedGate;
      await new Promise<void>((resolve) => setTimeout(resolve, 50));

      assert.strictEqual(
        calls,
        1,
        "timer ticks during a slow refresh must be coalesced",
      );
      assert.strictEqual(maxActive, 1, "periodic refreshes must never overlap");

      releaseFirst();
      await secondStartedGate;
      assert.strictEqual(maxActive, 1, "the next poll starts only after completion");
    } finally {
      polling.stop();
      releaseFirst();
    }
  });

  test("#95 per-scan I/O events cannot starve lifecycle graph refreshes", () => {
    const changes = [
      { kind: "lifecycle" as const },
      ...Array.from({ length: 1_000 }, () => ({ kind: "io" as const })),
      { kind: "lifecycle" as const },
    ];
    const scheduled = changes.filter(
      shouldRefreshNetworkCanvasForLifecycleChange
    );

    assert.deepStrictEqual(scheduled, [
      { kind: "lifecycle" },
      { kind: "lifecycle" },
    ]);
    assert.deepStrictEqual(immediateSimulatorLifecycleProjection("stopped"), {
      running: false,
      starting: false,
      stopped: true,
    });
    assert.deepStrictEqual(immediateSimulatorLifecycleProjection("starting"), {
      running: false,
      starting: true,
      stopped: false,
    });
    assert.deepStrictEqual(immediateSimulatorLifecycleProjection("running"), {
      running: true,
      starting: false,
      stopped: false,
    });
    assert.strictEqual(immediateSimulatorLifecycleProjection("connected"), undefined);
    const panelSource = readSrc("networkCanvas/networkCanvasPanel.ts");
    const lifecycleModelSource = readSrc("networkCanvas/lifecycleModel.ts");
    assert.ok(
      panelSource.includes("shouldRefreshNetworkCanvasForLifecycleChange(change)"),
      "Devices must apply the I/O flood policy at its lifecycle subscription"
    );
    assert.ok(
      panelSource.indexOf("postImmediateSimulatorLifecycleGraph(phase)") <
        panelSource.indexOf("void refreshNetworkCanvasPanel();"),
      "Starting/Running/Stopped must reach the visible graph before slow schema/topology refresh work"
    );
    assert.ok(
      panelSource.includes("runtimeLifecycleService.localFailure()") &&
        lifecycleModelSource.includes(
          "failure: asNetworkFailure(immediateFailure)"
        ),
      "failed starts must project the local lifecycle error immediately instead of flashing Stopped"
    );
  });

  test("#95 invalidated discovery requests cannot publish stale cards", () => {
    const tracker = new DiscoveryRequestTracker<object>();
    const owner = {};
    const scanA = tracker.start(owner);
    const scanB = tracker.start(owner);

    assert.strictEqual(tracker.isCurrent(scanA, owner), false);
    assert.strictEqual(tracker.isCurrent(scanB, owner), true);

    tracker.invalidate();
    assert.strictEqual(tracker.isCurrent(scanB, owner), false);
  });

  test("#95 stale or unsupported discovery cards are disabled with a reason", () => {
    assert.strictEqual(
      candidateDisabledReason("ads", new Set(["ads"]), true),
      undefined,
    );
    assert.match(
      candidateDisabledReason("ads", new Set(["ads"]), false) ?? "",
      /scan again/i,
    );
    assert.match(
      candidateDisabledReason("ads", new Set(["mqtt"]), true) ?? "",
      /no longer available/i,
    );
    assert.match(
      candidateDisabledReason("ads", new Set(["ads"]), true, false) ?? "",
      /runtime.*no longer available.*scan again/i,
    );
    assert.match(
      discoveryRuntimeFailureMessage("ads", new Error("request timed out")),
      /ADS discovery timed out.*reconnect.*scan again/i,
    );
    assert.match(
      discoveryRuntimeFailureMessage("ads", new Error("authentication failed")),
      /rejected authentication.*auth token/i,
    );
  });

  test("#95 panel visibility recovery refreshes instead of polling a suspended webview", () => {
    const source = readSrc("networkCanvas/networkCanvasPanel.ts");

    assert.ok(source.includes("onDidChangeViewState"));
    assert.strictEqual(becameVisible(true, true), false);
    assert.strictEqual(becameVisible(true, false), false);
    assert.strictEqual(becameVisible(false, true), true);
    assert.strictEqual(isActiveWebviewSession("new", "new"), true);
    assert.strictEqual(isActiveWebviewSession("old", "new"), false);
    assert.strictEqual(isActiveWebviewSession("old", undefined), false);
    assert.ok(source.includes("panelBecameVisible"));
    assert.ok(source.includes("webviewPanel.visible"));
    assert.ok(source.includes("panelRef.visible"));
    assert.ok(source.includes("refreshCoordinator.invalidate()"));
    assert.ok(source.includes("isActiveWebviewSession"));
  });

  test("#96 write-enabled offline refusal is complete and actionable", () => {
    const prompt = adsImportFailurePrompt(
      "Write-enabled ADS imports need a running runtime so truST can apply the explicit write acknowledgement.",
    );

    assert.strictEqual(
      prompt.modal,
      true,
      "the full guardrail reason must not be toast-truncated",
    );
    assert.match(prompt.message, /running runtime/i);
    assert.strictEqual(
      prompt.detail,
      "Open the truST sidebar and start the selected target, then import again."
    );
    assert.deepStrictEqual(prompt.actions, [OPEN_RUN_ACTION]);

    const source = readSrc("networkCanvas/protocolActions.ts");
    assert.ok(source.includes("adsImportFailurePrompt(report.message)"));
    assert.ok(
      source.includes('vscode.commands.executeCommand("trust.home.focus")') &&
        !source.includes("this.dependencies.startRuntime()"),
      "ADS import recovery must reveal the one Run surface without launching a hidden lifecycle action"
    );
    const panelSource = readSrc("networkCanvas/networkCanvasPanel.ts");
    const protocolActionWiring = panelSource.slice(
      panelSource.indexOf("const protocolActions ="),
      panelSource.indexOf("const adsServiceProbeController")
    );
    assert.ok(!protocolActionWiring.includes("startRuntime:"));

    const generic = adsImportFailurePrompt(
      "control request failed: os error 10060; live ADS import needs an ads-wire runtime build",
    );
    assert.strictEqual(generic.message, "Could not add ADS variables.");
    assert.strictEqual(
      generic.detail,
      "The selected runtime could not complete the ADS import. Reconnect or update it, then try again.",
    );
    assert.doesNotMatch(
      `${generic.message} ${generic.detail}`,
      /10060|ads-wire|runtime build/i,
    );
    assert.ok(
      !source.includes("live ADS import needs an ads-wire runtime build"),
      "the notification must not append backend build-feature jargon",
    );
  });

  test("#97 ADS ports validate and round-trip on discovered candidates", () => {
    assert.deepStrictEqual(parseAdsPortInput(""), { port: DEFAULT_ADS_PORT });
    assert.deepStrictEqual(parseAdsPortInput("301"), { port: 301 });
    assert.deepStrictEqual(parseAdsPortInput("501"), { port: 501 });
    assert.strictEqual(
      adsPortDraftIsStale("501", { ams_port: 301 }),
      true,
      "editing a successful 301 browse to 501 must invalidate the old tree",
    );
    assert.strictEqual(adsPortDraftIsStale("301", { ams_port: 301 }), false);
    assert.ok(parseAdsPortInput("0").error);
    assert.ok(parseAdsPortInput("65536").error);
    assert.ok(parseAdsPortInput("301.5").error);
    assert.ok(parseAdsPortInput("motion").error);

    const original = {
      id: "ads:5.23.91.12.1.1",
      label: "TwinCAT",
      source: "ads_broadcast",
      confidence: "observed",
      protocol: "ads",
      params: {
        host: "192.168.10.5",
        ams_net_id: "5.23.91.12.1.1",
        ams_port: 851,
      },
    };
    const updated = withCandidateAdsPort(original, 301);
    const motionTarget = withAdsTargetPort(original.params, 501);

    assert.strictEqual(updated.params.ams_port, 301);
    assert.strictEqual(motionTarget.ams_port, 501);
    assert.strictEqual(adsTargetNetId(motionTarget), "5.23.91.12.1.1");
    assert.strictEqual(
      original.params.ams_port,
      851,
      "port editing must not mutate scan evidence",
    );
    assert.deepStrictEqual(
      adsConnectionIdentityParts({
        ams_net_id: "5.23.91.12.1.1",
        ams_port: 301,
      }),
      ["AMS Net ID 5.23.91.12.1.1", "ADS port 301"],
      "reopening a saved ADS connection must display its selected port",
    );
  });

  test("#97 visual browse flow discovers PLC runtimes before advanced port editing", () => {
    const source = [
      readSrc("networkCanvas/webview/DiscoverPane.tsx"),
      readSrc("networkCanvas/webview/AdsDiscoveryFlow.tsx"),
      readSrc("networkCanvas/adsServiceProbeModel.ts"),
    ].join("\n");

    assert.ok(source.includes('data-role="ads-custom-ports"'));
    assert.ok(source.includes("AUTOMATIC_ADS_SERVICE_PORTS"));
    assert.ok(source.includes("MAX_ADS_SERVICE_PROBES"));
    assert.ok(source.includes('data-role="ads-browse-variables"'));
    assert.ok(
      !readSrc("networkCanvas/webview/DiscoverPane.tsx").includes(
        'data-role="ads-port"',
      ),
      "Discover must not present a raw ADS port before finding the TwinCAT computer",
    );

    const browseControls = readSrc(
      "networkCanvas/webview/AdsBrowseTargetControls.tsx",
    );
    assert.ok(browseControls.includes('data-role="ads-browse-port"'));
    assert.ok(browseControls.includes('data-role="browse-ads-symbols"'));
    assert.ok(
      browseControls.includes(
        "Each ADS service port exposes a separate variable namespace",
      ),
    );
    const browsePanel = readSrc("networkCanvas/webview/BrowseTagsPanel.tsx");
    assert.ok(browsePanel.includes("adsPortDraftStale"));
    assert.ok(
      browsePanel.includes(
        "Browse the edited ADS service before adding variables from it.",
      ),
    );
    assert.ok(
      browsePanel.includes("routeMissing && !adsPortDraftStale"),
      "an old route result must be hidden after the port draft changes",
    );
    assert.ok(
      browsePanel.includes("error && !adsPortDraftStale"),
      "an old browse error must be hidden after the port draft changes",
    );

    assert.strictEqual(
      classifyBrowseError("ads", { code: "ads_port_unavailable" }).title,
      "ADS port unavailable",
    );
    assert.strictEqual(
      classifyBrowseError("ads", { code: "symbol_upload_unsupported" }).title,
      "Symbol Upload unsupported",
    );
    assert.strictEqual(
      classifyBrowseError("ads", { code: "empty_symbol_table" }).title,
      "No compatible symbols",
    );
    const rawBrowseFailure = classifyBrowseError("ads", {
      code: "symbol_upload_failed",
      message:
        "receiving reply timed out (os error 10060); local router closed the connection (os error 10054); ads-wire feature missing",
    });
    assert.strictEqual(
      rawBrowseFailure.detail,
      "The selected ADS service could not return variables. Make sure it is running, then try again.",
    );
    assert.doesNotMatch(
      rawBrowseFailure.detail,
      /10060|10054|router|ads-wire/i,
    );
    assert.match(rawBrowseFailure.technicalDetail ?? "", /10060.*ads-wire/i);
    assert.strictEqual(
      classifyAdsBrowseCommandFailure("connect ADS target: Connection refused"),
      "ads_port_unavailable",
    );
    assert.strictEqual(
      classifyAdsBrowseCommandFailure("Service is not supported by server"),
      "symbol_upload_unsupported",
    );
    for (const message of [
      "Invalid AMS port",
      "Port disabled",
      "Port not connected",
      "ADS port not opened",
      "Router: port not registered",
      "Router: port is invalid",
      "Router: port removed",
    ]) {
      assert.strictEqual(
        classifyAdsBrowseCommandFailure(message),
        "ads_port_unavailable",
      );
    }
    for (const message of ["Unknown command ID", "Unknown AMS command"]) {
      assert.strictEqual(
        classifyAdsBrowseCommandFailure(message),
        "symbol_upload_unsupported",
      );
    }
    assert.strictEqual(
      classifyAdsBrowseCommandFailure("No more symbols in cache"),
      "empty_symbol_table",
    );

    const target = {
      host: "192.168.10.5",
      ams_net_id: "5.23.91.12.1.1",
      ams_port: 301,
    };
    const browseArgs = buildOfflineBrowseSymbolsArgs("ads", target, "symbols");
    assert.deepStrictEqual(
      JSON.parse(browseArgs[browseArgs.indexOf("--target") + 1]),
      target,
      "browse CLI target must preserve a non-851 ADS port",
    );
    const importArgs = buildOfflineAdsImportArgs(
      "/tmp/project",
      target,
      "io_server",
      ["GVL.Input"],
    );
    assert.strictEqual(
      importArgs[importArgs.indexOf("--ams-port") + 1],
      "301",
      "generated ads.toml import must receive the selected ADS port",
    );
  });
});
