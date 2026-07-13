import {
  assert,
  path,
  loadPackageJson,
  readSrc,
  readIoPanelDocumentSource,
} from "./ux-shell-contract-fixtures";

suite("Phase 4 — Live Values (v5 shell)", () => {
  test("Live Values makes the active target and table columns visible", () => {
    const html = readIoPanelDocumentSource();
    const web = readSrc("ioPanel.webview.js");
    for (const [name, source] of [["io-panel/html.ts", html]] as const) {
      assert.ok(
        source.includes('aria-label="Active Live Values target"') &&
          source.includes('id="targetLabel"') &&
          source.includes(".target-strip") &&
          source.includes(".target-label"),
        `${name} must render the active Live Values target above the table`
      );
      assert.ok(
        source.includes('id="scanLabel"') &&
          source.includes("scan --") &&
          source.includes(".scan-label"),
        `${name} must render the runtime scan number above the table`
      );
      assert.ok(
        source.includes(".row-header") &&
          source.includes(".actions-heading"),
        `${name} must style visible table headers for value rows`
      );
      assert.ok(
        source.includes('aria-label="Numeric display format"') &&
          source.includes('data-numeric-format="dec"') &&
          source.includes('data-numeric-format="hex"') &&
          source.includes('data-numeric-format="bin"') &&
          source.includes(".numeric-format") &&
          source.includes(".format-toggle"),
        `${name} must expose the DEC/HEX/BIN numeric display toggle in the Live Values header`
      );
    }
    assert.ok(
      web.includes("targetLabelForStatus") &&
        web.includes('return "Simulator"') &&
        web.includes('runtimeState === "connected"') &&
        web.includes("Connected runtime") &&
        web.includes("Runtime at ") &&
        web.includes("Local runtime (control socket)") &&
        web.includes('"local control socket"') &&
        !web.includes('"local socket "'),
      "the webview must label simulator and attached runtime targets in user-facing words"
    );
    assert.ok(
      web.includes("function updateScanLabel") &&
        web.includes('"scan #" + scan') &&
        web.includes("Rows are from runtime scan #"),
      "the webview must update the visible scan number from each I/O state payload"
    );
    for (const label of ["Name", "Value", "Type", "State", "Actions"]) {
      assert.ok(web.includes(`textContent = "${label}"`), `Live Values rows must label ${label}`);
    }
    assert.ok(
      !web.includes('textContent = "Source"') && web.includes("source-subtitle"),
      "source provenance must stay visible as row context without adding a sixth table column"
    );
  });
  test("Live Values can display word-like values as decimal hex or binary", () => {
    const web = readSrc("ioPanel.webview.js");
    assert.ok(
      web.includes('let numericDisplayBase = "dec"') &&
        web.includes("setNumericDisplayBase") &&
        web.includes("formatIntegerForBase") &&
        web.includes("displayValueForEntry"),
      "the webview must keep numeric display format as explicit panel state"
    );
    assert.ok(
      web.includes('return "16#" + normalized.toString(16).toUpperCase().padStart(width, "0")') &&
        web.includes('return "2#" + normalized.toString(2).padStart(bits, "0")'),
      "the webview must render IEC-style HEX/BIN literals for word-like values"
    );
    for (const type of ['case "BYTE":', 'case "WORD":', 'case "DWORD":']) {
      assert.ok(web.includes(type), `numeric display toggle must cover ${type}`);
    }
  });
  test("Live Values action buttons do not wrap safety verbs", () => {
    for (const [name, source] of [
      ["io-panel/html.ts", readIoPanelDocumentSource()],
      ["visual/runtime/webview/stRuntimePanel.css", readSrc("visual/runtime/webview/stRuntimePanel.css")],
    ] as const) {
      assert.ok(source.includes("white-space: nowrap"), `${name} must keep Write/Force/Release on one line`);
      assert.ok(source.includes(".mini-btn"), `${name} must style action buttons explicitly`);
      assert.ok(
        source.includes("secondary") || source.includes("button-secondary"),
        `${name} must render row Write/Force controls as quiet secondary actions`
      );
      assert.ok(
        !/\.mini-btn\s*\{[\s\S]*background:\s*var\(--trust-accent\)/.test(source) &&
          !/\.mini-btn\s*\{[\s\S]*background:\s*var\(--button-bg\)/.test(source),
        `${name} must not render every row Write/Force action as a filled primary button`
      );
      assert.ok(
        source.includes("minmax(160px, max-content)") &&
          source.includes("column-gap: 6px") &&
          source.includes("width: 46px") &&
          source.includes("width: 62px"),
        `${name} must reserve enough fixed action-column width for the write/force/release controls`
      );
    }
  });
  test("Live Values long signal names cannot collapse the table columns", () => {
    const html = readIoPanelDocumentSource();
    const visual = readSrc("visual/runtime/webview/stRuntimePanel.css");
    for (const [name, source] of [
      ["io-panel/html.ts", html],
      ["visual/runtime/webview/stRuntimePanel.css", visual],
    ] as const) {
      assert.ok(
        source.includes("minmax(116px, 1fr)") &&
          source.includes("minmax(52px, max-content)") &&
          source.includes("minmax(38px, max-content)") &&
          source.includes("minmax(64px, max-content)") &&
          source.includes("minmax(160px, max-content)"),
        `${name} must keep name/value/type/state/actions visible on narrow panes`
      );
      assert.ok(source.includes("overflow-x: auto"), `${name} must stay usable in narrow panes`);
      assert.ok(
        source.includes("text-overflow: ellipsis") && source.includes("white-space: nowrap"),
        `${name} must ellipsize long names instead of letting them push into other columns`
      );
    }
    const web = readSrc("ioPanel.webview.js");
    assert.ok(
      web.includes("[entry.name, entry.address].filter(Boolean).join") &&
        web.includes("nameCell.title = nameTitle"),
      "Live Values rows must expose the full signal name and address in the title when visible text is ellipsized"
    );
    for (const [name, source] of [
      ["io-panel/html.ts", html],
      ["visual/runtime/webview/stRuntimePanel.css", visual],
    ] as const) {
      assert.ok(
        source.includes("overflow-wrap: anywhere") && source.includes("white-space: normal"),
        `${name} must wrap source provenance in pixels instead of hiding it behind ellipsis`
      );
    }
  });
  test("Live Values uses the shared truST product theme tokens", () => {
    const html = readIoPanelDocumentSource();
    for (const [name, source] of [["io-panel/html.ts", html]] as const) {
      assert.ok(
        source.includes("--trust-canvas") &&
          source.includes("--trust-text") &&
          source.includes("--trust-accent"),
        `${name} must use the shared --trust-* product theme roles`
      );
      assert.ok(
        !/--(?:bg|text|muted|border|panel|table-header|row-hover|row-alt|button-bg|button-fg|button-hover|input-bg|input-fg|input-border|error|warning)\s*:/.test(
          source
        ),
        `${name} must not define a private Live Values color token layer`
      );
      assert.ok(
        !/var\(--(?:bg|text|muted|border|panel|table-header|row-hover|row-alt|button-bg|button-fg|button-hover|input-bg|input-fg|input-border|error|warning)\)/.test(
          source
        ),
        `${name} must not consume private Live Values color tokens`
      );
    }
  });
  test("stopped/no-session state is beginner-facing and clears stale values", () => {
    const host = readSrc("ioPanel.ts");
    assert.ok(
      host.includes("Start the Simulator to see live values.") &&
        host.includes("Start the selected runtime to see live values.") &&
        host.includes("Connect to the selected runtime to see live values."),
      "Live Values must explain each stopped or disconnected target in user-facing language"
    );
    assert.ok(
      host.includes("function postEmptyIoState"),
      "Live Values must have a single helper for clearing stale I/O rows"
    );
    const requestIoStateBody = host.slice(
      host.indexOf("async function requestIoState"),
      host.indexOf("async function writeInput")
    );
    assert.ok(
      requestIoStateBody.includes("postUnavailableLiveValues(status);"),
      "a no-session request must clear stale rows and publish stopped guidance through the unavailable helper"
    );
    const lifecycleEvents = readSrc("runtimeLifecycleEvents.ts");
    assert.ok(
      lifecycleEvents.includes("vscode.debug.onDidTerminateDebugSession((session) =>") &&
        lifecycleEvents.includes("deps.setIoState(EMPTY_IO_STATE)") &&
        lifecycleEvents.includes("deps.setAdsState(EMPTY_ADS_LIVE_VALUES_STATE)") &&
        host.includes("void refreshLiveValuesForLifecycle()"),
      "debug session termination must clear stale rows through the shared lifecycle refresh"
    );
    assert.ok(
      !/payload:\s*"No active Structured Text debug session\."/.test(host),
      "Live Values must not display the raw debug-adapter no-session message"
    );
    assert.ok(
      /No debugger available/i.test(host) && /stIoState/i.test(host),
      "Live Values must map disconnected attach-mode stIoState failures to the same beginner-facing empty state"
    );
    assert.ok(
      host.includes("Connect to the selected runtime to see live values."),
      "Live Values must tell disconnected remote users to Connect, not Start"
    );
    assert.ok(
      host.includes("runtimeMode === \"online\"") &&
        host.includes("runtimeState !== \"connected\""),
      "Live Values disconnected guidance must branch on the selected target state"
    );
  });
  test("Live Values does not expose runtime lifecycle controls", () => {
    const html = readIoPanelDocumentSource();
    assert.ok(
      !html.includes('id="runtimeStart"'),
      "Live Values must not render a Start/Stop/Connect/Disconnect lifecycle button"
    );
    assert.ok(
      !html.includes('aria-label="Runtime mode"') && !html.includes('class="mode-toggle"'),
      "Live Values must not render a Local/External target selector"
    );
    assert.ok(
      html.includes('id="releaseAllForces"'),
      "Live Values must keep value-safety controls such as Release all forces"
    );
  });
  test("attached runtimes are labelled Connected, not Stopped or Running", () => {
    const web = readSrc("ioPanel.webview.js");
    const status = readSrc("io-panel/status.ts");
    const visualController = readSrc("visual/runtime/webview/stRuntimePanelController.ts");
    for (const [name, source] of [
      ["ioPanel.webview.js", web],
      ["visual runtime controller", visualController],
    ] as const) {
      assert.ok(
        /runtimeState\s*===\s*"connected"[\s\S]{0,120}\?\s*"Connected"/.test(source),
        `${name} must show Connected for attach-mode Live Values sessions`
      );
      assert.ok(
        !source.includes('const label = isRunning ? "Running" : "Stopped"') &&
          !source.includes('runtimeStatusText.textContent = isRunning ? "Running" : "Stopped"'),
        `${name} must not use the old connected-as-running label pattern`
      );
    }
    assert.ok(
      status.includes('request === "attach"') &&
        status.includes("session.configuration.endpoint.trim()"),
      "Live Values status must source the active attach endpoint from the debug session"
    );
  });
  test("Live Values lifecycle pill is lifecycle-only and does not fake remote running", () => {
    const web = readSrc("ioPanel.webview.js");
    const status = readSrc("io-panel/status.ts");
    assert.ok(
      web.includes("runtimeStatusText.textContent = label"),
      "Live Values pill must render only the lifecycle label"
    );
    assert.ok(
      web.includes('payload.runtimeMode === "online"') && web.includes('"Not connected"'),
      "Live Values must label an unattached online target as Not connected"
    );
    assert.ok(
      !web.includes("`${label} · ${adsText}`") && !web.includes("payload.ads && payload.ads.text"),
      "Live Values pill must not append ADS/protocol commentary to lifecycle state"
    );
    const onlineReachableBranch = status.slice(
      status.indexOf('if (!running && runtimeMode === "online"'),
      status.indexOf("if (!access)")
    );
    assert.ok(
      onlineReachableBranch.includes("endpointReachable = await probeEndpointReachable(endpoint)") &&
        onlineReachableBranch.includes("fetchRuntimeStatusReport(endpoint, authToken)"),
      "unattached online targets may be probed for access/reachability"
    );
    assert.ok(
      !onlineReachableBranch.includes('runtimeState = "running"'),
      "a reachable remote without an attached Live Values session must stay Not connected, not Running"
    );
  });
  test("Live Values clears connected UI immediately when a debug session terminates", () => {
    const host = readSrc("ioPanel.ts");
    const lifecycleEvents = readSrc("runtimeLifecycleEvents.ts");
    assert.ok(
      lifecycleEvents.includes("vscode.debug.onDidTerminateDebugSession((session) =>") &&
        lifecycleEvents.includes("deps.setIoState(EMPTY_IO_STATE)") &&
        lifecycleEvents.includes("deps.setAdsState(EMPTY_ADS_LIVE_VALUES_STATE)") &&
        lifecycleEvents.includes("deps.emit()") &&
        host.includes("runtimeLifecycleService.onDidChange((change) =>") &&
        host.includes("isStructuralRuntimeLifecycleChange(change)") &&
        host.includes("void refreshLiveValuesForLifecycle()"),
      "terminated sessions must clear lifecycle values and drive the one structural Live Values refresh path"
    );
    const unavailableBody = host.slice(
      host.indexOf("function postUnavailableLiveValues"),
      host.indexOf("function terminatedSessionStatus")
    );
    assert.ok(
      unavailableBody.includes("postEmptyIoState();") &&
        unavailableBody.includes("message || liveValuesUnavailableMessage(status)") &&
        unavailableBody.includes("payload: statusMessage"),
      "terminated sessions must also clear stale rows and replace stale role/success banners with the correct unavailable message"
    );
    assert.ok(
      host.includes("/I\\/O state request failed:\\s*Canceled/i.test(message)"),
      "a canceled I/O request during Stop must clear stale Live Values instead of leaving old LIVE rows"
    );
    const web = readSrc("ioPanel.webview.js");
    assert.ok(
      web.includes("function clearUnavailableRuntimeStatus") &&
        web.includes("Start (?:the Simulator|the selected runtime) to see live values") &&
        web.includes('runtimeState: "stopped"') &&
        web.includes("clearUnavailableRuntimeStatus(payload)"),
      "the webview must clear stale Connected pills when the host reports Live Values unavailable"
    );
    assert.ok(
      host.includes("postUnavailableLiveValues(status)") &&
        host.includes("runtimeLifecycleService.acceptedDebugSession()"),
      "loss of the accepted session must publish unavailable state before any later poll"
    );
  });
  test("Structured Text Stop waits for termination before callers capture the UI", () => {
    const debug = readSrc("debug.ts");
    const stopBody = debug.slice(
      debug.indexOf('vscode.commands.registerCommand("trust-lsp.debug.stop"'),
      debug.indexOf('"trust-lsp.debug.io.write"')
    );
    assert.ok(
      debug.includes("function waitForStructuredTextSessionTerminated") &&
        debug.includes("vscode.debug.onDidTerminateDebugSession"),
      "Stop must have an explicit termination wait helper"
    );
    assert.ok(
      stopBody.includes("runtimeLifecycleService.acceptedDebugSession()") &&
        !stopBody.includes("vscode.debug.activeDebugSession"),
      "Stop must use the exact lifecycle-accepted Structured Text session instead of VS Code focus"
    );
    assert.ok(
      stopBody.includes("const terminated = waitForStructuredTextSessionTerminated(session)") &&
        stopBody.includes("await vscode.debug.stopDebugging(session)") &&
        stopBody.includes("const stopped = await terminated") &&
        stopBody.includes("await sleep(DEBUG_STOP_UI_SETTLE_MS)"),
      "Stop command must not resolve before the structured-text session termination event and UI settle"
    );
  });
  test("Structured Text debugger exposes a named truST simulator configuration", () => {
    const pkg = loadPackageJson();
    const debug = readSrc("debug.ts");
    const structuredTextDebugger = pkg.contributes?.debuggers?.find(
      (entry) => entry.type === "structured-text"
    );
    assert.ok(structuredTextDebugger, "package.json must contribute the ST debugger");
    assert.ok(
      structuredTextDebugger?.initialConfigurations?.some(
        (config) => config.name === "truST Simulator" && config.request === "launch"
      ),
      "the native Run and Debug selector must have a user-facing truST Simulator launch option"
    );
    assert.ok(
      debug.includes("provideDebugConfigurations") &&
        debug.includes('name: "truST Simulator"') &&
        debug.includes("DebugConfigurationProviderTriggerKind.Dynamic"),
      "the debug configuration provider must supply a dynamic truST Simulator option, not leave VS Code at No Configurations"
    );
  });
  test("Live Values uses the selected runtime label instead of exposing raw endpoints", () => {
    const web = readSrc("ioPanel.webview.js");
    const status = readSrc("io-panel/status.ts");
    const lifecycle = readSrc("runtimeLifecycle.ts");
    const managedAttach = readSrc("managedRuntimeSession.ts");
    const home = readSrc("trustHomeView.ts");
    const canvas = readSrc("networkCanvas/networkCanvasPanel.ts");
    const canvasLifecycle = readSrc("networkCanvas/lifecycleActions.ts");
    const inspector = readSrc("networkCanvas/webview/NodeInspector.tsx");

    assert.ok(
      web.includes("payload.targetLabel") &&
        web.indexOf("payload.targetLabel") <
          web.indexOf('return endpoint ? "Runtime at " + endpoint'),
      "Live Values must prefer the friendly selected target label before falling back to endpoint text"
    );
    assert.ok(
      status.includes("session?.configuration?.targetLabel") &&
        status.includes("targetLabel,"),
      "runtime status must carry the active attach target label"
    );
    assert.ok(
      lifecycle.includes("targetLabel?: string") &&
        lifecycle.includes("targetLabel,") &&
        lifecycle.includes("async connectRemote(") &&
        lifecycle.includes("endpoint: string"),
      "the shared lifecycle attach path must accept and pass a friendly target label"
    );
    assert.ok(
      managedAttach.includes("managedRuntimeLabel(name)") &&
        managedAttach.includes("runtimeLifecycleService.connectRemote("),
      "managed Start must label Live Values with the same name shown in the sidebar Target"
    );
    assert.ok(
      /runtimeLifecycleService\.connectRemote\(\s*selected\.id,\s*selected\.label,?\s*\)/.test(home),
      "sidebar Connect must pass its selected target label into Live Values"
    );
    assert.ok(
      inspector.includes('type: "runtimeConnect"') &&
        inspector.includes("label: str(node.data.label)") &&
        canvasLifecycle.includes('typeof message.label === "string"') &&
        canvasLifecycle.includes("this.dependencies.connectRemote(endpoint, label)") &&
        canvas.includes("runtimeLifecycleService.connectRemote(endpoint, label)"),
      "canvas Connect must pass the selected node label into Live Values"
    );
  });
  test("Live Values mirrors runtime lifecycle without re-polling every I/O event", () => {
    const host = readSrc("ioPanel.ts");
    assert.ok(
      host.includes("runtimeLifecycleService.onDidChange"),
      "Live Values must subscribe to the same lifecycle model as the sidebar and Devices canvas"
    );
    const subscriptionBody = host.slice(
      host.indexOf("runtimeLifecycleService.onDidChange"),
      host.indexOf("const activeSession = vscode.debug.activeDebugSession")
    );
    assert.ok(
      subscriptionBody.includes("void sendRuntimeStatus();"),
      "lifecycle changes must refresh the status badge"
    );
    assert.ok(
      !subscriptionBody.includes("void requestIoState();"),
      "I/O state events already update the table; requesting another state on every lifecycle change creates a DAP polling loop"
    );
  });
});
