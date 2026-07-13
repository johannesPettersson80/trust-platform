import {
  assert,
  fs,
  os,
  path,
  vscode,
  AdsServiceProbeController,
  probeAdsServicesSequentially,
  resolveSelectedAdsServicePort,
  runJsonCommand,
  discoverLabel,
  adsEmptyIdentityCopy,
  adsEmptyRecoveryFocusRole,
  adsServiceProbeResultsNeedRecheck,
  applyAdsEmptyRecovery,
  discoveryProgressCopy,
  validateAdsDiscoveryDraft,
  sendRuntimeControlRequest,
  reduceDiscoverySessionState,
  response,
  source,
} from "./ads-service-probe-safety-fixtures";
import type {
  AdsServiceProbeResult,
  AdsDiscoveryDraft,
  AdsDiscoveryScanSnapshot,
  DiscoverySessionState,
} from "./ads-service-probe-safety-fixtures";

suite("ADS service probe safety", () => {
  test("turns an empty ADS scan into contextual recovery instead of naked 0 found", () => {
    const draft: AdsDiscoveryDraft = {
      host: "192.168.50.42",
      amsNetId: "",
      customPorts: "",
      advanced: false,
    };
    const snapshot: AdsDiscoveryScanSnapshot = {
      origin: "this_host",
      host: draft.host,
      ports: [851, 852, 853, 854, 301, 501],
    };

    const recoveryCopy = adsEmptyIdentityCopy(snapshot);
    assert.match(recoveryCopy, /192\.168\.50\.42.*running.*firewall.*AMS Net ID/i);
    assert.doesNotMatch(recoveryCopy, /UDP|router|10060|10054|ads-wire/i);
    assert.deepStrictEqual(applyAdsEmptyRecovery(draft, snapshot), {
      ...draft,
      advanced: true,
    });
    assert.strictEqual(
      adsEmptyRecoveryFocusRole(
        { ...snapshot, host: undefined },
        {}
      ),
      "ads-host"
    );
    assert.strictEqual(adsEmptyRecoveryFocusRole(snapshot, {}), "ads-ams-net-id");
    assert.strictEqual(
      adsEmptyRecoveryFocusRole(
        { ...snapshot, targetAmsNetId: "10.20.30.40.1.1" },
        { customPortError: "invalid" }
      ),
      "ads-custom-ports"
    );
    assert.strictEqual(
      applyAdsEmptyRecovery(
        { ...draft, host: "" },
        { ...snapshot, host: undefined }
      ).advanced,
      true
    );
    assert.strictEqual(
      discoveryProgressCopy({
        protocol: "ads",
        label: "ADS",
        status: "scanning",
      }),
      "Searching this computer and local network…"
    );
    assert.strictEqual(
      discoveryProgressCopy({
        protocol: "ads",
        label: "ADS @ 192.168.50.42",
        status: "done",
        count: 0,
      }),
      "ADS @ 192.168.50.42 … no ADS devices found"
    );
    assert.strictEqual(
      discoveryProgressCopy({
        protocol: "ads",
        label: "ADS @ 192.168.50.42",
        status: "done",
        count: 1,
      }),
      "ADS @ 192.168.50.42 … 1 ADS device found"
    );
    assert.match(
      discoveryProgressCopy({
        protocol: "ads",
        label: "ADS",
        status: "failed",
      }),
      /failed$/
    );
    assert.strictEqual(
      discoverLabel("ads", "127.0.0.1"),
      "ADS on the discovery computer"
    );
    assert.strictEqual(
      discoverLabel("ads", "192.168.50.42"),
      "ADS @ 192.168.50.42"
    );
  });
  test("surfaces invalid persisted Advanced values even while collapsed", () => {
    const validation = validateAdsDiscoveryDraft({
      host: "192.168.50.42",
      amsNetId: "100.67.6.999.1.1",
      customPorts: "9000, nope",
      advanced: false,
    });

    assert.match(validation.amsNetIdError ?? "", /six decimal numbers.*0.*255/i);
    assert.match(validation.customPortError ?? "", /whole number/i);
    const flow = source("networkCanvas/webview/AdsDiscoveryFlow.tsx");
    assert.ok(flow.includes('data-role="ads-advanced-attention"'));
    assert.ok(flow.includes("Advanced settings need attention"));
    assert.ok(flow.includes("hostError || amsNetIdError || customPortError"));
    assert.ok(
      flow.indexOf('data-role="ads-advanced-toggle"') <
        flow.indexOf('data-role="ads-known-host"'),
      "all recovery fields must render below their Advanced disclosure control"
    );

    const invalidHost = validateAdsDiscoveryDraft({
      host: "192.168.50.42:48898",
      amsNetId: "",
      customPorts: "",
      advanced: false,
    });
    assert.match(invalidHost.hostError ?? "", /without a port/i);
    assert.ok(
      flow.includes(
        "draft.host.trim() || draft.amsNetId.trim() || draft.customPorts.trim()"
      ),
      "a valid collapsed known Host must be visibly summarized as an active recovery setting"
    );
  });
  test("does not preserve an automatic choice when a recheck finds two runtimes", () => {
    const available = (port: number): AdsServiceProbeResult => ({
      port,
      status: "available",
      symbolCount: 1,
      usable: true,
    });

    assert.strictEqual(resolveSelectedAdsServicePort([available(851)]), 851);
    assert.strictEqual(
      resolveSelectedAdsServicePort([available(851), available(852)]),
      undefined,
      "A formerly automatic 851 must not survive as an implicit choice after 852 appears."
    );
    assert.strictEqual(
      resolveSelectedAdsServicePort(
        [available(851), available(852)],
        852
      ),
      852,
      "A deliberate radio selection may be preserved while it remains usable."
    );
    assert.strictEqual(
      resolveSelectedAdsServicePort([available(9000)], 9000, false),
      undefined,
      "Results from a previous port plan must never keep a removed ADS service browseable."
    );
    assert.strictEqual(
      adsServiceProbeResultsNeedRecheck(
        "851,852,853,854,301,501,9000",
        "851,852,853,854,301,501",
        "Each logical ADS service port must be a whole number."
      ),
      true,
      "Invalid edited port text must stale earlier results even when parsing falls back to the preset plan."
    );
  });
  test("a later service check cancels an earlier status preflight before ADS browse starts", async () => {
    const statusCalls: Array<{
      token: vscode.CancellationToken | undefined;
      resolve: (value: unknown) => void;
    }> = [];
    const posts: unknown[] = [];
    const panel = {
      visible: true,
      webview: {
        postMessage: async (message: unknown) => {
          posts.push(message);
          return true;
        },
      },
    } as unknown as vscode.WebviewPanel;
    const runtime = {
      mode: "online",
      endpoint: "tcp://127.0.0.1:9901",
      endpointEnabled: true,
      reachable: true,
      status: "online_reachable",
      label: "Local runtime",
      credentialChannel: "trusted_same_host",
    } as const;
    const runtimeControlRequest = (<T>(
      _endpoint: string,
      _authToken: string | undefined,
      _requestType: string,
      _params?: unknown,
      options?: { cancellationToken?: vscode.CancellationToken }
    ) =>
      new Promise<T>((resolve) => {
        statusCalls.push({
          token: options?.cancellationToken,
          resolve: resolve as (value: unknown) => void,
        });
      })) as typeof sendRuntimeControlRequest;
    const controller = new AdsServiceProbeController({
      panel: () => panel,
      extensionContext: () => ({} as vscode.ExtensionContext),
      runtimeTargetForOrigin: () => undefined,
      runtimeTargetOnDiscoveryComputer: () => runtime,
      requestIsCurrent: () => true,
      runtimeControlRequest,
    });
    const request = (requestId: number): Record<string, unknown> => ({
      sessionId: "session-a",
      requestId,
      origin: "this_host",
      candidate: {
        id: "plc-a",
        label: "TwinCAT computer",
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

    const first = controller.probe(request(1));
    assert.strictEqual(statusCalls.length, 1);
    assert.ok(statusCalls[0].token, "The status preflight must be cancellable.");

    const second = controller.probe(request(2));
    assert.strictEqual(statusCalls.length, 2);
    assert.strictEqual(statusCalls[0].token?.isCancellationRequested, true);
    controller.cancel();
    assert.strictEqual(statusCalls[1].token?.isCancellationRequested, true);

    const stoppedReport = {
      overall: "disabled",
      summary: "ADS is not configured.",
      connections: [],
    };
    statusCalls[0].resolve(stoppedReport);
    statusCalls[1].resolve(stoppedReport);
    await Promise.all([first, second]);
    assert.deepStrictEqual(
      posts,
      [],
      "Canceled preflights must not progress into a service probe or publish stale results."
    );
  });
  test("checks cancellation before every logical ADS service", async () => {
    const calls: number[] = [];
    let active = true;

    const results = await probeAdsServicesSequentially(
      [851, 852, 853],
      async (port) => {
        calls.push(port);
        active = false;
        return response("empty");
      },
      { isActive: () => active }
    );

    assert.deepStrictEqual(calls, [851]);
    assert.deepStrictEqual(results.map((result) => result.port), [851]);
  });
  test("cancellation terminates the local ADS command instead of leaving network I/O running", async function () {
    this.timeout(5_000);
    const markerRoot = fs.mkdtempSync(
      path.join(os.tmpdir(), "trust-ads-cancel-")
    );
    const marker = path.join(markerRoot, "still-running.txt");
    const cancellation = new vscode.CancellationTokenSource();
    try {
      const started = Date.now();
      const pending = runJsonCommand<unknown>(
        process.execPath,
        [
          "-e",
          `setTimeout(() => require("fs").writeFileSync(${JSON.stringify(marker)}, "late"), 1000); setTimeout(() => process.stdout.write("{}"), 4000);`,
        ],
        undefined,
        cancellation.token
      );
      setTimeout(() => cancellation.cancel(), 100);
      const result = await pending;

      assert.strictEqual(result.ok, false);
      assert.strictEqual(result.message, "Command cancelled.");
      assert.ok(Date.now() - started < 2_000, "Cancellation must be prompt.");
      await new Promise((resolve) => setTimeout(resolve, 1_100));
      assert.strictEqual(
        fs.existsSync(marker),
        false,
        "The canceled process must not continue its delayed work."
      );
    } finally {
      cancellation.dispose();
      fs.rmSync(markerRoot, { recursive: true, force: true });
    }
  });
  test("reports each port before its network request", async () => {
    const events: string[] = [];
    await probeAdsServicesSequentially(
      [851, 852],
      async (port) => {
        events.push(`probe:${port}`);
        return response("empty");
      },
      {
        onBeforeProbe: (port) => {
          events.push(`progress:${port}`);
        },
      }
    );

    assert.deepStrictEqual(events, [
      "progress:851",
      "probe:851",
      "progress:852",
      "probe:852",
    ]);
  });
  test("terminal results reject late discovery and probe progress", () => {
    const initial: DiscoverySessionState = {
      scanning: true,
      progress: [],
      results: [],
      sessionCurrent: true,
      terminal: false,
      adsServiceProbes: {},
    };
    const terminal = reduceDiscoverySessionState(initial, {
      type: "results",
      candidates: [],
    });
    assert.strictEqual(
      reduceDiscoverySessionState(terminal, {
        type: "progress",
        row: { protocol: "ads", label: "TwinCAT", status: "scanning" },
      }),
      terminal
    );

    const probing = reduceDiscoverySessionState(terminal, {
      type: "adsProbeStarted",
      candidateId: "plc-a",
    });
    const completed = reduceDiscoverySessionState(probing, {
      type: "adsProbeResults",
      candidateId: "plc-a",
      results: [],
    });
    assert.strictEqual(
      reduceDiscoverySessionState(completed, {
        type: "adsProbeProgress",
        candidateId: "plc-a",
        port: 852,
      }),
      completed
    );
  });
});
