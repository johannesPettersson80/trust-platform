import {
  assert,
  loadPackageJson,
  readSrc,
  readSrcSet,
  commandTitles,
} from "./ux-shell-contract-fixtures";

suite("Phases 2–3 — naming + nav (v5 shell)", () => {
  test("the graph is user-facing 'Devices & Connections', never 'Network Canvas'", () => {
    const titles = commandTitles(loadPackageJson());
    assert.strictEqual(
      titles.get("trust-lsp.networkCanvas.open"),
      "Open Devices & Connections",
      "the canvas command title must use the same Open-verb pattern as other destinations"
    );
  });
  test("NO user-facing command title contains the jargon 'Network Canvas'", () => {
    for (const [command, title] of commandTitles(loadPackageJson())) {
      assert.ok(
        !/network canvas/i.test(title),
        `${command} title must not contain 'Network Canvas' (got "${title}")`
      );
    }
  });
  test("native Testing view explains empty Structured Text test workspaces", () => {
    const welcomes = loadPackageJson().contributes?.viewsWelcome ?? [];
    const testingWelcome = welcomes.find(
      (entry) => entry.view === "workbench.view.testing"
    );
    assert.ok(
      testingWelcome,
      "truST must use the native Testing view welcome area for the no-tests state"
    );
    assert.match(
      testingWelcome.contents ?? "",
      /No Structured Text tests found\./,
      "the no-tests state must be honest and specific to Structured Text tests"
    );
    assert.match(
      testingWelcome.contents ?? "",
      /TEST_PROGRAM|TEST_FUNCTION_BLOCK/,
      "the no-tests state must tell a first-time user how to add an ST test"
    );
  });
  test("package contribution labels and descriptions use current product names", () => {
    const pkg = loadPackageJson();
    const contributedText = JSON.stringify({
      commands: (pkg.contributes?.commands ?? []).map((command: { command?: string; title?: string; category?: string }) => ({
        command: command.command,
        title: command.title,
        category: command.category,
      })),
      configuration: pkg.contributes?.configuration,
    });
    for (const forbidden of ["Network Canvas", "Runtime Panel", "Structured Text Runtime"]) {
      assert.ok(
        !contributedText.includes(forbidden),
        `package contribution text must not expose stale product wording: ${forbidden}`
      );
    }
  });
  test("Discover exposes Modbus host and subnet targets separately", () => {
    const source = readSrc("networkCanvas/webview/DiscoverPane.tsx");
    assert.match(
      source,
      /key:\s*"modbus-host"[\s\S]*protocol:\s*"modbus_tcp"[\s\S]*label:\s*"Modbus device"[\s\S]*input:\s*"host"/,
      "Discover must let a user scan one known Modbus host:port"
    );
    assert.match(
      source,
      /key:\s*"modbus-custom"[\s\S]*protocol:\s*"modbus_tcp"[\s\S]*label:\s*"Modbus \(custom subnet\)"[\s\S]*input:\s*"cidr"/,
      "Discover must keep the Modbus subnet scan for OT LAN sweeps"
    );
  });

	  test("Discover result cards show runtime endpoints and candidate confidence", () => {
	    const source = readSrc("networkCanvas/webview/DiscoverPane.tsx");
	    assert.ok(
	      source.includes("c.params.control_endpoint") &&
	        source.includes("c.params.host") &&
	        source.includes("formatDiscoveredEndpoint(endpoint)"),
	      "runtime discovery results must show a user-facing host:port address so Adopt is understandable"
	    );
    assert.ok(
      source.includes("c.confidence"),
      "non-runtime discovery results must still render confidence such as observed instead of hiding it"
    );
    assert.ok(
      source.includes("protocolName(c.protocol)") &&
        !source.includes("[c.protocol, c.source, c.confidence]"),
      "Discover results must display user-facing protocol names, not raw ids such as modbus_tcp/discovery"
    );
    assert.ok(
      source.includes("discoverySourceLabel(c.source)") &&
        !source.includes("[protocolName(c.protocol), c.source"),
      "Discover results must display user-facing source labels, not raw ids such as tcp_connect"
    );
    assert.ok(
      source.includes("discoveryConfidenceLabel(c.confidence)") &&
        !source.includes('"port reachable"') &&
        !source.includes('"tcp-only"'),
      "Discover results must use the shared user-facing confidence label, not private wording"
    );
	    assert.ok(
	      source.includes('overflowWrap: "anywhere"'),
	      "runtime discovery endpoint detail must wrap instead of clipping the control endpoint"
	    );
	    assert.ok(
	      source.includes('value.startsWith("tcp://")') &&
	        source.includes('value.slice("tcp://".length)'),
	      "runtime discovery must not expose tcp:// in the visible result card"
	    );
	    assert.ok(
	      source.includes("runtimeDiscoveryDetail(host, displayEndpoint)") &&
	        source.includes("return cleanEndpoint || cleanHost"),
	      "runtime discovery must show one actionable address, not conflicting host plus endpoint details"
	    );
	  });
  test("Discover copy stays first-user-facing and avoids rejected network jargon", () => {
    const source = readSrc("networkCanvas/webview/DiscoverPane.tsx");
	    for (const required of [
	      "trust-inspector",
	      "trust-inspector__header",
	      "trust-inspector__eyebrow",
	      "trust-section",
	      "trust-input",
	      "trust-button",
	    ]) {
	      assert.ok(source.includes(required), `Discover pane must use shared product chrome: ${required}`);
	    }
	    for (const forbidden of [
	      "Field devices",
	      "origin's local subnet",
	      "connect-only",
	      "Targeted (needs a host/subnet)",
	      "Runtime-only",
      "var(--vscode-editorHoverWidget-background",
      "var(--vscode-editorWidget-border",
      "Discovery needs a runtime that serves it",
    ]) {
      assert.ok(
        !source.includes(forbidden),
        `Discover pane must not expose rejected first-user wording: ${forbidden}`
      );
    }
    assert.ok(
      source.includes("device is powered on") &&
        source.includes("same network") &&
        source.includes("port or firewall") &&
        source.includes("address or subnet"),
      "empty discovery results must give concrete recovery checks instead of a vague runtime hint"
    );
  });
  test("Discover hardware scans are disabled with a reason until an origin can run them", () => {
    const pane = readSrcSet(
      "networkCanvas/webview/DiscoverPane.tsx",
      "networkCanvas/webview/discoverPaneModel.ts"
    );
    assert.ok(
      pane.includes("runtimeDiscoveryReady") &&
        pane.includes("selectedStoppedRuntimeReason") &&
        pane.includes('selectedHardwareOrigin.id !== "this_host"') &&
        pane.includes("runtimeScanDisabledReason") &&
        pane.includes("disabled={Boolean(disabledReason) || discoveryBusy}") &&
        pane.includes("selectedScanRows") &&
        pane.includes('data-role="scan-selected"') &&
        pane.includes("disabled={scanDisabled}") &&
        pane.includes("Start or connect a runtime before scanning EtherCAT or GPIO."),
      "runtime-only scans must stay visible but disabled-with-reason, and stopped runtime origins must disable all scan rows"
    );
    const theme = readSrc("webview/theme.css");
    assert.ok(
      theme.includes("button.trust-button:disabled") &&
        theme.includes(".trust-inspector button.trust-button:disabled") &&
        /\.trust-button:disabled[\s\S]*background:\s*var\(--trust-surface-raised\)\s*!important/.test(theme) &&
        /background-color:\s*var\(--trust-surface-raised\)\s*!important/.test(theme) &&
        /border:\s*1px solid var\(--trust-border\)\s*!important/.test(theme) &&
        /color:\s*var\(--trust-text-subtle\)\s*!important/.test(theme) &&
        /transition:\s*none\s*!important/.test(theme),
      "disabled buttons must render as neutral disabled controls using shared trust tokens, not VS Code primary blue"
    );

    const app = readSrcSet(
      "networkCanvas/webview/NetworkCanvasApp.tsx",
      "networkCanvas/webview/DiscoverPane.tsx",
      "networkCanvas/webview/discoverPaneModel.ts"
    );
    assert.ok(
      app.includes("runtimeDiscoveryReady") &&
        app.includes('health === "connected"') &&
        app.includes('health === "running"') &&
        app.includes('health === "online"') &&
        app.includes("before scanning from it") &&
        app.includes("Choose a running runtime for EtherCAT or GPIO scans."),
      "Discover origins must derive hardware-scan readiness from the rendered runtime node state, not from hardcoded availability"
    );
  });
  test("Discover Adopt preserves the runtime label and focuses the adopted node", () => {
    const app = readSrcSet(
      "networkCanvas/webview/NetworkCanvasApp.tsx",
      "networkCanvas/webview/useDiscoverPane.ts",
      "networkCanvas/webview/useCanvasHostState.ts"
    );
    assert.ok(
      app.includes('post({ type: "addHost"') &&
        app.includes("endpoint: host.endpoint") &&
        app.includes("label: host.label"),
      "Adopt must pass the discovered runtime label to the extension"
    );
    assert.ok(
      app.includes('message.type === "focusNode"') &&
        app.includes("onFocusNode(message.nodeId)") &&
        app.includes("setSelectedId(nodeId)"),
      "the canvas must select the adopted runtime after the extension refreshes the graph"
    );

    const panel = readSrcSet(
      "networkCanvas/networkCanvasPanel.ts",
      "networkCanvas/fleetActions.ts",
      "networkCanvas/fleetTargetResolver.ts"
    );
    assert.ok(
      panel.includes("fleetEndpointLabels") &&
        panel.includes("endpointLabels: fleetEndpointLabels") &&
        panel.includes("label: endpointLabels.get(endpoint)"),
      "fleet targets must keep the discovered runtime label when rendering the configured peer"
    );
    assert.ok(
      panel.includes("focusEndpoint(`fleet:${endpoint}:runtime`)") &&
        panel.includes("pendingFocusNodeId = nodeId") &&
        panel.includes('type: "focusNode"'),
      "adopting a runtime must return to the graph with the new runtime node selected"
    );
  });
});
