import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";

import {
  normalizeAdsLiveValuesState,
} from "../../adsLiveValuesModel";
import {
  __testLiveValuesUnavailableMessage,
  __testStatusForSelectedTarget,
} from "../../ioPanel";
import type { RuntimeStatusPayload } from "../../io-panel/types";

function extensionRoot(): string {
  return path.resolve(__dirname, "../../..");
}

function source(relativePath: string): string {
  return fs.readFileSync(path.join(extensionRoot(), "src", relativePath), "utf8");
}

function stoppedStatus(): RuntimeStatusPayload {
  return {
    running: false,
    inlineValuesEnabled: true,
    runtimeMode: "simulate",
    runtimeState: "stopped",
    endpoint: "",
    endpointConfigured: false,
    endpointEnabled: true,
    endpointReachable: false,
  };
}

suite("ADS Live Values", () => {
  test("normalizes the versioned read-only ADS snapshot without inventing addresses", () => {
    const state = normalizeAdsLiveValuesState({
      schemaVersion: 1,
      scan: 42,
      entries: [
        {
          connection: " line_1 ",
          name: " ads_line_1_main_speed ",
          remoteSymbol: " MAIN.speed ",
          value: "Real(12.5)",
          valueType: "real",
          access: "read",
          quality: { state: "good", lastUpdateMs: 18 },
        },
        {
          connection: "line_1",
          name: "ads_line_1_main_alarm",
          remoteSymbol: "MAIN.alarm",
          value: "Bool(true)",
          valueType: "BOOL",
          access: "read_write",
          quality: {
            state: "stale",
            detail: "No update in the last two scans",
          },
        },
        { name: "missing required fields" },
      ],
    });

    assert.strictEqual(state.schemaVersion, 1);
    assert.strictEqual(state.scan, 42);
    assert.strictEqual(state.entries.length, 2);
    assert.deepStrictEqual(state.entries[0], {
      connection: "line_1",
      name: "ads_line_1_main_speed",
      remoteSymbol: "MAIN.speed",
      value: "Real(12.5)",
      valueType: "REAL",
      access: "read",
      quality: { state: "good", lastUpdateMs: 18 },
    });
    assert.strictEqual("address" in state.entries[0], false);
    assert.strictEqual(state.entries[1].quality.state, "stale");
    assert.strictEqual(state.problem?.kind, "invalid_entries");
    assert.match(state.problem?.detail ?? "", /1 ADS entry did not match/);
  });

  test("distinguishes incompatible schemas from a valid empty ADS snapshot", () => {
    const incompatible = normalizeAdsLiveValuesState({
      schemaVersion: 2,
      scan: 1,
      entries: [],
    });
    assert.deepStrictEqual(incompatible.entries, []);
    assert.strictEqual(incompatible.problem?.kind, "incompatible_schema");
    assert.match(incompatible.problem?.detail ?? "", /schema 2/);

    const empty = normalizeAdsLiveValuesState({
      schemaVersion: 1,
      scan: 0,
      entries: [],
    });
    assert.deepStrictEqual(empty.entries, []);
    assert.strictEqual(empty.problem, undefined);
  });

  test("omits malformed entries but surfaces an explicit contract problem", () => {
    const state = normalizeAdsLiveValuesState({
      schemaVersion: 1,
      scan: -1,
      entries: [
        {
          connection: "line",
          name: "tag",
          remoteSymbol: "MAIN.tag",
          value: "Int(1)",
          valueType: "INT",
          access: "force",
          quality: { state: "unknown" },
        },
      ],
    });
    assert.strictEqual(state.scan, 0);
    assert.deepStrictEqual(state.entries, []);
    assert.strictEqual(state.problem?.kind, "invalid_entries");
    assert.match(state.problem?.detail ?? "", /scan number was also invalid/);

    const invalidList = normalizeAdsLiveValuesState({
      schemaVersion: 1,
      scan: 3,
      entries: "not-an-array",
    });
    assert.strictEqual(invalidList.problem?.kind, "invalid_snapshot");
    assert.match(
      invalidList.problem?.message ?? "",
      /ADS values are unavailable/,
    );
  });

  test("stopped guidance follows the shared selected target, not raw runtime mode", () => {
    const rawSimulate = stoppedStatus();
    assert.strictEqual(
      __testLiveValuesUnavailableMessage(rawSimulate, "simulator"),
      "Start the Simulator to see live values.",
    );
    assert.strictEqual(
      __testLiveValuesUnavailableMessage(
        rawSimulate,
        "tcp://192.0.2.11:9902",
      ),
      "Connect to the selected runtime to see live values.",
    );
    const selectedRemote = __testStatusForSelectedTarget(
      rawSimulate,
      "tcp://192.0.2.11:9902",
    );
    assert.strictEqual(selectedRemote.runtimeMode, "online");
    assert.strictEqual(selectedRemote.targetLabel, "192.0.2.11:9902");
    assert.strictEqual(selectedRemote.endpoint, "tcp://192.0.2.11:9902");
  });

  test("lifecycle accepts, requests, caches, and clears ADS state under exact-session authority", () => {
    const lifecycle = source("runtimeLifecycle.ts");
    const liveValues = source("runtimeLifecycleLiveValues.ts");
    const events = source("runtimeLifecycleEvents.ts");
    const panel = source("ioPanel.ts");

    assert.ok(
      lifecycle.includes(
        "acceptedSession: () => this.acceptedLifecycleSession()",
      ) &&
        lifecycle.includes("isAcceptedAndTracked: (session) =>") &&
        lifecycle.includes("this.acceptedSessions.has(key)") &&
        lifecycle.includes("this.sessions.get(key) === session") &&
        liveValues.includes(
          "private adsState: AdsLiveValuesState = EMPTY_ADS_LIVE_VALUES_STATE",
        ) &&
        liveValues.includes("const session = this.dependencies.acceptedSession()") &&
        liveValues.includes('session.customRequest("stAdsState")') &&
        liveValues.includes("this.dependencies.isAcceptedAndTracked(session)") &&
        liveValues.includes("this.adsState = normalizeAdsLiveValuesState(body)") &&
        liveValues.includes("this.adsState = EMPTY_ADS_LIVE_VALUES_STATE"),
      "ADS cache requests and clears must stay owned by the accepted lifecycle session",
    );
    assert.ok(
      events.includes('event.event !== "stAdsState"') &&
        events.includes('if (event.event === "stAdsState")') &&
        events.includes("if (!deps.acceptedSessions.has(key))") &&
        events.includes("deps.setAdsState(EMPTY_ADS_LIVE_VALUES_STATE)"),
      "unaccepted/rejected ADS events must not enter lifecycle state",
    );
    assert.ok(
      panel.includes('type: "adsState"') &&
        panel.includes("normalizeAdsLiveValuesState(event.body)") &&
        panel.includes("liveValuesEventIsAccepted(accepted, event.session)") &&
        panel.includes("requestLiveValuesStateAfterScan(previousScan)") &&
        panel.includes("onDidChangeSelectedRuntime(() =>") &&
        panel.includes("void refreshLiveValuesForLifecycle()"),
      "Live Values must forward only normalized accepted-session ADS state and refresh it after scans",
    );
  });

  test("webview renders ADS separately with quality and no I/O actions or addresses", () => {
    const webview = source("ioPanel.webview.js");
    const adsRows = source("ioPanelAdsRows.webview.js");
    assert.ok(
      webview.includes('createNode("Connected variables"') &&
        webview.includes('"ADS",') &&
        webview.includes(
          "Imported ADS variables are read-only in Live Values.",
        ) &&
        webview.includes('type === "adsState"') &&
        webview.includes("globalThis.trustAdsRows.render(") &&
        adsRows.includes("entry.name") &&
        adsRows.includes("entry.remoteSymbol") &&
        adsRows.includes("entry.connection") &&
        adsRows.includes("entry.valueType") &&
        adsRows.includes("ads-contract-problem") &&
        adsRows.includes("problem.message") &&
        /problem\s*\?\s*"ADS values unavailable"/.test(adsRows) &&
        adsRows.includes('quality.className = "state-badge " + entry.quality.state') &&
        adsRows.includes('entry.quality.state.slice(0, 1).toUpperCase()'),
      "ADS rows must expose generated identity, source, value type, and visible quality",
    );
    for (const forbidden of [
      "entry.address",
      "writeInput",
      "forceInput",
      "releaseInput",
      'document.createElement("button")',
    ]) {
      assert.ok(
        !adsRows.includes(forbidden),
        `read-only ADS rows must not contain ${forbidden}`,
      );
    }
  });

  test("ADS import points users to the actual post-restart surface", () => {
    const actions = source("networkCanvas/protocolActions.ts");
    assert.ok(
      actions.includes(
        "Restart the Simulator, then view the imported variables in Live Values → ADS.",
      ),
    );
  });
});
