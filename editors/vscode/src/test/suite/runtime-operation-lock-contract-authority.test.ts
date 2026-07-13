import * as assert from "assert";
import type * as vscode from "vscode";

import {
  runtimeNodeControls,
  runtimeNodeControlsForNode,
} from "../../networkCanvas/webview/runtimeNodeControls";
import { NetworkCanvasLifecycleActions } from "../../networkCanvas/lifecycleActions";
import { projectCanvasLifecycleAuthority } from "../../networkCanvas/lifecycleAuthorityProjection";
import { AUTHORITY_CHECK_RUNTIME_NODE_ID } from "../../networkCanvas/webview/types";
import { initialNetworkCanvasGraph } from "../../networkCanvas/initialGraph";
import { NetworkCanvasRuntimeAuthority } from "../../networkCanvas/runtimeAuthorityState";
import {
  buildImmediateSimulatorLifecycleGraph,
  modelInputForSnapshot,
} from "../../networkCanvas/lifecycleModel";
import {
  normalizeRuntimeAuthorityTarget,
  runtimeModelSnapshotForLifecycle,
} from "../../runtimeAuthoritySelection";
import { SIMULATOR_RUNTIME_ID } from "../../trustHomeModel";
import {
  graphFixture,
  lifecycleSnapshot,
  managedGraph,
} from "./runtime-operation-lock-contract-fixtures";

suite("Runtime operation lock contract", () => {
  test("canvas lifecycle state is target-scoped and accepted sessions override stale selection", () => {
    const connecting = projectCanvasLifecycleAuthority(graphFixture(), {
      phase: "starting",
      target: {
        kind: "remote",
        endpoint: "tcp://cell-a:9902",
        label: "Packaging cell A",
      },
    });
    const connectingRuntimes = connecting.hosts.flatMap(
      (host) => host.runtimes,
    );
    const simulator = connectingRuntimes.find(
      (runtime) => runtime.id === "runtime:local",
    );
    const remote = connectingRuntimes.find(
      (runtime) => runtime.controlEndpoint === "tcp://cell-a:9902",
    );
    assert.strictEqual(
      simulator?.health,
      "stopped",
      "remote Connect must not paint Simulator Starting",
    );
    assert.strictEqual(
      simulator?.runTarget,
      false,
      "remote Connect must move Run-target authority off the Simulator",
    );
    assert.deepStrictEqual(
      {
        name: remote?.name,
        health: remote?.health,
        runTarget: remote?.runTarget,
        attached: remote?.attached,
      },
      {
        name: "Packaging cell A",
        health: "starting",
        runTarget: true,
        attached: false,
      },
      "an unconfigured transition target is visible ephemerally without mutating fleet settings",
    );

    const attached = projectCanvasLifecycleAuthority(graphFixture(), {
      phase: "connected",
      target: {
        kind: "remote",
        endpoint: "tcp://cell-a:9902",
        label: "Packaging cell A",
      },
    });
    const attachedRemote = attached.hosts
      .flatMap((host) => host.runtimes)
      .find((runtime) => runtime.controlEndpoint === "tcp://cell-a:9902");
    assert.strictEqual(attachedRemote?.attached, true);
    assert.strictEqual(attachedRemote?.runTarget, true);

    const launched = projectCanvasLifecycleAuthority(graphFixture(), {
      phase: "running",
      target: { kind: "simulator" },
    });
    const launchedRuntimes = launched.hosts.flatMap((host) => host.runtimes);
    assert.strictEqual(
      launchedRuntimes.find((runtime) => runtime.id === "runtime:local")
        ?.runTarget,
      true,
      "accepted launch must override a stale persisted remote selection",
    );
    assert.strictEqual(
      launchedRuntimes.find((runtime) => runtime.id === "runtime:stale")
        ?.runTarget,
      false,
    );
  });

  test("sidebar and canvas share managed identity normalization", () => {
    const managed = [
      {
        name: "cell-a",
        controlEndpoint: "tcp://127.0.0.1:9910",
        state: "running" as const,
      },
      {
        name: "cell-b",
        controlEndpoint: "tcp://127.0.0.1:9920",
        state: "running" as const,
      },
    ];
    const staleTarget = {
      kind: "managed" as const,
      id: "cell-a",
      endpoint: "tcp://127.0.0.1:9920",
    };
    const normalized = normalizeRuntimeAuthorityTarget(
      staleTarget,
      managed,
      "cell-a (this computer)",
    );
    assert.deepStrictEqual(normalized, {
      kind: "managed",
      id: "cell-b",
      endpoint: "tcp://127.0.0.1:9920",
    });

    const projected = projectCanvasLifecycleAuthority(
      managedGraph(
        managed.map((runtime) => ({
          name: runtime.name,
          endpoint: runtime.controlEndpoint,
        })),
      ),
      { phase: "connected", target: normalized },
    );
    const projectedManaged = projected.hosts[0].runtimes.filter(
      (runtime) => runtime.managed,
    );
    assert.deepStrictEqual(
      projectedManaged.map((runtime) => ({
        name: runtime.managedName,
        attached: runtime.attached ?? false,
        runTarget: runtime.runTarget ?? false,
      })),
      [
        { name: "cell-a", attached: false, runTarget: false },
        { name: "cell-b", attached: true, runTarget: true },
      ],
    );

    const transition = lifecycleSnapshot({
      starting: true,
      transitionTarget: staleTarget,
    });
    assert.strictEqual(
      runtimeModelSnapshotForLifecycle(transition, normalized)
        .transitionTargetId,
      "cell-b",
      "the remapped managed target must paint Starting on the same runtime",
    );
  });

  test("duplicate legacy managed endpoints assign no managed owner", () => {
    const endpoint = "tcp://127.0.0.1:9910";
    const managed = [
      { name: "cell-a", controlEndpoint: endpoint, state: "running" as const },
      { name: "cell-b", controlEndpoint: endpoint, state: "running" as const },
    ];
    const normalized = normalizeRuntimeAuthorityTarget(
      { kind: "remote", endpoint, label: "ambiguous local runtime" },
      managed,
    );
    assert.deepStrictEqual(normalized, {
      kind: "remote",
      endpoint,
      label: "ambiguous local runtime",
    });

    const projected = projectCanvasLifecycleAuthority(
      managedGraph(
        managed.map((runtime) => ({ name: runtime.name, endpoint })),
      ),
      { phase: "connected", target: normalized },
    );
    const managedNodes = projected.hosts[0].runtimes.filter(
      (runtime) => runtime.managed,
    );
    assert.ok(
      managedNodes.every((runtime) => !runtime.attached && !runtime.runTarget),
      "ambiguous managed nodes must remain unattached",
    );
    assert.ok(
      managedNodes.every(
        (runtime) =>
          !runtimeNodeControls({
            isLocal: false,
            managed: true,
            health: runtime.health,
            attached: runtime.attached === true,
            controlEndpoint: runtime.controlEndpoint,
            lifecyclePhase: "connected",
          }).some(
            (control) => control.action === "managedStop" && control.enabled,
          ),
      ),
      "no ambiguous managed node may expose enabled Stop",
    );
    const ephemeral = projected.hosts
      .flatMap((host) => host.runtimes)
      .find((runtime) => runtime.id === `runtime:active:${endpoint}`);
    assert.strictEqual(ephemeral?.attached, true);
    assert.strictEqual(ephemeral?.mode, "remote");
  });

  test("inventory-validated authority authorizes the exact canvas mutation", async () => {
    const endpointA = "tcp://127.0.0.1:9910";
    const endpointB = "tcp://127.0.0.1:9920";
    const managed = [
      { name: "cell-a", controlEndpoint: endpointA, state: "running" as const },
      { name: "cell-b", controlEndpoint: endpointB, state: "running" as const },
    ];
    const authorityState = new NetworkCanvasRuntimeAuthority();
    assert.strictEqual(
      authorityState.beginFirstPaint({
        kind: "managed",
        id: "cell-a",
        endpoint: endpointB,
      }),
      null,
      "raw managed identity must not authorize first-paint mutation",
    );
    let authority = authorityState.acceptInventory(
      { kind: "managed", id: "cell-a", endpoint: endpointB },
      managed,
    );
    const stoppedNames: string[] = [];
    const disconnectedAuthorities: unknown[] = [];
    const operationTargets: unknown[] = [];
    const blocked: string[] = [];
    let remoteDisconnects = 0;
    const controller = new NetworkCanvasLifecycleActions({
      extensionContext: () => ({}) as vscode.ExtensionContext,
      refresh: async () => undefined,
      clearFailure: () => undefined,
      recordResult: () => undefined,
      stopRuntime: async () => {
        remoteDisconnects += 1;
        return { ok: true, message: "disconnected" };
      },
      connectRemote: async () => ({ ok: true, message: "connected" }),
      runExclusiveOperation: async (_kind, target, operation) => {
        operationTargets.push(target);
        return {
          acquired: true as const,
          value: await operation("validated-operation"),
        };
      },
      lifecyclePhase: () => "connected",
      activeTarget: () => authority,
      managedTarget: (name, endpoint) =>
        authorityState.managedTarget(name, endpoint),
      operationInProgress: () => false,
      reportBlocked: (reason) => blocked.push(reason),
      stopManagedRuntime: async (_context, name) => {
        stoppedNames.push(name);
        return {
          ok: true,
          status: "stopped",
          controlEndpoint: endpointB,
        };
      },
      disconnectManagedRuntimeAfterStop: async (
        _name,
        _result,
        _operationId,
        validated,
      ) => {
        disconnectedAuthorities.push(validated);
        return { ok: true, message: "disconnected exact session" };
      },
    });

    await controller.handleMessage({
      type: "runtimeManagedStop",
      name: "cell-b",
      endpoint: endpointB,
    });
    assert.deepStrictEqual(stoppedNames, ["cell-b"]);
    assert.deepStrictEqual(operationTargets, [authority]);
    assert.deepStrictEqual(disconnectedAuthorities, [authority]);
    assert.deepStrictEqual(blocked, []);

    authorityState.invalidateInventory({
      kind: "remote",
      endpoint: endpointB,
      label: "legacy attach",
    });
    assert.strictEqual(authorityState.activeTarget(), undefined);
    authority = authorityState.acceptInventory(
      { kind: "remote", endpoint: endpointB, label: "legacy attach" },
      managed,
    );
    await controller.handleMessage({
      type: "runtimeManagedStop",
      name: "cell-b",
      endpoint: endpointB,
    });
    assert.deepStrictEqual(
      stoppedNames,
      ["cell-b", "cell-b"],
      "a unique legacy endpoint must normalize to the same managed owner before mutation",
    );

    const duplicateManaged = [
      { name: "cell-a", controlEndpoint: endpointB, state: "running" as const },
      { name: "cell-b", controlEndpoint: endpointB, state: "running" as const },
    ];
    authority = authorityState.acceptInventory(
      { kind: "managed", id: "missing", endpoint: endpointB },
      duplicateManaged,
    );
    assert.strictEqual(authority?.kind, "remote");
    await controller.handleMessage({
      type: "runtimeManagedStop",
      name: "cell-a",
      endpoint: endpointB,
    });
    assert.strictEqual(
      stoppedNames.length,
      2,
      "ambiguous inventory must never stop a managed process",
    );
    await controller.handleMessage({
      type: "runtimeDisconnect",
      endpoint: endpointB,
    });
    assert.strictEqual(remoteDisconnects, 1);
    assert.strictEqual(blocked.length, 1);
  });

  test("first paint exposes no managed mutation before inventory validation", () => {
    const graph = initialNetworkCanvasGraph(
      "connected",
      "welcome",
      "cell-a",
      null,
    );
    const runtimes = graph.hosts.flatMap((host) => host.runtimes);
    assert.strictEqual(graph.summary, "Checking active runtime…");
    assert.strictEqual(runtimes[0]?.name, "Checking active runtime…");
    assert.strictEqual(runtimes[0]?.id, AUTHORITY_CHECK_RUNTIME_NODE_ID);
    assert.strictEqual(runtimes[0]?.health, "starting");
    assert.match(runtimes[0]?.detail ?? "", /before connection controls/i);
    assert.doesNotMatch(
      `${graph.summary} ${runtimes[0]?.name} ${runtimes[0]?.detail}`,
      /Simulator stopped/i,
    );
    assert.ok(runtimes.every((runtime) => runtime.attached !== true));
    assert.ok(runtimes.every((runtime) => runtime.managed !== true));
    const controls = runtimeNodeControlsForNode({
      nodeId: runtimes[0]?.id ?? "",
      isLocal: false,
      managed: false,
      health: runtimes[0]?.health ?? "starting",
      attached: false,
      controlEndpoint: runtimes[0]?.controlEndpoint,
      lifecyclePhase: "stopped",
    });
    assert.deepStrictEqual(
      controls,
      [],
      "pending first paint must expose no controls even with the client's initial stopped policy",
    );

    const stoppedAuthority = new NetworkCanvasRuntimeAuthority();
    const noTarget = stoppedAuthority.beginFirstPaint(undefined);
    assert.strictEqual(noTarget, undefined);
    const genuinelyStopped = initialNetworkCanvasGraph(
      "stopped",
      "welcome",
      SIMULATOR_RUNTIME_ID,
      noTarget,
    );
    const stoppedRuntime = genuinelyStopped.hosts[0]?.runtimes[0];
    assert.strictEqual(stoppedRuntime?.name, "Simulator");
    assert.strictEqual(stoppedRuntime?.health, "stopped");
  });

  test("a stopped Simulator is projected immediately after its accepted session ends", () => {
    const authorityState = new NetworkCanvasRuntimeAuthority();
    assert.deepStrictEqual(
      authorityState.beginFirstPaint({ kind: "simulator" }),
      { kind: "simulator" },
    );

    assert.strictEqual(
      authorityState.reconcile(undefined),
      undefined,
      "a terminated session must lose mutation authority immediately",
    );
    assert.deepStrictEqual(
      authorityState.lifecycleProjectionTarget(),
      { kind: "simulator" },
      "rendering must retain the just-stopped Simulator long enough to publish Stopped",
    );

    const stopped = buildImmediateSimulatorLifecycleGraph({
      phase: "stopped",
      stage: "welcome",
      managedRuntimes: [],
      selectedRuntimeId: SIMULATOR_RUNTIME_ID,
      deviceRequested: false,
      authorityTarget: authorityState.lifecycleProjectionTarget(),
    });
    const simulator = stopped?.hosts
      .flatMap((host) => host.runtimes)
      .find((runtime) => runtime.id === "runtime:local");
    assert.strictEqual(simulator?.name, "Simulator");
    assert.strictEqual(simulator?.health, "stopped");
    assert.strictEqual(
      simulator?.runTarget,
      true,
      "Stopped and selected are independent: the Simulator stays the chosen Run target without appearing Running",
    );

    authorityState.reconcile({
      kind: "remote",
      endpoint: "tcp://cell-a:9902",
      label: "Cell A",
    });
    assert.strictEqual(
      authorityState.lifecycleProjectionTarget(),
      undefined,
      "a new lifecycle target must clear the terminal Simulator projection",
    );
  });

  test("remote lifecycle authority never paints the local Simulator as running", () => {
    const snapshot = lifecycleSnapshot({
      status: {
        ...lifecycleSnapshot().status,
        running: true,
        runtimeMode: "online",
        runtimeState: "connected",
        endpoint: "tcp://cell-a:9902",
        endpointConfigured: true,
        endpointReachable: true,
      },
      activeTarget: {
        kind: "remote",
        endpoint: "tcp://cell-a:9902",
        label: "Cell A",
      },
    });
    const input = modelInputForSnapshot(
      "welcome",
      snapshot,
      { deviceRequested: false },
      { authorityTarget: snapshot.activeTarget },
    );

    assert.strictEqual(
      input.runtime,
      undefined,
      "connected remote evidence cannot be reused as local Simulator evidence",
    );
  });

  test("stopped-phase managed messages require current name and endpoint inventory", async () => {
    const endpointX = "tcp://127.0.0.1:9910";
    const endpointY = "tcp://127.0.0.1:9920";
    const authorityState = new NetworkCanvasRuntimeAuthority();
    authorityState.acceptInventory(undefined, [
      {
        name: "cell-a",
        controlEndpoint: endpointX,
        state: "stopped",
      },
    ]);
    const backendCalls: string[] = [];
    const blocked: string[] = [];
    const controller = new NetworkCanvasLifecycleActions({
      extensionContext: () => ({}) as vscode.ExtensionContext,
      refresh: async () => undefined,
      clearFailure: () => undefined,
      recordResult: () => undefined,
      stopRuntime: async () => ({ ok: true, message: "disconnected" }),
      connectRemote: async () => ({ ok: true, message: "connected" }),
      runExclusiveOperation: async (_kind, _target, operation) => ({
        acquired: true as const,
        value: await operation("inventory-operation"),
      }),
      lifecyclePhase: () => "stopped",
      activeTarget: () => authorityState.activeTarget(),
      managedTarget: (name, endpoint) =>
        authorityState.managedTarget(name, endpoint),
      operationInProgress: () => false,
      reportBlocked: (reason) => blocked.push(reason),
      startManagedRuntime: async (_context, name) => {
        backendCalls.push(`start:${name}`);
        return {
          ok: true,
          status: "running",
          controlEndpoint: endpointX,
        };
      },
      stopManagedRuntime: async (_context, name) => {
        backendCalls.push(`stop:${name}`);
        return {
          ok: true,
          status: "stopped",
          controlEndpoint: endpointX,
        };
      },
      attachManagedRuntimeAfterStart: async () => ({ ok: true }),
      disconnectManagedRuntimeAfterStop: async () => ({
        ok: true,
        message: "disconnected",
      }),
    });

    await controller.handleMessage({
      type: "runtimeManagedStart",
      name: "cell-a",
      endpoint: endpointX,
    });
    await controller.handleMessage({
      type: "runtimeManagedStop",
      name: "cell-a",
      endpoint: endpointX,
    });
    assert.deepStrictEqual(backendCalls, ["start:cell-a", "stop:cell-a"]);

    authorityState.invalidateInventory(undefined);
    await controller.handleMessage({
      type: "runtimeManagedStart",
      name: "cell-a",
      endpoint: endpointX,
    });
    await controller.handleMessage({
      type: "runtimeManagedStop",
      name: "cell-a",
      endpoint: endpointX,
    });
    assert.strictEqual(backendCalls.length, 2);

    authorityState.acceptInventory(undefined, [
      {
        name: "cell-a",
        controlEndpoint: endpointY,
        state: "stopped",
      },
    ]);
    await controller.handleMessage({
      type: "runtimeManagedStart",
      name: "cell-a",
      endpoint: endpointX,
    });
    await controller.handleMessage({
      type: "runtimeManagedStop",
      name: "cell-a",
      endpoint: endpointX,
    });
    assert.strictEqual(backendCalls.length, 2);
    assert.strictEqual(blocked.length, 4);
  });
});
