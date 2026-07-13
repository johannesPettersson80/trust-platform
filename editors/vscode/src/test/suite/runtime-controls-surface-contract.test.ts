import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";

import { remoteLabelFromEndpoint } from "../../trustHomeModel";
import { normalizeIoState } from "../../runtimeLifecycle";

type MenuItem = { command?: string; when?: string; group?: string };
type Pkg = {
  contributes?: {
    commands?: Array<{ command?: string }>;
    menus?: { "editor/title"?: MenuItem[] };
    viewsContainers?: { activitybar?: Array<{ id?: string }> };
    views?: Record<string, Array<{ id?: string; type?: string }>>;
  };
};

function extensionRoot(): string {
  return path.resolve(__dirname, "..", "..", "..");
}

function loadPackageJson(): Pkg {
  return JSON.parse(
    fs.readFileSync(path.join(extensionRoot(), "package.json"), "utf8"),
  ) as Pkg;
}

function loadSource(file: string): string {
  return fs.readFileSync(path.join(extensionRoot(), "src", file), "utf8");
}

suite("truST sidebar — control surface contract", () => {
  test("exactly one truST activity container + one view, and the view is a WebviewView", () => {
    const contributes = loadPackageJson().contributes ?? {};
    const containers = (contributes.viewsContainers?.activitybar ?? []).map(
      (container) => container.id,
    );
    assert.deepStrictEqual(
      containers,
      ["trust"],
      "Exactly one truST activity-bar container.",
    );
    const views = contributes.views?.trust ?? [];
    assert.deepStrictEqual(
      views.map((view) => view.id),
      ["trust.home"],
      "Exactly one truST sidebar view.",
    );
    assert.strictEqual(
      views[0]?.type,
      "webview",
      "trust.home must be a WebviewView — the runtime selector needs a real dropdown.",
    );
  });

  test("no status-bar / palette Start/Stop commands (one run surface)", () => {
    const runtimeCommands = (loadPackageJson().contributes?.commands ?? [])
      .map((command) => command.command)
      .filter(
        (command): command is string =>
          typeof command === "string" &&
          command.startsWith("trust-lsp.runtime."),
      );
    assert.deepStrictEqual(
      runtimeCommands,
      [],
      "There must be NO trust-lsp.runtime.* commands — the sidebar drives the lifecycle directly.",
    );
  });

  test("the status bar is passive: it only reveals the sidebar, never starts/stops", () => {
    const source = loadSource("runtimeControls.ts");
    assert.ok(
      source.includes("trust.home.focus"),
      "status bar click must reveal the truST sidebar (trust.home.focus)",
    );
    assert.ok(
      !source.includes("registerCommand"),
      "the passive status bar must NOT register any command",
    );
    assert.ok(
      !source.includes("startLocalSimulator") &&
        !source.includes("stopRuntime"),
      "the passive status bar must NOT start/stop the runtime",
    );
  });

  test("the status bar follows the selected target, not a separate simulator-only state", () => {
    const source = loadSource("runtimeControls.ts");
    assert.ok(
      source.includes("getSelectedRuntimeId") &&
        source.includes("onDidChangeSelectedRuntime"),
      "status bar must read and refresh from the shared selected-run-target store",
    );
    assert.ok(
      source.includes("onDidChangeManagedRuntimes"),
      "status bar must refresh when managed-runtime Start/Stop changes the selected target state",
    );
    assert.ok(
      source.includes("selectedRuntime({") &&
        source.includes("listManagedRuntimes(context)") &&
        source.includes("readRemotes()") &&
        source.includes("runtimeAuthoritySelection(") &&
        source.includes(
          "runtimeModelSnapshotForLifecycle(snapshot, authority.target)",
        ) &&
        source.includes("managedSessionId: authority.managedSessionId"),
      "status bar must render through the same accepted-session authority and selectedRuntime model as the sidebar",
    );
    assert.ok(
      source.includes("statusTargetLabel(selected)") &&
        source.includes('return "Simulator"') &&
        source.includes("return selected.id"),
      "status bar text must name the selected target instead of always saying Simulator",
    );
  });

  test("stopping a runtime emits a fresh lifecycle refresh after the session is gone", () => {
    const source = loadSource("runtimeLifecycle.ts");
    const liveValues = loadSource("runtimeLifecycleLiveValues.ts");
    const stopBody = loadSource("runtimeStopOperation.ts");
    assert.ok(
      source.includes("runRuntimeStopOperation(operationId") &&
        stopBody.includes("catch (error)") &&
        stopBody.includes("dependencies.executeStop(activeSession)") &&
        stopBody.includes("DEBUG_STOP_REQUEST_TIMEOUT_MS") &&
        stopBody.includes("await waitForSessionPresence(") &&
        stopBody.includes("dependencies.hasSession") &&
        stopBody.includes(
          'return dependencies.markStopped("Runtime stopped.");',
        ),
      "Stop must stop and wait for the exact lifecycle session even if VS Code focus changes",
    );
    assert.ok(
      /private markStopped\(message: string\): RuntimeLifecycleResult \{[\s\S]*?this\.liveValues\.reset\(\);[\s\S]*?this\.acceptedSessions\.clear\(\);[\s\S]*?this\.operations\.cancel\(\);[\s\S]*?this\.starting = false;[\s\S]*?this\.failure = undefined;[\s\S]*?this\.emitChanged\(\);[\s\S]*?return \{ ok: true, message \};[\s\S]*?\}/.test(
        source,
      ) &&
        /reset\(\): void \{[\s\S]*?this\.ioState = EMPTY_IO_STATE;[\s\S]*?this\.adsState = EMPTY_ADS_LIVE_VALUES_STATE;[\s\S]*?\}/.test(
          liveValues,
        ),
      "successful Stop must emit after the debug session is gone so the passive status bar cannot stay stuck on Running",
    );
  });

  test("the status bar does not pretend a simulator target exists before a project exists", () => {
    const source = loadSource("runtimeControls.ts");
    const projectSource = loadSource("workspaceProject.ts");
    assert.ok(
      source.includes("workspaceHasReadableTrustProject"),
      "status bar must check whether the workspace is a readable truST project before showing a runtime",
    );
    assert.ok(
      source.includes("truST: No project"),
      "no-project and non-truST workspaces must have a neutral status-bar state",
    );
    assert.ok(
      source.indexOf("workspaceHasReadableTrustProject") <
        source.indexOf("selectedRuntime({"),
      "project detection must happen before falling back to the simulator selected-runtime model",
    );
    assert.ok(
      projectSource.includes(
        'readonly kind: "none" | "nonTrust" | "trust" | "malformed"',
      ) &&
        projectSource.includes("manifestReadabilityIssue") &&
        projectSource.includes("vscode.workspace.fs.readFile(manifest)") &&
        projectSource.includes("workspaceHasReadableTrustProject"),
      "status bar and sidebar must share malformed-manifest project detection",
    );
    assert.ok(
      source.includes("createFileSystemWatcher(pattern)") &&
        source.includes('"**/trust-lsp.toml"') &&
        source.includes('"**/runtime.toml"'),
      "status bar must refresh when project marker files appear after first-run project creation",
    );
    assert.ok(
      source.includes("runtimeAuthoritySelection(") &&
        source.includes(
          "runtimeModelSnapshotForLifecycle(snapshot, authority.target)",
        ) &&
        source.includes("managedSessionId: authority.managedSessionId") &&
        !source.includes('snapshot.status.runtimeState === "connected"'),
      "an active attached runtime must override selected-target fallback through shared session authority",
    );
    assert.ok(
      source.includes("return selected.label") &&
        remoteLabelFromEndpoint("unix:///tmp/trust-runtime.sock") === "runtime",
      "attached local runtime endpoints must render as a friendly runtime label, not a raw unix socket path",
    );
  });

  test("Live Values normalizes runtime debug values before webview rendering", () => {
    const state = normalizeIoState({
      scan: 12841,
      inputs: [],
      outputs: [
        {
          address: "%QX0.0",
          name: "Out",
          source: "MQTT topic trust/examples/mqtt/out",
          value: "Bool(true)",
        },
      ],
      memory: [
        { address: "%MX0.0", name: "Flag", value: "Bool(false)" },
        { address: "%MW0", name: "Count", value: "Int(42)" },
        { address: "%MD4", name: "Delay", value: "T#250ms", valueType: "TIME" },
      ],
    });
    assert.strictEqual(state.outputs[0].value, "TRUE");
    assert.strictEqual(
      state.outputs[0].source,
      "MQTT topic trust/examples/mqtt/out",
    );
    assert.strictEqual(state.memory[0].value, "FALSE");
    assert.strictEqual(state.memory[1].value, "42");
    assert.strictEqual(state.memory[2].value, "T#250ms");
    assert.strictEqual(state.memory[2].valueType, "TIME");
    assert.strictEqual(state.scan, 12841);

    const ioPanelSource = loadSource("ioPanel.ts");
    assert.ok(
      ioPanelSource.includes("normalizeIoState(body)") &&
        !ioPanelSource.includes("payload: body ??"),
      "Live Values must not forward raw stIoState values directly to the webview",
    );
  });

  test("simulator launch keeps raw adapter logs out of the first-run surface", () => {
    const source = loadSource("debug/startCommand.ts");
    assert.ok(
      source.includes('internalConsoleOptions: "neverOpen"'),
      "sidebar Start must not auto-open VS Code's Debug Console with raw adapter logs",
    );
    assert.ok(
      source.includes("const lifecycleOwnedStart = Boolean(") &&
        source.includes("startOptions?.lifecycleAttemptId?.trim()") &&
        source.includes("lifecycleOwnedStart") &&
        source.includes("suppressDebugToolbar: true") &&
        source.includes("suppressDebugStatusbar: true") &&
        source.includes("suppressDebugView: true") &&
        source.includes("lifecycleOwnedDebugUi,"),
      "lifecycle-owned sidebar Start must suppress VS Code's duplicate debug toolbar/status/view while no-proof/F5 paths keep normal debugger UI",
    );
  });

  test("ERR-04 control-endpoint override is test-mode only", () => {
    const debugSource = loadSource("debug/startCommand.ts");
    const launchControlSource = loadSource("debug/launchControl.ts");
    assert.ok(
      launchControlSource.includes('"TRUST_UX_DEBUG_CONTROL_ENDPOINT"') &&
        launchControlSource.includes("allowTestControlEndpointOverride"),
      "debug launch endpoint override must exist only for evidence/test runners",
    );
    assert.ok(
      debugSource.includes(
        "context.extensionMode === vscode.ExtensionMode.Test",
      ),
      "the control-endpoint override must be disabled outside VS Code test mode",
    );
    assert.ok(
      launchControlSource.indexOf(
        "process.env[TEST_CONTROL_ENDPOINT_OVERRIDE_ENV]",
      ) < launchControlSource.indexOf("localSimControl(folder?.uri.fsPath)"),
      "ERR-04 evidence must be able to force a real bind-conflict endpoint before the normal local-sim socket is chosen",
    );
    assert.ok(
      debugSource.includes("launchControlEndpointError") &&
        launchControlSource.includes("The runtime port is already in use.") &&
        debugSource.indexOf("await launchControlEndpointError") <
          debugSource.indexOf("vscode.debug.startDebugging("),
      "local launch control-endpoint conflicts must be caught before VS Code starts the debug session",
    );
  });

  test("attach sessions keep raw adapter logs out of canvas and Live Values workflows", () => {
    const debugSource = loadSource("debug.ts");
    const lifecycleSource = loadSource("runtimeOnlineConnection.ts");
    for (const [name, source] of [
      ["debug command attach", debugSource],
      ["runtime lifecycle attach", lifecycleSource],
    ] as const) {
      assert.ok(
        source.includes('request: "attach"') &&
          source.includes('internalConsoleOptions: "neverOpen"'),
        `${name} must not auto-open VS Code's Debug Console with raw adapter logs`,
      );
    }
    const ioPanelSource = loadSource("ioPanel.ts");
    assert.ok(
      ioPanelSource.includes(
        "runtimeLifecycleService.acceptedDebugSession()",
      ) && !ioPanelSource.includes("vscode.debug.startDebugging"),
      "Live Values must reuse the accepted lifecycle session instead of opening its own attach/Debug Console",
    );
    assert.ok(
      debugSource.includes(
        "remoteAttachDebugSessionName(controlConfig.endpoint)",
      ) &&
        lifecycleSource.includes(
          "remoteDebugSessionName(options.targetLabel, status.endpoint)",
        ),
      "native attach sessions must use remote-specific labels when VS Code surfaces the session",
    );
  });

  test("unreachable runtime messages are human-facing and do not expose local socket paths", () => {
    const lifecycleSource = loadSource("runtimeOnlineConnection.ts");
    const modelSource = loadSource("runtimeLifecycleModel.ts");
    assert.ok(
      lifecycleSource.includes("runtimeNotReachableMessage(status.endpoint)") &&
        modelSource.includes(
          "Local runtime is stopped. Start it to connect.",
        ) &&
        modelSource.includes("shortRuntimeEndpointLabel(endpoint)"),
      "runtime lifecycle must humanize unreachable endpoints before surfacing them",
    );
    assert.ok(
      !lifecycleSource.includes(
        "message: `Runtime not reachable: ${status.endpoint}`",
      ),
      "user-facing runtime-unreachable messages must not expose raw socket paths",
    );
  });

  test("remote attach refuses debug-disabled runtimes before reporting connected", () => {
    const lifecycleSource = loadSource("runtimeOnlineConnection.ts");
    assert.ok(
      lifecycleSource.includes("runtimeDebugDisabled(runtimeInfo)") &&
        lifecycleSource.includes(
          "Remote debugging is disabled for this runtime",
        ),
      "remote Connect must fail visibly when runtime.control.debug_enabled is false",
    );
    assert.ok(
      lifecycleSource.indexOf("runtimeDebugDisabled(runtimeInfo)") <
        lifecycleSource.indexOf(
          "vscode.debug.startDebugging(folder, debugConfig)",
        ),
      "debug-disabled status must be checked before launching the debug adapter",
    );
  });

  test("Connect failures show state-specific next actions, not auth for every failure", () => {
    const source = loadSource("trustHomeView.ts");
    const failures = loadSource("trustHomeFailures.ts");
    assert.ok(
      source.includes("connectFailureChoices(result)"),
      "Connect failure actions must be selected by failure kind",
    );
    assert.ok(
      source.includes("OPEN_DEVICES_ACTION") &&
        source.includes("SET_AUTH_TOKEN_ACTION"),
      "Connect failures must distinguish diagnose/open-devices from auth-token entry",
    );
    assert.ok(
      !source.includes(
        'actionFailureMessage(selected, result),\n          "Set auth token"',
      ),
      "sidebar must not offer Set auth token for every failed Connect",
    );
    const choicesBody = failures.slice(
      failures.indexOf("function connectFailureChoices"),
      failures.indexOf("function isRuntimeUnreachableFailure"),
    );
    assert.ok(
      choicesBody.includes("isRuntimeUnreachableFailure") &&
        choicesBody.includes("isAuthTokenFailure"),
      "unreachable and auth failures must be separate branches",
    );
    assert.ok(
      failures.includes(
        "Open Devices & Connections to start or diagnose this runtime.",
      ),
      "unreachable Connect failures must show a visible recovery step in the sidebar",
    );
  });

  test("simulator Start treats a failed I/O probe as a failed launch", () => {
    const source = loadSource("runtimeLifecycle.ts");
    const liveValues = loadSource("runtimeLifecycleLiveValues.ts");
    const coordinator = loadSource("localSimulatorStartCoordinator.ts");
    const transition = loadSource("runtimeDebugTransition.ts");
    const startLocal = source.slice(
      source.indexOf("async startLocalSimulator("),
      source.indexOf("async connectRemote("),
    );
    assert.ok(
      startLocal.includes("coordinateLocalSimulatorStart({") &&
        startLocal.includes("this.liveValues.waitForSimulatorSessionReady(") &&
        liveValues.includes("waitForSimulatorSessionReady(") &&
        liveValues.includes("this.requestIoState({") &&
        liveValues.includes("session: candidate") &&
        coordinator.includes("runOwnedDebugTransition({") &&
        transition.includes(
          "const ready = await dependencies.waitForReady(session)",
        ) &&
        transition.includes("if (!ready.ok)") &&
        transition.includes("return { result: ready, session }") &&
        source.includes("this.failure = outcome.result.ok"),
      "Start must await the exact session readiness contract before claiming the simulator is running",
    );
    assert.ok(
      coordinator.includes("waitForSessionStable(") &&
        coordinator.includes("Simulator stopped during startup") &&
        coordinator.includes("runtime port or target settings"),
      "a debug session that immediately terminates must not be reported as a successful Start",
    );
    assert.ok(
      coordinator.includes("withTimeout(") &&
        coordinator.includes("DEBUG_START_COMMAND_TIMEOUT_MS") &&
        coordinator.includes("Start debugging timed out"),
      "Start must not wait forever on VS Code debug startup errors before rendering an inline sidebar failure",
    );
  });

  test("background I/O refresh failures do not persist as sidebar start failures", () => {
    const source = loadSource("runtimeLifecycle.ts");
    const liveValues = loadSource("runtimeLifecycleLiveValues.ts");
    const readiness = loadSource("runtimeSessionReadiness.ts");
    const requestBody = liveValues.slice(
      liveValues.indexOf("async requestIoState("),
      liveValues.indexOf("async requestIoStateAfterScan("),
    );
    assert.ok(
      liveValues.includes("readonly session?: vscode.DebugSession;") &&
        requestBody.includes(
          "options.session ?? this.dependencies.acceptedSession()",
        ),
      "I/O refresh must be bindable to the exact debug session being accepted",
    );
    assert.ok(
      requestBody.includes("if (options.persistFailure)") &&
        requestBody.includes(
          "this.dependencies.persistFailure(ioFailure, session)",
        ) &&
        source.includes("persistFailure: (failure, session) =>") &&
        source.includes("this.failure = failure") &&
        source.includes("runtimeFailureScopeForSession(session)"),
      "only the Start acceptance probe may persist an I/O failure into the sidebar lifecycle state",
    );
    assert.ok(
      source.includes("waitForReady: (session, timeoutMs) =>") &&
        source.includes("this.liveValues.waitForSimulatorSessionReady(") &&
        liveValues.includes("waitForSimulatorSessionReady(") &&
        readiness.includes("const result = await requestIoState(") &&
        readiness.includes("Math.min(750") &&
        source.includes("this.failure = outcome.result.ok") &&
        source.includes(": outcome.result.failure"),
      "the exact-session acceptance probe must still persist a failed startup so Running cannot be faked",
    );
  });

  test("sidebar renders start failure messages even after simulator stays stopped", () => {
    const controllerSource = loadSource("trustHomeView.ts");
    const webviewSource = loadSource("trustHomeWebview.ts");
    const source = `${controllerSource}\n${webviewSource}`;
    const renderBody = controllerSource.slice(
      controllerSource.indexOf("private async render()"),
      controllerSource.indexOf("private async onMessage("),
    );
    assert.ok(
      renderBody.includes('this.applyMessageKind === "error"') &&
        renderBody.includes("snapshot.failure") &&
        renderBody.includes("lifecycleFailureMessage"),
      "Start/Connect failures must stay visible in the sidebar even when the simulator remains stopped",
    );
    assert.ok(
      !source.includes("withSidebarActionTimeout") &&
        !source.includes("SIDEBAR_ACTION_TIMEOUT_MS") &&
        source.includes("result ??= await this.dispatch(selected)"),
      "the sidebar must await the lifecycle owner instead of racing it with a second timeout",
    );
    assert.ok(
      source.includes(
        'applyMessageEl.style.display = applyMessage ? "block" : "none"',
      ),
      "inline sidebar failures must be visibly rendered; empty display falls back to the CSS hidden rule",
    );
    assert.ok(
      source.includes("} else if (result?.ok)") &&
        source.includes('this.applyMessage = ""') &&
        source.includes('this.applyMessageKind = ""'),
      "successful Start/Stop/Connect/Disconnect must clear stale sidebar action failures",
    );
  });

  test("no ST editor-title Run/Stop controls", () => {
    const items = loadPackageJson().contributes?.menus?.["editor/title"] ?? [];
    const runtimeItems = items.filter((item) =>
      (item.command ?? "").startsWith("trust-lsp.runtime."),
    );
    assert.deepStrictEqual(
      runtimeItems,
      [],
      "editor/title must contribute no runtime Run/Stop — there is one run surface.",
    );
  });

  test("the truST panel is a WebviewView with examples-first onboarding and a compact action surface", () => {
    const controllerSource = loadSource("trustHomeView.ts");
    const webviewSource = loadSource("trustHomeWebview.ts");
    const presentationSource = loadSource("trustHomePresentation.ts");
    const source = `${controllerSource}\n${webviewSource}\n${presentationSource}`;
    const projectSource = loadSource("workspaceProject.ts");
    assert.ok(
      source.includes("registerWebviewViewProvider"),
      "trust.home must be a WebviewViewProvider",
    );
    // Two sidebar states.
    assert.ok(
      source.includes('id="welcome"'),
      "must render the no-project welcome state",
    );
    assert.ok(
      source.includes('id="project"'),
      "must render the project-open state",
    );
    // No-project welcome = Examples first, then Create/Open; no transport controls.
    assert.ok(
      source.includes(">+ Create project<"),
      "welcome offers Create project",
    );
    assert.ok(source.includes(">Open project<"), "welcome offers Open project");
    assert.ok(
      source.includes("Start from example"),
      "welcome offers Start from example",
    );
    assert.ok(
      source.indexOf("Start from example") < source.indexOf("+ Create project"),
      "Start from example must be the headline first-run action",
    );
    const welcomeStart = source.indexOf('id="welcome"');
    const welcomeEnd = source.indexOf('id="project"', welcomeStart);
    const welcome = source.slice(welcomeStart, welcomeEnd);
    assert.ok(
      !welcome.includes('id="action"') && !welcome.includes('id="compile"'),
      "no-project state must not show transport/compile controls",
    );
    assert.ok(
      source.includes("No truST project") &&
        source.includes("does not contain a truST project yet"),
      "an open non-truST folder must explain that it can be initialized as a truST project",
    );
    assert.ok(
      source.includes("Initialize truST here"),
      "an open non-truST folder must offer an explicit initialize action",
    );
    assert.ok(
      source.includes("targetUri: workspaceState.folder.uri") &&
        source.includes("openWorkspace: false"),
      "initializing an open non-truST folder must scaffold that folder instead of opening an unrelated picker",
    );
    assert.ok(
      projectSource.includes('"trust" | "malformed"') &&
        projectSource.includes("manifestReadabilityIssue") &&
        projectSource.includes("vscode.workspace.fs.readFile(manifest)") &&
        source.includes('msg.workspaceKind === "malformed"'),
      "a folder with an unreadable truST manifest must not be classified as an active project",
    );
    assert.ok(
      source.includes("Project needs repair") &&
        source.includes("project settings file cannot be read"),
      "malformed project manifests must render a clear repair state before Compile/Start",
    );
    // Compact action row + visible destinations.
    assert.ok(
      source.includes(">Target<"),
      "the target label must read 'Target'",
    );
    assert.ok(
      source.includes('id="compile"'),
      "sidebar must expose Compile in the fixed action row",
    );
    assert.ok(
      source.includes('id="action"'),
      "sidebar must expose one selected-target lifecycle action",
    );
    assert.ok(
      !source.includes('id="debug"') &&
        !source.includes('id="deploy"') &&
        !source.includes('case "debug"') &&
        !source.includes('case "deploy"'),
      "the novice action row must not expose duplicate Debug or permanently unsupported Deploy controls",
    );
    assert.ok(
      source.includes("node_modules") &&
        source.includes("@vscode") &&
        source.includes("codicons") &&
        source.includes("codicon-play") &&
        source.includes("codicon-debug-stop"),
      "sidebar action buttons must use real VS Code Codicons, not emoji/text glyphs",
    );
    const runtimeButtons = presentationSource;
    const stopButton = runtimeButtons.slice(
      runtimeButtons.indexOf('case "stop"'),
      runtimeButtons.indexOf('case "disconnect"'),
    );
    assert.ok(
      stopButton.includes('label: "Stop"') &&
        stopButton.includes('icon: "codicon-debug-stop"'),
      "running state must show a literal Stop label with the dedicated square stop icon",
    );
    assert.ok(
      source.includes("#action .label { display: inline; }") &&
        source.includes('if (btn.id === "action")'),
      "narrow action-row fitting must never collapse any literal lifecycle label to an ambiguous icon",
    );
    assert.ok(
      !source.includes("🐞") && !source.includes("⚒") && !source.includes("⤓"),
      "sidebar action buttons must not use emoji glyphs that can render as missing squares",
    );
    assert.ok(
      source.includes("showQuickPick") &&
        source.includes("QuickPickItemKind.Separator"),
      "the Target button must open a grouped native QuickPick",
    );
    assert.ok(
      source.includes("Devices &amp; Connections"),
      "nav must offer Devices & Connections",
    );
    assert.ok(
      source.includes(">Libraries<"),
      "nav must offer Libraries as a first-class destination",
    );
    assert.ok(source.includes(">Live Values<"), "nav must offer Live Values");
    assert.ok(source.includes('id="navHmi"'), "nav must offer HMI");
    assert.ok(
      !source.includes('id="navProject"') &&
        !source.includes("projectActionsMenu"),
      "the retired Project bucket must not remain in the sidebar",
    );
    assert.ok(
      source.includes("hmiLabel") && source.includes('"Create HMI"'),
      "the HMI launcher may say Create HMI when the project has no HMI descriptors",
    );
    assert.ok(
      source.includes('createFileSystemWatcher("**/hmi/*.toml")'),
      "the HMI launcher label must refresh when HMI descriptor files are created or removed",
    );
    // Honesty / no jargon.
    assert.ok(
      !source.includes("Network Canvas"),
      "the panel must not surface the jargon 'Network Canvas' (command id stays the same)",
    );
    assert.ok(
      !/>\s*Runtime\s*<\/label>/.test(source),
      "the target label must not regress to the bare backend word 'Runtime'",
    );
  });

  test("stopRuntime is idempotent (a disappeared session after Stop is success, not a warning)", () => {
    const source = loadSource("runtimeStopOperation.ts");
    assert.ok(
      source.includes("waitForSessionPresence"),
      "stop must verify success by the session actually going away",
    );
    assert.ok(
      source.includes("Runtime already stopped."),
      "stopping an already-stopped runtime must be a no-op success",
    );
    // The old bug: returning the 'No active Structured Text debug session.' failure from a Stop.
    const stopBody = source.slice(
      source.indexOf("export async function runRuntimeStopOperation"),
    );
    assert.ok(
      stopBody.length > 0 &&
        !stopBody.includes("No active Structured Text debug session."),
      "stopRuntime must not treat a gone session as the 'No active … session' failure",
    );
  });

  test("managed Stop disconnects Live Values even when fleet stop omits the endpoint", () => {
    const managedSession = loadSource("managedRuntimeSession.ts");
    const home = loadSource("trustHomeView.ts");
    const canvas = loadSource("networkCanvas/lifecycleActions.ts");
    assert.ok(
      managedSession.includes('validatedAuthority?.kind === "managed"') &&
        managedSession.includes("validatedAuthority.id === name") &&
        managedSession.includes("authorityEndpoint === attachedEndpoint") &&
        managedSession.includes("sameLegacyRemoteTarget") &&
        managedSession.includes(
          "sameManagedTarget || sameLegacyRemoteTarget",
        ) &&
        managedSession.includes("return lifecycle.stopRuntime(operationId)"),
      "managed Stop must disconnect by explicit managed-runtime identity, with an exact-endpoint fallback only for legacy remote sessions",
    );
    assert.ok(
      home.includes("disconnect: await disconnectManagedRuntimeAfterStop(") &&
        home.includes("const disconnectResult = disconnect ??") &&
        home.includes("if (!disconnectResult.ok)") &&
        home.includes('action: "disconnect"'),
      "the sidebar Stop path must surface an attached-session disconnect failure",
    );
    assert.ok(
      canvas.includes("disconnectManagedRuntimeAfterStop") &&
        canvas.includes("validatedAuthority") &&
        canvas.includes("const disconnectResult = disconnect ??") &&
        canvas.includes("if (!disconnectResult.ok)") &&
        canvas.includes('recordResult(disconnectResult, "disconnect")'),
      "the canvas Stop path must retain and render an attached-session disconnect failure",
    );
  });

  test("Update running simulation cannot hang forever on a stuck stReload request", () => {
    const source = loadSource("debug.ts");
    const homeSource = loadSource("trustHomeView.ts");
    const homeFailuresSource = loadSource("trustHomeFailures.ts");
    const reloadCommand = source.slice(
      source.indexOf('registerCommand("trust-lsp.debug.reload"'),
      source.indexOf(
        "\n  );\n}",
        source.indexOf('registerCommand("trust-lsp.debug.reload"'),
      ),
    );
    assert.ok(
      source.includes("HOT_RELOAD_REQUEST_TIMEOUT_MS"),
      "Update running simulation must define an explicit timeout",
    );
    assert.ok(
      source.includes("function withTimeout"),
      "Update running simulation must use a timeout helper for adapter requests",
    );
    assert.ok(
      reloadCommand.includes('session.customRequest("stReload"') &&
        reloadCommand.includes("withTimeout(") &&
        reloadCommand.includes("HOT_RELOAD_REQUEST_TIMEOUT_MS"),
      "trust-lsp.debug.reload must bound the stReload custom request",
    );
    assert.ok(
      reloadCommand.includes(
        'diagnosticsGateReason(validityLine(), "update")',
      ) &&
        reloadCommand.includes(
          "return { ok: false, message: gateReason, gated: true }",
        ) &&
        reloadCommand.indexOf("diagnosticsGateReason") <
          reloadCommand.indexOf('session.customRequest("stReload"'),
      "trust-lsp.debug.reload must share the sidebar compile gate before attempting update",
    );
    assert.ok(
      reloadCommand.includes("Update running simulation timed out") &&
        reloadCommand.includes("try again or restart"),
      "a timed-out Update running simulation must fail with a user-facing recovery message",
    );
    assert.ok(
      source.includes("function summarizeReloadCommandMessage") &&
        source.includes("Compile failed — ${sourceErrorCount} error") &&
        reloadCommand.includes("summarizeReloadCommandMessage(rawMessage)") &&
        reloadCommand.includes("Update failed: ${message}") &&
        !reloadCommand.includes("Update failed: ${rawMessage}"),
      "Update notifications must summarize compile failures instead of leaking raw source paths",
    );
    assert.ok(
      source.includes("onDidDebugReload") &&
        source.includes("debugReloadEmitter.fire({ ok: true })") &&
        source.includes("debugReloadEmitter.fire({ ok: false, message })"),
      "reload command must publish success/failure so the sidebar stays in sync",
    );
    assert.ok(
      homeFailuresSource.includes(
        "export function isReloadSuccess(value: unknown)",
      ) &&
        homeFailuresSource.includes("export function reloadFailureMessage(") &&
        homeSource.includes("onDidDebugReload") &&
        homeSource.includes("this.sourceChanged = false") &&
        homeSource.includes("Running simulation updated.") &&
        homeSource.includes("Update failed:") &&
        homeFailuresSource.includes("value.gated === true") &&
        homeFailuresSource.includes("Open Problems, then try again."),
      "sidebar must clear or explain pending Update state from the shared reload result",
    );
  });
});
