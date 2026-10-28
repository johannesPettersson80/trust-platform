import * as assert from "assert";

import {
  EMPTY_BROWSE_SESSION,
  isCurrentBrowseMessage,
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
  test("ADS browse opens the target's port without an extra confirmation", () => {
    // The server sets the port a target is browsed at: `comm.discover` on a find and
    // `trust-lsp.devicesTopology` on a configured connection (851 when it names none).
    const plan = planBrowseOpen(
      "ads",
      { host: "192.168.10.5", ams_net_id: "5.23.91.12.1.1", ams_port: 851 },
      "ADS device"
    );

    assert.ok(plan);
    assert.strictEqual(plan.panel.target.ams_port, 851);
    assert.strictEqual(plan.loading, true);
    assert.deepStrictEqual(plan.request, {
      protocol: "ads",
      target: {
        host: "192.168.10.5",
        ams_net_id: "5.23.91.12.1.1",
        ams_port: 851,
      },
      kind: "symbols",
    });

    const discovered = planBrowseOpen(
      "ads",
      {
        host: "192.168.10.5",
        ams_net_id: "5.23.91.12.1.1",
        ams_port: 301,
        responding_ads_ports: [301, 851],
        ads_port_browse_results: [
          { port: 301, tree: [], routeMissing: false },
          { port: 851, tree: [], routeMissing: false },
        ],
      },
      "ADS device"
    );
    assert.strictEqual(discovered?.loading, false);
    assert.strictEqual(discovered?.request, undefined);
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

  // The target a configured connection's browse opens with (OPC UA's first connection, an ADS
  // device's ports and imported tags) is the language server's: the Rust tests
  // `an_ads_client_browses_its_first_connection_with_the_devices_ports_and_tags` and
  // `a_client_connection_browses_its_first_connection_and_others_their_parameters`.

  // The rules (which runtime a search can start from, which runtime a found device goes to) are
  // the language server's, proven by trust-devices' Rust tests
  // `a_search_starts_only_from_a_runtime_that_runs_or_is_attached` and
  // `a_found_device_goes_to_the_runtime_that_found_it_and_a_found_runtime_becomes_a_host`; this
  // proves that the pane shows its answers.
  test("discover model shows the server's origins and device drafts", () => {
    const nodes = [
      {
        id: "runtime:stopped",
        type: "runtime",
        data: {
          label: "Stopped",
          discovery: { ready: false, reason: "Start or connect Stopped before scanning from it." },
        },
      },
      {
        id: "runtime:live",
        type: "runtime",
        data: {
          label: "Live",
          controlEndpoint: "tcp://192.168.10.20:5680",
          discovery: { ready: true },
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
    assert.strictEqual(
      origins[1].runtimeDiscoveryDisabledReason,
      "Start or connect Stopped before scanning from it."
    );
    assert.strictEqual(origins[2].runtimeDiscoveryReady, true);
    assert.strictEqual(origins[2].controlEndpoint, "tcp://192.168.10.20:5680");

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

    const candidate = {
      id: "mqtt:broker",
      label: "Broker",
      protocol: "mqtt",
      source: "targeted",
      confidence: "observed",
      originRuntimeId: "runtime:live",
      params: { host: "broker.local" },
    };
    assert.deepStrictEqual(
      draftForDiscoveredCandidate(
        candidate,
        { id: "mqtt:broker", on_canvas: false, imported_ads_symbols: [], draft_runtime_id: "runtime:live" },
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
        candidate,
        { id: "mqtt:broker", on_canvas: false, imported_ads_symbols: [] },
        nodes
      ),
      {
        runtimeId: "",
        runtimeName: "runtime",
        protocol: "mqtt",
        prefillParams: { host: "broker.local" },
      },
      "a result the server gives no runtime is drafted for none"
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
