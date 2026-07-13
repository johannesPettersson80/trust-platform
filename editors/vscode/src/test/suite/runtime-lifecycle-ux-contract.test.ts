import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";

import {
  remoteLabelFromEndpoint,
  runtimeOptions,
  selectedRuntime,
  SIMULATOR_RUNTIME_ID,
  type RuntimeModelSnapshot,
} from "../../trustHomeModel";
import {
  runtimeNodeControlLayout,
  runtimeNodeControls,
} from "../../networkCanvas/webview/runtimeNodeControls";
import {
  isStructuralRuntimeLifecycleChange,
  normalizeIoState,
  withTimeout,
} from "../../runtimeLifecycle";
import {
  lifecycleStartAttemptId,
  RuntimeStartAttemptRegistry,
} from "../../debug/startAttempt";
import {
  debugSessionAcceptancePath,
  selectLifecycleDebugSession,
  terminatedSessionOwnsLifecycleState,
} from "../../debug/sessionSelection";
import { LatestOnlyRevision } from "../../latestOnlyRevision";
import {
  effectiveLifecycleEntryFailure,
  lifecycleActionSucceeded,
  runtimeLifecyclePhase,
} from "../../lifecycleEntryFailure";
import {
  formatManagedRuntimeLogs,
  isManagedLifecycleSuccess,
  managedRuntimeLabel,
  normalizeManagedState,
  parseRuntimeControlAuthToken,
  toManagedRuntimes,
} from "../../localRuntimeModel";

// Regression guard for the v3 UX RESET (vscode-ux-overhaul-plan.md §0/§6/§8/§9): ONE run surface — a
// truST sidebar WebviewView with a target selector and a SINGLE state-specific action. Literal verbs
// (Start/Stop/Connect/Disconnect). No duplicate Start buttons (no status-bar Start/Stop, no ST
// editor-title Run/Stop). NEVER a fake remote "Stop". Comms route is "Connect runtime or device",
// not the words "Network Canvas".

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

function snap(over: Partial<RuntimeModelSnapshot> = {}): RuntimeModelSnapshot {
  return {
    runtimeMode: "simulate",
    runtimeState: "stopped",
    endpoint: "",
    endpointConfigured: false,
    endpointReachable: false,
    starting: false,
    ...over,
  };
}

suite("Runtime lifecycle UX contracts", function () {
  test("runtime lifecycle change reasons isolate scan-rate I/O from structural renders", () => {
    assert.strictEqual(
      isStructuralRuntimeLifecycleChange({ kind: "lifecycle" }),
      true,
    );
    assert.strictEqual(
      isStructuralRuntimeLifecycleChange({ kind: "io" }),
      false,
    );
  });

  test("the shared lifecycle phase model keeps every surface on one state", () => {
    assert.strictEqual(runtimeLifecyclePhase(true, undefined, false), "starting");
    assert.strictEqual(runtimeLifecyclePhase(false, "launch", true), "running");
    assert.strictEqual(runtimeLifecyclePhase(false, "attach", true), "connected");
    assert.strictEqual(runtimeLifecyclePhase(false, "launch", false), "stopped");
  });

  test("structural UI consumers ignore scan-rate lifecycle I/O events", () => {
    for (const file of [
      "trustHomeView.ts",
      "runtimeControls.ts",
      "ioPanel.ts",
    ]) {
      const source = loadSource(file);
      assert.ok(
        source.includes("isStructuralRuntimeLifecycleChange(change)"),
        `${file} must not rerender structural chrome for every stIoState scan`,
      );
    }
    assert.ok(
      loadSource("networkCanvas/lifecycleRefreshPolicy.ts").includes(
        "isStructuralRuntimeLifecycleChange(change)",
      ),
      "Devices must share the same structural-event policy",
    );
  });

  test("a successful lifecycle transition clears stale failures from every entry point", () => {
    const staleCanvasFailure = { message: "Canvas start failed" };
    const staleSidebarFailure = { message: "Sidebar start failed" };
    assert.strictEqual(
      effectiveLifecycleEntryFailure(
        staleCanvasFailure,
        undefined,
        "start",
        "starting",
      ),
      undefined,
      "a new sidebar start must clear an earlier canvas failure while Starting",
    );
    assert.strictEqual(
      effectiveLifecycleEntryFailure(
        staleSidebarFailure,
        undefined,
        "start",
        "running",
      ),
      undefined,
      "a successful canvas start must clear an earlier sidebar failure",
    );
    const currentLifecycleFailure = { message: "Current start failed" };
    assert.strictEqual(
      effectiveLifecycleEntryFailure(
        staleCanvasFailure,
        currentLifecycleFailure,
        "start",
        "stopped",
      ),
      currentLifecycleFailure,
      "the shared lifecycle failure must supersede entry-point-local history",
    );
    assert.strictEqual(
      effectiveLifecycleEntryFailure(
        { message: "Stop timed out" },
        undefined,
        "stop",
        "running",
      )?.message,
      "Stop timed out",
      "ordinary I/O while still running must not erase a failed Stop",
    );
    const staleIoFailure = { message: "Start the Simulator before adding I/O" };
    for (const phase of ["starting", "running"] as const) {
      assert.strictEqual(
        effectiveLifecycleEntryFailure(
          staleIoFailure,
          undefined,
          "other",
          phase,
        ),
        undefined,
        `a structural ${phase} transition must clear an earlier canvas I/O failure`,
      );
    }
    assert.strictEqual(
      effectiveLifecycleEntryFailure(
        staleIoFailure,
        undefined,
        "other",
        "stopped",
      ),
      staleIoFailure,
      "the canvas I/O recovery message must remain while the Simulator is stopped",
    );
    let hiddenCanvasFailure: { message: string } | undefined = {
      message: "Earlier canvas start failed",
    };
    if (lifecycleActionSucceeded("start", "running")) {
      hiddenCanvasFailure = undefined;
    }
    assert.strictEqual(
      hiddenCanvasFailure,
      undefined,
      "after a hidden-panel sidebar success, a later Stop/reopen cannot resurrect the old canvas failure",
    );
    const canvas = loadSource("networkCanvas/networkCanvasPanel.ts");
    const home = loadSource("trustHomeView.ts");
    assert.ok(
      canvas.includes("lifecycleActionSucceeded(") &&
        canvas.indexOf("lifecycleActionSucceeded(") <
          canvas.indexOf("void refreshNetworkCanvasPanel();") &&
        home.includes("lifecycleActionSucceeded(") &&
        home.includes("runtimeLifecycleService.phase()"),
      "hidden surfaces must reconcile action outcomes synchronously, while ordinary I/O events preserve a failed Stop",
    );
  });

  test("changing the selected runtime clears entry-point-specific lifecycle errors", () => {
    const source = loadSource("trustHomeView.ts");
    const onSelect = source.slice(
      source.indexOf("private async onSelect("),
      source.indexOf("private async chooseTarget("),
    );
    assert.ok(
      onSelect.includes("this.lifecycleActionFailure = undefined"),
      "a failed remote Connect must not be rendered under Simulator after target selection changes",
    );
  });

  test("only the newest asynchronous lifecycle render may commit", () => {
    const revision = new LatestOnlyRevision();
    const runningRender = revision.begin();
    const stoppedRender = revision.begin();
    assert.strictEqual(revision.isCurrent(runningRender), false);
    assert.strictEqual(revision.isCurrent(stoppedRender), true);
    revision.invalidate();
    assert.strictEqual(revision.isCurrent(stoppedRender), false);
  });

  test("late lifecycle sessions cannot become the active simulator", () => {
    const attempts = new RuntimeStartAttemptRegistry("test-host");
    const first = attempts.begin();
    assert.strictEqual(attempts.disposition(first, false), "active");
    attempts.reject(first);
    assert.strictEqual(attempts.disposition(first, false), "rejected");

    const second = attempts.begin();
    assert.notStrictEqual(second, first);
    assert.strictEqual(attempts.disposition(first, false), "rejected");
    assert.strictEqual(attempts.disposition(second, false), "active");
    attempts.accept(second);
    assert.strictEqual(attempts.disposition(second, true), "accepted");
    assert.strictEqual(attempts.disposition(undefined, false), "external");
    assert.strictEqual(attempts.disposition(undefined, true, true), "rejected");
    assert.strictEqual(
      lifecycleStartAttemptId({ lifecycleAttemptId: " attempt-7 " }),
      "attempt-7",
    );

    const lifecycleSource = loadSource("runtimeLifecycle.ts");
    const eventSource = loadSource("runtimeLifecycleEvents.ts");
    const startCommandSource = loadSource("debug/startCommand.ts");
    assert.ok(
      lifecycleSource.includes("lifecycleAttemptId: attemptId") &&
        startCommandSource.includes("config[LIFECYCLE_START_ATTEMPT_FIELD]") &&
        eventSource.includes("Rejecting late Structured Text session") &&
        lifecycleSource.includes("waitForLifecycleSessionResult("),
      "lifecycle-owned starts must carry an attempt id from command to session and reject late attempts",
    );
  });

  test("an active duplicate cannot replace the accepted simulator", () => {
    const accepted = { id: "session-a" };
    const duplicate = { id: "session-b" };
    const selected = selectLifecycleDebugSession(
      duplicate,
      [accepted],
      (session) => session.id,
      (session) => session.id === accepted.id,
      (session) => session.id === duplicate.id,
    );
    assert.strictEqual(
      selected,
      accepted,
      "all lifecycle surfaces and exact Stop must remain bound to accepted session A while duplicate B terminates",
    );

    const lifecycleSource = loadSource("runtimeLifecycle.ts");
    assert.ok(
      lifecycleSource.includes("this.rejectedSessions.add(key)") &&
        lifecycleSource.includes("this.sessions.delete(key)") &&
        lifecycleSource.includes("selectLifecycleDebugSession("),
      "duplicate rejection must become visible synchronously, before VS Code finishes stopDebugging",
    );
  });

  test("terminating a rejected duplicate cannot cancel another external start", () => {
    assert.strictEqual(
      terminatedSessionOwnsLifecycleState(false, false),
      false,
      "untracked duplicate B must have zero effect while external session A remains Starting",
    );
    assert.strictEqual(
      terminatedSessionOwnsLifecycleState(true, false),
      true,
      "tracked external session A owns its eventual termination transition",
    );
    assert.strictEqual(
      terminatedSessionOwnsLifecycleState(false, true),
      true,
      "the current lifecycle attempt owns its termination even if failure cleanup already untracked it",
    );
  });

  test("local simulator lifecycle stays exclusively in the truST sidebar", () => {
    for (const health of ["stopped", "connected", "starting"]) {
      const controls = runtimeNodeControls({
        isLocal: true,
        health,
        attached: health === "connected",
      });
      assert.ok(
        controls.every((control) => !/^(start|stop)$/i.test(control.label)),
        `${health} must not create a second Simulator lifecycle surface`,
      );
    }

    const controlSource = loadSource(
      "networkCanvas/webview/runtimeNodeControls.ts",
    );
    const hostLifecycleSource = [
      loadSource("networkCanvas/networkCanvasPanel.ts"),
      loadSource("networkCanvas/lifecycleActions.ts"),
    ].join("\n");
    assert.ok(
      !controlSource.includes('"startLocalSimulator"') &&
        !controlSource.includes('"stopLocalSimulator"') &&
        !hostLifecycleSource.includes('"startLocalSimulator"') &&
        !hostLifecycleSource.includes('"stopLocalSimulator"'),
      "neither the webview nor host controller may advertise dead Simulator lifecycle variants",
    );

    const inspector = [
      loadSource("networkCanvas/webview/NodeInspector.tsx"),
      loadSource("networkCanvas/webview/NodeSummaryView.tsx"),
    ].join("\n");
    assert.ok(
      inspector.includes('node.type === "runtime"') &&
        inspector.includes("runtimeNodeControlsForNode({") &&
        inspector.includes("nodeId: node.id") &&
        inspector.includes(
          "Use Start and Stop in the truST sidebar on the left.",
        ),
      "the local Simulator inspector must be status-only and point to the one visible sidebar control",
    );
  });

  test("Start and Connect preserve editor focus until Live Values is explicitly opened", () => {
    const source = loadSource("trustHomeView.ts");
    const runAction = source.slice(
      source.indexOf("private async runAction()"),
      source.indexOf("private async runManagedAction("),
    );
    const managedAction = source.slice(
      source.indexOf("private async runManagedAction("),
      source.indexOf("private async applyChanges("),
    );
    const messageHandler = source.slice(
      source.indexOf("private async onMessage("),
      source.indexOf("private async createProjectFromWelcome("),
    );

    assert.ok(
      !runAction.includes('executeCommand("trust-lsp.debug.openIoPanel")'),
      "successful simulator Start and remote Connect must not reveal Live Values",
    );
    assert.ok(
      managedAction.includes("attachManagedRuntimeAfterStart") &&
        !managedAction.includes(
          'executeCommand("trust-lsp.debug.openIoPanel")',
        ),
      "managed Start must keep its background attach without revealing Live Values",
    );
    assert.ok(
      messageHandler.includes('case "navLiveValues"') &&
        messageHandler.includes(
          'await vscode.commands.executeCommand("trust-lsp.debug.openIoPanel")',
        ),
      "Live Values must remain available from its explicit sidebar destination",
    );
  });

  test("failed Simulator Start offers one kind-specific recovery action immediately", () => {
    const source = loadSource("trustHomeView.ts");
    const webview = loadSource("trustHomeWebview.ts");
    const failures = loadSource("trustHomeFailures.ts");
    const runAction = source.slice(
      source.indexOf("private async runAction()"),
      source.indexOf("private async runManagedAction("),
    );
    assert.ok(
      runAction.includes('selected.primary.action === "start"') &&
        runAction.includes("...startFailureChoices(result.failure)") &&
        runAction.includes("await openSelectedRuntimeToml()") &&
        runAction.includes("openStructuredTextDebuggerLogs()"),
      "the immediate Start warning must execute the same safe config/log recovery used by Devices & Connections",
    );

    const choicesBody = failures.slice(
      failures.indexOf("function startFailureChoices("),
      failures.indexOf("function connectFailureChoices("),
    );
    assert.ok(
      choicesBody.includes('failure.kind === "configuration"') &&
        choicesBody.includes("return [OPEN_RUNTIME_TOML_ACTION]") &&
        choicesBody.includes("return [OPEN_RUNTIME_LOGS_ACTION]") &&
        !choicesBody.includes("return []") &&
        !choicesBody.includes("failure.detail"),
      "configuration gets only Open runtime.toml, every other failed Start gets exactly Open logs, and raw detail cannot select or enter visible copy",
    );
    assert.ok(
      source.includes('action: "openRuntimeToml"') &&
        webview.includes('id="recoveryAction"') &&
        webview.includes("msg.recoveryAction"),
      "configuration recovery must remain visible in the sidebar after the warning closes",
    );

    const panelSource = loadSource("networkCanvas/networkCanvasPanel.ts");
    const canvasLifecycleSource = loadSource(
      "networkCanvas/lifecycleActions.ts",
    );
    assert.ok(
      panelSource.includes("new NetworkCanvasLifecycleActions(") &&
        canvasLifecycleSource.includes("openSelectedRuntimeToml()") &&
        canvasLifecycleSource.includes("openStructuredTextDebuggerLogs()") &&
        !panelSource.includes("async function openRuntimeToml()"),
      "sidebar and canvas must share one recovery implementation instead of duplicating file-opening logic",
    );
  });

  test("a debug-session event stays Starting until the simulator is accepted", () => {
    const lifecycleSource = loadSource("runtimeLifecycle.ts");
    const eventSource = loadSource("runtimeLifecycleEvents.ts");
    const statusSource = loadSource("io-panel/status.ts");
    const startListener = eventSource.slice(
      eventSource.indexOf("vscode.debug.onDidStartDebugSession"),
      eventSource.indexOf("vscode.debug.onDidTerminateDebugSession"),
    );
    assert.ok(
      !startListener.includes("this.starting = false"),
      "VS Code creating a debug session must not immediately publish Running",
    );
    assert.ok(
      lifecycleSource.includes("private readonly acceptedSessions") &&
        lifecycleSource.includes("this.acceptedSessions.add(key)") &&
        lifecycleSource.includes("isSessionAccepted: (session) =>"),
      "the lifecycle must explicitly accept a simulator only after its startup probe",
    );
    assert.ok(
      statusSource.includes("deps.isSessionAccepted?.(session) ?? true"),
      "runtime status must distinguish an announced session from an accepted running session",
    );
  });

  test("failed simulator acceptance terminates the misleading debug session", () => {
    const source = loadSource("runtimeLifecycle.ts");
    const readiness = loadSource("runtimeSessionReadiness.ts");
    assert.ok(
      source.includes(
        "const ownedSession = outcome.session ?? this.sessionForAttempt(attemptId)",
      ) &&
        source.includes("await this.terminateUnacceptedSession(ownedSession)"),
      "failed I/O and stability checks must remove the session that drives the Stop button",
    );
    assert.ok(
      readiness.includes("stopDebugging(session)") &&
        readiness.includes(
          "Terminating the rejected Structured Text session timed out.",
        ),
      "rejected sessions must be stopped through a bounded exact-session VS Code request",
    );
  });

  test("simulator acceptance proves DAP, exact control credentials, and I/O readiness", () => {
    const readyBody = loadSource("runtimeSessionReadiness.ts");
    assert.ok(
      readyBody.includes('session.customRequest("trustSimulatorStatus")') &&
        readyBody.includes("readiness.ready !== true"),
      "an empty stIoState reply must not be mistaken for a loaded simulator",
    );
    assert.ok(
      readyBody.includes("simulatorControlFromDebugConfiguration(") &&
        readyBody.includes("session.configuration") &&
        readyBody.includes(
          "requestRuntimeStatus(control.endpoint, control.authToken",
        ),
      "acceptance must authenticate the exact endpoint and token injected into this DAP session",
    );
    assert.ok(
      readyBody.includes("requestIoState(") &&
        readyBody.includes("Math.min(750"),
      "the final I/O probe must stay bound to the exact session being accepted",
    );
    assert.ok(
      readyBody.includes("simulatorStartupIncompleteFailure()") &&
        readyBody.includes(
          "Simulator startup could not finish: debug session did not provide required control metadata.",
        ) &&
        !readyBody.includes(
          "Simulator started without its control endpoint or authentication token.",
        ),
      "missing extension-owned control metadata must direct the user to logs without exposing endpoint/token internals in the UI",
    );
  });

  test("online runtime status failures keep raw errors out of visible copy", () => {
    const onlineBody = loadSource("runtimeOnlineConnection.ts");
    assert.ok(
      onlineBody.includes("failure: runtimeStatusCheckFailure(err)") &&
        !onlineBody.includes("message: `Runtime status check failed:"),
      "online runtime status errors must use generic visible copy while retaining raw detail only in the failure object",
    );
  });

  test("remote Connect authenticates before its stamped exact-session attach", () => {
    const online = loadSource("runtimeOnlineConnection.ts");
    const lifecycle = loadSource("runtimeLifecycle.ts");
    const readiness = loadSource("runtimeSessionReadiness.ts");
    const authCheck = online.indexOf(
      "runtimeInfo = await requestRuntimeStatus(",
    );
    const attach = online.indexOf(
      "vscode.debug.startDebugging(folder, debugConfig)",
    );
    assert.ok(
      authCheck >= 0 &&
        attach > authCheck &&
        online.slice(authCheck, attach).includes("status.endpoint") &&
        online.slice(authCheck, attach).includes("authToken || undefined"),
      "remote Connect must authenticate before creating the attach session",
    );
    assert.ok(
      online.includes("debugConfig[LIFECYCLE_START_ATTEMPT_FIELD]") &&
        lifecycle.includes("waitForAttachedSessionReady(") &&
        readiness.includes("waitForAttachedSessionReady(") &&
        !readiness
          .slice(
            readiness.indexOf(
              "export async function waitForAttachedSessionReady(",
            ),
            readiness.indexOf(
              "export async function waitForSimulatorSessionReady(",
            ),
          )
          .includes("trustSimulatorStatus"),
      "attach must carry the owned operation id and use remote I/O readiness rather than Simulator readiness",
    );
    assert.strictEqual(debugSessionAcceptancePath("attach"), "remote_attach");
    assert.strictEqual(debugSessionAcceptancePath("launch"), "local_simulator");
    assert.ok(
      lifecycle.includes('kind: "remote"') &&
        lifecycle.includes("endpoint,") &&
        loadSource("networkCanvas/lifecycleModel.ts").includes(
          'snapshot?.failureScope?.kind === "remote"',
        ),
      "remote attach failure stays scoped to its endpoint and never paints Simulator",
    );
  });

  test("wedged DAP requests cannot leave lifecycle Starting forever", async () => {
    const neverReplies = new Promise<void>(() => undefined);
    await assert.rejects(
      withTimeout(neverReplies, 15, "DAP readiness timed out"),
      /DAP readiness timed out/,
    );
    const source = loadSource("runtimeLifecycle.ts");
    const liveValues = loadSource("runtimeLifecycleLiveValues.ts");
    const readiness = loadSource("runtimeSessionReadiness.ts");
    assert.ok(
      readiness.includes("Simulator readiness request timed out.") &&
        liveValues.includes("I/O state request timed out.") &&
        readiness.includes("deadline - Date.now()"),
      "simulator and attach DAP calls must be bounded by the remaining acceptance deadline",
    );
  });

  test("activation never publishes an existing DAP session as Running without acceptance", () => {
    const source = loadSource("runtimeLifecycleEvents.ts");
    const activationBody = source.slice(
      source.indexOf("const active = vscode.debug.activeDebugSession"),
      source.indexOf("context.subscriptions.push("),
    );
    assert.ok(
      activationBody.includes("deps.setStarting(true)") &&
        activationBody.includes("deps.acceptExternal(active)") &&
        !activationBody.includes("acceptedSessions.add"),
      "reload/activation must show Starting until the existing session passes the same readiness proof",
    );
    assert.ok(
      source.includes("deps.acceptExternal(active)"),
      "activation must use the normal external-session acceptance path",
    );
  });

  test("sidebar and status bar discard stale asynchronous renders", () => {
    const home = loadSource("trustHomeView.ts");
    const controls = loadSource("runtimeControls.ts");
    assert.ok(
      home.includes(
        "private readonly renderRevision = new LatestOnlyRevision()",
      ) &&
        home.includes("const revision = this.renderRevision.begin()") &&
        home.includes(
          "!this.renderRevision.isCurrent(revision) || this.view !== view",
        ),
      "an older sidebar snapshot must not overwrite a newer lifecycle state",
    );
    assert.ok(
      controls.includes("const refreshRevision = new LatestOnlyRevision()") &&
        controls.includes("const revision = refreshRevision.begin()") &&
        controls.includes("!refreshRevision.isCurrent(revision)"),
      "an older status-bar snapshot must not overwrite a newer lifecycle state",
    );
  });

  test("automatic multi-root debug selection follows the active project first", () => {
    const source = loadSource("debug/configuration.ts");
    const debugCommand = loadSource("debug/startCommand.ts");
    const lifecycle = loadSource("runtimeLifecycle.ts");
    const coordinator = loadSource("localSimulatorStartCoordinator.ts");
    const localDebugStart = loadSource("localSimulatorDebugStart.ts");
    const targetTracker = loadSource("runtimeConfigTargetTracker.ts");
    assert.ok(
      source.includes("findConfigurationUris(preferredFolder)") &&
        source.includes("findStructuredTextUris(preferredFolder)") &&
        debugCommand.includes("ensureConfigurationEntryAuto(folder)") &&
        /projectRoot:\s*this\.runtimeConfigTarget\(\)/.test(lifecycle) &&
        /executeDebugStart:\s*this\.executeLocalSimulatorDebugStart/.test(
          lifecycle,
        ) &&
        /dependencies\.executeDebugStart\(\s*dependencies\.attemptId,\s*dependencies\.projectRoot,/.test(
          coordinator,
        ) &&
        /workspaceFolder,/.test(localDebugStart) &&
        !localDebugStart.includes("validationProofToken") &&
        /let folder = startOptions\?\.workspaceFolder/.test(debugCommand) &&
        lifecycle.includes(
          "private readonly runtimeConfigTargets = new RuntimeConfigTargetTracker()",
        ) &&
        lifecycle.includes(
          "this.runtimeConfigTargets.capture(vscode.window.activeTextEditor)",
        ) &&
        lifecycle.includes(
          "captureEditor: (editor) => this.runtimeConfigTargets.capture(editor)",
        ) &&
        targetTracker.includes("private lastTarget: vscode.Uri | undefined") &&
        targetTracker.includes(
          "const editor = vscode.window.activeTextEditor",
        ) &&
        targetTracker.includes(
          "vscode.workspace.getWorkspaceFolder(editor.document.uri)",
        ) &&
        targetTracker.includes("return this.lastTarget") &&
        targetTracker.includes("return vscode.workspace.workspaceFolders?.[0]?.uri"),
      "Play must carry the lifecycle-selected root through configuration discovery, including when a non-ST editor or webview owns focus",
    );
  });
});
