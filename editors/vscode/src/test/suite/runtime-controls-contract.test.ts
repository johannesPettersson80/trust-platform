import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";

import {
  remoteLabelFromEndpoint,
  runtimeOptions,
  selectedRuntime,
  SIMULATOR_RUNTIME_ID,
  withPrimaryActionGate,
  type RuntimeModelSnapshot,
} from "../../trustHomeModel";
import {
  runtimeNodeControlLayout,
  runtimeNodeControls,
} from "../../networkCanvas/webview/runtimeNodeControls";
import {
  normalizeIoState,
} from "../../runtimeLifecycle";
import {
  compileGateReason,
  diagnosticsGateReason,
} from "../../compileGate";
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
    fs.readFileSync(path.join(extensionRoot(), "package.json"), "utf8")
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

suite("truST sidebar — selected target model", () => {
  test("remote labels keep the port so same-host runtimes are distinguishable", () => {
    assert.strictEqual(
      remoteLabelFromEndpoint("tcp://127.0.0.1:9902"),
      "127.0.0.1:9902"
    );
    assert.strictEqual(
      remoteLabelFromEndpoint("tcp://raspberrypi:5680"),
      "raspberrypi:5680"
    );
  });

  test("simulator: stopped → Start, running → Stop, starting → disabled (no action)", () => {
    const stopped = selectedRuntime({
      snapshot: snap(),
      remotes: [],
      managed: [],
      selectedId: SIMULATOR_RUNTIME_ID,
    });
    assert.strictEqual(stopped.kind, "simulator");
    assert.strictEqual(stopped.primary.action, "start");
    assert.strictEqual(stopped.primary.label, "Start");
    assert.ok(stopped.primary.enabled);

    const running = selectedRuntime({
      snapshot: snap({ runtimeMode: "simulate", runtimeState: "running" }),
      remotes: [],
      managed: [],
      selectedId: SIMULATOR_RUNTIME_ID,
    });
    assert.strictEqual(running.primary.action, "stop");
    assert.strictEqual(running.primary.label, "Stop");

    const starting = selectedRuntime({
      snapshot: snap({ starting: true }),
      remotes: [],
      managed: [],
      selectedId: SIMULATOR_RUNTIME_ID,
    });
    assert.strictEqual(starting.primary.action, "none");
    assert.strictEqual(starting.primary.enabled, false);
  });

  test("primary Start can be disabled with a visible reason without hiding the affordance", () => {
    const stopped = selectedRuntime({
      snapshot: snap(),
      remotes: [],
      managed: [],
      selectedId: SIMULATOR_RUNTIME_ID,
    });
    const gated = withPrimaryActionGate(stopped, {
      reason: "Fix 2 errors to start.",
    });
    assert.strictEqual(gated.primary.action, "start");
    assert.strictEqual(gated.primary.label, "Start");
    assert.strictEqual(gated.primary.enabled, false);
    assert.strictEqual(gated.primary.hint, "Fix 2 errors to start.");
  });

  test("compile/update gates use one shared user-facing reason model", () => {
    assert.strictEqual(
      diagnosticsGateReason(
        { ok: false, label: "2 errors", errors: 2, sourceErrors: 2, configErrors: 0 },
        "update"
      ),
      "Fix 2 errors to update."
    );
    assert.strictEqual(
      diagnosticsGateReason(
        { ok: false, label: "1 error", errors: 1, sourceErrors: 0, configErrors: 1 },
        "start"
      ),
      "Fix runtime.toml to start."
    );
    assert.strictEqual(
      compileGateReason(
        { kind: "failed", errors: 3, configErrors: 0, summary: "Compile failed" },
        { ok: true, label: "No known errors", errors: 0, sourceErrors: 0, configErrors: 0 },
        "debug"
      ),
      "Fix 3 errors to debug."
    );
  });

  test("remote: not connected → Connect, connected → Disconnect", () => {
    const remotes = [{ id: "tcp://raspberrypi:5680", label: "raspberrypi" }];
    const notConnected = selectedRuntime({
      snapshot: snap(),
      remotes,
      managed: [],
      selectedId: "tcp://raspberrypi:5680",
    });
    assert.strictEqual(notConnected.kind, "remote");
    assert.strictEqual(notConnected.primary.action, "connect");
    assert.strictEqual(notConnected.primary.label, "Connect");

    const connected = selectedRuntime({
      snapshot: snap({
        runtimeMode: "online",
        runtimeState: "connected",
        endpoint: "tcp://raspberrypi:5680",
        endpointConfigured: true,
        endpointReachable: true,
      }),
      remotes,
      managed: [],
      selectedId: "tcp://raspberrypi:5680",
    });
    assert.strictEqual(connected.primary.action, "disconnect");
    assert.strictEqual(connected.primary.label, "Disconnect");
    assert.strictEqual(connected.statusLabel, "Connected");
  });

  test("unreachable selected remote: Connect is DISABLED with a reason (never a button that just fails)", () => {
    const remotes = [{ id: "tcp://pi:5680", label: "pi" }];
    const unreachable = selectedRuntime({
      snapshot: snap({
        runtimeMode: "online",
        runtimeState: "stopped",
        endpoint: "tcp://pi:5680",
        endpointConfigured: true,
        endpointReachable: false,
      }),
      remotes,
      managed: [],
      selectedId: "tcp://pi:5680",
    });
    assert.strictEqual(unreachable.primary.action, "connect");
    assert.strictEqual(unreachable.primary.enabled, false, "unreachable → Connect disabled");
    assert.ok(
      unreachable.primary.hint && /reachable|Devices & Connections/i.test(unreachable.primary.hint),
      "must explain why + point to Devices & Connections"
    );
  });

  test("HONESTY: a connected remote NEVER renders Stop", () => {
    const connected = selectedRuntime({
      snapshot: snap({
        runtimeMode: "online",
        runtimeState: "connected",
        endpoint: "tcp://raspberrypi:5680",
        endpointConfigured: true,
        endpointReachable: true,
      }),
      remotes: [{ id: "tcp://raspberrypi:5680", label: "raspberrypi" }],
      managed: [],
      selectedId: "tcp://raspberrypi:5680",
    });
    assert.notStrictEqual(connected.primary.action, "stop");
    assert.notStrictEqual(connected.primary.label, "Stop");
    assert.ok(!/stop/i.test(connected.primary.label));
  });

  test("remote Connect verifies control auth before opening an attach session", () => {
    const source = loadSource("runtimeLifecycle.ts");
    const authCheck = source.indexOf("requestRuntimeStatus(status.endpoint");
    const attach = source.indexOf("vscode.debug.startDebugging(folder, debugConfig)");
    assert.ok(authCheck >= 0, "remote Connect must request runtime status with the selected token");
    assert.ok(attach >= 0, "remote Connect still attaches after the preflight succeeds");
    assert.ok(
      authCheck < attach,
      "remote Connect must not create an attach session before auth is verified"
    );
    assert.ok(
      source.includes("isRuntimeControlAuthError") &&
        source.includes("runtimeControlAuthErrorKind") &&
        source.includes("No auth token provided") &&
        source.includes("Auth token rejected"),
      "missing and wrong tokens must be classified as distinct recovery prompts"
    );
  });

  test("dropdown is SELECT-ONLY: simulator first, then remotes — NO Add/Connect sentinel; invalid selection falls back to simulator", () => {
    const remotes = [{ id: "tcp://pi:5680", label: "pi" }];
    const options = runtimeOptions(remotes, []);
    assert.strictEqual(options[0].id, SIMULATOR_RUNTIME_ID);
    assert.strictEqual(
      options[options.length - 1].id,
      "tcp://pi:5680",
      "the last option is a real runtime — no trailing Add/Connect sentinel"
    );
    // No option is a non-runtime sentinel (every option is a selectable target).
    assert.ok(
      options.every((option) => !option.id.startsWith("__")),
      "the dropdown must contain only real, selectable runtimes"
    );

    const fallback = selectedRuntime({
      snapshot: snap(),
      remotes,
      managed: [],
      selectedId: "does-not-exist",
    });
    assert.strictEqual(fallback.id, SIMULATOR_RUNTIME_ID);
  });

  test("managed local runtime: projected into the dropdown; Start when stopped, Stop when running (we own it)", () => {
    const managed = [
      { name: "cell1", controlEndpoint: "tcp://127.0.0.1:9902", state: "stopped" as const },
    ];
    const options = runtimeOptions([], managed);
    const local = options.find((option) => option.id === "cell1");
    assert.ok(local, "the managed runtime is in the dropdown");
    assert.strictEqual(local?.kind, "local");
    assert.strictEqual(local?.label, "cell1 (this computer)");

    const stopped = selectedRuntime({ snapshot: snap(), remotes: [], managed, selectedId: "cell1" });
    assert.strictEqual(stopped.primary.action, "start");
    assert.strictEqual(stopped.primary.label, "Start");

    const running = selectedRuntime({
      snapshot: snap(),
      remotes: [],
      managed: [{ ...managed[0], state: "running" }],
      selectedId: "cell1",
    });
    assert.strictEqual(running.primary.action, "stop");
    // We OWN a managed local runtime → never "Connect".
    assert.notStrictEqual(running.primary.action, "connect");
  });
});

suite("Managed local runtime model (Phase 9)", () => {
  test("normalizeManagedState + label", () => {
    assert.strictEqual(normalizeManagedState("running"), "running");
    assert.strictEqual(normalizeManagedState("stopped"), "stopped");
    assert.strictEqual(normalizeManagedState(undefined), "stopped");
    assert.strictEqual(managedRuntimeLabel("cell1"), "cell1 (this computer)");
  });

  test("lifecycle success is HONEST: Start only at 'running', Stop only at 'stopped'", () => {
    // Backend can report transient "starting"/"stopping" (didn't reach the target) — NOT success.
    assert.strictEqual(isManagedLifecycleSuccess("start", "running"), true);
    assert.strictEqual(isManagedLifecycleSuccess("start", "starting"), false);
    assert.strictEqual(isManagedLifecycleSuccess("start", "stopped"), false);
    assert.strictEqual(isManagedLifecycleSuccess("start", undefined), false);
    assert.strictEqual(isManagedLifecycleSuccess("stop", "stopped"), true);
    assert.strictEqual(isManagedLifecycleSuccess("stop", "stopping"), false);
    assert.strictEqual(isManagedLifecycleSuccess("stop", "running"), false);
  });

  test("toManagedRuntimes merges fleet list + per-name status", () => {
    const list = {
      runtimes: [
        { name: "cell1", control_endpoint: "tcp://127.0.0.1:9902", path: "cell1" },
        { name: "cell2", control_endpoint: "tcp://127.0.0.1:9903", path: "cell2" },
      ],
    };
    const statuses = new Map([
      [
        "cell1",
        { status: "running", path: "/fleet/cell1", log_path: "/tmp/cell1.log" },
      ],
    ]);
    const managed = toManagedRuntimes(list, statuses);
    assert.strictEqual(managed.length, 2);
    assert.strictEqual(managed[0].name, "cell1");
    assert.strictEqual(managed[0].state, "running");
    assert.strictEqual(managed[0].projectPath, "/fleet/cell1");
    assert.strictEqual(managed[0].logPath, "/tmp/cell1.log");
    // No status reported → stopped (honest default, never "running").
    assert.strictEqual(managed[1].state, "stopped");
    assert.strictEqual(managed[1].projectPath, "cell2");
  });

  test("managed local runtime auth token is parsed only from runtime.control", () => {
    const token = parseRuntimeControlAuthToken(`
[runtime.control]
endpoint = "tcp://127.0.0.1:9910"
auth_token = "managed-secret" # local runtime token

[mesh]
auth_token = "mesh-secret"
`);
    assert.strictEqual(token, "managed-secret");
    assert.strictEqual(
      parseRuntimeControlAuthToken(`[mesh]\nauth_token = "mesh-secret"\n`),
      undefined,
      "must not import unrelated protocol secrets as runtime control tokens"
    );
  });

  test("managed local runtime auth token parser accepts top-level dotted runtime.control form", () => {
    const token = parseRuntimeControlAuthToken(`
runtime.control.endpoint = "tcp://127.0.0.1:9910"
runtime.control.auth_token = "managed-dotted-secret" # local runtime token

[mesh]
auth_token = "mesh-secret"
`);
    assert.strictEqual(token, "managed-dotted-secret");
    assert.strictEqual(
      parseRuntimeControlAuthToken(`[mesh]\nruntime.control.auth_token = "wrong"\n`),
      undefined,
      "dotted runtime.control auth_token inside another table must not be imported"
    );
  });

  test("managed runtime logs are formatted for humans instead of raw JSON", () => {
    const formatted = formatManagedRuntimeLogs(
      '{"data":{"backend":"vm","source":"config"},"event":"execution_backend_selected","level":"info","ts":1782568993146}\n' +
        '{"data":{"affinity_applied":false,"errors":[],"warnings":[]},"event":"linux_rt_profile","level":"info","ts":1782568993147}\n',
      "",
      "cell1"
    );
    assert.ok(
      formatted.includes("[info] execution_backend_selected backend=vm source=config"),
      "structured logs should be summarized as readable event lines"
    );
    assert.ok(
      formatted.includes("[info] linux_rt_profile affinity_applied=false"),
      "structured log details should stay visible without dumping raw JSON objects"
    );
    assert.ok(!formatted.includes('{"data"'), "raw JSON log records must not be shown directly");
    assert.strictEqual(
      formatManagedRuntimeLogs("", "", "cell1"),
      "No logs available for cell1.\n"
    );
  });
});

suite("Canvas runtime-node controls — honest per-runtime lifecycle (§8 P3b)", () => {
  test("local simulator: stopped → Start, running → Stop, starting → disabled", () => {
    const stopped = runtimeNodeControls({ isLocal: true, health: "stopped", attached: false });
    assert.strictEqual(stopped[0].label, "Start");
    assert.strictEqual(stopped[0].action, "startLocalSimulator");
    assert.ok(stopped[0].enabled);

    const running = runtimeNodeControls({ isLocal: true, health: "connected", attached: true });
    assert.strictEqual(running[0].label, "Stop");
    assert.strictEqual(running[0].action, "stopLocalSimulator");

    const starting = runtimeNodeControls({ isLocal: true, health: "pending", attached: false });
    assert.strictEqual(starting[0].action, "none");
    assert.strictEqual(starting[0].enabled, false);
  });

  test("remote: not attached → Connect, attached → Disconnect; Connect disabled without an endpoint", () => {
    const connect = runtimeNodeControls({
      isLocal: false,
      health: "connected",
      attached: false,
      controlEndpoint: "tcp://pi:5680",
    });
    assert.strictEqual(connect[0].label, "Connect");
    assert.strictEqual(connect[0].action, "runtimeConnect");
    assert.ok(connect[0].enabled);

    const noEndpoint = runtimeNodeControls({ isLocal: false, health: "connected", attached: false });
    assert.strictEqual(noEndpoint[0].label, "Connect");
    assert.strictEqual(noEndpoint[0].enabled, false, "cannot connect without a control endpoint");

    const disconnect = runtimeNodeControls({
      isLocal: false,
      health: "connected",
      attached: true,
      controlEndpoint: "tcp://pi:5680",
    });
    assert.strictEqual(disconnect[0].label, "Disconnect");
    assert.strictEqual(disconnect[0].action, "runtimeDisconnect");
  });

  test("HONESTY: a remote runtime NEVER renders Start or Stop (we don't own its process)", () => {
    for (const attached of [false, true]) {
      const controls = runtimeNodeControls({
        isLocal: false,
        health: "connected",
        attached,
        controlEndpoint: "tcp://pi:5680",
      });
      for (const control of controls) {
        assert.notStrictEqual(control.action, "startLocalSimulator");
        assert.notStrictEqual(control.action, "stopLocalSimulator");
        assert.ok(!/^stop$/i.test(control.label) && !/^start$/i.test(control.label));
      }
    }
  });

  test("runtime node offers Set as run target + Settings; Logs only when a log backend exists", () => {
    const local = runtimeNodeControls({
      isLocal: true,
      health: "stopped",
      attached: false,
      logsAvailable: true,
    });
    const localActions = local.map((control) => control.action);
    assert.ok(localActions.includes("setAsRunTarget"), "must offer Set as run target");
    assert.ok(localActions.includes("openRuntimeSettings"), "must offer Settings");
    assert.ok(localActions.includes("openRuntimeLogs"), "local sim exposes logs");

    // Remote logs are phase 14 — no Logs control until a log backend exists (honest, not a dead button).
    const remote = runtimeNodeControls({
      isLocal: false,
      health: "connected",
      attached: false,
      controlEndpoint: "tcp://pi:5680",
      logsAvailable: false,
    });
    const remoteActions = remote.map((control) => control.action);
    assert.ok(
      !remoteActions.includes("openRuntimeLogs"),
      "remote Logs is gated until a log backend exists"
    );
    assert.ok(
      remoteActions.includes("setAsRunTarget"),
      "remote offers Set as run target (select without connecting)"
    );
  });

  test("remote auth failures make Set auth token the primary recovery without changing lifecycle ownership", () => {
    const controls = runtimeNodeControls({
      isLocal: false,
      health: "error",
      attached: false,
      controlEndpoint: "tcp://pi:5680",
      authTokenRequired: true,
    });
    const actions = controls.map((control) => control.action);
    assert.strictEqual(controls[0].action, "setAuthToken");
    assert.strictEqual(controls[0].kind, "primary");
    assert.ok(
      controls.some((control) => control.action === "runtimeConnect" && control.kind === "secondary"),
      "Connect remains available as a retry, but not as the first recovery action"
    );
    assert.ok(actions.includes("setAuthToken"), "auth failure needs direct credential recovery");
    assert.ok(actions.includes("setAsRunTarget"), "remote still offers select-only run target");
    assert.ok(!actions.includes("startLocalSimulator"), "remote auth recovery must not imply Start");
    assert.ok(!actions.includes("stopLocalSimulator"), "remote auth recovery must not imply Stop");
  });

  test("Devices & Connections wires remote auth recovery through the SecretStorage command", () => {
    const inspector = loadSource("networkCanvas/webview/NodeInspector.tsx");
    const panel = loadSource("networkCanvas/networkCanvasPanel.ts");
    assert.ok(
      inspector.includes('type: "setRuntimeAuthToken"') &&
        inspector.includes("isRuntimeAuthTokenFailure(node)") &&
        inspector.includes("no auth token provided") &&
        inspector.includes("auth token rejected"),
      "runtime auth failures must surface a Set auth token action in the inspector"
    );
    assert.ok(
      panel.includes('case "setRuntimeAuthToken"') &&
        panel.includes('"trust-lsp.runtime.setAuthToken"'),
      "inspector Set auth token must reuse the SecretStorage-backed runtime auth command"
    );
  });

  test("S-14: node inspector caps visible secondary actions at two with an overflow disclosure", () => {
    const controls = runtimeNodeControls({
      isLocal: false,
      managed: true,
      health: "stopped",
      attached: false,
      logsAvailable: true,
    });
    const layout = runtimeNodeControlLayout(
      controls,
      () => undefined,
      [{ key: "focus", label: "Focus", enabled: true, onClick: () => undefined }],
      false
    );
    assert.strictEqual(layout.primary?.action, "managedStart");
    assert.strictEqual(layout.visibleSecondary.length, 2);
    assert.ok(layout.hasOverflow, "managed footer has more secondary actions than can be shown");
    assert.ok(
      layout.overflowSecondary.some((item) => item.label === "Focus"),
      "Focus moves behind the overflow when runtime actions already fill the footer"
    );

    const inspector = loadSource("networkCanvas/webview/NodeInspector.tsx");
    const runtimeControlsSource = loadSource("networkCanvas/webview/runtimeNodeControls.ts");
    assert.ok(
      runtimeControlsSource.includes("MAX_VISIBLE_RUNTIME_NODE_SECONDARY = 2"),
      "runtime control layout must cap visible secondary actions at two (S-14)"
    );
    assert.ok(
      inspector.includes("runtimeNodeControlLayout"),
      "inspector must use the shared runtime control layout, not a separate footer policy"
    );
    assert.ok(
      inspector.includes("runtimeControlLayout.hasOverflow") &&
        inspector.includes("setShowAllActions") &&
        inspector.includes("More actions"),
      "extra secondary actions must collapse behind an overflow disclosure, not a toolbar of buttons"
    );
  });

  test("managed local runtime node: Start when stopped, Stop when running — never Connect (we own it)", () => {
    const stopped = runtimeNodeControls({
      isLocal: false,
      managed: true,
      health: "stopped",
      attached: false,
      logsAvailable: true,
    });
    assert.strictEqual(stopped[0].action, "managedStart");
    assert.strictEqual(stopped[0].label, "Start");

    const running = runtimeNodeControls({
      isLocal: false,
      managed: true,
      health: "connected",
      attached: false,
      logsAvailable: true,
    });
    assert.strictEqual(running[0].action, "managedStop");
    assert.strictEqual(running[0].label, "Stop");

    const actions = [...stopped, ...running].map((control) => control.action);
    assert.ok(
      !actions.includes("runtimeConnect") && !actions.includes("runtimeDisconnect"),
      "a managed local runtime is owned → never Connect/Disconnect"
    );
    assert.ok(stopped.some((c) => c.action === "openRuntimeLogs"), "managed has Logs");
    assert.ok(stopped.some((c) => c.action === "setAsRunTarget"), "managed offers Set as run target");
  });
});

suite("truST sidebar — control surface contract", () => {
  test("exactly one truST activity container + one view, and the view is a WebviewView", () => {
    const contributes = loadPackageJson().contributes ?? {};
    const containers = (contributes.viewsContainers?.activitybar ?? []).map(
      (container) => container.id
    );
    assert.deepStrictEqual(
      containers,
      ["trust"],
      "Exactly one truST activity-bar container."
    );
    const views = contributes.views?.trust ?? [];
    assert.deepStrictEqual(
      views.map((view) => view.id),
      ["trust.home"],
      "Exactly one truST sidebar view."
    );
    assert.strictEqual(
      views[0]?.type,
      "webview",
      "trust.home must be a WebviewView — the runtime selector needs a real dropdown."
    );
  });

  test("no status-bar / palette Start/Stop commands (one run surface)", () => {
    const runtimeCommands = (loadPackageJson().contributes?.commands ?? [])
      .map((command) => command.command)
      .filter(
        (command): command is string =>
          typeof command === "string" &&
          command.startsWith("trust-lsp.runtime.")
      );
    assert.deepStrictEqual(
      runtimeCommands,
      [],
      "There must be NO trust-lsp.runtime.* commands — the sidebar drives the lifecycle directly."
    );
  });

  test("the status bar is passive: it only reveals the sidebar, never starts/stops", () => {
    const source = loadSource("runtimeControls.ts");
    assert.ok(
      source.includes("trust.home.focus"),
      "status bar click must reveal the truST sidebar (trust.home.focus)"
    );
    assert.ok(
      !source.includes("registerCommand"),
      "the passive status bar must NOT register any command"
    );
    assert.ok(
      !source.includes("startLocalSimulator") &&
        !source.includes("stopRuntime"),
      "the passive status bar must NOT start/stop the runtime"
    );
  });

  test("the status bar follows the selected target, not a separate simulator-only state", () => {
    const source = loadSource("runtimeControls.ts");
    assert.ok(
      source.includes("getSelectedRuntimeId") &&
        source.includes("onDidChangeSelectedRuntime"),
      "status bar must read and refresh from the shared selected-run-target store"
    );
    assert.ok(
      source.includes("onDidChangeManagedRuntimes"),
      "status bar must refresh when managed-runtime Start/Stop changes the selected target state"
    );
    assert.ok(
      source.includes("selectedRuntime({") &&
        source.includes("listManagedRuntimes(context)") &&
        source.includes("readRemotes()"),
      "status bar must render through the same selectedRuntime model as the sidebar"
    );
    assert.ok(
      source.includes("statusTargetLabel(selected)") &&
        source.includes('return "Simulator"') &&
        source.includes("return selected.id"),
      "status bar text must name the selected target instead of always saying Simulator"
    );
  });

  test("stopping a runtime emits a fresh lifecycle refresh after the session is gone", () => {
    const source = loadSource("runtimeLifecycle.ts");
    const stopBody = source.slice(
      source.indexOf("async stopRuntime("),
      source.indexOf("private async startOnlineRuntime(")
    );
    assert.ok(
      stopBody.includes("catch (err)") &&
        stopBody.includes("await this.waitForSessionGone(SESSION_WAIT_TIMEOUT_MS)") &&
        stopBody.includes('return this.markStopped("Runtime stopped.");'),
      "Stop must wait for the session to disappear even if the debug stop command throws Canceled"
    );
    assert.ok(
      /private markStopped\(message: string\): RuntimeLifecycleResult \{\s*this\.lastIoState = EMPTY_IO_STATE;\s*this\.starting = false;\s*this\.failure = undefined;\s*this\.emitChanged\(\);\s*return \{ ok: true, message \};\s*\}/.test(source),
      "successful Stop must emit after the debug session is gone so the passive status bar cannot stay stuck on Running"
    );
  });

  test("the status bar does not pretend a simulator target exists before a project exists", () => {
    const source = loadSource("runtimeControls.ts");
    assert.ok(
      source.includes("workspaceHasTrustProject"),
      "status bar must check whether the workspace is a truST project before showing a runtime"
    );
    assert.ok(
      source.includes("truST: No project"),
      "no-project and non-truST workspaces must have a neutral status-bar state"
    );
    assert.ok(
      source.indexOf("workspaceHasTrustProject") < source.indexOf("selectedRuntime({"),
      "project detection must happen before falling back to the simulator selected-runtime model"
    );
    assert.ok(
      source.includes('createFileSystemWatcher(pattern)') &&
        source.includes('"**/trust-lsp.toml"') &&
        source.includes('"**/runtime.toml"'),
      "status bar must refresh when project marker files appear after first-run project creation"
    );
    assert.ok(
      source.indexOf('snapshot.status.runtimeState === "connected"') <
        source.indexOf("selectedRuntime({"),
      "an active attached runtime must render as connected before selected-target fallback can say Simulator stopped"
    );
    assert.ok(
      source.includes("connectedEndpointLabel(snapshot.status.endpoint)") &&
        source.includes('return "runtime"'),
      "attached local runtime endpoints must render as a friendly runtime label, not a raw unix socket path"
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
    assert.strictEqual(state.outputs[0].source, "MQTT topic trust/examples/mqtt/out");
    assert.strictEqual(state.memory[0].value, "FALSE");
    assert.strictEqual(state.memory[1].value, "42");
    assert.strictEqual(state.memory[2].value, "T#250ms");
    assert.strictEqual(state.memory[2].valueType, "TIME");
    assert.strictEqual(state.scan, 12841);

    const ioPanelSource = loadSource("ioPanel.ts");
    assert.ok(
      ioPanelSource.includes("normalizeIoState(body)") &&
        !ioPanelSource.includes("payload: body ??"),
      "Live Values must not forward raw stIoState values directly to the webview"
    );
  });

  test("simulator launch keeps raw adapter logs out of the first-run surface", () => {
    const source = loadSource("debug.ts");
    assert.ok(
      source.includes('internalConsoleOptions: "neverOpen"'),
      "sidebar Start must not auto-open VS Code's Debug Console with raw adapter logs"
    );
  });

  test("ERR-04 control-endpoint override is test-mode only", () => {
    const source = loadSource("debug.ts");
    assert.ok(
      source.includes('"TRUST_UX_DEBUG_CONTROL_ENDPOINT"') &&
        source.includes("allowTestControlEndpointOverride"),
      "debug launch endpoint override must exist only for evidence/test runners"
    );
    assert.ok(
      source.includes("context.extensionMode === vscode.ExtensionMode.Test"),
      "the control-endpoint override must be disabled outside VS Code test mode"
    );
    assert.ok(
      source.indexOf("process.env[TEST_CONTROL_ENDPOINT_OVERRIDE_ENV]") <
        source.indexOf("localSimControl(folder?.uri.fsPath)"),
      "ERR-04 evidence must be able to force a real bind-conflict endpoint before the normal local-sim socket is chosen"
    );
    assert.ok(
      source.includes("launchControlEndpointError") &&
        source.includes("The runtime port is already in use.") &&
        source.indexOf("await launchControlEndpointError") <
          source.indexOf("vscode.debug.startDebugging(folder, config)"),
      "local launch control-endpoint conflicts must be caught before VS Code starts the debug session"
    );
  });

  test("attach sessions keep raw adapter logs out of canvas and Live Values workflows", () => {
    const debugSource = loadSource("debug.ts");
    const lifecycleSource = loadSource("runtimeLifecycle.ts");
    const ioPanelSource = loadSource("ioPanel.ts");
    for (const [name, source] of [
      ["debug command attach", debugSource],
      ["runtime lifecycle attach", lifecycleSource],
      ["Live Values attach", ioPanelSource],
    ] as const) {
      assert.ok(
        source.includes('request: "attach"') &&
          source.includes('internalConsoleOptions: "neverOpen"'),
        `${name} must not auto-open VS Code's Debug Console with raw adapter logs`
      );
    }
  });

  test("unreachable runtime messages are human-facing and do not expose local socket paths", () => {
    const lifecycleSource = loadSource("runtimeLifecycle.ts");
    assert.ok(
      lifecycleSource.includes("runtimeNotReachableMessage(status.endpoint)") &&
        lifecycleSource.includes("Local runtime is stopped. Start it to connect.") &&
        lifecycleSource.includes("shortRuntimeEndpointLabel(endpoint)"),
      "runtime lifecycle must humanize unreachable endpoints before surfacing them"
    );
    assert.ok(
      !lifecycleSource.includes("message: `Runtime not reachable: ${status.endpoint}`"),
      "user-facing runtime-unreachable messages must not expose raw socket paths"
    );
  });

  test("remote attach refuses debug-disabled runtimes before reporting connected", () => {
    const lifecycleSource = loadSource("runtimeLifecycle.ts");
    assert.ok(
      lifecycleSource.includes("runtimeDebugDisabled(runtimeInfo)") &&
        lifecycleSource.includes("Remote debugging is disabled for this runtime"),
      "remote Connect must fail visibly when runtime.control.debug_enabled is false"
    );
    assert.ok(
      lifecycleSource.indexOf("runtimeDebugDisabled(runtimeInfo)") <
        lifecycleSource.indexOf("vscode.debug.startDebugging(folder, debugConfig)"),
      "debug-disabled status must be checked before launching the debug adapter"
    );
  });

  test("Connect failures show state-specific next actions, not auth for every failure", () => {
    const source = loadSource("trustHomeView.ts");
    assert.ok(
      source.includes("connectFailureChoices(result)"),
      "Connect failure actions must be selected by failure kind"
    );
    assert.ok(
      source.includes("OPEN_DEVICES_ACTION") &&
        source.includes("SET_AUTH_TOKEN_ACTION"),
      "Connect failures must distinguish diagnose/open-devices from auth-token entry"
    );
    assert.ok(
      !source.includes(
        'actionFailureMessage(selected, result),\n          "Set auth token"'
      ),
      "sidebar must not offer Set auth token for every failed Connect"
    );
    const choicesBody = source.slice(
      source.indexOf("function connectFailureChoices"),
      source.indexOf("function isRuntimeUnreachableFailure")
    );
    assert.ok(
      choicesBody.includes("isRuntimeUnreachableFailure") &&
        choicesBody.includes("isAuthTokenFailure"),
      "unreachable and auth failures must be separate branches"
    );
    assert.ok(
      source.includes(
        "Open Devices & Connections to start or diagnose this runtime."
      ),
      "unreachable Connect failures must show a visible recovery step in the sidebar"
    );
  });

  test("Start compiles first and does not launch after a failed Compile", () => {
    const source = loadSource("trustHomeView.ts");
    const runAction = source.slice(
      source.indexOf("private async runAction()"),
      source.indexOf("private async runManagedAction(")
    );
    const compileIndex = runAction.indexOf("CHECK_PROGRAM_COMMAND");
    const dispatchIndex = runAction.indexOf("const dispatched = this.dispatch(selected)");
    assert.ok(compileIndex >= 0, "Run must invoke Compile before simulator Start");
    assert.ok(dispatchIndex >= 0, "Run must still dispatch after a clean Compile");
    assert.ok(
      compileIndex < dispatchIndex,
      "Compile must happen before Start dispatch so config/ST failures cannot still start debugging"
    );
    assert.ok(
      runAction.includes("if (!compile.ok)") &&
        runAction.includes("this.applyMessageKind = \"error\"") &&
        runAction.includes("return;"),
      "failed Compile must render an error state and return before dispatch"
    );
  });

  test("Start and Update disable with reasons when compile/config validity cannot succeed", () => {
    const source = loadSource("trustHomeView.ts");
    const gateSource = loadSource("compileGate.ts");
    assert.ok(
      source.includes("primaryActionGateReason(") &&
        source.includes("withPrimaryActionGate("),
      "sidebar Start must be gated before rendering and dispatch"
    );
    assert.ok(
      source.includes("compileGateReason(") &&
        gateSource.includes("Fix runtime.toml to ${verb}.") &&
        gateSource.includes("Fix ${count} error"),
      "disabled primary/update actions must explain config and compile-error recovery"
    );
    assert.ok(
      source.includes("applyEl.disabled = !msg.applyEnabled") &&
        source.includes("applyEl.title = msg.applyTitle"),
      "Update running simulation must remain visible but disabled with a reason"
    );
    assert.ok(
      source.includes("isConfigDiagnosticPath") &&
        gateSource.includes("(runtime|trust-lsp)") &&
        gateSource.includes("configErrors"),
      "runtime.toml/trust-lsp.toml diagnostics must be classified as config blockers"
    );
  });

  test("simulator Start treats a failed I/O probe as a failed launch", () => {
    const source = loadSource("runtimeLifecycle.ts");
    const startLocal = source.slice(
      source.indexOf("async startLocalSimulator()"),
      source.indexOf("async stopRuntime()")
    );
    const requestIndex = startLocal.indexOf(
      "const ioStateResult = await this.requestIoState({ persistFailure: true })"
    );
    const failedProbeIndex = startLocal.indexOf("if (!ioStateResult.ok)");
    const clearFailureIndex = startLocal.indexOf(
      "this.failure = undefined",
      failedProbeIndex
    );
    assert.ok(requestIndex >= 0, "Start must probe I/O state before claiming the simulator is running");
    assert.ok(
      failedProbeIndex > requestIndex,
      "Start must inspect a failed I/O probe instead of ignoring it"
    );
    assert.ok(
      startLocal.includes("this.failure = ioStateResult.failure") &&
        startLocal.includes("return ioStateResult"),
      "a failed I/O probe must remain a visible Start failure"
    );
    assert.ok(
      failedProbeIndex < clearFailureIndex,
      "Start must not clear failure before handling a failed I/O probe"
    );
    assert.ok(
      startLocal.includes("waitForSessionStillPresent") &&
        startLocal.includes("Simulator stopped during startup") &&
        startLocal.includes("runtime port or target settings"),
      "a debug session that immediately terminates must not be reported as a successful Start"
    );
    assert.ok(
      startLocal.includes("withTimeout(") &&
        source.includes("DEBUG_START_COMMAND_TIMEOUT_MS") &&
        source.includes("Start debugging timed out"),
      "Start must not wait forever on VS Code debug startup errors before rendering an inline sidebar failure"
    );
  });

  test("background I/O refresh failures do not persist as sidebar start failures", () => {
    const source = loadSource("runtimeLifecycle.ts");
    const requestBody = source.slice(
      source.indexOf("async requestIoState("),
      source.indexOf("async startLocalSimulator()")
    );
    assert.ok(
      requestBody.includes(
        "options: { readonly persistFailure?: boolean; readonly afterScan?: number } = {}"
      ),
      "I/O refresh must distinguish background polling from the Start acceptance probe"
    );
    assert.ok(
      requestBody.includes("if (options.persistFailure)") &&
        requestBody.includes("this.failure = ioFailure"),
      "only the Start acceptance probe may persist an I/O failure into the sidebar lifecycle state"
    );
    assert.ok(
      source.includes("this.requestIoState({ persistFailure: true })"),
      "Start must still persist a failed I/O probe so startup cannot fake a running state"
    );
  });

  test("sidebar renders start failure messages even after simulator stays stopped", () => {
    const source = loadSource("trustHomeView.ts");
    const renderBody = source.slice(
      source.indexOf("private async render()"),
      source.indexOf("private async onMessage(")
    );
    assert.ok(
      renderBody.includes('this.applyMessageKind === "error"') &&
        renderBody.includes("snapshot.failure") &&
        renderBody.includes("lifecycleFailureMessage"),
      "Start/Connect failures must stay visible in the sidebar even when the simulator remains stopped"
    );
    assert.ok(
      source.includes("withSidebarActionTimeout") &&
        source.includes("SIDEBAR_ACTION_TIMEOUT_MS") &&
        source.includes("Start timed out. Check the runtime port or target settings."),
      "sidebar Start must regain control and render an inline failure if VS Code debug startup hangs"
    );
    assert.ok(
      source.includes('applyMessageEl.style.display = applyMessage ? "block" : "none"'),
      "inline sidebar failures must be visibly rendered; empty display falls back to the CSS hidden rule"
    );
    assert.ok(
      source.includes("} else if (result?.ok)") &&
        source.includes('this.applyMessage = ""') &&
        source.includes('this.applyMessageKind = ""'),
      "successful Start/Stop/Connect/Disconnect must clear stale sidebar action failures"
    );
  });

  test("no ST editor-title Run/Stop controls", () => {
    const items = loadPackageJson().contributes?.menus?.["editor/title"] ?? [];
    const runtimeItems = items.filter((item) =>
      (item.command ?? "").startsWith("trust-lsp.runtime.")
    );
    assert.deepStrictEqual(
      runtimeItems,
      [],
      "editor/title must contribute no runtime Run/Stop — there is one run surface."
    );
  });

  test("the truST panel is a WebviewView with examples-first onboarding and a compact action surface", () => {
    const source = loadSource("trustHomeView.ts");
    assert.ok(
      source.includes("registerWebviewViewProvider"),
      "trust.home must be a WebviewViewProvider"
    );
    // Two sidebar states.
    assert.ok(source.includes('id="welcome"'), "must render the no-project welcome state");
    assert.ok(source.includes('id="project"'), "must render the project-open state");
    // No-project welcome = Examples first, then Create/Open; no transport controls.
    assert.ok(source.includes(">+ Create project<"), "welcome offers Create project");
    assert.ok(source.includes(">Open project<"), "welcome offers Open project");
    assert.ok(source.includes("Start from example"), "welcome offers Start from example");
    assert.ok(
      source.indexOf("Start from example") < source.indexOf("+ Create project"),
      "Start from example must be the headline first-run action"
    );
    const welcomeStart = source.indexOf('id="welcome"');
    const welcomeEnd = source.indexOf('id="project"', welcomeStart);
    const welcome = source.slice(welcomeStart, welcomeEnd);
    assert.ok(
      !welcome.includes('id="action"') && !welcome.includes('id="compile"'),
      "no-project state must not show transport/compile controls"
    );
    assert.ok(
      source.includes("No truST project") &&
        source.includes("does not contain a truST project yet"),
      "an open non-truST folder must explain that it can be initialized as a truST project"
    );
    assert.ok(
      source.includes("Initialize truST here"),
      "an open non-truST folder must offer an explicit initialize action"
    );
    assert.ok(
      source.includes("targetUri: workspaceState.folder.uri") &&
        source.includes("openWorkspace: false"),
      "initializing an open non-truST folder must scaffold that folder instead of opening an unrelated picker"
    );
    // Compact action row + visible destinations.
    assert.ok(source.includes(">Target<"), "the target label must read 'Target'");
    assert.ok(source.includes('id="compile"'), "sidebar must expose Compile in the fixed action row");
    assert.ok(source.includes('id="debug"'), "sidebar must expose Debug in the fixed action row");
    assert.ok(source.includes('id="deploy"'), "sidebar must expose state-aware Deploy in the fixed action row");
    assert.ok(
      source.includes("node_modules") &&
        source.includes("@vscode") &&
        source.includes("codicons") &&
        source.includes("codicon-debug-alt") &&
        source.includes("codicon-play") &&
        source.includes("codicon-stop") &&
        source.includes("codicon-rocket"),
      "sidebar action buttons must use real VS Code Codicons, not emoji/text glyphs"
    );
    assert.ok(
      !source.includes("🐞") && !source.includes("⚒") && !source.includes("⤓"),
      "sidebar action buttons must not use emoji glyphs that can render as missing squares"
    );
    assert.ok(
      source.includes("showQuickPick") && source.includes("QuickPickItemKind.Separator"),
      "the Target button must open a grouped native QuickPick"
    );
    assert.ok(source.includes("Devices &amp; Connections"), "nav must offer Devices & Connections");
    assert.ok(source.includes(">Libraries<"), "nav must offer Libraries as a first-class destination");
    assert.ok(source.includes(">Live Values<"), "nav must offer Live Values");
    assert.ok(source.includes('id="navHmi"'), "nav must offer HMI");
    assert.ok(
      !source.includes('id="navProject"') && !source.includes("projectActionsMenu"),
      "the retired Project bucket must not remain in the sidebar"
    );
    assert.ok(
      source.includes('hmiLabel') && source.includes('"Create HMI"'),
      "the HMI launcher may say Create HMI when the project has no HMI descriptors"
    );
    assert.ok(
      source.includes('createFileSystemWatcher("**/hmi/*.toml")'),
      "the HMI launcher label must refresh when HMI descriptor files are created or removed"
    );
    // Honesty / no jargon.
    assert.ok(
      !source.includes("Network Canvas"),
      "the panel must not surface the jargon 'Network Canvas' (command id stays the same)"
    );
    assert.ok(
      !/>\s*Runtime\s*<\/label>/.test(source),
      "the target label must not regress to the bare backend word 'Runtime'"
    );
  });

  test("stopRuntime is idempotent (a disappeared session after Stop is success, not a warning)", () => {
    const source = loadSource("runtimeLifecycle.ts");
    assert.ok(
      source.includes("waitForSessionGone"),
      "stop must verify success by the session actually going away"
    );
    assert.ok(
      source.includes("Runtime already stopped."),
      "stopping an already-stopped runtime must be a no-op success"
    );
    // The old bug: returning the 'No active Structured Text debug session.' failure from a Stop.
    const stopBody = source.slice(
      source.indexOf("async stopRuntime("),
      source.indexOf("private async startOnlineRuntime(")
    );
    assert.ok(
      stopBody.length > 0 &&
        !stopBody.includes("No active Structured Text debug session."),
      "stopRuntime must not treat a gone session as the 'No active … session' failure"
    );
  });

  test("managed Stop disconnects Live Values even when fleet stop omits the endpoint", () => {
    const managedSession = loadSource("managedRuntimeSession.ts");
    const home = loadSource("trustHomeView.ts");
    const canvas = loadSource("networkCanvas/networkCanvasPanel.ts");
    assert.ok(
      managedSession.includes("managedRuntimeLabel(name)") &&
        managedSession.includes("sameManagedTarget") &&
        managedSession.includes("sameEndpoint || sameManagedTarget") &&
        managedSession.includes("runtimeLifecycleService.stopRuntime()"),
      "managed Stop must disconnect the attached session by endpoint OR selected managed label"
    );
    assert.ok(
      home.includes("disconnectManagedRuntimeAfterStop(selected.id, result)"),
      "the sidebar Stop path must pass the stopped managed runtime name"
    );
    assert.ok(
      canvas.includes("disconnectManagedRuntimeAfterStop(name, result)"),
      "the canvas Stop path must pass the stopped managed runtime name"
    );
  });

  test("Update running simulation cannot hang forever on a stuck stReload request", () => {
    const source = loadSource("debug.ts");
    const homeSource = loadSource("trustHomeView.ts");
    const reloadCommand = source.slice(
      source.indexOf('registerCommand("trust-lsp.debug.reload"'),
      source.indexOf("\n  );\n}", source.indexOf('registerCommand("trust-lsp.debug.reload"'))
    );
    assert.ok(
      source.includes("HOT_RELOAD_REQUEST_TIMEOUT_MS"),
      "Update running simulation must define an explicit timeout"
    );
    assert.ok(
      source.includes("function withTimeout"),
      "Update running simulation must use a timeout helper for adapter requests"
    );
    assert.ok(
      reloadCommand.includes('session.customRequest("stReload"') &&
        reloadCommand.includes("withTimeout(") &&
        reloadCommand.includes("HOT_RELOAD_REQUEST_TIMEOUT_MS"),
      "trust-lsp.debug.reload must bound the stReload custom request"
    );
    assert.ok(
      reloadCommand.includes("diagnosticsGateReason(validityLine(), \"update\")") &&
        reloadCommand.includes("return { ok: false, message: gateReason, gated: true }") &&
        reloadCommand.indexOf("diagnosticsGateReason") <
          reloadCommand.indexOf('session.customRequest("stReload"'),
      "trust-lsp.debug.reload must share the sidebar compile gate before attempting update"
    );
    assert.ok(
      reloadCommand.includes("Update running simulation timed out") &&
        reloadCommand.includes("try again or restart"),
      "a timed-out Update running simulation must fail with a user-facing recovery message"
    );
    assert.ok(
      source.includes("function summarizeReloadCommandMessage") &&
        source.includes("Compile failed — ${sourceErrorCount} error") &&
        reloadCommand.includes("summarizeReloadCommandMessage(rawMessage)") &&
        reloadCommand.includes("Update failed: ${message}") &&
        !reloadCommand.includes("Update failed: ${rawMessage}"),
      "Update notifications must summarize compile failures instead of leaking raw source paths"
    );
    assert.ok(
      source.includes("onDidDebugReload") &&
        source.includes("debugReloadEmitter.fire({ ok: true })") &&
        source.includes("debugReloadEmitter.fire({ ok: false, message })"),
      "reload command must publish success/failure so the sidebar stays in sync"
    );
    assert.ok(
      homeSource.includes("onDidDebugReload") &&
        homeSource.includes("this.sourceChanged = false") &&
        homeSource.includes("Running simulation updated.") &&
        homeSource.includes("Update failed:") &&
        homeSource.includes("value.gated === true") &&
        homeSource.includes("Open Problems, then try again."),
      "sidebar must clear or explain pending Update state from the shared reload result"
    );
  });
});
