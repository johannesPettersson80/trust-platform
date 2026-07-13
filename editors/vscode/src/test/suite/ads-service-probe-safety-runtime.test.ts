import {
  assert,
  path,
  vscode,
  hasActiveOrRecoveringAdsConnection,
  ACTIVE_ADS_CONNECTION_PROBE_SAFETY_MESSAGE,
  AdsServiceProbeController,
  adsStatusProbeSafetyMessage,
  failedBrowseResponse,
  localRuntimeTargetForAdsProbe,
  UNKNOWN_ADS_CONNECTION_PROBE_SAFETY_MESSAGE,
  classifyAdsServiceProbe,
  sendRuntimeControlRequest,
  response,
  source,
} from "./ads-service-probe-safety-fixtures";

suite("ADS service probe safety", () => {
  test("preserves the real browse failure in a complete versioned ADS contract", () => {
    const response = failedBrowseResponse(
      new Error("native ADS read failed: target port 501 did not answer")
    );
    assert.strictEqual(response.schema_version, 1);
    assert.strictEqual(response.protocol, "ads");
    assert.strictEqual(response.kind, "symbols");

    const result = classifyAdsServiceProbe(501, response);
    assert.strictEqual(result.status, "check_failed");
    assert.notStrictEqual(result.error?.code, "invalid_browse_response");
    assert.match(result.error?.message ?? "", /target port 501 did not answer/i);
  });
  test("offline ADS CLI failure reaches the classifier without becoming invalid JSON", async () => {
    const childProcess = require("child_process") as {
      execFile: (...args: unknown[]) => unknown;
    };
    const offline = require("../../networkCanvas/offlineComm") as {
      offlineBrowseSymbols: typeof import("../../networkCanvas/offlineComm").offlineBrowseSymbols;
    };
    const originalExecFile = childProcess.execFile;
    childProcess.execFile = (...args: unknown[]): unknown => {
      const callback = args[args.length - 1];
      assert.strictEqual(typeof callback, "function");
      (
        callback as (error: Error, stdout: string, stderr: string) => void
      )(
        new Error("command failed"),
        "",
        "native ADS read failed: target port 501 did not answer"
      );
      return { kill: () => undefined };
    };

    try {
      const response = await offline.offlineBrowseSymbols(
        {
          extensionMode: vscode.ExtensionMode.Test,
          extensionPath: path.resolve(__dirname, "../../.."),
        } as vscode.ExtensionContext,
        "ads",
        {
          host: "192.168.50.42",
          ams_net_id: "10.20.30.40.1.1",
          ams_port: 501,
        }
      );
      assert.ok(response);
      const result = classifyAdsServiceProbe(501, response);
      assert.strictEqual(result.status, "unavailable");
      assert.strictEqual(result.error?.code, "ads_port_unavailable");
      assert.match(result.error?.message ?? "", /target port 501 did not answer/i);
    } finally {
      childProcess.execFile = originalExecFile;
    }
  });
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
    for (const status of ["auth_failed"] as const) {
      const unverifiableLocalRuntime = {
        ...localRuntime,
        reachable: true,
        status,
      };
      assert.strictEqual(
        localRuntimeTargetForAdsProbe(unverifiableLocalRuntime),
        undefined,
        `${status} local runtimes must not make read-only ADS discovery depend on simulator authentication.`
      );
    }
    assert.strictEqual(
      localRuntimeTargetForAdsProbe({
        ...localRuntime,
        reachable: false,
        status: "online_unreachable",
      }),
      undefined,
      "An unreachable stale loopback target must fall through to the packaged offline ADS probe."
    );
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
  test("a stale or auth-failed loopback target falls through to offline ADS service probes", async () => {
    const posts: Array<Record<string, unknown>> = [];
    let statusRequests = 0;
    const offline = require("../../networkCanvas/offlineComm") as {
      offlineBrowseSymbols: typeof import("../../networkCanvas/offlineComm").offlineBrowseSymbols;
    };
    const originalBrowse = offline.offlineBrowseSymbols;
    const probedPorts: number[] = [];
    offline.offlineBrowseSymbols = async (
      _context,
      protocol,
      target
    ) => {
      assert.strictEqual(protocol, "ads");
      probedPorts.push(Number(target.ams_port));
      return response("empty");
    };
    const panel = {
      visible: true,
      webview: {
        postMessage: async (message: Record<string, unknown>) => {
          posts.push(message);
          return true;
        },
      },
    } as unknown as vscode.WebviewPanel;
    const localTargets = [
      {
        mode: "online",
        endpoint: "tcp://127.0.0.1:9901",
        endpointEnabled: true,
        reachable: false,
        status: "online_unreachable",
        label: "Stale local runtime",
        credentialChannel: "trusted_same_host",
      },
      {
        mode: "online",
        endpoint: "tcp://127.0.0.1:9901",
        endpointEnabled: true,
        reachable: true,
        status: "auth_failed",
        label: "Auth-failed local runtime",
        credentialChannel: "trusted_same_host",
      },
    ] as const;
    try {
      for (const localTarget of localTargets) {
        const controller = new AdsServiceProbeController({
          panel: () => panel,
          extensionContext: () => ({} as vscode.ExtensionContext),
          runtimeTargetForOrigin: () => undefined,
          runtimeTargetOnDiscoveryComputer: () =>
            localRuntimeTargetForAdsProbe(localTarget),
          requestIsCurrent: () => true,
          runtimeControlRequest: (async () => {
            statusRequests += 1;
            throw new Error("must not query an unusable loopback runtime");
          }) as typeof sendRuntimeControlRequest,
        });

        await controller.probe({
          sessionId: `offline-fallback-${localTarget.status}`,
          requestId: 1,
          origin: "this_host",
          candidate: {
            id: "plc-a",
            label: "ADS device",
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

        assert.strictEqual(statusRequests, 0);
        assert.deepStrictEqual(probedPorts, [851, 852, 853, 854, 301, 501]);
        const result = posts.find(
          (message) => message.type === "adsServiceProbeResults"
        );
        assert.ok(result, "the packaged offline probes must publish their result");
        assert.strictEqual(result?.error, undefined);
        assert.ok(Array.isArray(result?.results));
        posts.length = 0;
        probedPorts.length = 0;
      }
    } finally {
      offline.offlineBrowseSymbols = originalBrowse;
    }
  });
});
