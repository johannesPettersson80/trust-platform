import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";

import {
  deduplicateDiscoveryCandidates,
  runNetworkCanvasDiscovery,
} from "../../networkCanvas/discoveryController";
import { DiscoveryRequestTracker } from "../../networkCanvas/discoverySession";
import { DiscoveryOriginTargetStore } from "../../networkCanvas/discoveryOriginTargets";
import { isCurrentAdsServiceProbeRequest } from "../../networkCanvas/adsServiceProbeController";
import { isDiscoveryErrorCode } from "../../networkCanvas/discoveryErrors";
import { planBrowseOpen } from "../../networkCanvas/webview/browseSessionModel";
import { confirmedAdsBrowseRetryTarget } from "../../networkCanvas/webview/adsTargetPort";
import {
  localSimControl,
  simulatorControlFromDebugConfiguration,
} from "../../simControl";
import type { RuntimeTarget } from "../../runtimeTarget";
import type {
  BrowseSymbolsResponse,
  DiscoverResponse,
} from "../../networkCanvas/offlineComm";

interface AdsDiscoveryModelContract {
  readonly PLC_RUNTIME_PORTS: readonly number[];
  readonly COMMON_ADS_SERVICE_PORTS: readonly number[];
  readonly AUTOMATIC_ADS_SERVICE_PORTS: readonly number[];
  adsDiscoveryFields(advanced: boolean): readonly string[];
  validateAdsDiscoveryHost(host: string): string | undefined;
  validateAdsAmsNetId(value: string): string | undefined;
  autoSelectAdsServicePort(
    availablePorts: readonly number[],
  ): number | undefined;
  adsServicePresentation(port: number): {
    readonly primary: string;
    readonly secondary: string;
  };
  createAdsDiscoveryScanSnapshot(
    origin: string,
    draft: {
      readonly host: string;
      readonly amsNetId: string;
      readonly customPorts: string;
      readonly advanced: boolean;
    },
  ): {
    readonly origin: string;
    readonly host?: string;
    readonly targetAmsNetId?: string;
    readonly ports: readonly number[];
  };
  createAutomaticAdsDiscoveryItems(snapshot: {
    readonly host?: string;
    readonly targetAmsNetId?: string;
  }): readonly {
    readonly protocol: string;
    readonly host?: string;
    readonly targetAmsNetId?: string;
    readonly amsPort?: number;
  }[];
}

interface OfflineCommModule {
  offlineCommDiscover(
    context: vscode.ExtensionContext,
    protocol: string,
    origin: string,
    scope?: { cidr?: string; host?: string; timeoutMs?: number },
  ): Promise<DiscoverResponse | undefined>;
}

interface MutableChildProcessModule {
  execFile: (...args: unknown[]) => unknown;
}

interface NewProjectModuleContract {
  buildRuntimeTomlSource?: (platform: NodeJS.Platform) => string;
}

type AdsServiceProbeStatus =
  | "available"
  | "unsupported"
  | "empty"
  | "unavailable"
  | "check_failed"
  | "route_missing";

interface AdsServiceProbeResult {
  readonly port: number;
  readonly status: AdsServiceProbeStatus;
  readonly symbolCount: number;
  readonly usable: boolean;
}

interface AdsServiceProbeModelContract {
  readonly PLC_RUNTIME_PORTS: readonly number[];
  readonly COMMON_ADS_SERVICE_PORTS: readonly number[];
  readonly AUTOMATIC_ADS_SERVICE_PORTS: readonly number[];
  readonly MAX_ADS_SERVICE_PROBES: number;
  planAdsServicePorts(customPorts: readonly number[]): readonly number[];
  parseCustomAdsPorts(input: string): {
    readonly ports: readonly number[];
    readonly error?: string;
  };
  classifyAdsServiceProbe(
    port: number,
    response: BrowseSymbolsResponse,
  ): AdsServiceProbeResult;
  autoSelectUsableAdsService(
    results: readonly AdsServiceProbeResult[],
  ): number | undefined;
  didAnyAdsServiceRespond(results: readonly AdsServiceProbeResult[]): boolean;
  probeAdsServicesSequentially(
    ports: readonly number[],
    probe: (port: number) => Promise<BrowseSymbolsResponse>,
  ): Promise<readonly AdsServiceProbeResult[]>;
}

function extensionRoot(): string {
  return path.resolve(__dirname, "../../..");
}

function readSource(relativePath: string): string {
  return fs
    .readFileSync(path.join(extensionRoot(), "src", relativePath), "utf8")
    .replace(/\r\n?/g, "\n");
}

function readMedia(relativePath: string): string {
  return fs.readFileSync(
    path.join(extensionRoot(), "media", relativePath),
    "utf8",
  );
}

function adsDiscoveryModel(): AdsDiscoveryModelContract {
  return require("../../networkCanvas/webview/discoverPaneModel") as AdsDiscoveryModelContract;
}

function offlineCommModule(): OfflineCommModule {
  return require("../../networkCanvas/offlineComm") as OfflineCommModule;
}

function adsServiceProbeModel(): AdsServiceProbeModelContract {
  return require("../../networkCanvas/adsServiceProbeModel") as AdsServiceProbeModelContract;
}

function testExtensionContext(): vscode.ExtensionContext {
  return {
    extensionMode: vscode.ExtensionMode.Test,
    extensionPath: extensionRoot(),
  } as vscode.ExtensionContext;
}

function extractControlToken(runtimeToml: string): string {
  const match = /^auth_token\s*=\s*"([^"]+)"\s*$/m.exec(runtimeToml);
  assert.ok(
    match,
    "Windows TCP runtime control must include a non-empty auth_token.",
  );
  return match[1];
}

suite("Windows ADS zero-input discovery contracts", function () {
  test("ships the rebuilt one-button ADS webview bundle", () => {
    for (const file of [
      "networkCanvasWebview.js",
      "networkCanvasWebview.js.map",
    ]) {
      const bundle = readMedia(file);
      for (const expected of [
        "Discover ADS devices",
        "Route setup",
        "Retry browse",
        "Address entered manually",
        "ADS service responded",
      ]) {
        assert.ok(bundle.includes(expected), `${file} is missing ${expected}`);
      }
      for (const retired of [
        "Where is TwinCAT?",
        "ADS / TwinCAT",
        "Find TwinCAT",
        "Active ADS connections are never interrupted",
        "Checks are read-only",
        "using ADS, checks wait",
      ]) {
        assert.ok(!bundle.includes(retired), `${file} still ships ${retired}`);
      }
    }
  });

  test("Browse variables inspector header has separate hierarchy lines", () => {
    const source = readSource("networkCanvas/webview/BrowseTagsPanel.tsx");
    assert.ok(
      source.includes(
        '<div className="trust-inspector__eyebrow">Devices & Connections</div>',
      ) &&
        source.includes(
          '<div className="trust-inspector__title">{title}</div>',
        ),
      "Browse header eyebrow and title must be block rows, not concatenated inline text",
    );
    assert.ok(
      !source.includes('<span className="trust-inspector__eyebrow">') &&
        !source.includes('<strong className="trust-inspector__title">'),
      "the old inline header elements must stay retired",
    );
  });

  test("offers one zero-input ADS flow without a target-location wizard", () => {
    const pane = readSource("networkCanvas/webview/DiscoverPane.tsx");
    const adsFlow = readSource("networkCanvas/webview/AdsDiscoveryFlow.tsx");
    const protocolCatalog = readSource(
      "communication/communicationProtocols.ts",
    );
    const protocolGrouping = readSource("networkCanvas/webview/grouping.ts");
    const discoveryUi = `${pane}\n${adsFlow}`;
    const rowDefinitions = pane.slice(
      0,
      pane.indexOf("export function DiscoverPane"),
    );
    assert.strictEqual(
      (rowDefinitions.match(/protocol:\s*"ads"/g) ?? []).length,
      1,
      "Discover must render one ADS entry, not separate local, network, and targeted choices.",
    );
    assert.ok(!rowDefinitions.includes('key: "ads-host"'));
    assert.ok(
      discoveryUi.includes('label: "ADS devices"') &&
        discoveryUi.includes('note: "this computer and local network"'),
      "The default entry must describe the zero-input ADS search in user terms.",
    );
    assert.ok(
      !discoveryUi.includes("Where is TwinCAT?") &&
        !discoveryUi.includes("ADS_DISCOVERY_LOCATIONS"),
      "ADS discovery must not ask users to choose among locations before scanning.",
    );
    assert.ok(discoveryUi.includes("Discover ADS devices"));
    assert.strictEqual(
      (discoveryUi.match(/"Discover ADS devices"/g) ?? []).length,
      1,
      "The Discover surface must expose exactly one ADS discovery action.",
    );
    assert.ok(discoveryUi.includes("Discovering ADS devices…"));
    assert.ok(!discoveryUi.includes("Find TwinCAT"));
    assert.ok(
      discoveryUi.includes(
        "Searches this computer and the local network, then shows responding",
      ) &&
        discoveryUi.includes("ADS services (851–854, 301, and 501).") &&
        !discoveryUi.includes("Checks are read-only") &&
        !discoveryUi.includes("using ADS, checks wait") &&
        !discoveryUi.includes("Active ADS connections are never interrupted"),
      "default ADS copy must remain one calm outcome sentence; contention guidance is contextual",
    );
    assert.ok(discoveryUi.includes('data-role="ads-discover"'));
    assert.ok(discoveryUi.includes('"ads-discovery-section"'));
    assert.ok(
      protocolCatalog.includes('id: "ads",\n    title: "ADS"') &&
        !protocolCatalog.includes('title: "ADS / TwinCAT"') &&
        !protocolGrouping.includes("TwinCAT or ADS"),
      "Protocol pickers and discovery must use one protocol-first ADS name instead of presenting TwinCAT as a second ADS choice.",
    );
    assert.ok(
      !discoveryUi.includes('data-role="ads-discovery-flow"'),
      "The fixed ADS action must not be presented as a selectable checkbox.",
    );
    assert.ok(discoveryUi.includes("selected type"));
    assert.ok(discoveryUi.includes('row.status === "failed"'));
    assert.match(
      discoveryUi,
      /!error[\s\S]*row\.status === "failed"[\s\S]*results\.length === 0/,
      "A failed discovery must not also render the Nothing found state.",
    );
  });

  test("one toolbar action opens the ADS drawer and starts exactly one automatic scan", () => {
    const app = readSource("networkCanvas/webview/NetworkCanvasApp.tsx");
    const header = readSource("networkCanvas/webview/NetworkCanvasHeader.tsx");
    const pane = readSource("networkCanvas/webview/DiscoverPane.tsx");
    const flow = readSource("networkCanvas/webview/AdsDiscoveryFlow.tsx");

    assert.ok(
      header.includes(">\n        Discover ADS devices\n      </button>") &&
        app.includes("<DiscoverPane\n            autoStartAds"),
      "the canvas toolbar must name and pass through the user's primary ADS action",
    );
    assert.ok(
      pane.includes("const autoAdsStartConsumed = useRef(false)") &&
        pane.includes("autoAdsStartConsumed.current = true") &&
        pane.includes("discoverAds();") &&
        pane.includes('startScan([ADS], "ads")'),
      "opening the drawer must consume one guarded intent through the normal scan snapshot path",
    );
    assert.ok(
      flow.includes('hasRun\n              ? "Scan ADS again"') &&
        pane.includes("adsScanSnapshot.current !== undefined"),
      "after the automatic scan, the drawer control must be an explicit retry rather than a second primary Discover action",
    );
    assert.ok(
      !pane.includes("runtimeStart") && !pane.includes("startSimulator"),
      "ADS discovery must never start the Simulator as a side effect",
    );
  });

  test("keeps every non-ADS capability behind one collapsed disclosure", () => {
    const pane = readSource("networkCanvas/webview/DiscoverPane.tsx");
    const renderStart = pane.indexOf("{adsRows.map(renderRow)}");
    const resultsStart = pane.indexOf(
      "(scanning || progress.length > 0 || results.length > 0)",
    );
    const discoverControls = pane.slice(renderStart, resultsStart);
    const disclosureGate = discoverControls.indexOf(
      "{showOtherDiscoveryTypes && (",
    );

    assert.ok(renderStart >= 0 && resultsStart > renderStart);
    assert.ok(
      pane.includes(
        "const [showOtherDiscoveryTypes, setShowOtherDiscoveryTypes] = useState(false)",
      ),
      "Other protocols must be collapsed when Discover first opens.",
    );
    assert.strictEqual(
      (discoverControls.match(/Other discovery types/g) ?? []).length,
      1,
      "There must be one disclosure for every non-ADS discovery type.",
    );
    assert.ok(
      discoverControls.includes('data-role="other-discovery-types-toggle"') &&
        discoverControls.includes("aria-expanded={showOtherDiscoveryTypes}") &&
        disclosureGate >= 0,
      "The single disclosure must expose its collapsed/expanded state.",
    );
    for (const hiddenRows of [
      "{otherAutomatic.map(renderRow)}",
      "{otherKnownAddressRows.map(renderRow)}",
      "{runtimeOnly.map(renderRow)}",
    ]) {
      assert.ok(
        discoverControls.indexOf(hiddenRows) > disclosureGate,
        `${hiddenRows} must render only inside Other discovery types.`,
      );
    }
    for (const nonAdsProtocol of [
      'protocol: "discovery"',
      'protocol: "modbus_tcp"',
      'protocol: "opcua_client"',
      'protocol: "mqtt"',
      'protocol: "ethercat"',
      'protocol: "gpio"',
    ]) {
      assert.ok(pane.includes(nonAdsProtocol));
    }
    assert.ok(
      discoverControls.includes(
        "Known address or subnet for other protocols",
      ),
      "Known-address inputs must identify themselves as recovery for non-ADS protocols.",
    );
    assert.ok(
      discoverControls.indexOf('data-role="runtime-scan-origin"') >
        disclosureGate &&
        pane.includes(
          "{showOtherDiscoveryTypes && hasSelectedNonAdsScan && (",
        ) &&
        pane.includes('data-role="scan-selected"'),
      "Origin selection and the selected-type scan action must remain available after expansion.",
    );
    assert.ok(
      !pane.includes("Recommended") &&
        !pane.includes("showTargeted") &&
        !pane.includes("showRuntime"),
      "The old default heading and competing disclosures must stay retired.",
    );
  });

  test("zero-input ADS discovery searches this computer and the local network", () => {
    const model = adsDiscoveryModel();
    const automatic = model.createAdsDiscoveryScanSnapshot("this_host", {
      host: "",
      amsNetId: "",
      customPorts: "",
      advanced: false,
    });
    assert.deepStrictEqual(model.createAutomaticAdsDiscoveryItems(automatic), [
      { protocol: "ads" },
    ]);

    const snapshot = model.createAdsDiscoveryScanSnapshot("runtime:a", {
      host: "192.168.50.42",
      amsNetId: "10.20.30.40.1.1",
      customPorts: "9000, 9001",
      advanced: true,
    });

    assert.deepStrictEqual(snapshot, {
      origin: "runtime:a",
      host: "192.168.50.42",
      targetAmsNetId: "10.20.30.40.1.1",
      ports: [851, 852, 853, 854, 301, 501, 9000, 9001],
    });
    assert.deepStrictEqual(model.createAutomaticAdsDiscoveryItems(snapshot), [
      { protocol: "ads" },
      {
        protocol: "ads",
        host: "192.168.50.42",
        targetAmsNetId: "10.20.30.40.1.1",
      },
    ]);
    assert.strictEqual(snapshot.origin, "runtime:a");
    assert.deepStrictEqual(
      snapshot.ports,
      [851, 852, 853, 854, 301, 501, 9000, 9001],
    );

    const collapsed = model.createAdsDiscoveryScanSnapshot("runtime:a", {
      host: "192.168.50.42",
      amsNetId: "10.20.30.40.1.1",
      customPorts: "9000",
      advanced: false,
    });
    assert.strictEqual(
      collapsed.targetAmsNetId,
      "10.20.30.40.1.1",
      "Collapsing Advanced must not silently discard a persisted manual identity.",
    );
    assert.deepStrictEqual(
      collapsed.ports,
      [851, 852, 853, 854, 301, 501, 9000],
    );

    const pane = [
      readSource("networkCanvas/webview/DiscoverPane.tsx"),
      readSource("networkCanvas/webview/AdsDiscoveryFlow.tsx"),
      readSource("networkCanvas/webview/discoverPaneModel.ts"),
    ].join("\n");
    assert.ok(pane.includes("createAdsDiscoveryScanSnapshot"));
    assert.ok(pane.includes("createAutomaticAdsDiscoveryItems"));
    assert.ok(pane.includes("adsScanSnapshot"));
    assert.ok(pane.includes('draft.host.trim() ? "known address"'));
    assert.ok(pane.includes('`custom ports ${draft.customPorts.trim()}`'));
  });

  test("deduplicates the same ADS identity found locally and on the network", () => {
    const local = {
      id: "ads:10_20_30_40_1_1",
      label: "PLC-LAPTOP · 10.20.30.40.1.1",
      protocol: "ads",
      source: "ads_local_router",
      confidence: "observed",
      params: {
        host: "127.0.0.1",
        ams_net_id: "10.20.30.40.1.1",
        ams_port: 851,
        responding_ads_ports: [851, 301],
      },
    };
    const network = {
      ...local,
      id: "ads:duplicate-wire-id",
      source: "ads_broadcast",
      params: {
        ...local.params,
        host: "192.168.50.42",
        ams_port: 501,
        responding_ads_ports: [501, 301],
      },
    };

    assert.deepStrictEqual(
      deduplicateDiscoveryCandidates([local, network]),
      [
        {
          ...local,
          params: {
            ...local.params,
            host: "192.168.50.42",
            responding_ads_ports: [301, 501, 851],
          },
        },
      ],
      "AMS Net ID is the ADS device identity; keep one card, preserve its routable LAN address, and union every observed responding service instead of losing ports behind one scalar ams_port.",
    );
  });

  test("combined zero-input ADS discovery preserves candidates and a partial-path warning", async () => {
    const offline = offlineCommModule();
    const originalDiscover = offline.offlineCommDiscover;
    const found = {
      id: "ads:10_20_30_40_1_1",
      label: "PLC-LAPTOP · 10.20.30.40.1.1",
      source: "ads_broadcast",
      confidence: "observed",
      protocol: "ads",
      params: {
        host: "192.168.50.42",
        ams_net_id: "10.20.30.40.1.1",
        ams_port: 851,
      },
    };
    offline.offlineCommDiscover = async (
      _context,
      _protocol,
      _origin,
      scope,
    ) => {
      assert.strictEqual(
        scope?.host,
        undefined,
        "the ordinary button must use the combined no-host runtime discovery contract",
      );
      return {
        protocol: "ads",
        candidates: [found],
        warnings: [
          "ADS broadcast discovery timed out while receiving a UDP reply (os error 10060).",
          "Local AMS router identity was forcibly closed (os error 10054).",
          "Directed fallback is unavailable because the runtime lacks the ads-wire feature.",
        ],
      };
    };

    const posted: Record<string, unknown>[] = [];
    const panel = {
      visible: true,
      webview: {
        postMessage: async (message: Record<string, unknown>) => {
          posted.push(message);
          return true;
        },
      },
    } as unknown as vscode.WebviewPanel;
    const tracker = new DiscoveryRequestTracker<vscode.WebviewPanel>();
    const token = tracker.start(panel);

    try {
      await runNetworkCanvasDiscovery(
        {
          sessionId: "automatic-ads",
          requestId: 8,
          request: {
            origin: "this_host",
            items: [{ protocol: "ads" }],
          },
        },
        {
          panel,
          extensionContext: testExtensionContext(),
          tracker,
          token,
        },
      );
    } finally {
      offline.offlineCommDiscover = originalDiscover;
    }

    assert.ok(
      posted.some(
        (message) =>
          message.type === "discoverProgress" &&
          message.status === "done" &&
          message.count === 1,
      ),
      "the combined runtime response must still render its discovered ADS device",
    );
    const result = posted[posted.length - 1];
    assert.strictEqual(result?.type, "discoverResults");
    assert.deepStrictEqual(result?.candidates, [found]);
    assert.strictEqual(result?.error, undefined);
    assert.strictEqual(
      result?.warning,
      "Some ADS checks did not answer. Results from responding devices are shown.",
    );
    assert.doesNotMatch(
      String(result?.warning),
      /UDP|router|10060|10054|ads-wire/i,
    );
    assert.deepStrictEqual(
      result?.warningDetails,
      [
        "ADS broadcast discovery timed out while receiving a UDP reply (os error 10060).",
        "Local AMS router identity was forcibly closed (os error 10054).",
        "Directed fallback is unavailable because the runtime lacks the ads-wire feature.",
      ],
      "raw per-leg evidence must remain available only for collapsed Technical details",
    );
  });

  test("automatic ADS warnings with no candidates become blocked or unavailable, never clean zero", async () => {
    assert.strictEqual(isDiscoveryErrorCode("ads_discovery_blocked"), true);
    assert.strictEqual(isDiscoveryErrorCode("ads_discovery_unavailable"), true);
    assert.strictEqual(isDiscoveryErrorCode("ads_udp_identify_blocked"), true);
    assert.strictEqual(
      isDiscoveryErrorCode("ads_local_router_unavailable"),
      true,
    );
    assert.strictEqual(isDiscoveryErrorCode("something_else"), false);

    const offline = offlineCommModule();
    const originalDiscover = offline.offlineCommDiscover;
    const cases = [
      {
        warning:
          "Automatic ADS discovery is unavailable because this runtime build does not include the ads-wire feature.",
        code: "ads_discovery_unavailable",
        message:
          "ADS discovery is not available in this runtime build. Update or reinstall truST, then try again.",
      },
      {
        warning:
          "ADS broadcast discovery timed out while receiving a UDP reply (os error 10060).",
        code: "ads_discovery_blocked",
        message:
          "ADS discovery could not finish. Make sure the device is running and your firewall allows truST on this network, then try again. If you know its address, use Advanced.",
      },
    ] as const;

    try {
      for (const [index, testCase] of cases.entries()) {
        offline.offlineCommDiscover = async () => ({
          protocol: "ads",
          candidates: [],
          warnings: [testCase.warning],
        });
        const posted: Record<string, unknown>[] = [];
        const panel = {
          visible: true,
          webview: {
            postMessage: async (message: Record<string, unknown>) => {
              posted.push(message);
              return true;
            },
          },
        } as unknown as vscode.WebviewPanel;
        const tracker = new DiscoveryRequestTracker<vscode.WebviewPanel>();
        const token = tracker.start(panel);
        await runNetworkCanvasDiscovery(
          {
            sessionId: `ads-warning-${index}`,
            requestId: index + 1,
            request: {
              origin: "this_host",
              items: [{ protocol: "ads" }],
            },
          },
          {
            panel,
            extensionContext: testExtensionContext(),
            tracker,
            token,
          },
        );

        const result = posted[posted.length - 1];
        assert.strictEqual(result?.type, "discoverResults");
        assert.strictEqual(result?.errorCode, testCase.code);
        assert.strictEqual(result?.error, testCase.message);
        assert.deepStrictEqual(result?.errorDetails, [testCase.warning]);
        assert.deepStrictEqual(result?.candidates, []);
        assert.strictEqual(
          posted.some(
            (message) =>
              message.type === "discoverProgress" &&
              message.status === "done" &&
              message.count === 0,
          ),
          false,
          `${testCase.code} must not look like a clean no-device scan`,
        );
      }
    } finally {
      offline.offlineCommDiscover = originalDiscover;
    }
  });

  test("same-computer native ADS failure recommends the local router, not UDP or firewall", async () => {
    const offline = offlineCommModule();
    const originalDiscover = offline.offlineCommDiscover;
    const rawFailure =
      "LocalRouterUnavailable: local ADS router/runtime check failed for 127.0.0.1: open installed TcAdsDll.dll: library not found";
    offline.offlineCommDiscover = async () => {
      throw new Error(rawFailure);
    };

    const posted: Record<string, unknown>[] = [];
    const panel = {
      visible: true,
      webview: {
        postMessage: async (message: Record<string, unknown>) => {
          posted.push(message);
          return true;
        },
      },
    } as unknown as vscode.WebviewPanel;
    const tracker = new DiscoveryRequestTracker<vscode.WebviewPanel>();
    const token = tracker.start(panel);

    try {
      await runNetworkCanvasDiscovery(
        {
          sessionId: "ads-local-router-failure",
          requestId: 92,
          request: {
            origin: "this_host",
            items: [{ protocol: "ads" }],
          },
        },
        {
          panel,
          extensionContext: testExtensionContext(),
          tracker,
          token,
        },
      );
    } finally {
      offline.offlineCommDiscover = originalDiscover;
    }

    const result = posted[posted.length - 1];
    assert.strictEqual(result?.type, "discoverResults");
    assert.strictEqual(result?.errorCode, "ads_local_router_unavailable");
    assert.match(String(result?.error), /local ADS runtime|ADS router/i);
    assert.doesNotMatch(
      String(result?.error),
      /UDP|firewall|static route|Advanced/i,
    );
    assert.deepStrictEqual(result?.errorDetails, [rawFailure]);
    assert.deepStrictEqual(result?.candidates, []);
  });

  test("partial automatic ADS failures keep raw evidence only in Technical details", async () => {
    const offline = offlineCommModule();
    const originalDiscover = offline.offlineCommDiscover;
    const rawFailure =
      "UdpIdentifyBlocked: receiving UDP reply failed (os error 10060); local router closed (os error 10054); ads-wire unavailable";
    let invocation = 0;
    offline.offlineCommDiscover = async () => {
      invocation += 1;
      if (invocation === 1) {
        return {
          protocol: "ads",
          candidates: [
            {
              id: "ads:partial",
              label: "PLC workstation",
              source: "ads_broadcast",
              confidence: "observed",
              protocol: "ads",
              params: {
                host: "192.168.50.42",
                ams_net_id: "10.20.30.40.1.1",
                ams_port: 851,
              },
            },
          ],
          warnings: [],
        };
      }
      throw new Error(rawFailure);
    };

    const posted: Record<string, unknown>[] = [];
    const panel = {
      visible: true,
      webview: {
        postMessage: async (message: Record<string, unknown>) => {
          posted.push(message);
          return true;
        },
      },
    } as unknown as vscode.WebviewPanel;
    const tracker = new DiscoveryRequestTracker<vscode.WebviewPanel>();
    const token = tracker.start(panel);

    try {
      await runNetworkCanvasDiscovery(
        {
          sessionId: "ads-partial-failure",
          requestId: 91,
          request: {
            origin: "this_host",
            items: [
              { protocol: "ads" },
              { protocol: "ads", host: "192.168.50.42" },
            ],
          },
        },
        {
          panel,
          extensionContext: testExtensionContext(),
          tracker,
          token,
        },
      );
    } finally {
      offline.offlineCommDiscover = originalDiscover;
    }

    const result = posted[posted.length - 1];
    assert.strictEqual(
      result?.warning,
      "Some ADS checks did not answer. Results from responding devices are shown.",
    );
    assert.doesNotMatch(
      String(result?.warning),
      /UDP|router|10060|10054|ads-wire/i,
    );
    assert.deepStrictEqual(result?.warningDetails, [rawFailure]);
    assert.strictEqual(result?.error, undefined);
    const pane = readSource("networkCanvas/webview/DiscoverPane.tsx");
    assert.ok(
      pane.includes('data-role="discovery-warning-technical"') &&
        pane.includes("<summary>Technical details</summary>"),
      "raw partial-failure evidence must render only under the collapsed Technical details disclosure",
    );
  });

  test("Devices uses the active simulator session credentials instead of deriving another workspace target", () => {
    assert.deepStrictEqual(
      simulatorControlFromDebugConfiguration({
        request: "launch",
        controlEndpoint: " tcp://127.0.0.1:23001 ",
        controlAuthToken: " session-secret ",
      }),
      {
        endpoint: "tcp://127.0.0.1:23001",
        authToken: "session-secret",
      },
    );
    assert.strictEqual(
      simulatorControlFromDebugConfiguration({
        request: "attach",
        controlEndpoint: "tcp://127.0.0.1:23001",
        controlAuthToken: "session-secret",
      }),
      undefined,
      "remote attach credentials must not be mistaken for the local simulator",
    );

    const panelSource = readSource("networkCanvas/networkCanvasPanel.ts");
    const resolutionSource = readSource(
      "networkCanvas/runtimeTargetResolution.ts",
    );
    assert.ok(
      panelSource.includes("resolveNetworkCanvasRuntimeTarget(") &&
        panelSource.includes(
          "runtimeLifecycleService.acceptedDebugSession()?.configuration",
        ) &&
        resolutionSource.includes("simulatorControlFromDebugConfiguration(") &&
        resolutionSource.includes("endpoint: simulatorControl.endpoint") &&
        resolutionSource.includes("authToken: simulatorControl.authToken"),
      "the canvas must read the exact endpoint/token from the active debug session",
    );
    assert.ok(
      !panelSource.includes(
        "localSimControl(vscode.workspace.workspaceFolders?.[0]",
      ),
      "the canvas must not derive simulator credentials from the first workspace folder",
    );
  });
});
