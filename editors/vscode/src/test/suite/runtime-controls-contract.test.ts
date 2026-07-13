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
  selectLifecycleDebugSession,
  terminatedSessionOwnsLifecycleState,
} from "../../debug/sessionSelection";
import { LatestOnlyRevision } from "../../latestOnlyRevision";
import {
  effectiveLifecycleEntryFailure,
  lifecycleActionSucceeded,
} from "../../lifecycleEntryFailure";
import { compileGateReason, diagnosticsGateReason } from "../../compileGate";

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

suite("truST sidebar — selected target model", () => {
  test("remote labels keep the port so same-host runtimes are distinguishable", () => {
    assert.strictEqual(
      remoteLabelFromEndpoint("tcp://127.0.0.1:9902"),
      "127.0.0.1:9902",
    );
    assert.strictEqual(
      remoteLabelFromEndpoint("tcp://raspberrypi:5680"),
      "raspberrypi:5680",
    );
    assert.strictEqual(
      remoteLabelFromEndpoint("unix:///tmp/trust-runtime.sock"),
      "runtime",
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
      snapshot: snap({
        starting: true,
        transitionTargetId: SIMULATOR_RUNTIME_ID,
      }),
      remotes: [],
      managed: [],
      selectedId: SIMULATOR_RUNTIME_ID,
    });
    assert.strictEqual(starting.primary.action, "none");
    assert.strictEqual(starting.primary.enabled, false);
  });

  test("compile/update gates use one shared user-facing reason model", () => {
    assert.strictEqual(
      diagnosticsGateReason(
        {
          ok: false,
          label: "2 errors",
          errors: 2,
          sourceErrors: 2,
          configErrors: 0,
        },
        "update",
      ),
      "Fix 2 errors to update.",
    );
    assert.strictEqual(
      diagnosticsGateReason(
        {
          ok: false,
          label: "1 error",
          errors: 1,
          sourceErrors: 0,
          configErrors: 1,
        },
        "start",
      ),
      "Fix runtime.toml to start.",
    );
    assert.strictEqual(
      compileGateReason(
        {
          kind: "failed",
          errors: 3,
          configErrors: 0,
          summary: "Compile failed",
        },
        {
          ok: true,
          label: "No known errors",
          errors: 0,
          sourceErrors: 0,
          configErrors: 0,
        },
        "debug",
      ),
      "Fix 3 errors to debug.",
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
    assert.strictEqual(
      unreachable.primary.enabled,
      false,
      "unreachable → Connect disabled",
    );
    assert.ok(
      unreachable.primary.hint &&
        /reachable|Devices & Connections/i.test(unreachable.primary.hint),
      "must explain why + point to Devices & Connections",
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
      "the last option is a real runtime — no trailing Add/Connect sentinel",
    );
    // No option is a non-runtime sentinel (every option is a selectable target).
    assert.ok(
      options.every((option) => !option.id.startsWith("__")),
      "the dropdown must contain only real, selectable runtimes",
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
      {
        name: "cell1",
        controlEndpoint: "tcp://127.0.0.1:9902",
        state: "stopped" as const,
      },
    ];
    const options = runtimeOptions([], managed);
    const local = options.find((option) => option.id === "cell1");
    assert.ok(local, "the managed runtime is in the dropdown");
    assert.strictEqual(local?.kind, "local");
    assert.strictEqual(local?.label, "cell1 (this computer)");

    const stopped = selectedRuntime({
      snapshot: snap(),
      remotes: [],
      managed,
      selectedId: "cell1",
    });
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
        {
          name: "cell1",
          control_endpoint: "tcp://127.0.0.1:9902",
          path: "cell1",
        },
        {
          name: "cell2",
          control_endpoint: "tcp://127.0.0.1:9903",
          path: "cell2",
        },
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
      "must not import unrelated protocol secrets as runtime control tokens",
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
      parseRuntimeControlAuthToken(
        `[mesh]\nruntime.control.auth_token = "wrong"\n`,
      ),
      undefined,
      "dotted runtime.control auth_token inside another table must not be imported",
    );
  });

  test("managed runtime logs are formatted for humans instead of raw JSON", () => {
    const formatted = formatManagedRuntimeLogs(
      '{"data":{"backend":"vm","source":"config"},"event":"execution_backend_selected","level":"info","ts":1782568993146}\n' +
        '{"data":{"affinity_applied":false,"errors":[],"warnings":[]},"event":"linux_rt_profile","level":"info","ts":1782568993147}\n',
      "",
      "cell1",
    );
    assert.ok(
      formatted.includes(
        "[info] execution_backend_selected backend=vm source=config",
      ),
      "structured logs should be summarized as readable event lines",
    );
    assert.ok(
      formatted.includes("[info] linux_rt_profile affinity_applied=false"),
      "structured log details should stay visible without dumping raw JSON objects",
    );
    assert.ok(
      !formatted.includes('{"data"'),
      "raw JSON log records must not be shown directly",
    );
    assert.strictEqual(
      formatManagedRuntimeLogs("", "", "cell1"),
      "No logs available for cell1.\n",
    );
  });
});

suite(
  "Canvas runtime-node controls — honest per-runtime lifecycle (§8 P3b)",
  () => {
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

      const noEndpoint = runtimeNodeControls({
        isLocal: false,
        health: "connected",
        attached: false,
      });
      assert.strictEqual(noEndpoint[0].label, "Connect");
      assert.strictEqual(
        noEndpoint[0].enabled,
        false,
        "cannot connect without a control endpoint",
      );

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
          assert.ok(
            !/^stop$/i.test(control.label) && !/^start$/i.test(control.label),
          );
        }
      }
    });

    test("runtime node offers Select as target + Settings; Logs only when a log backend exists", () => {
      const local = runtimeNodeControls({
        isLocal: true,
        health: "stopped",
        attached: false,
        logsAvailable: true,
      });
      const localActions = local.map((control) => control.action);
      assert.ok(
        localActions.includes("setAsRunTarget"),
        "must offer target selection",
      );
      assert.strictEqual(
        local.find((control) => control.action === "setAsRunTarget")?.label,
        "Select as target",
      );
      assert.ok(
        localActions.includes("openRuntimeSettings"),
        "must offer Settings",
      );
      assert.ok(
        localActions.includes("openRuntimeLogs"),
        "local sim exposes logs",
      );

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
        "remote Logs is gated until a log backend exists",
      );
      assert.ok(
        remoteActions.includes("setAsRunTarget"),
        "remote offers target selection without connecting",
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
        controls.some(
          (control) =>
            control.action === "runtimeConnect" && control.kind === "secondary",
        ),
        "Connect remains available as a retry, but not as the first recovery action",
      );
      assert.ok(
        actions.includes("setAuthToken"),
        "auth failure needs direct credential recovery",
      );
      assert.ok(
        actions.includes("setAsRunTarget"),
        "remote still offers select-only run target",
      );
      assert.ok(
        controls.every((control) => !/^(start|stop)$/i.test(control.label)),
        "remote auth recovery must not imply local lifecycle ownership",
      );
    });

    test("Devices & Connections wires remote auth recovery through the SecretStorage command", () => {
      const inspector = [
        loadSource("networkCanvas/webview/NodeInspector.tsx"),
        loadSource("networkCanvas/webview/NodeSummaryView.tsx"),
      ].join("\n");
      const lifecycleActions = loadSource("networkCanvas/lifecycleActions.ts");
      assert.ok(
        inspector.includes('type: "setRuntimeAuthToken"') &&
          inspector.includes("isRuntimeAuthTokenFailure(node)") &&
          inspector.includes("no auth token provided") &&
          inspector.includes("auth token rejected"),
        "runtime auth failures must surface a Set auth token action in the inspector",
      );
      assert.ok(
        lifecycleActions.includes('case "setRuntimeAuthToken"') &&
          lifecycleActions.includes('"trust-lsp.runtime.setAuthToken"'),
        "inspector Set auth token must reuse the SecretStorage-backed runtime auth command",
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
        [
          {
            key: "focus",
            label: "Focus",
            enabled: true,
            onClick: () => undefined,
          },
        ],
        false,
      );
      assert.strictEqual(layout.primary?.action, "managedStart");
      assert.strictEqual(layout.visibleSecondary.length, 2);
      assert.ok(
        layout.hasOverflow,
        "managed footer has more secondary actions than can be shown",
      );
      assert.ok(
        layout.overflowSecondary.some((item) => item.label === "Focus"),
        "Focus moves behind the overflow when runtime actions already fill the footer",
      );

      const inspector = [
        loadSource("networkCanvas/webview/NodeInspector.tsx"),
        loadSource("networkCanvas/webview/NodeSummaryView.tsx"),
      ].join("\n");
      const runtimeControlsSource = loadSource(
        "networkCanvas/webview/runtimeNodeControls.ts",
      );
      assert.ok(
        runtimeControlsSource.includes(
          "MAX_VISIBLE_RUNTIME_NODE_SECONDARY = 2",
        ),
        "runtime control layout must cap visible secondary actions at two (S-14)",
      );
      assert.ok(
        inspector.includes("runtimeNodeControlLayout"),
        "inspector must use the shared runtime control layout, not a separate footer policy",
      );
      assert.ok(
        inspector.includes("runtimeControlLayout.hasOverflow") &&
          inspector.includes("setShowAllActions") &&
          inspector.includes("More actions"),
        "extra secondary actions must collapse behind an overflow disclosure, not a toolbar of buttons",
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
        !actions.includes("runtimeConnect") &&
          !actions.includes("runtimeDisconnect"),
        "a managed local runtime is owned → never Connect/Disconnect",
      );
      assert.ok(
        stopped.some((c) => c.action === "openRuntimeLogs"),
        "managed has Logs",
      );
      assert.ok(
        stopped.some((c) => c.action === "setAsRunTarget"),
        "managed offers target selection",
      );
    });
  },
);
