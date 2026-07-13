import * as assert from "assert";
import type * as vscode from "vscode";

import {
  lockedOperationForCanvasMessage,
  managedRuntimeOwnsActiveTarget,
  NetworkCanvasLifecycleActions,
} from "../../networkCanvas/lifecycleActions";
import type { LifecyclePhase } from "../../lifecycleEntryFailure";
import {
  runtimeAuthoritySelection,
  runtimeModelSnapshotForLifecycle,
} from "../../runtimeAuthoritySelection";
import { selectedRuntime, SIMULATOR_RUNTIME_ID } from "../../trustHomeModel";
import {
  lifecycleSnapshot,
  source,
} from "./runtime-operation-lock-contract-fixtures";

suite("Runtime operation lock contract", () => {
  test("sidebar authority renders direct launch and ephemeral attach instead of stale selection", () => {
    const configured = [{ id: "tcp://stale:9902", label: "Stale remote" }];
    const managed = [
      {
        name: "cell-other",
        controlEndpoint: "tcp://127.0.0.1:9920",
        state: "running" as const,
      },
      {
        name: "cell-local",
        controlEndpoint: "tcp://127.0.0.1:9910",
        state: "running" as const,
      },
    ];
    const launch = runtimeAuthoritySelection(
      lifecycleSnapshot({
        status: {
          ...lifecycleSnapshot().status,
          running: true,
          runtimeMode: "simulate",
          runtimeState: "running",
        },
        activeTarget: { kind: "simulator" },
      }),
      configured,
      managed,
      "tcp://stale:9902",
    );
    assert.strictEqual(launch.selectedId, "simulator");

    const attach = runtimeAuthoritySelection(
      lifecycleSnapshot({
        status: {
          ...lifecycleSnapshot().status,
          running: true,
          runtimeMode: "online",
          runtimeState: "connected",
          endpoint: "tcp://new-cell:9902",
          endpointConfigured: true,
          targetLabel: "Packaging cell A",
        },
        activeTarget: {
          kind: "remote",
          endpoint: "tcp://new-cell:9902",
          label: "Packaging cell A",
        },
      }),
      configured,
      managed,
      "tcp://stale:9902",
    );
    assert.strictEqual(attach.selectedId, "tcp://new-cell:9902");
    assert.deepStrictEqual(attach.remotes[attach.remotes.length - 1], {
      id: "tcp://new-cell:9902",
      label: "Packaging cell A",
    });
    assert.strictEqual(
      configured.length,
      1,
      "ephemeral attach must not mutate configured fleet input",
    );

    const managedAttach = runtimeAuthoritySelection(
      lifecycleSnapshot({
        status: {
          ...lifecycleSnapshot().status,
          running: true,
          runtimeMode: "online",
          runtimeState: "connected",
          endpoint: "tcp://127.0.0.1:9910",
          endpointConfigured: true,
          targetLabel: "cell-other (this computer)",
        },
        activeTarget: {
          kind: "remote",
          endpoint: "tcp://127.0.0.1:9910",
          label: "cell-other (this computer)",
        },
      }),
      configured,
      managed,
      "tcp://stale:9902",
    );
    assert.strictEqual(managedAttach.selectedId, "cell-local");
    assert.strictEqual(managedAttach.managedSessionId, "cell-local");

    const conflictingManagedIdentity = runtimeAuthoritySelection(
      lifecycleSnapshot({
        status: {
          ...lifecycleSnapshot().status,
          runtimeMode: "online",
          runtimeState: "connected",
          endpoint: "tcp://127.0.0.1:9910",
          endpointConfigured: true,
          targetLabel: "cell-other (this computer)",
        },
        activeTarget: {
          kind: "managed",
          id: "cell-other",
          endpoint: "tcp://127.0.0.1:9910",
        },
      }),
      configured,
      managed,
      "tcp://stale:9902",
    );
    assert.strictEqual(
      conflictingManagedIdentity.selectedId,
      "cell-local",
      "the exact endpoint must win when an explicit managed ID disagrees",
    );
    assert.strictEqual(
      conflictingManagedIdentity.managedSessionId,
      "cell-local",
    );
    assert.strictEqual(
      managedRuntimeOwnsActiveTarget("cell-other", "tcp://127.0.0.1:9920", {
        kind: "managed",
        id: "cell-local",
        endpoint: "tcp://127.0.0.1:9910",
      }),
      false,
      "another managed runtime must not inherit Stop authority",
    );
    assert.strictEqual(
      managedRuntimeOwnsActiveTarget("cell-local", "tcp://127.0.0.1:9910", {
        kind: "managed",
        id: "cell-local",
        endpoint: "tcp://127.0.0.1:9910",
      }),
      true,
    );

    const staleManagedProcess = selectedRuntime({
      snapshot: runtimeModelSnapshotForLifecycle(
        lifecycleSnapshot({
          status: {
            ...lifecycleSnapshot().status,
            running: true,
            runtimeMode: "online",
            runtimeState: "connected",
            endpoint: "tcp://127.0.0.1:9910",
            endpointConfigured: true,
            endpointReachable: true,
            targetLabel: "cell-local (this computer)",
          },
          activeTarget: {
            kind: "remote",
            endpoint: "tcp://127.0.0.1:9910",
            label: "cell-local (this computer)",
          },
        }),
      ),
      remotes: managedAttach.remotes,
      managed: [managed[0], { ...managed[1], state: "stopped" }],
      selectedId: managedAttach.selectedId,
      managedSessionId: managedAttach.managedSessionId,
    });
    assert.deepStrictEqual(
      {
        kind: staleManagedProcess.kind,
        status: staleManagedProcess.status,
        statusLabel: staleManagedProcess.statusLabel,
        action: staleManagedProcess.primary.action,
      },
      {
        kind: "local",
        status: "connected",
        statusLabel: "Live Values still connected",
        action: "stop",
      },
      "stopped fleet inventory must not hide the still-accepted managed attach",
    );

    assert.deepStrictEqual(
      runtimeModelSnapshotForLifecycle(
        lifecycleSnapshot({
          status: {
            ...lifecycleSnapshot().status,
            runtimeMode: "online",
            runtimeState: "connected",
            endpoint: "tcp://stale:9902",
            endpointConfigured: true,
            endpointReachable: true,
          },
          activeTarget: { kind: "simulator" },
        }),
      ),
      {
        runtimeMode: "simulate",
        runtimeState: "running",
        endpoint: "",
        endpointConfigured: false,
        endpointReachable: true,
        starting: false,
        transitionTargetId: "simulator",
      },
      "an accepted direct launch must override stale online configuration",
    );
    assert.deepStrictEqual(
      runtimeModelSnapshotForLifecycle(
        lifecycleSnapshot({
          starting: true,
          transitionTarget: {
            kind: "remote",
            endpoint: "tcp://new-cell:9902",
            label: "Packaging cell A",
          },
        }),
      ),
      {
        runtimeMode: "online",
        runtimeState: "stopped",
        endpoint: "tcp://new-cell:9902",
        endpointConfigured: true,
        endpointReachable: false,
        starting: true,
        transitionTargetId: "tcp://new-cell:9902",
      },
      "a remote transition must expose its exact endpoint instead of the stale selected target",
    );
    const simulatorDuringRemoteTransition = selectedRuntime({
      snapshot: runtimeModelSnapshotForLifecycle(
        lifecycleSnapshot({
          starting: true,
          transitionTarget: {
            kind: "remote",
            endpoint: "tcp://packaging-cell:5680",
          },
        }),
      ),
      remotes: [],
      managed: [],
      selectedId: SIMULATOR_RUNTIME_ID,
    });
    assert.strictEqual(
      simulatorDuringRemoteTransition.primary.action,
      "start",
      "a remote Connect transition must not paint the Simulator as Starting",
    );
  });

  test("TrustHome visibly mirrors the operation policy and rechecks stale host messages", () => {
    const home = source("trustHomeView.ts");
    const webview = source("trustHomeWebview.ts");
    const managed = source("managedRuntimeSession.ts");
    const statusBar = source("runtimeControls.ts");
    const onlineConnection = source("runtimeOnlineConnection.ts");
    const lifecycleModel = source("runtimeLifecycleModel.ts");
    const nodeInspector = source("networkCanvas/webview/NodeInspector.tsx");
    const render = home.slice(
      home.indexOf("private async render"),
      home.indexOf("private async onMessage"),
    );
    const select = home.slice(
      home.indexOf("private async onSelect"),
      home.indexOf("private setCompileState"),
    );
    const managedAction = home.slice(
      home.indexOf("private async runManagedAction"),
      home.indexOf("private async applyChanges"),
    );

    assert.ok(
      render.includes(
        'runtimeOperationBlockReason(\n      phase,\n      "compile"',
      ) &&
        render.includes('"select_target"') &&
        render.includes("lockedActionForSelectedRuntime(selected)") &&
        render.includes("targetEnabled: !targetReason"),
      "Compile, Target, and the lifecycle verb must use one visible policy",
    );
    assert.ok(
      webview.includes("targetButton.disabled = !msg.targetEnabled") &&
        webview.includes("button.disabled = !view.enabled"),
      "the webview must visibly disable every locked operation",
    );
    assert.ok(
      webview.includes("#action .label { display: inline; }") &&
        webview.includes('if (btn.id === "action")') &&
        webview.includes("Compile may collapse first"),
      "the one lifecycle action must keep Start/Starting/Stop/Connect/Disconnect literal at narrow widths",
    );
    assert.ok(
      render.includes('"apply_changes"') &&
        render.includes(
          "applyEnabled: canApply && !updateGate && !applyReason",
        ) &&
        home.includes('rejectBlockedOperation("apply_changes")') &&
        home.includes('runExclusiveOperation(\n      "apply_changes"'),
      "Update running simulation must visibly and host-side hold the same mutation lease through reload completion",
    );
    assert.ok(
      render.includes('selected.status === "starting"') &&
        render.includes(
          'const actionHint = actionReason || selected.primary.hint || ""',
        ),
      "ordinary Starting/Connecting progress must stay calm instead of showing a warning",
    );
    assert.ok(
      select.includes('rejectBlockedOperation("select_target")') &&
        select.includes('rejectBlockedOperation("compile")') &&
        home.includes("lockedActionForSelectedRuntime(selected)") &&
        home.includes("runtimeLifecycleService.operationState() !== undefined"),
      "queued Target, Compile, and action clicks must be rejected again by the extension host",
    );
    assert.ok(
      managedAction.includes("runExclusiveOperation(") &&
        managedAction.includes('"managed_start"') &&
        managedAction.includes('"managed_stop"') &&
        managedAction.includes("operationId"),
      "managed Start/Stop must own one lifecycle lease",
    );
    assert.ok(
      managed.includes("connectRemoteWithinOperation") &&
        managed.includes("operationId") &&
        onlineConnection.includes("MANAGED_RUNTIME_ID_FIELD") &&
        onlineConnection.includes("options.managedRuntimeId.trim()") &&
        lifecycleModel.includes(
          "session.configuration[MANAGED_RUNTIME_ID_FIELD]",
        ) &&
        nodeInspector.includes("endpoint: str(node.data.controlEndpoint)"),
      "managed process Start and attach must not release/reacquire the lifecycle lease",
    );
    assert.ok(
      home.includes("managedSessionId: authority.managedSessionId") &&
        statusBar.includes("managedSessionId: authority.managedSessionId"),
      "sidebar and passive status bar must keep a matching accepted managed session authoritative",
    );
    assert.ok(
      statusBar.includes("runtimeAuthoritySelection(") &&
        statusBar.includes(
          "runtimeModelSnapshotForLifecycle(snapshot, authority.target)",
        ) &&
        !statusBar.includes("function toModelSnapshot"),
      "the passive status bar must project the exact transition/accepted target too",
    );
  });

  test("stale queued canvas lifecycle messages are rejected again at the host", async () => {
    let phase: LifecyclePhase = "starting";
    let connects = 0;
    let stops = 0;
    let refreshes = 0;
    const blocked: string[] = [];
    const controller = new NetworkCanvasLifecycleActions({
      extensionContext: () => {
        throw new Error(
          "a blocked managed action must not reach its host mutation",
        );
      },
      refresh: async () => {
        refreshes += 1;
      },
      clearFailure: () => undefined,
      recordResult: () => undefined,
      stopRuntime: async () => {
        stops += 1;
        return { ok: true, message: "stopped" };
      },
      connectRemote: async () => {
        connects += 1;
        return { ok: true, message: "connected" };
      },
      runExclusiveOperation: async (_kind, _target, operation) => ({
        acquired: true,
        value: await operation("test-operation"),
      }),
      lifecyclePhase: () => phase,
      activeTarget: () =>
        phase === "connected"
          ? { kind: "remote", endpoint: "tcp://pi:9902" }
          : { kind: "simulator" },
      managedTarget: () => undefined,
      operationInProgress: () => false,
      reportBlocked: (reason) => blocked.push(reason),
    });

    const staleMessages = [
      { type: "runtimeConnect", endpoint: "tcp://pi:9902" },
      { type: "runtimeDisconnect" },
      { type: "setAsRunTarget", endpoint: "tcp://pi:9902" },
      { type: "runtimeManagedStart", name: "cell-a" },
      { type: "runtimeManagedStop", name: "cell-a" },
    ];
    for (const message of staleMessages) {
      assert.ok(lockedOperationForCanvasMessage(message), message.type);
      assert.strictEqual(await controller.handleMessage(message), true);
    }
    assert.deepStrictEqual(
      { connects, stops, refreshes, blocked: blocked.length },
      { connects: 0, stops: 0, refreshes: 0, blocked: staleMessages.length },
      "a stale webview click must not mutate, select, attach, stop, or refresh",
    );

    phase = "connected";
    await controller.handleMessage({
      type: "runtimeDisconnect",
      endpoint: "tcp://pi:9902",
    });
    await controller.handleMessage({
      type: "runtimeConnect",
      endpoint: "tcp://other:9902",
    });
    assert.deepStrictEqual(
      { connects, stops, refreshes },
      { connects: 0, stops: 1, refreshes: 1 },
      "the owned Disconnect stays live while a new Connect remains rejected",
    );
  });

  test("the webview mirrors host phase while logs and ADS diagnosis stay outside the lock", () => {
    const hostState = source("networkCanvas/webview/useCanvasHostState.ts");
    const app = source("networkCanvas/webview/NetworkCanvasApp.tsx");
    const inspector = source("networkCanvas/webview/NodeInspector.tsx");
    const panel = source("networkCanvas/networkCanvasPanel.ts");
    const actions = source("networkCanvas/lifecycleActions.ts");

    assert.ok(
      hostState.includes('message.type === "lifecyclePolicy"') &&
        hostState.includes("setLifecyclePhase") &&
        app.includes("lifecyclePhase={lifecyclePhase}") &&
        inspector.includes("lifecyclePhase"),
      "visible runtime controls must consume the same host lifecycle phase",
    );
    assert.ok(
      panel.includes('type: "lifecyclePolicy"') &&
        panel.includes("lifecyclePhase: phase") &&
        panel.includes("lifecyclePhase: () => runtimeLifecycleService.phase()"),
      "every structural phase change must immediately update the webview and host guard",
    );
    assert.ok(
      actions.includes("lockedOperationForCanvasMessage(message)") &&
        actions.includes("!this.allowOperation(operation, message)"),
      "all mutating canvas messages must pass one host guard before their switch branch",
    );
    assert.ok(
      !actions
        .slice(
          actions.indexOf('case "openRuntimeLogs"'),
          actions.indexOf('case "runtimeConnect"'),
        )
        .includes("allowOperation") &&
        !panel
          .slice(
            panel.indexOf("async function handleDiscover"),
            panel.indexOf("async function handleWebviewMessage"),
          )
          .includes("runtimeOperation"),
      "logs and ADS discovery remain available while lifecycle mutation is locked",
    );
  });
});
