import * as assert from "assert";

import {
  EMPTY_BROWSE_SESSION,
  isCurrentBrowseMessage,
  normalizeEndpointBrowseTarget,
  planBrowseOpen,
  reduceBrowseSessionState,
} from "../../networkCanvas/webview/browseSessionModel";
import {
  buildDiscoverOrigins,
  discoverableProtocols,
  draftForDiscoveredCandidate,
  shouldShowDiscoveryUnavailable,
} from "../../networkCanvas/webview/discoverPaneModel";

suite("Network Canvas session models", function () {
  test("ADS browse waits for an explicit port confirmation", () => {
    const plan = planBrowseOpen(
      "ads",
      { host: "192.168.10.5", ams_net_id: "5.23.91.12.1.1" },
      "TwinCAT"
    );

    assert.ok(plan);
    assert.strictEqual(plan.panel.target.ams_port, 851);
    assert.strictEqual(plan.loading, false);
    assert.strictEqual(plan.request, undefined);
  });

  test("non-ADS and local browse plans retain their immediate request behavior", () => {
    const opcua = planBrowseOpen(
      "opcua_client",
      { endpoint_url: "opc.tcp://plc:4840" },
      "PLC"
    );
    assert.deepStrictEqual(opcua?.request, {
      protocol: "opcua_client",
      target: { endpoint_url: "opc.tcp://plc:4840" },
      kind: "nodes",
    });
    assert.strictEqual(opcua?.loading, true);

    const local = planBrowseOpen("ads_server", { ignored: true }, "truST");
    assert.deepStrictEqual(local?.panel.target, { local: true });
    assert.deepStrictEqual(local?.request, {
      protocol: "ads_server",
      target: { local: true },
      kind: "symbols",
    });
  });

  test("browse reducer clears request data and retains completed data when the drawer closes", () => {
    const plan = planBrowseOpen(
      "opcua_client",
      { endpoint_url: "opc.tcp://plc:4840" },
      "PLC"
    );
    assert.ok(plan);
    const opened = reduceBrowseSessionState(EMPTY_BROWSE_SESSION, {
      type: "open",
      panel: plan.panel,
      loading: plan.loading,
    });
    const retargeted = reduceBrowseSessionState(opened, {
      type: "request",
      target: { endpoint_url: "opc.tcp://plc:4841" },
    });
    assert.strictEqual(retargeted.loading, true);
    assert.strictEqual(retargeted.tree, undefined);
    assert.strictEqual(retargeted.panel?.target.endpoint_url, "opc.tcp://plc:4841");

    const completed = reduceBrowseSessionState(retargeted, {
      type: "result",
      tree: [{ id: "temperature", name: "Temperature", path: "GVL.Temperature" }],
      routeMissing: false,
    });
    const closed = reduceBrowseSessionState(completed, { type: "close" });
    assert.strictEqual(closed.panel, undefined);
    assert.deepStrictEqual(closed.tree, completed.tree);
    assert.strictEqual(closed.loading, false);
    assert.deepStrictEqual(
      reduceBrowseSessionState(completed, { type: "reset" }),
      EMPTY_BROWSE_SESSION
    );
  });

  test("browse responses are scoped to the current webview request", () => {
    assert.strictEqual(
      isCurrentBrowseMessage(
        {
          type: "symbolTree",
          browseSessionId: "session-b",
          browseRequestId: 7,
        },
        "session-b",
        7
      ),
      true
    );
    assert.strictEqual(
      isCurrentBrowseMessage(
        { browseSessionId: "session-a", browseRequestId: 6 },
        "session-b",
        7
      ),
      false
    );
  });

  test("endpoint browse keeps the first configured ADS or OPC UA connection", () => {
    const first = { endpoint_url: "opc.tcp://primary:4840" };
    const params = { connections: [first, { endpoint_url: "opc.tcp://backup:4840" }] };

    assert.strictEqual(normalizeEndpointBrowseTarget("opcua_client", params), first);
    assert.strictEqual(normalizeEndpointBrowseTarget("ads", params), first);
    assert.strictEqual(normalizeEndpointBrowseTarget("mqtt", params), params);
  });

  test("discover model derives honest origins, protocols, and device drafts", () => {
    const nodes = [
      { id: "runtime:stopped", type: "runtime", data: { label: "Stopped", health: "stopped" } },
      {
        id: "runtime:live",
        type: "runtime",
        data: {
          label: "Live",
          attached: true,
          controlEndpoint: "tcp://192.168.10.20:5680",
        },
      },
      { id: "endpoint:ads", type: "endpoint", data: { label: "ADS" } },
    ];
    const origins = buildDiscoverOrigins(nodes);
    assert.deepStrictEqual(origins.map((origin) => origin.id), [
      "this_host",
      "runtime:stopped",
      "runtime:live",
    ]);
    assert.strictEqual(origins[1].runtimeDiscoveryReady, false);
    assert.match(origins[1].runtimeDiscoveryDisabledReason ?? "", /Start or connect Stopped/);
    assert.strictEqual(origins[2].runtimeDiscoveryReady, true);
    assert.strictEqual(
      origins[2].controlEndpoint,
      "tcp://192.168.10.20:5680"
    );

    assert.deepStrictEqual(
      [...discoverableProtocols({
        protocols: [
          { id: "ads", actions: ["discover"] },
          { id: "mqtt", actions: ["apply"] },
          { id: "ethercat", actions: ["discover", "apply"] },
        ],
      })],
      ["ads", "ethercat"]
    );

    assert.deepStrictEqual(
      draftForDiscoveredCandidate(
        {
          id: "mqtt:broker",
          label: "Broker",
          protocol: "mqtt",
          source: "targeted",
          confidence: "observed",
          originRuntimeId: "runtime:live",
          params: { host: "broker.local" },
        },
        nodes
      ),
      {
        runtimeId: "runtime:live",
        runtimeName: "Live",
        protocol: "mqtt",
        prefillParams: { host: "broker.local" },
      }
    );

    assert.deepStrictEqual(
      draftForDiscoveredCandidate(
        {
          id: "ads:stale",
          label: "Stale ADS target",
          protocol: "ads",
          source: "ads_broadcast",
          confidence: "observed",
          originRuntimeId: "runtime:gone",
          params: { host: "192.168.10.5", ams_port: 301 },
        },
        nodes
      ),
      {
        runtimeId: "",
        runtimeName: "runtime",
        protocol: "ads",
        prefillParams: { host: "192.168.10.5", ams_port: 301 },
      },
      "an explicit missing origin must not silently fall back to another runtime"
    );
  });

  test("missing discovery capability never contradicts retained session activity", () => {
    assert.strictEqual(
      shouldShowDiscoveryUnavailable(0, false, 0, 0),
      true
    );
    assert.strictEqual(
      shouldShowDiscoveryUnavailable(0, true, 0, 0),
      false,
      "an active scan owns the pane copy"
    );
    assert.strictEqual(
      shouldShowDiscoveryUnavailable(0, false, 1, 0),
      false,
      "retained progress must not coexist with the no-capability copy"
    );
    assert.strictEqual(
      shouldShowDiscoveryUnavailable(0, false, 0, 1),
      false,
      "retained results must not coexist with the no-capability copy"
    );
    assert.strictEqual(
      shouldShowDiscoveryUnavailable(0, false, 0, 0, "Discovery failed"),
      false,
      "an explicit error owns the pane copy"
    );
  });
});
