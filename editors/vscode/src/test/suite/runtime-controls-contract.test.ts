import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";

import {
  runtimeOptions,
  selectedRuntime,
  SIMULATOR_RUNTIME_ID,
  type RuntimeModelSnapshot,
} from "../../trustHomeModel";
import { runtimeNodeControls } from "../../networkCanvas/webview/runtimeNodeControls";
import {
  isManagedLifecycleSuccess,
  managedRuntimeLabel,
  normalizeManagedState,
  toManagedRuntimes,
} from "../../localRuntimeModel";

// Regression guard for the v3 UX RESET (vscode-ux-overhaul-plan.md §0/§6/§8/§9): ONE run surface — a
// Run card (WebviewView) with a runtime selector and a SINGLE state-specific action. Literal verbs
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

suite("Run card — selected runtime model (v3 reset)", () => {
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
        { name: "cell1", control_endpoint: "tcp://127.0.0.1:9902" },
        { name: "cell2", control_endpoint: "tcp://127.0.0.1:9903" },
      ],
    };
    const statuses = new Map([
      ["cell1", { status: "running", log_path: "/tmp/cell1.log" }],
    ]);
    const managed = toManagedRuntimes(list, statuses);
    assert.strictEqual(managed.length, 2);
    assert.strictEqual(managed[0].name, "cell1");
    assert.strictEqual(managed[0].state, "running");
    assert.strictEqual(managed[0].logPath, "/tmp/cell1.log");
    // No status reported → stopped (honest default, never "running").
    assert.strictEqual(managed[1].state, "stopped");
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

suite("Run card — surface contract (v3 reset)", () => {
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
      "Exactly one truST view (the Run card)."
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
      "There must be NO trust-lsp.runtime.* commands — the Run card drives the lifecycle directly."
    );
  });

  test("the status bar is passive: it only reveals the Run card, never starts/stops", () => {
    const source = loadSource("runtimeControls.ts");
    assert.ok(
      source.includes("trust.home.focus"),
      "status bar click must reveal the Run card (trust.home.focus)"
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

  test("the truST panel is a WebviewView with the v5 two states + nav launchers, no 'Network Canvas' jargon", () => {
    const source = loadSource("trustHomeView.ts");
    assert.ok(
      source.includes("registerWebviewViewProvider"),
      "trust.home must be a WebviewViewProvider"
    );
    // Two sidebar states.
    assert.ok(source.includes('id="welcome"'), "must render the no-project welcome state");
    assert.ok(source.includes('id="project"'), "must render the project-open state");
    // No-project welcome = Create / Open / Start from example, only.
    assert.ok(source.includes(">Create project<"), "welcome offers Create project");
    assert.ok(source.includes(">Open project<"), "welcome offers Open project");
    assert.ok(source.includes(">Start from example<"), "welcome offers Start from example");
    // Run bar label + nav launchers (the v4 in-card links are now proper nav areas).
    assert.ok(source.includes("Run target:"), "the Run bar label must read 'Run target:'");
    assert.ok(source.includes("Devices &amp; Connections"), "nav must offer Devices & Connections");
    assert.ok(source.includes(">Live Values<"), "nav must offer Live Values");
    assert.ok(source.includes('id="navHmi"'), "nav must offer HMI");
    // Honesty / no jargon.
    assert.ok(
      !source.includes("Network Canvas"),
      "the panel must not surface the jargon 'Network Canvas' (command id stays the same)"
    );
    assert.ok(
      !/>\s*Runtime\s*<\/label>/.test(source),
      "the dropdown label must be 'Run target:', not the bare 'Runtime'"
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
});
