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
import {
  START_RUNTIME_ACTION,
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
    assert.match(prompt.detail ?? "", /Start the runtime, then import again/i);
    assert.deepStrictEqual(prompt.actions, [START_RUNTIME_ACTION]);

    const source = readSrc("networkCanvas/protocolActions.ts");
    assert.ok(source.includes("adsImportFailurePrompt(report.message)"));
    assert.ok(source.includes("this.dependencies.startRuntime()"));
    const panelSource = readSrc("networkCanvas/networkCanvasPanel.ts");
    assert.ok(panelSource.includes("startRuntime: startConfiguredRuntime"));
    assert.ok(panelSource.includes("runtimeLifecycleService.startRuntime()"));
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

  test("#97 visual browse flow exposes an inline ADS port field", () => {
    const source = readSrc("networkCanvas/webview/DiscoverPane.tsx");

    assert.ok(source.includes('data-role="ads-port"'));
    assert.ok(source.includes("min={1}") && source.includes("max={65535}"));
    assert.ok(source.includes("withCandidateAdsPort"));

    const browseControls = readSrc(
      "networkCanvas/webview/AdsBrowseTargetControls.tsx",
    );
    assert.ok(browseControls.includes('data-role="ads-browse-port"'));
    assert.ok(browseControls.includes('data-role="browse-ads-symbols"'));
    assert.ok(
      browseControls.includes(
        "Each ADS port is a separate server and symbol namespace",
      ),
    );
    const browsePanel = readSrc("networkCanvas/webview/BrowseTagsPanel.tsx");
    assert.ok(browsePanel.includes("adsPortDraftStale"));
    assert.ok(
      browsePanel.includes(
        "Browse the edited ADS port before adding tags from that server.",
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
