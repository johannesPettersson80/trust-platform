import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";

import {
  liveValueActionTarget,
} from "../../liveValueActionTarget";

// The ADS port rules (the scan list, which ports answer, probing one port at a time, the port a find
// keeps) are the runtime's `comm.discover` (Rust tests in
// crates/trust-runtime/src/host/search/sources/ads_ports_tests.rs); the tag batch's rules (ports and
// paths kept once and sorted, the batch summary, the tags already imported from the same device, the
// per-port connection names, the import into the project) are `trust-lsp.devicesCanvasAct` and
// `trust-lsp.devicesTopology` (Rust tests in crates/trust-devices/src/canvas/tests.rs and
// crates/trust-devices/src/actions/tests/plans.rs). These tests prove the wiring.

function readJson(relativePath: string): Record<string, unknown> {
  return JSON.parse(
    fs.readFileSync(path.join(__dirname, "..", "..", "..", relativePath), "utf8"),
  ) as Record<string, unknown>;
}

suite("ADS multi-port discovery and existing Live Values contract", function () {
  test("the configurable scan list has the common ADS ports and every search sends it", () => {
    // Specification 11: without a list the ports are 301, 501, 851, 852, 853 and 854.
    const packageJson = readJson("package.json");
    const contributes = packageJson.contributes as Record<string, unknown>;
    const configuration = contributes.configuration as Record<string, unknown>;
    const properties = configuration.properties as Record<string, Record<string, unknown>>;
    assert.deepStrictEqual(
      properties["trust.ads.discoveryPorts"].default,
      [301, 501, 851, 852, 853, 854],
    );
    const root = path.join(__dirname, "..", "..", "..", "src", "networkCanvas");
    const controller = fs.readFileSync(path.join(root, "discoveryController.ts"), "utf8");
    const offline = fs.readFileSync(path.join(root, "offlineComm.ts"), "utf8");
    const panel = fs.readFileSync(path.join(root, "networkCanvasPanel.ts"), "utf8");
    assert.ok(
      panel.includes('adsPorts: trustConfig().get<unknown[]>("ads.discoveryPorts")'),
      "the panel hands the ADS port setting to the search",
    );
    assert.ok(
      controller.includes("ads_ports: adsPorts"),
      "a runtime's search gets the ports as scope.ads_ports",
    );
    assert.ok(
      offline.includes('args.push("--ads-port", String(port))'),
      "a search from this computer gets each port as --ads-port",
    );
    assert.ok(
      !controller.includes("comm.browse_symbols"),
      "the extension no longer probes ADS ports itself",
    );
  });

  test("the ADS inspector summarizes devices and ports instead of dumping raw connections", () => {
    // The summary (devices by address and AMS Net ID, the tags of each port, the state sentence, the
    // settings' defaults) is the server's: the Rust tests
    // `connections_say_when_they_need_attention_and_an_ads_client_sums_up_its_devices`
    // (crates/trust-devices/src/canvas/local_tests.rs) and the ADS client view tests
    // (crates/trust-devices/src/view/tests/connections.rs). The inspector draws it.
    const root = path.join(__dirname, "..", "..", "..", "src", "networkCanvas", "webview");
    const inspector = fs.readFileSync(path.join(root, "NodeInspector.tsx"), "utf8");
    const summary = fs.readFileSync(path.join(root, "AdsClientSummary.tsx"), "utf8");
    assert.ok(inspector.includes('title = adsSummary ? "ADS device"'));
    assert.ok(inspector.includes("d.adsSummary"), "the inspector draws the server's summary");
    assert.ok(inspector.includes("Manage tags"));
    assert.ok(summary.includes('data-role="ads-advanced-settings"'));
    assert.ok(!summary.includes("connection.name"));
  });

  test("ADS discovery and tag browsing expose one purposeful multi-port workflow", () => {
    const root = path.join(__dirname, "..", "..", "..", "src", "networkCanvas", "webview");
    const discover = fs.readFileSync(path.join(root, "DiscoverPane.tsx"), "utf8");
    const browser = fs.readFileSync(path.join(root, "AdsMultiPortTagBrowser.tsx"), "utf8");
    const session = fs.readFileSync(path.join(root, "useBrowseSession.ts"), "utf8");
    const discoverActions = fs.readFileSync(path.join(root, "useDiscoverPane.ts"), "utf8");

    assert.ok(discover.includes('label: "ADS devices"'));
    assert.ok(discover.includes('"Add to canvas"'));
    assert.ok(discover.includes('"Manage tags"'));
    assert.ok(discover.includes("Advanced scan settings"));
    assert.ok(!discover.includes('data-role="ads-port"'));
    assert.ok(browser.includes("Search tags on all ADS ports"));
    assert.ok(!browser.includes("Add selected tags"));
    assert.ok(
      browser.includes("checked={selected.has(key)}"),
      "the checkbox must be the configured-state control",
    );
    assert.ok(browser.includes(">\n          Done\n"));
    assert.ok(browser.includes("Start or restart the simulator to use the new tags."));
    assert.ok(session.includes('type: "addAdsTagsBatch"'));
    assert.ok(session.includes('type: "removeAdsTag"'));
    assert.ok(discoverActions.includes('type: "addAdsDevice"'));
    assert.ok(
      !discoverActions.includes("configuredAdsConnections(nodes, candidate.params).length === 0"),
      "Manage tags must merge newly discovered ports into an existing ADS device",
    );
    const adsAddHandler = session.slice(
      session.indexOf("const addAdsTags ="),
      session.indexOf("const addTags ="),
    );
    assert.ok(
      !adsAddHandler.includes("close();"),
      "adding ADS tags must keep the combined browser open until Done",
    );
  });

  test("ADS globals reuse the existing Live Values write force release actions", () => {
    assert.deepStrictEqual(liveValueActionTarget("global:line1_speed"), {
      kind: "global",
      name: "line1_speed",
    });
    assert.deepStrictEqual(liveValueActionTarget("%IX0.0"), {
      kind: "io",
      address: "%IX0.0",
    });

    const webview = fs.readFileSync(
      path.join(__dirname, "..", "..", "..", "src", "ioPanel.webview.js"),
      "utf8",
    );
    assert.ok(webview.includes('"Write"'));
    assert.ok(webview.includes('"Force"'));
    assert.ok(webview.includes('"Release"'));
    assert.ok(webview.includes('"ADS tags"'));
    assert.ok(
      webview.includes("renderRows(adsEntries"),
      "ADS tags must reuse the ordinary Live Values row renderer"
    );
    assert.ok(
      webview.includes("entry.writable === false"),
      "read-only ADS tags must disable write and force instead of reporting false success"
    );
    // The read-only ADS tag's reason is the language server's (the Rust test in
    // crates/trust-lsp/src/handlers/commands/run_targets/live_values_tests.rs).
  });

  test("Live Values feedback keeps a fixed layout slot", () => {
    const panel = fs.readFileSync(
      path.join(__dirname, "..", "..", "..", "src", "ioPanel.ts"),
      "utf8",
    );
    const webview = fs.readFileSync(
      path.join(__dirname, "..", "..", "..", "src", "ioPanel.webview.js"),
      "utf8",
    );
    const statusRule = /\.status \{([\s\S]*?)\n\s*\}/.exec(panel)?.[1] ?? "";
    assert.match(statusRule, /display: block;/);
    assert.match(statusRule, /height: 27px;/);
    assert.match(statusRule, /visibility: hidden;/);
    assert.match(statusRule, /white-space: nowrap;/);
    assert.match(panel, /\.status:not\(:empty\) \{\s*visibility: visible;/);
    assert.ok(webview.includes("status.title = text;"));
  });

  test("Boolean drafts survive scans and forced rows keep their value controls", () => {
    const webview = fs.readFileSync(
      path.join(__dirname, "..", "..", "..", "src", "ioPanel.webview.js"),
      "utf8",
    );
    assert.ok(webview.includes("editCache.set(key, next);"));
    assert.ok(webview.includes("actions.appendChild(valueControl);"));
    assert.ok(!webview.includes("const valueControl = forced"));
    assert.ok(!webview.includes('writeSlot.className = "value-input-spacer"'));
  });
});
