import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";

import {
  DEFAULT_ADS_DISCOVERY_PORTS,
  adsConnectionNameForTarget,
  adsDiscoveryPorts,
  adsPortBrowseEvidence,
  adsPortResponded,
  probeAdsCandidatePorts,
  respondingAdsPorts,
} from "../../networkCanvas/adsDiscoveryPorts";
import {
  adsTagSelectionsFromConnections,
  adsTagBatchSummary,
  normalizeAdsTagSelections,
} from "../../networkCanvas/adsTagBatch";
import {
  buildOfflineAdsImportArgs,
} from "../../networkCanvas/adsBrowseContract";
import {
  liveValueActionTarget,
} from "../../liveValueActionTarget";
import {
  buildAdsClientSummaryModel,
} from "../../networkCanvas/webview/adsClientSummaryModel";
import {
  ensureAdsRuntimeEnabled,
  existingAdsSnapshotPaths,
} from "../../networkCanvas/offlineComm";

function readJson(relativePath: string): Record<string, unknown> {
  return JSON.parse(
    fs.readFileSync(path.join(__dirname, "..", "..", "..", relativePath), "utf8"),
  ) as Record<string, unknown>;
}

suite("ADS multi-port discovery and existing Live Values contract", function () {
  test("the configurable scan list has the common ADS ports and normalizes user settings", () => {
    assert.deepStrictEqual(DEFAULT_ADS_DISCOVERY_PORTS, [301, 501, 851, 852, 853, 854]);
    assert.deepStrictEqual(
      adsDiscoveryPorts([851, 301, 851, 0, 65536, 501.5, "852"]),
      [301, 851],
    );

    const packageJson = readJson("package.json");
    const contributes = packageJson.contributes as Record<string, unknown>;
    const configuration = contributes.configuration as Record<string, unknown>;
    const properties = configuration.properties as Record<string, Record<string, unknown>>;
    assert.deepStrictEqual(
      properties["trust.ads.discoveryPorts"].default,
      DEFAULT_ADS_DISCOVERY_PORTS,
    );
  });

  test("only ADS services that answer are reported on one discovered device", async () => {
    const symbol = { id: "MAIN.value", name: "value", path: "MAIN.value" };
    assert.strictEqual(adsPortResponded({ tree: [symbol] }), true);
    assert.strictEqual(
      adsPortResponded({
        tree: [],
        error: { code: "symbol_upload_unsupported", message: "unsupported" },
      }),
      true,
    );
    assert.strictEqual(
      adsPortResponded({
        tree: [],
        error: { code: "ads_port_unavailable", message: "not open" },
      }),
      false,
    );

    const candidate = {
      id: "ads:100.67.6.217.1.1",
      label: "PLC laptop",
      source: "ads_broadcast",
      confidence: "confirmed",
      protocol: "ads",
      params: {
        host: "192.168.77.11",
        ams_net_id: "100.67.6.217.1.1",
        ams_port: 851,
      },
    };
    let activeProbes = 0;
    let maxConcurrentProbes = 0;
    const probed = await probeAdsCandidatePorts(
      candidate,
      [851, 301, 501],
      async (_target, port) => {
        activeProbes += 1;
        maxConcurrentProbes = Math.max(maxConcurrentProbes, activeProbes);
        await new Promise((resolve) => setTimeout(resolve, 5));
        activeProbes -= 1;
        return {
          tree: port === 301 || port === 851
            ? [{ ...symbol, id: `${symbol.id}:${port}` }]
            : [],
          error:
            port === 501
              ? { code: "ads_port_unavailable", message: "not open" }
              : undefined,
        };
      },
    );

    assert.deepStrictEqual(respondingAdsPorts(probed.params), [301, 851]);
    assert.deepStrictEqual(
      adsPortBrowseEvidence(probed.params).map((result) => result.port),
      [301, 851],
      "the combined tag browser must reuse the symbol trees collected during discovery",
    );
    assert.strictEqual(probed.params.ams_port, 851, "keep an already responding selection");
    assert.strictEqual(maxConcurrentProbes, 1, "ADS port probes must not race the shared route");
  });

  test("one ADS import request keeps selections grouped by responding port", () => {
    assert.deepStrictEqual(
      normalizeAdsTagSelections([
        { port: 851, paths: ["GVL.Start", "GVL.Start"] },
        { port: 301, paths: ["Task.Input"] },
        { port: 0, paths: ["invalid"] },
      ]),
      [
        { port: 301, paths: ["Task.Input"] },
        { port: 851, paths: ["GVL.Start"] },
      ],
    );
    assert.strictEqual(
      adsTagBatchSummary({
        applied: true,
        addedCount: 2,
        restartRequired: true,
        ports: [
          { port: 301, paths: ["Task.Input"], applied: true, addedCount: 1, message: "Added." },
          { port: 851, paths: ["GVL.Start"], applied: true, addedCount: 1, message: "Added." },
        ],
      }),
      "Added 2 tags from ADS ports 301, 851.",
    );

    assert.deepStrictEqual(
      adsTagSelectionsFromConnections(
        [
          {
            host: "192.168.77.11",
            target_net_id: "100.67.6.217.1.1",
            ams_port: 301,
            points: [{ symbol: "Task.Input" }],
          },
          {
            host: "192.168.77.12",
            target_net_id: "192.168.77.12.1.1",
            ams_port: 851,
            points: [{ symbol: "OTHER.Start" }],
          },
        ],
        {
          host: "192.168.77.11",
          ams_net_id: "100.67.6.217.1.1",
        },
      ),
      [{ port: 301, paths: ["Task.Input"] }],
      "rediscovery must only mark tags from the same ADS device as already added",
    );
  });

  test("the ADS inspector summarizes devices and ports instead of dumping raw connections", () => {
    const model = buildAdsClientSummaryModel(
      {
        connections: [
          {
            name: "internal_port_301",
            host: "192.168.77.11",
            target_net_id: "100.67.6.217.1.1",
            ams_port: 301,
            points: [{ symbol: "Task.Input" }],
          },
          {
            name: "internal_port_851",
            host: "192.168.77.11",
            target_net_id: "100.67.6.217.1.1",
            ams_port: 851,
            points: [{ symbol: "GVL.Read" }, { symbol: "GVL.Write" }],
          },
        ],
      },
      "configured_policy",
      "Configured · runtime is not running.",
    );

    assert.deepStrictEqual(model.devices, [
      {
        address: "192.168.77.11",
        amsNetId: "100.67.6.217.1.1",
        ports: [
          { port: 301, tagCount: 1 },
          { port: 851, tagCount: 2 },
        ],
      },
    ]);
    assert.strictEqual(model.status, "Runtime stopped — start it to read tags.");
    assert.strictEqual(model.configPath, "ads.toml");
    assert.strictEqual(model.updateIntervalMs, 20);

    const runningModel = buildAdsClientSummaryModel(
      {
        connections: [
          {
            host: "192.168.77.11",
            target_net_id: "100.67.6.217.1.1",
            ams_port: 301,
            points: [{ symbol: "Task.Input" }],
          },
        ],
      },
      "configured_policy",
      "Configured in project files; restart the runtime to apply this change.",
      "simulate",
    );
    assert.strictEqual(
      runningModel.status,
      "Runtime running — ADS is configured.",
      "the ADS inspector must not call a running simulator stopped",
    );

    const root = path.join(__dirname, "..", "..", "..", "src", "networkCanvas", "webview");
    const inspector = fs.readFileSync(path.join(root, "NodeInspector.tsx"), "utf8");
    const summary = fs.readFileSync(path.join(root, "AdsClientSummary.tsx"), "utf8");
    assert.ok(inspector.includes('title = adsSummary ? "ADS device"'));
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

  test("per-port imports keep distinct connection identities and prior snapshots", () => {
    const target = {
      name: "PLC laptop",
      host: "192.168.77.11",
      ams_net_id: "100.67.6.217.1.1",
      ams_port: 301,
      responding_ads_ports: [301, 851],
    };
    assert.strictEqual(
      adsConnectionNameForTarget(target, "ads_import"),
      "PLC laptop_port_301",
    );

    const args = buildOfflineAdsImportArgs(
      "/tmp/project",
      target,
      "PLC laptop_port_301",
      ["GVL.Input"],
      ["/tmp/project/ads/snapshots/PLC_laptop_port_851.symbols.json"],
    );
    assert.strictEqual(args[args.indexOf("--ams-port") + 1], "301");
    assert.strictEqual(
      args[args.indexOf("--existing-snapshot") + 1],
      "/tmp/project/ads/snapshots/PLC_laptop_port_851.symbols.json",
    );
  });

  test("repeated ADS imports preserve prior snapshots and enable runtime.toml once", () => {
    const project = fs.mkdtempSync(path.join(os.tmpdir(), "trust-ads-multiport-"));
    try {
      fs.mkdirSync(path.join(project, "ads", "snapshots"), { recursive: true });
      fs.writeFileSync(path.join(project, "ads", "snapshots", "b.symbols.json"), "{}");
      fs.writeFileSync(path.join(project, "ads", "snapshots", "a.symbols.json"), "{}");
      fs.writeFileSync(
        path.join(project, "runtime.toml"),
        "[runtime.ads]\nenabled = false\n\n[runtime.control]\nendpoint = \"tcp://127.0.0.1:0\"\n",
      );

      const first = ensureAdsRuntimeEnabled(project);
      const second = ensureAdsRuntimeEnabled(project);
      assert.strictEqual(first.ok, true);
      assert.strictEqual(second.ok, true);
      const runtimeToml = fs.readFileSync(path.join(project, "runtime.toml"), "utf8");
      assert.strictEqual((runtimeToml.match(/^enabled\s*=/gm) ?? []).length, 1);
      assert.strictEqual((runtimeToml.match(/^config_path\s*=/gm) ?? []).length, 1);
      assert.match(runtimeToml, /\[runtime\.control\][\s\S]*endpoint/);
      assert.deepStrictEqual(
        existingAdsSnapshotPaths(project).map((file) => path.basename(file)),
        ["a.symbols.json", "b.symbols.json"],
      );
    } finally {
      fs.rmSync(project, { recursive: true, force: true });
    }
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
    assert.ok(webview.includes("Read-only ADS tag"));
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
