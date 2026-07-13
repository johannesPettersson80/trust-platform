import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";

import {
  buildNetworkCanvasModel,
  type BuildNetworkCanvasModelInput,
} from "../../networkCanvas/model";
import {
  mergeFleetTopologies,
  offlineTopologyForTarget,
  type FleetTopologyResponse,
} from "../../networkCanvas/fleetTopology";
import { ensureAdsRuntimeEnabled } from "../../networkCanvas/offlineComm";
import { mergeConnectorStatusIntoTopology } from "../../networkCanvas/connectorsStatus";
import { buildCanvasGraph } from "../../networkCanvas/graphData";
import { initialNetworkCanvasGraph } from "../../networkCanvas/initialGraph";
import { buildGraph } from "../../networkCanvas/webview/layout";
import {
  LOCAL_RUNTIME_NODE_ID,
  type EndpointNodeData,
} from "../../networkCanvas/webview/types";
import type { RuntimeTarget } from "../../runtimeTarget";
import {
  classifyRuntimeStartFailure,
  runtimeStatusCheckFailure,
  simulatorStartupIncompleteFailure,
} from "../../networkCanvas/runtimeFailures";
import { runtimeNodeControls } from "../../networkCanvas/webview/runtimeNodeControls";
import { simulatorLifecycleLabel } from "../../networkCanvas/webview/simulatorLifecyclePresentation";
import {
  ADD_PICKER_GROUPS,
  groupForAddPicker,
} from "../../networkCanvas/webview/grouping";
import { applyFilter, filterReport } from "../../networkCanvas/webview/filter";
import { buildExposeApplyParams } from "../../networkCanvas/exposeConfig";
import { commTestMessage } from "../../communication/runtimeComm";
import {
  connectorConnectionLabel,
  connectorHealthLabel,
  connectorSignalsSummary,
  discoveryConfidenceLabel,
  discoverySourceLabel,
} from "../../networkCanvas/webview/connectorPresentation";
import {
  validateSchemaValues,
  visibleSchemaFields,
  type CommProtocolSchema,
} from "../../communication/schemaForm";
import {
  protocolColor,
  protocolName,
} from "../../networkCanvas/webview/protocolMeta";
import {
  formatExposedGlobals,
  serverEndpointSummaryRows,
} from "../../networkCanvas/webview/serverEndpointSummary";
import {
  headerFaultsForBanner,
  visibleFaultsForValidationState,
} from "../../networkCanvas/webview/faults";

const RUNNING = {
  running: true,
  runtimeState: "running" as const,
  runtimeMode: "simulate" as const,
};

function offlineLocalProjectTopology(): FleetTopologyResponse {
  return {
    schema_version: 4,
    hosts: [
      {
        host_id: "host:offline-project",
        hostname: os.hostname(),
        arch: process.arch,
        os: process.platform,
        ips: ["127.0.0.1"],
        containers: [],
        runtimes: [
          {
            runtime_id: "runtime:offline-project",
            name: "RESOURCE",
            control_endpoint: "tcp://127.0.0.1:9902",
            mode: "stopped",
            cycle_ms: 20,
            health: "configured_policy",
            detail: "Configured in project files; runtime is not running.",
            source: "config",
            endpoints: [
              {
                id: "endpoint:offline-project:ads",
                kind: "service",
                protocol: "ads",
                name: "ADS client",
                role: "client",
                health: "configured_policy",
                detail: "Configured in ads.toml.",
                owned: true,
                supports_test: true,
                source: "config",
              },
            ],
          },
        ],
      },
    ],
    links: [
      {
        id: "link:offline-project:ads",
        from: "runtime:offline-project",
        to: "endpoint:offline-project:ads",
        protocol: "ads",
        direction: "outbound",
        same_host: true,
        status: "configured_policy",
        secure: false,
      },
    ],
    shared: [],
    external: [],
  };
}

suite("Network Canvas lifecycle and failure contracts", function () {
  test("simulator running uses lifecycle vocabulary while remote runtimes keep connection vocabulary", () => {
    assert.strictEqual(
      simulatorLifecycleLabel("connected", "simulate"),
      "Running",
    );
    assert.strictEqual(
      simulatorLifecycleLabel("connected", "online"),
      undefined,
    );
    assert.strictEqual(
      simulatorLifecycleLabel("starting", "simulate"),
      undefined,
    );
  });

  test("first-paint graph is lifecycle-owned before schema/topology resolve", () => {
    const stopped = initialNetworkCanvasGraph("stopped");
    const starting = initialNetworkCanvasGraph("starting");
    const running = initialNetworkCanvasGraph("running");
    const connected = initialNetworkCanvasGraph(
      "connected",
      "welcome",
      "tcp://192.0.2.10:5510",
      {
        kind: "remote",
        endpoint: "tcp://192.0.2.10:5510",
        label: "Cell runtime",
      },
    );
    const remoteConnecting = initialNetworkCanvasGraph(
      "starting",
      "welcome",
      "simulator",
      {
        kind: "remote",
        endpoint: "tcp://192.168.77.20:5510",
        label: "Packaging cell",
      },
    );

    assert.strictEqual(stopped.hosts[0]?.runtimes[0]?.health, "stopped");
    assert.strictEqual(starting.hosts[0]?.runtimes[0]?.health, "starting");
    assert.strictEqual(running.hosts[0]?.runtimes[0]?.health, "connected");
    const connectedRemote = connected.hosts
      .flatMap((host) => host.runtimes)
      .find(
        (runtime) => runtime.controlEndpoint === "tcp://192.0.2.10:5510",
      );
    assert.strictEqual(connectedRemote?.health, "connected");
    assert.strictEqual(connectedRemote?.mode, "remote");
    assert.strictEqual(connectedRemote?.attached, true);
    const firstPaintSimulator = remoteConnecting.hosts
      .flatMap((host) => host.runtimes)
      .find((runtime) => runtime.name === "Simulator");
    const firstPaintRemote = remoteConnecting.hosts
      .flatMap((host) => host.runtimes)
      .find(
        (runtime) => runtime.controlEndpoint === "tcp://192.168.77.20:5510",
      );
    assert.strictEqual(
      firstPaintSimulator?.health,
      "stopped",
      "remote Connect must never paint the Simulator as Starting on first paint",
    );
    assert.strictEqual(firstPaintRemote?.health, "starting");
    assert.strictEqual(firstPaintRemote?.runTarget, true);
    assert.match(
      starting.hosts[0]?.runtimes[0]?.detail ?? "",
      /Starting Simulator/i,
    );
  });

  test("local offline topology projects every Simulator lifecycle state without losing topology", () => {
    const cases: ReadonlyArray<{
      label: string;
      input: BuildNetworkCanvasModelInput;
      health: string;
      detail: RegExp;
      summary: RegExp;
    }> = [
      {
        label: "stopped",
        input: { stage: "runtime_live" },
        health: "stopped",
        detail: /Use Start in the truST sidebar/i,
        summary: /Simulator stopped/i,
      },
      {
        label: "starting",
        input: { stage: "runtime_live", starting: true },
        health: "starting",
        detail: /Starting Simulator/i,
        summary: /Simulator starting/i,
      },
      {
        label: "running",
        input: { stage: "runtime_live", runtime: RUNNING },
        health: "connected",
        detail: /Running/i,
        summary: /endpoint/i,
      },
      {
        label: "error",
        input: {
          stage: "runtime_live",
          failure: {
            kind: "failed_spawn",
            message: "Simulator launch failed acceptance.",
          },
        },
        health: "error",
        detail: /Simulator launch failed acceptance/i,
        summary: /Simulator needs attention/i,
      },
    ];

    for (const testCase of cases) {
      const topology = offlineLocalProjectTopology();
      const model = buildNetworkCanvasModel({ ...testCase.input, topology });
      const graph = buildCanvasGraph(
        {
          ...model,
          faults: [
            ...model.faults,
            {
              id: "fault:project-runtime",
              label: "Project runtime fault",
              targetNodeId: "runtime:offline-project",
              severity: "warning",
            },
            {
              id: "fault:project-endpoint",
              label: "Project endpoint fault",
              targetNodeId: "endpoint:offline-project:ads",
              severity: "warning",
            },
          ],
        },
        topology,
      );
      const runtime = graph.hosts[0]?.runtimes.find(
        (candidate) => candidate.id === LOCAL_RUNTIME_NODE_ID,
      );

      assert.ok(
        runtime,
        `${testCase.label}: project runtime remains in topology`,
      );
      assert.strictEqual(
        runtime.mode,
        "simulate",
        `${testCase.label}: lifecycle mode`,
      );
      assert.strictEqual(
        runtime.health,
        testCase.health,
        `${testCase.label}: lifecycle health`,
      );
      assert.match(
        runtime.detail,
        testCase.detail,
        `${testCase.label}: lifecycle detail`,
      );
      assert.match(
        graph.summary,
        testCase.summary,
        `${testCase.label}: graph summary`,
      );
      assert.deepStrictEqual(
        runtime.endpoints.map((endpoint) => endpoint.id),
        ["endpoint:offline-project:ads"],
        `${testCase.label}: configured endpoints are preserved`,
      );
      assert.deepStrictEqual(
        graph.links.map(({ id, from, to }) => ({ id, from, to })),
        [
          {
            id: "link:offline-project:ads",
            from: LOCAL_RUNTIME_NODE_ID,
            to: "endpoint:offline-project:ads",
          },
        ],
        `${testCase.label}: topology links follow the canonical Simulator identity`,
      );
      const renderedLink = buildGraph(graph).edges.find(
        (edge) => edge.id === "link:offline-project:ads",
      );
      assert.deepStrictEqual(
        renderedLink && {
          id: renderedLink.id,
          source: renderedLink.source,
          target: renderedLink.target,
        },
        {
          id: "link:offline-project:ads",
          source: LOCAL_RUNTIME_NODE_ID,
          target: "endpoint:offline-project:ads",
        },
        `${testCase.label}: canonicalized topology link renders as an edge`,
      );
      assert.strictEqual(
        graph.faults.find((fault) => fault.id === "fault:project-runtime")
          ?.targetNodeId,
        LOCAL_RUNTIME_NODE_ID,
        `${testCase.label}: runtime-targeted faults follow the canonical Simulator identity`,
      );
      assert.strictEqual(
        graph.faults.find((fault) => fault.id === "fault:project-endpoint")
          ?.targetNodeId,
        "endpoint:offline-project:ads",
        `${testCase.label}: unrelated fault targets remain unchanged`,
      );
    }
  });

  test("Simulator lifecycle projection leaves live local and remote config runtimes unchanged", () => {
    const topology = offlineLocalProjectTopology();
    topology.hosts[0].runtimes.push({
      runtime_id: "runtime:local-live",
      name: "Managed local runtime",
      mode: "online",
      cycle_ms: 20,
      health: "degraded",
      detail: "Live runtime owns its own lifecycle.",
      source: "self",
      endpoints: [],
    });
    topology.hosts.push({
      host_id: "host:remote-config",
      hostname: "remote-cell",
      arch: "x64",
      os: "win32",
      ips: ["192.168.50.42"],
      containers: [],
      runtimes: [
        {
          runtime_id: "runtime:remote-config",
          name: "Remote configured runtime",
          mode: "stopped",
          cycle_ms: 20,
          health: "configured_policy",
          detail: "Remote runtime state remains topology-owned.",
          source: "config",
          endpoints: [],
        },
      ],
    });

    const graph = buildCanvasGraph(
      buildNetworkCanvasModel({
        stage: "runtime_live",
        runtime: RUNNING,
        topology,
      }),
      topology,
    );
    const runtimes = graph.hosts.flatMap((host) => host.runtimes);
    const projected = runtimes.find(
      (runtime) => runtime.id === LOCAL_RUNTIME_NODE_ID,
    );
    const liveLocal = runtimes.find(
      (runtime) => runtime.id === "runtime:local-live",
    );
    const remote = runtimes.find(
      (runtime) => runtime.id === "runtime:remote-config",
    );

    assert.strictEqual(projected?.health, "connected");
    assert.strictEqual(
      liveLocal?.health,
      "degraded",
      "self-reported local runtime is unchanged",
    );
    assert.strictEqual(
      liveLocal?.detail,
      "Live runtime owns its own lifecycle.",
    );
    assert.strictEqual(
      remote?.health,
      "stopped",
      "remote config runtime is unchanged",
    );
    assert.strictEqual(
      remote?.detail,
      "Remote runtime state remains topology-owned.",
    );
  });

  test("a runtime start failure renders its classified recovery verbatim", () => {
    const failure = classifyRuntimeStartFailure(
      "EADDRINUSE: address already in use at C:\\private\\runtime.sock",
    );
    const graph = buildCanvasGraph(
      buildNetworkCanvasModel({
        stage: "runtime_live",
        failure,
      }),
      undefined,
    );
    assert.strictEqual(graph.hosts[0].runtimes[0].health, "error");
    assert.ok(graph.banner, "failure surfaces an inline banner");
    assert.strictEqual(graph.banner?.text, failure.message);
    assert.strictEqual(
      JSON.stringify(graph).includes("C:\\private\\runtime.sock"),
      false,
    );
    assert.deepStrictEqual(graph.banner?.actions, [
      { label: "Open logs", action: "openRuntimeLogs" },
    ]);
    assert.deepStrictEqual(graph.banner?.representedFaultIds, [
      `runtime:${failure.kind}`,
    ]);
    assert.deepStrictEqual(
      headerFaultsForBanner(graph.faults, graph.banner),
      [],
      "the actionable recovery banner must replace the same runtime issue in the header",
    );
    assert.strictEqual(
      graph.faults.filter((fault) => fault.id === `runtime:${failure.kind}`)
        .length,
      1,
      "the graph must carry one canonical runtime failure",
    );
  });

  test("a configured topology preserves the Simulator failure recovery banner", () => {
    const failure = classifyRuntimeStartFailure(
      "debug adapter exited before DAP initialize",
    );
    const topology = offlineLocalProjectTopology();
    const graph = buildCanvasGraph(
      buildNetworkCanvasModel({
        stage: "runtime_live",
        failure,
        topology,
      }),
      topology,
    );

    assert.strictEqual(
      graph.hosts
        .flatMap((host) => host.runtimes)
        .find((runtime) => runtime.id === LOCAL_RUNTIME_NODE_ID)?.health,
      "error",
    );
    assert.strictEqual(graph.banner?.text, failure.message);
    assert.deepStrictEqual(graph.banner?.actions, [
      { label: "Open logs", action: "openRuntimeLogs" },
    ]);
    assert.deepStrictEqual(graph.banner?.representedFaultIds, [
      `runtime:${failure.kind}`,
    ]);
    assert.deepStrictEqual(
      headerFaultsForBanner(graph.faults, graph.banner),
      [],
      "configured topology must not repeat the banner-owned runtime fault in the header",
    );
  });

  test("the actionable Simulator banner replaces only its duplicate header fault", () => {
    const message =
      "Simulator could not start. The logs show what blocked startup.";
    const banner = {
      kind: "error" as const,
      text: message,
      representedFaultIds: ["runtime:stale_runtime"],
      actions: [{ label: "Open logs", action: "openRuntimeLogs" }],
    };
    const unrelated = {
      id: "fault:field-device",
      label: "Field device needs attention",
      targetNodeId: "endpoint:ads-line",
      severity: "warning" as const,
    };
    const distinctSimulatorFault = {
      id: "fault:simulator-different",
      label: "Simulator: A configured endpoint also needs attention",
      targetNodeId: LOCAL_RUNTIME_NODE_ID,
      severity: "warning" as const,
    };
    const faults = [
      {
        id: "runtime:stale_runtime",
        label: message,
        targetNodeId: LOCAL_RUNTIME_NODE_ID,
        severity: "error" as const,
      },
      {
        id: "fault:project-runtime",
        label: `Simulator: ${message}`,
        targetNodeId: LOCAL_RUNTIME_NODE_ID,
        severity: "error" as const,
      },
      unrelated,
      distinctSimulatorFault,
    ];

    assert.deepStrictEqual(headerFaultsForBanner(faults, banner), [
      faults[1],
      unrelated,
      distinctSimulatorFault,
    ]);
    assert.deepStrictEqual(
      headerFaultsForBanner(faults, { ...banner, kind: "info" }),
      faults,
      "neutral guidance must not hide real faults",
    );
    assert.deepStrictEqual(
      headerFaultsForBanner(
        [
          {
            ...faults[0],
            id: "runtime:another-fault-id",
          },
        ],
        banner,
      ),
      [
        {
          ...faults[0],
          id: "runtime:another-fault-id",
        },
      ],
      "the same words under another fault identity remain visible",
    );

    const appSource = fs.readFileSync(
      path.resolve(
        __dirname,
        "..",
        "..",
        "..",
        "src",
        "networkCanvas",
        "webview",
        "NetworkCanvasApp.tsx",
      ),
      "utf8",
    );
    assert.ok(
      appSource.includes(
        "headerFaultsForBanner(visibleFaults, graph.banner)",
      ) && appSource.includes("faultCount={headerFaults.length}"),
      "the real header must consume the de-duplicated fault list",
    );
  });

  test("classifyRuntimeStartFailure owns safe kind-specific recovery copy", () => {
    const cases = [
      {
        raw: "spawn C:\\truST\\trust-runtime.exe ENOENT",
        kind: "missing_binary",
        message:
          "Required runtime/debug binary was not found. Update or reinstall the truST extension, then start the simulator again.",
      },
      {
        raw: "EADDRINUSE: address already in use by pid 321",
        kind: "port_conflict",
        message:
          "The runtime port is already in use. Close the other truST/debug session or process using the port, then start again. Open logs to identify it.",
      },
      {
        raw: "EACCES: permission denied for C:\\private\\runtime.toml",
        kind: "workspace_permission",
        message:
          "The workspace or runtime path is not writable. Make it writable, then start the simulator again.",
      },
      {
        raw: "debug session timed out after 8000ms",
        kind: "readiness_timeout",
        message:
          "The simulator did not become ready in time. Open the Structured Text Debugger logs to see what blocked startup.",
      },
      {
        raw: "zombie debug session id=private-session",
        kind: "stale_runtime",
        message:
          "A stale runtime or debug session blocked startup. Stop the existing session or reload the VS Code window, then start again.",
      },
      {
        raw: "adapter internal state private-session broke",
        kind: "failed_spawn",
        message:
          "Simulator startup failed. Check the Structured Text Debugger output for details.",
      },
      {
        raw: "/tmp/trust-vscode-workspace/src/main.st: error[E206]: missing return value",
        kind: "failed_spawn",
        message:
          "Simulator startup failed. Check the Structured Text Debugger output for details.",
      },
      {
        raw: "project source changed after Compile: C:\\project\\src\\Main.st",
        kind: "failed_spawn",
        message:
          "Project files changed after Compile. Start again to compile the latest files.",
      },
    ] as const;

    for (const testCase of cases) {
      const failure = classifyRuntimeStartFailure(testCase.raw);
      assert.strictEqual(failure.kind, testCase.kind);
      assert.strictEqual(failure.message, testCase.message);
      assert.strictEqual(failure.detail, testCase.raw);
      assert.strictEqual(failure.message.includes(testCase.raw), false);

      const graph = buildCanvasGraph(
        buildNetworkCanvasModel({ stage: "runtime_live", failure }),
        undefined,
      );
      assert.strictEqual(graph.banner?.text, testCase.message);
      assert.strictEqual(JSON.stringify(graph).includes(testCase.raw), false);
      assert.deepStrictEqual(graph.banner?.actions, [
        { label: "Open logs", action: "openRuntimeLogs" },
      ]);
    }
  });

  test("runtime auth/config errors are not misreported as missing executables", () => {
    const originalTrace =
      "[trust-debug] reload_program error: failed to load runtime.toml: " +
      "invalid config 'runtime.toml: runtime.control.auth_token required for tcp endpoint'";
    const failure = classifyRuntimeStartFailure(originalTrace);

    assert.strictEqual(failure.kind, "configuration");
    assert.strictEqual(
      failure.message,
      "Simulator needs control authentication in runtime.toml, and truST could not add it automatically. Make the file writable or open it to configure the token.",
    );
    assert.strictEqual(failure.detail, originalTrace);
    const invalidConfig = classifyRuntimeStartFailure(
      "failed to parse runtime.toml: private parser offset 17",
    );
    assert.strictEqual(invalidConfig.kind, "configuration");
    assert.strictEqual(
      invalidConfig.message,
      "Runtime configuration could not be loaded. Open runtime.toml and fix the reported setting.",
    );
    const invalidGraph = buildCanvasGraph(
      buildNetworkCanvasModel({
        stage: "runtime_live",
        failure: invalidConfig,
      }),
      undefined,
    );
    assert.strictEqual(invalidGraph.banner?.text, invalidConfig.message);
    assert.strictEqual(
      JSON.stringify(invalidGraph).includes("private parser offset 17"),
      false,
    );
    assert.deepStrictEqual(invalidGraph.banner?.actions, [
      { label: "Open runtime.toml", action: "openRuntimeToml" },
    ]);
    assert.strictEqual(
      classifyRuntimeStartFailure("trust-runtime exited with code 1").kind,
      "failed_spawn",
    );
    assert.strictEqual(
      classifyRuntimeStartFailure("ADS target computer was not found").kind,
      "failed_spawn",
    );
  });

  test("generic startup timeouts stay distinct from explicit stale-session evidence", () => {
    const pending =
      'startDebugging still pending after 5s: active=<none> type=<none> config={"type":"structured-text"} log=C:\\temp\\trust-debug.log';
    const timedOut = classifyRuntimeStartFailure(pending);

    assert.strictEqual(timedOut.kind, "readiness_timeout");
    assert.strictEqual(
      timedOut.message,
      "The simulator did not become ready in time. Open the Structured Text Debugger logs to see what blocked startup.",
    );
    assert.strictEqual(timedOut.detail, pending);
    assert.strictEqual(
      classifyRuntimeStartFailure("zombie debug session is still registered")
        .kind,
      "stale_runtime",
    );
  });

  test("missing simulator control metadata has a user-facing startup failure", () => {
    const failure = simulatorStartupIncompleteFailure();

    assert.strictEqual(failure.kind, "internal_startup");
    assert.strictEqual(
      failure.message,
      "Simulator startup could not finish. Check the Structured Text Debugger output for details.",
    );
    assert.strictEqual(failure.detail, undefined);
    assert.doesNotMatch(failure.message, /endpoint|auth(?:entication)? token/i);

    const graph = buildCanvasGraph(
      buildNetworkCanvasModel({ stage: "runtime_live", failure }),
      undefined,
    );
    assert.strictEqual(graph.banner?.text, failure.message);
    assert.doesNotMatch(graph.banner?.text ?? "", /Use Start|retry/i);
    assert.deepStrictEqual(graph.banner?.actions, [
      { label: "Open logs", action: "openRuntimeLogs" },
    ]);
  });

  test("runtime status errors keep technical detail out of visible recovery copy", () => {
    const raw = "socket reset while reading C:\\private\\runtime-status.json";
    const failure = runtimeStatusCheckFailure(new Error(raw));

    assert.strictEqual(failure.kind, "internal_startup");
    assert.strictEqual(
      failure.message,
      "Runtime status check failed. Check the Structured Text Debugger output for details.",
    );
    assert.strictEqual(failure.detail, raw);
    assert.strictEqual(failure.message.includes(raw), false);

    const graph = buildCanvasGraph(
      buildNetworkCanvasModel({ stage: "runtime_live", failure }),
      undefined,
    );
    assert.strictEqual(JSON.stringify(graph).includes(raw), false);
    assert.strictEqual(graph.banner?.text, failure.message);
    assert.deepStrictEqual(graph.banner?.actions, [
      { label: "Open logs", action: "openRuntimeLogs" },
    ]);
  });

  test("unknown internal startup errors stay in technical detail, not visible copy", () => {
    const raw =
      "[DAP] error: Error: read error; adapter=trust-debug.exe; config={request:launch}";
    const failure = classifyRuntimeStartFailure(raw);

    assert.strictEqual(failure.kind, "failed_spawn");
    assert.strictEqual(
      failure.message,
      "Simulator startup failed. Check the Structured Text Debugger output for details.",
    );
    assert.strictEqual(failure.detail, raw);
    assert.strictEqual(failure.message.includes(raw), false);

    const graph = buildCanvasGraph(
      buildNetworkCanvasModel({ stage: "runtime_live", failure }),
      undefined,
    );
    assert.strictEqual(JSON.stringify(graph).includes(raw), false);
    assert.ok(graph.banner?.text.includes("Simulator startup failed."));

    const homeFailures = fs.readFileSync(
      path.resolve(__dirname, "..", "..", "..", "src", "trustHomeFailures.ts"),
      "utf8",
    );
    const actionFailure = homeFailures.slice(
      homeFailures.indexOf("function actionFailureMessage("),
      homeFailures.indexOf("function startFailureChoices("),
    );
    assert.ok(actionFailure.includes("result.failure.message"));
    assert.strictEqual(actionFailure.includes("result.failure.detail"), false);
  });

  test("runtime configuration failures offer only the primary runtime.toml recovery", () => {
    const graph = buildCanvasGraph(
      buildNetworkCanvasModel({
        stage: "runtime_live",
        failure: {
          kind: "configuration",
          message:
            "Simulator needs control authentication in runtime.toml, and truST could not add it automatically.",
          detail: "runtime.control.auth_token required for tcp endpoint",
        },
      }),
      undefined,
    );
    assert.strictEqual(
      graph.banner?.text,
      "Simulator needs control authentication in runtime.toml, and truST could not add it automatically.",
    );
    assert.doesNotMatch(graph.banner?.text ?? "", /Use Start|retry/i);
    assert.deepStrictEqual(graph.banner?.actions, [
      { label: "Open runtime.toml", action: "openRuntimeToml" },
    ]);

    const panelSource = fs.readFileSync(
      path.resolve(
        __dirname,
        "..",
        "..",
        "..",
        "src",
        "networkCanvas",
        "networkCanvasPanel.ts",
      ),
      "utf8",
    );
    assert.ok(panelSource.includes("new NetworkCanvasLifecycleActions("));

    const lifecycleActionsSource = fs.readFileSync(
      path.resolve(
        __dirname,
        "..",
        "..",
        "..",
        "src",
        "networkCanvas",
        "lifecycleActions.ts",
      ),
      "utf8",
    );
    assert.ok(lifecycleActionsSource.includes('case "openRuntimeToml":'));
    assert.ok(lifecycleActionsSource.includes("openSelectedRuntimeToml()"));

    const recoverySource = fs.readFileSync(
      path.resolve(
        __dirname,
        "..",
        "..",
        "..",
        "src",
        "runtimeRecoveryActions.ts",
      ),
      "utf8",
    );
    assert.ok(recoverySource.includes("findRuntimeControlToml(projectRoot)"));
    assert.ok(recoverySource.includes("vscode.workspace.openTextDocument("));
    assert.ok(recoverySource.includes("vscode.window.showTextDocument("));
  });
});
