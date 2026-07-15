import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";

import { runNetworkCanvasDiscovery } from "../../networkCanvas/discoveryController";
import { DiscoveryRequestTracker } from "../../networkCanvas/discoverySession";
import { DiscoveryOriginTargetStore } from "../../networkCanvas/discoveryOriginTargets";
import { isCurrentAdsServiceProbeRequest } from "../../networkCanvas/adsServiceProbeController";
import { planBrowseOpen } from "../../networkCanvas/webview/browseSessionModel";
import { localSimControl } from "../../simControl";
import type { RuntimeTarget } from "../../runtimeTarget";
import type {
  BrowseSymbolsResponse,
  DiscoverResponse,
} from "../../networkCanvas/offlineComm";

type AdsDiscoveryLocationId =
  | "same_computer"
  | "local_network"
  | "known_address";

interface AdsDiscoveryModelContract {
  readonly ADS_DISCOVERY_LOCATIONS: readonly {
    readonly id: AdsDiscoveryLocationId;
    readonly label: string;
    readonly recommended: boolean;
  }[];
  readonly PLC_RUNTIME_PORTS: readonly number[];
  readonly COMMON_TWINCAT_SERVICE_PORTS: readonly number[];
  readonly AUTOMATIC_TWINCAT_SERVICE_PORTS: readonly number[];
  adsDiscoveryFields(
    location: AdsDiscoveryLocationId,
    advanced: boolean
  ): readonly string[];
  validateAdsDiscoveryHost(host: string): string | undefined;
  validateAdsAmsNetId(value: string): string | undefined;
  autoSelectTwinCatServicePort(availablePorts: readonly number[]): number | undefined;
  twinCatServicePresentation(port: number): {
    readonly primary: string;
    readonly secondary: string;
  };
  createAdsDiscoveryScanSnapshot(
    origin: string,
    draft: {
      readonly location: AdsDiscoveryLocationId;
      readonly host: string;
      readonly amsNetId: string;
      readonly customPorts: string;
      readonly advanced: boolean;
    }
  ): {
    readonly origin: string;
    readonly location: AdsDiscoveryLocationId;
    readonly host?: string;
    readonly targetAmsNetId?: string;
    readonly ports: readonly number[];
  };
}

interface OfflineCommModule {
  offlineCommDiscover(
    context: vscode.ExtensionContext,
    protocol: string,
    origin: string,
    scope?: { cidr?: string; host?: string; timeoutMs?: number }
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
  readonly COMMON_TWINCAT_SERVICE_PORTS: readonly number[];
  readonly AUTOMATIC_TWINCAT_SERVICE_PORTS: readonly number[];
  readonly MAX_ADS_SERVICE_PROBES: number;
  planAdsServicePorts(customPorts: readonly number[]): readonly number[];
  parseCustomAdsPorts(input: string): {
    readonly ports: readonly number[];
    readonly error?: string;
  };
  classifyAdsServiceProbe(
    port: number,
    response: BrowseSymbolsResponse
  ): AdsServiceProbeResult;
  autoSelectUsableAdsService(
    results: readonly AdsServiceProbeResult[]
  ): number | undefined;
  probeAdsServicesSequentially(
    ports: readonly number[],
    probe: (port: number) => Promise<BrowseSymbolsResponse>
  ): Promise<readonly AdsServiceProbeResult[]>;
}

function extensionRoot(): string {
  return path.resolve(__dirname, "../../..");
}

function readSource(relativePath: string): string {
  return fs.readFileSync(path.join(extensionRoot(), "src", relativePath), "utf8");
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
  assert.ok(match, "Windows TCP runtime control must include a non-empty auth_token.");
  return match[1];
}

suite("Windows ADS discovery and simulator regression contract", function () {
  test("offers one TwinCAT flow with explicit target-location choices", () => {
    const model = adsDiscoveryModel();

    assert.deepStrictEqual(model.ADS_DISCOVERY_LOCATIONS, [
      {
        id: "same_computer",
        label: "On the discovery computer",
        recommended: false,
      },
      {
        id: "local_network",
        label: "On the discovery computer's network",
        recommended: true,
      },
      {
        id: "known_address",
        label: "At known address",
        recommended: false,
      },
    ]);

    const pane = readSource("networkCanvas/webview/DiscoverPane.tsx");
    const adsFlow = readSource("networkCanvas/webview/AdsDiscoveryFlow.tsx");
    const discoveryUi = `${pane}\n${adsFlow}`;
    const rowDefinitions = pane.slice(0, pane.indexOf("export function DiscoverPane"));
    assert.strictEqual(
      (rowDefinitions.match(/protocol:\s*"ads"/g) ?? []).length,
      1,
      "Discover must render one TwinCAT flow, not separate broadcast and targeted ADS checkboxes."
    );
    assert.ok(!rowDefinitions.includes('key: "ads-host"'));
    assert.ok(
      discoveryUi.includes("Discovery runs from"),
      "The execution origin must be named separately from the TwinCAT target location."
    );
    assert.ok(
      discoveryUi.includes("Where is TwinCAT?"),
      "The target-location choice must not be presented as another scan origin."
    );
    assert.ok(discoveryUi.includes("Find TwinCAT"));
    assert.ok(discoveryUi.includes("Finding TwinCAT…"));
    assert.ok(discoveryUi.includes("selected type"));
    assert.ok(discoveryUi.includes("ads-find-twincat"));
    assert.ok(discoveryUi.includes('row.status === "failed"'));
    assert.match(
      discoveryUi,
      /!error[\s\S]*row\.status === "failed"[\s\S]*results\.length === 0/,
      "A failed discovery must not also render the Nothing found state."
    );
  });

  test("progressively discloses known-address and advanced ADS identity fields", () => {
    const model = adsDiscoveryModel();

    assert.deepStrictEqual(model.adsDiscoveryFields("same_computer", false), []);
    assert.deepStrictEqual(model.adsDiscoveryFields("local_network", false), []);
    assert.deepStrictEqual(model.adsDiscoveryFields("same_computer", true), ["ads_port"]);
    assert.deepStrictEqual(model.adsDiscoveryFields("local_network", true), ["ads_port"]);
    assert.deepStrictEqual(model.adsDiscoveryFields("known_address", false), ["host"]);
    assert.deepStrictEqual(model.adsDiscoveryFields("known_address", true), [
      "host",
      "ams_net_id",
      "ads_port",
    ]);

    assert.strictEqual(model.validateAdsDiscoveryHost("192.168.77.11"), undefined);
    assert.strictEqual(model.validateAdsDiscoveryHost("plc-line-1"), undefined);
    assert.match(
      model.validateAdsDiscoveryHost("192.168.77.11:851") ?? "",
      /host.*without.*port|do not include.*port/i,
      "ADS Host/IP must reject host:port inline instead of running a doomed scan."
    );
    assert.strictEqual(model.validateAdsAmsNetId("100.67.6.217.1.1"), undefined);
    assert.strictEqual(model.validateAdsAmsNetId(""), undefined);
    assert.match(model.validateAdsAmsNetId("100.67.6.999.1.1") ?? "", /six.*0.*255/i);
    assert.match(model.validateAdsAmsNetId("100.67.6.1.1") ?? "", /six.*0.*255/i);
    const adsFlow = readSource("networkCanvas/webview/AdsDiscoveryFlow.tsx");
    assert.ok(adsFlow.includes("Advanced settings need attention"));
    assert.ok(adsFlow.includes('data-role="ads-advanced-attention"'));
    assert.ok(adsFlow.includes("Expand"));
  });

  test("snapshots discovery origin and service ports at Scan click", () => {
    const model = adsDiscoveryModel();
    const snapshot = model.createAdsDiscoveryScanSnapshot("runtime:a", {
      location: "known_address",
      host: "192.168.77.11",
      amsNetId: "100.67.6.217.1.1",
      customPorts: "9000, 9001",
      advanced: true,
    });

    assert.deepStrictEqual(snapshot, {
      origin: "runtime:a",
      location: "known_address",
      host: "192.168.77.11",
      targetAmsNetId: "100.67.6.217.1.1",
      ports: [851, 852, 853, 854, 301, 501, 9000, 9001],
    });
    assert.strictEqual(snapshot.origin, "runtime:a");
    assert.deepStrictEqual(snapshot.ports, [851, 852, 853, 854, 301, 501, 9000, 9001]);

    const collapsed = model.createAdsDiscoveryScanSnapshot("runtime:a", {
      location: "known_address",
      host: "192.168.77.11",
      amsNetId: "100.67.6.217.1.1",
      customPorts: "9000",
      advanced: false,
    });
    assert.strictEqual(
      collapsed.targetAmsNetId,
      "100.67.6.217.1.1",
      "Collapsing Advanced must not silently discard a persisted manual identity."
    );
    assert.deepStrictEqual(collapsed.ports, [851, 852, 853, 854, 301, 501, 9000]);

    const pane = `${readSource("networkCanvas/webview/DiscoverPane.tsx")}\n${readSource(
      "networkCanvas/webview/AdsDiscoveryFlow.tsx"
    )}`;
    assert.ok(pane.includes("createAdsDiscoveryScanSnapshot"));
    assert.ok(pane.includes("adsScanSnapshot"));
    assert.ok(pane.includes("Advanced settings applied"));
  });

  test("pins the exact discovery runtime and rejects stale or mismatched probes", () => {
    const runtimeA: RuntimeTarget = {
      mode: "online",
      endpoint: "tcp://runtime-a:9901",
      endpointEnabled: true,
      reachable: true,
      status: "online_reachable",
      label: "Runtime A",
      credentialChannel: "untrusted_remote_plain_tcp",
    };
    const runtimeB: RuntimeTarget = {
      ...runtimeA,
      endpoint: "tcp://runtime-b:9901",
      label: "Runtime B",
    };
    const store = new DiscoveryOriginTargetStore();
    store.pin("runtime:a", runtimeA);
    const currentGlobalTarget = runtimeB;
    assert.strictEqual(currentGlobalTarget, runtimeB);
    assert.strictEqual(
      store.resolve("runtime:a"),
      runtimeA,
      "Probe and browse must resolve the immutable discovery origin, not current Runtime B."
    );

    const active = {
      sessionId: "session-a",
      requestId: 4,
      origin: "runtime:a",
    };
    const valid = {
      sessionId: "session-a",
      requestId: 4,
      origin: "runtime:a",
      candidate: { originRuntimeId: "runtime:a" },
    };
    assert.strictEqual(
      isCurrentAdsServiceProbeRequest(valid, active, "session-a"),
      true
    );
    assert.strictEqual(
      isCurrentAdsServiceProbeRequest(
        { ...valid, origin: undefined },
        active,
        "session-a"
      ),
      false,
      "A probe without an explicit origin must never fall back to this_host."
    );
    assert.strictEqual(
      isCurrentAdsServiceProbeRequest(
        { ...valid, candidate: { originRuntimeId: "runtime:b" } },
        active,
        "session-a"
      ),
      false
    );
    assert.strictEqual(
      isCurrentAdsServiceProbeRequest(
        { ...valid, requestId: 3 },
        active,
        "session-a"
      ),
      false
    );
    assert.strictEqual(
      isCurrentAdsServiceProbeRequest(valid, undefined, "session-a"),
      false,
      "A cancelled discovery must not perform probe network I/O."
    );

    store.clear();
    assert.strictEqual(store.resolve("runtime:a"), undefined);

    const controller = readSource("networkCanvas/adsServiceProbeController.ts");
    const browse = readSource("networkCanvas/protocolActions.ts");
    const panel = readSource("networkCanvas/networkCanvasPanel.ts");
    for (const source of [controller, browse]) {
      assert.ok(source.includes("runtimeTargetForOrigin"));
      assert.ok(
        source.toLowerCase().includes(
          "selected discovery runtime is no longer reachable"
        )
      );
    }
    assert.ok(panel.includes("isCurrentAdsServiceProbeRequest"));
    assert.ok(panel.includes("clearDiscoveryOriginContext()"));
    assert.ok(
      readSource("networkCanvas/discoveryOriginContext.ts").includes(
        "this.targets.clear()"
      )
    );
  });

  test("offers bounded TwinCAT service checks and auto-selects only an unambiguous result", () => {
    const model = adsDiscoveryModel();

    assert.deepStrictEqual(model.PLC_RUNTIME_PORTS, [851, 852, 853, 854]);
    assert.deepStrictEqual(model.COMMON_TWINCAT_SERVICE_PORTS, [301, 501]);
    assert.deepStrictEqual(model.AUTOMATIC_TWINCAT_SERVICE_PORTS, [
      851,
      852,
      853,
      854,
      301,
      501,
    ]);
    assert.strictEqual(model.autoSelectTwinCatServicePort([]), undefined);
    assert.strictEqual(model.autoSelectTwinCatServicePort([852]), 852);
    assert.strictEqual(
      model.autoSelectTwinCatServicePort([851, 852]),
      undefined,
      "Multiple available PLC runtimes require an explicit user choice."
    );
    assert.deepStrictEqual(model.twinCatServicePresentation(851), {
      primary: "PLC runtime 1",
      secondary: "ADS 851",
    });
    assert.deepStrictEqual(model.twinCatServicePresentation(854), {
      primary: "PLC runtime 4",
      secondary: "ADS 854",
    });
    assert.deepStrictEqual(model.twinCatServicePresentation(9000), {
      primary: "Custom service",
      secondary: "ADS 9000",
    });
    assert.deepStrictEqual(model.twinCatServicePresentation(301), {
      primary: "Additional task 1",
      secondary: "ADS 301",
    });
    assert.deepStrictEqual(model.twinCatServicePresentation(501), {
      primary: "NC SAF service",
      secondary: "ADS 501",
    });

    const pane = `${readSource("networkCanvas/webview/DiscoverPane.tsx")}\n${readSource(
      "networkCanvas/webview/AdsDiscoveryFlow.tsx"
    )}`;
    assert.ok(
      pane.includes("AUTOMATIC_TWINCAT_SERVICE_PORTS"),
      "The confirmed service check must include standard services 851-854, 301, and 501."
    );
    assert.ok(pane.includes('data-role="ads-probe-safety-confirmation"'));
    assert.ok(pane.includes('data-role="ads-check-services"'));
    assert.ok(!pane.includes("requestedAdsProbeIds"));
    assert.ok(
      pane.includes("Advanced"),
      "Custom ADS port and manual AMS identity must stay behind progressive disclosure."
    );
    for (const role of [
      "ads-location",
      "ads-host",
      "ads-advanced-toggle",
      "ads-custom-ports",
      "ads-computer",
      "ads-identity-status",
      "ads-probe-safety",
      "ads-probe-safety-confirmation",
      "ads-check-services",
      "ads-plc-runtime",
      "ads-browse-variables",
    ]) {
      assert.ok(pane.includes(`data-role=\"${role}\"`), `Missing CDP role ${role}.`);
    }
    assert.ok(pane.includes("Entered manually — identity not verified yet"));
    assert.ok(pane.includes('case "ads_local_router"'));
    assert.ok(pane.includes('return "Local AMS router"'));
  });

  test("Browse variables immediately browses the confirmed PLC runtime", () => {
    const plan = planBrowseOpen(
      "ads",
      {
        host: "192.168.77.11",
        ams_net_id: "100.67.6.217.1.1",
        ams_port: 851,
        ads_port_confirmed: true,
      },
      "TWINCAT-LAPTOP"
    );

    assert.ok(plan);
    assert.strictEqual(plan.loading, true);
    assert.strictEqual(plan.request?.protocol, "ads");
    assert.strictEqual(plan.request?.kind, "symbols");
    assert.strictEqual(plan.request?.target.ams_port, 851);
  });

  test("plans a bounded ordered ADS service probe without changing host discovery", () => {
    const probes = adsServiceProbeModel();
    assert.deepStrictEqual(probes.PLC_RUNTIME_PORTS, [851, 852, 853, 854]);
    assert.deepStrictEqual(probes.COMMON_TWINCAT_SERVICE_PORTS, [301, 501]);
    assert.deepStrictEqual(probes.AUTOMATIC_TWINCAT_SERVICE_PORTS, [
      851,
      852,
      853,
      854,
      301,
      501,
    ]);
    assert.strictEqual(probes.MAX_ADS_SERVICE_PROBES, 10);
    assert.deepStrictEqual(
      probes.planAdsServicePorts([854, 9000, 851, 9001, 9000, 9002, 9003, 9004]),
      [851, 852, 853, 854, 301, 501, 9000, 9001, 9002, 9003],
      "Preset TwinCAT services come first; custom ports preserve order, dedupe, and respect the cap."
    );
    assert.deepStrictEqual(probes.parseCustomAdsPorts("9000, 852, 9000"), {
      ports: [9000, 852],
    });
    assert.deepStrictEqual(probes.planAdsServicePorts([301, 501]), [
      851,
      852,
      853,
      854,
      301,
      501,
    ]);
    assert.match(probes.parseCustomAdsPorts("9000, 70000").error ?? "", /1.*65535/);
    assert.match(probes.parseCustomAdsPorts("9000, plc").error ?? "", /whole number/i);
    assert.match(
      probes.parseCustomAdsPorts("9000, 9001, 9002, 9003, 9004").error ?? "",
      /up to 4 additional.*10 total/i
    );

    const discover = readSource("networkCanvas/offlineComm.ts");
    const discoverFunction = discover.slice(
      discover.indexOf("export async function offlineCommDiscover"),
      discover.indexOf("export async function offlineFleetRuntimeAdd")
    );
    assert.ok(!discoverFunction.includes("ams_port"));
    assert.ok(!discoverFunction.includes("AUTOMATIC_TWINCAT_SERVICE_PORTS"));

    const controller = readSource("networkCanvas/adsServiceProbeController.ts");
    assert.ok(
      controller.includes("offlineBrowseSymbols"),
      "PLC runtime probes must reuse structured comm browse-symbols."
    );
    assert.ok(
      !controller.includes("offlineCommDiscover"),
      "Host identity discovery must remain separate from ADS service probing."
    );
  });

  test("classifies ADS service probe outcomes and selects only one usable runtime", () => {
    const probes = adsServiceProbeModel();
    const available = probes.classifyAdsServiceProbe(851, {
      protocol: "ads",
      tree: [
        { id: "a", name: "A", path: "GVL.A" },
        { id: "b", name: "B", path: "GVL.B" },
      ],
    });
    const unsupported = probes.classifyAdsServiceProbe(301, {
      protocol: "ads",
      tree: [],
      error: { code: "symbol_upload_unsupported", message: "not supported" },
    });
    const empty = probes.classifyAdsServiceProbe(852, {
      protocol: "ads",
      tree: [],
      error: { code: "empty_symbol_table", message: "no symbols" },
    });
    const unavailable = probes.classifyAdsServiceProbe(853, {
      protocol: "ads",
      tree: [],
      error: { code: "ads_port_unavailable", message: "target port not found" },
    });
    const routeMissing = probes.classifyAdsServiceProbe(854, {
      protocol: "ads",
      route: { status: "missing" },
      tree: [],
    });
    const checkFailed = probes.classifyAdsServiceProbe(9000, {
      protocol: "ads",
      tree: [],
      error: { code: "symbol_upload_failed", message: "invalid AMS Net ID" },
    });

    assert.deepStrictEqual(available, {
      port: 851,
      status: "available",
      symbolCount: 2,
      usable: true,
    });
    assert.deepStrictEqual(unsupported, {
      port: 301,
      status: "unsupported",
      symbolCount: 0,
      usable: false,
      error: { code: "symbol_upload_unsupported", message: "not supported" },
    });
    assert.deepStrictEqual(empty, {
      port: 852,
      status: "empty",
      symbolCount: 0,
      usable: false,
      error: { code: "empty_symbol_table", message: "no symbols" },
    });
    assert.deepStrictEqual(unavailable, {
      port: 853,
      status: "unavailable",
      symbolCount: 0,
      usable: false,
      error: { code: "ads_port_unavailable", message: "target port not found" },
    });
    assert.deepStrictEqual(routeMissing, {
      port: 854,
      status: "route_missing",
      symbolCount: 0,
      usable: false,
    });
    assert.deepStrictEqual(checkFailed, {
      port: 9000,
      status: "check_failed",
      symbolCount: 0,
      usable: false,
      error: { code: "symbol_upload_failed", message: "invalid AMS Net ID" },
    });
    assert.strictEqual(probes.autoSelectUsableAdsService([available]), 851);
    assert.strictEqual(
      probes.autoSelectUsableAdsService([available, { ...available, port: 852 }]),
      undefined
    );
    assert.strictEqual(
      probes.autoSelectUsableAdsService([
        unsupported,
        empty,
        unavailable,
        routeMissing,
        checkFailed,
      ]),
      undefined
    );
  });

  test("probes ADS services sequentially in the planned order", async () => {
    const probes = adsServiceProbeModel();
    const calls: number[] = [];
    let active = 0;
    let peakActive = 0;

    const results = await probes.probeAdsServicesSequentially(
      [851, 852, 853],
      async (port) => {
        calls.push(port);
        active += 1;
        peakActive = Math.max(peakActive, active);
        await new Promise((resolve) => setTimeout(resolve, 1));
        active -= 1;
        return port === 851
          ? {
              protocol: "ads",
              tree: [{ id: "x", name: "X", path: "GVL.X" }],
            }
          : {
              protocol: "ads",
              tree: [],
              error: {
                code: "ads_port_unavailable",
                message: "target port not found",
              },
            };
      }
    );

    assert.deepStrictEqual(calls, [851, 852, 853]);
    assert.strictEqual(peakActive, 1, "Do not fan out ADS symbol uploads in parallel.");
    assert.deepStrictEqual(results.map((result) => result.port), [851, 852, 853]);
    assert.deepStrictEqual(results.map((result) => result.status), [
      "available",
      "unavailable",
      "unavailable",
    ]);
  });

  test("stops service fanout when route setup is required", async () => {
    const probes = adsServiceProbeModel();
    const calls: number[] = [];
    const results = await probes.probeAdsServicesSequentially(
      [851, 852, 853, 854],
      async (port) => {
        calls.push(port);
        return port === 852
          ? { protocol: "ads", route: { status: "missing" }, tree: [] }
          : { protocol: "ads", tree: [] };
      }
    );

    assert.deepStrictEqual(calls, [851, 852]);
    assert.deepStrictEqual(results.map((result) => result.status), [
      "empty",
      "route_missing",
    ]);
  });

  test("offline discovery rejects with the runtime stderr instead of returning no candidates", async () => {
    const childProcess = require("child_process") as MutableChildProcessModule;
    const originalExecFile = childProcess.execFile;
    const stderr = "ADS host must be an IP or hostname without a port";
    childProcess.execFile = (...args: unknown[]): unknown => {
      const callback = args[args.length - 1];
      assert.strictEqual(typeof callback, "function");
      (callback as (error: Error, stdout: string, stderr: string) => void)(
        new Error("command failed"),
        "",
        stderr
      );
      return undefined;
    };

    try {
      await assert.rejects(
        offlineCommModule().offlineCommDiscover(
          testExtensionContext(),
          "ads",
          "this-host",
          { host: "127.0.0.1:851" }
        ),
        (error: unknown) =>
          error instanceof Error && error.message.includes(stderr),
        "The extension must preserve actionable stderr from trust-runtime."
      );
    } finally {
      childProcess.execFile = originalExecFile;
    }
  });

  test("controller classifies UDP Identify no-reply as a recoverable error and never converts it into 0 found", async () => {
    const offline = offlineCommModule();
    const originalDiscover = offline.offlineCommDiscover;
    const stderr =
      "ADS discovery failed: UdpIdentifyBlocked: ADS UDP identify failed for 192.168.77.11: no target answered";
    offline.offlineCommDiscover = async () => {
      throw new Error(stderr);
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
          sessionId: "ads-error-session",
          requestId: 7,
          request: {
            origin: "this_host",
            items: [{ protocol: "ads", host: "192.168.77.11" }],
          },
        },
        {
          panel,
          extensionContext: testExtensionContext(),
          tracker,
          token,
        }
      );
    } finally {
      offline.offlineCommDiscover = originalDiscover;
    }

    const failure = posted.find(
      (message) => message.type === "discoverResults" && typeof message.error === "string"
    );
    assert.ok(failure, "A CLI discovery failure must reach the visible error state.");
    assert.strictEqual(
      failure.error,
      "TwinCAT identity did not answer UDP discovery. Enter the target AMS Net ID to continue manually."
    );
    assert.ok(
      !String(failure.error).includes("UdpIdentifyBlocked") &&
        !String(failure.error).includes("no target answered"),
      "Typed wire details must select recovery without leaking backend vocabulary into the UI."
    );
    assert.strictEqual(failure.errorCode, "ads_udp_identify_blocked");
    assert.deepStrictEqual(failure.candidates, []);
    assert.strictEqual(
      posted.some(
        (message) =>
          message.type === "discoverProgress" &&
          message.status === "done" &&
          message.count === 0
      ),
      false,
      "A failed scan must never be represented as a successful scan with zero results."
    );
  });

  test("Windows project scaffold generates a fresh token for its TCP control endpoint", () => {
    const newProject = require("../../newProject") as NewProjectModuleContract;
    assert.strictEqual(
      typeof newProject.buildRuntimeTomlSource,
      "function",
      "newProject must expose the platform-aware runtime.toml builder used by the scaffold."
    );
    const buildRuntimeTomlSource = newProject.buildRuntimeTomlSource;
    assert.ok(buildRuntimeTomlSource);

    const first = buildRuntimeTomlSource("win32");
    const second = buildRuntimeTomlSource("win32");
    assert.match(first, /^endpoint\s*=\s*"tcp:\/\/127\.0\.0\.1:\d+"\s*$/m);
    const firstToken = extractControlToken(first);
    const secondToken = extractControlToken(second);
    assert.ok(firstToken.length >= 24, "Generated control tokens must have useful entropy.");
    assert.notStrictEqual(
      firstToken,
      secondToken,
      "Each newly scaffolded Windows project needs its own control token."
    );

    const source = readSource("newProject.ts");
    assert.ok(
      source.includes("buildRuntimeTomlSource(process.platform)"),
      "The New Project command must write the platform-aware generated source, not a stale constant."
    );
  });

  test("localSimControl returns stable per-workspace TCP credentials on win32", () => {
    const descriptor = Object.getOwnPropertyDescriptor(process, "platform");
    assert.ok(descriptor?.configurable, "The test requires a restorable process.platform seam.");
    Object.defineProperty(process, "platform", { ...descriptor, value: "win32" });

    try {
      const first = localSimControl("C:\\projects\\line-one");
      const repeated = localSimControl("C:\\projects\\line-one");
      const other = localSimControl("C:\\projects\\line-two");

      assert.ok(first, "Windows must have a managed local simulator control channel.");
      assert.match(first.endpoint, /^tcp:\/\/127\.0\.0\.1:(\d+)$/);
      assert.ok(first.authToken.length >= 24);
      assert.deepStrictEqual(repeated, first, "A workspace must reuse one endpoint/token pair.");
      assert.ok(other);
      assert.notStrictEqual(other.endpoint, first.endpoint);
      assert.notStrictEqual(other.authToken, first.authToken);
    } finally {
      Object.defineProperty(process, "platform", descriptor);
    }
  });
});
