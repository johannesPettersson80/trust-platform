import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";

import { RuntimeLifecycleService } from "../../runtimeLifecycle";
import { disconnectManagedRuntimeAfterStop } from "../../managedRuntimeSession";
import {
  MANAGED_RUNTIME_ID_FIELD,
  runtimeTargetForSession,
  type RuntimeLifecycleResult,
  type RuntimeLifecycleSnapshot,
} from "../../runtimeLifecycleModel";
import {
  VALID,
  acceptSession,
  deferred,
  fakeSession,
} from "./runtime-operation-lock-fixtures";

suite("Runtime lifecycle operation lock", () => {
  test("Apply holds the shared lease without repainting Running as Starting", async () => {
    const service = new RuntimeLifecycleService();
    acceptSession(service, fakeSession("launch", "accepted-launch"));
    const applyGate = deferred<string>();
    const apply = service.runExclusiveOperation(
      "apply_changes",
      { kind: "simulator" },
      async () => applyGate.promise,
    );
    await Promise.resolve();

    assert.strictEqual(service.phase(), "running");
    assert.strictEqual(service.operationState()?.kind, "apply_changes");
    let validatorCalls = 0;
    const start = await service.startLocalSimulator(async () => {
      validatorCalls += 1;
      return VALID;
    });
    assert.strictEqual(start.ok, false);
    assert.strictEqual(validatorCalls, 0);
    assert.strictEqual((await service.stopRuntime()).ok, false);
    assert.strictEqual(
      (
        await service.runExclusiveOperation(
          "compile",
          { kind: "simulator" },
          async () => "compile",
        )
      ).acquired,
      false,
    );

    applyGate.resolve("applied");
    const applied = await apply;
    assert.deepStrictEqual(applied, { acquired: true, value: "applied" });
    assert.strictEqual(service.phase(), "running");
    assert.strictEqual(service.operationState(), undefined);
  });

  test("Stop owns the shared lease through exact-session disappearance", async () => {
    const stopGate = deferred<void>();
    const service = new RuntimeLifecycleService(
      async () => true,
      async () => stopGate.promise,
    );
    acceptSession(service, fakeSession("launch", "accepted-launch"));
    const stop = service.stopRuntime();
    await Promise.resolve();

    assert.strictEqual(service.phase(), "running");
    assert.strictEqual(service.operationState()?.kind, "local_stop");
    assert.strictEqual((await service.stopRuntime()).ok, false);
    for (const kind of ["compile", "apply_changes"] as const) {
      const overlap = await service.runExclusiveOperation(
        kind,
        { kind: "simulator" },
        async () => kind,
      );
      assert.strictEqual(overlap.acquired, false, kind);
    }

    stopGate.resolve(undefined);
    const stopped = await stop;
    assert.strictEqual(stopped.ok, true);
    assert.strictEqual(service.phase(), "stopped");
    assert.strictEqual(service.operationState(), undefined);
  });

  test("remote Disconnect blocks same-endpoint Connect until the exact session is gone", async () => {
    const stopGate = deferred<void>();
    const endpoint = "tcp://remote.test:5680";
    const service = new RuntimeLifecycleService(
      async () => true,
      async () => stopGate.promise,
    );
    acceptSession(service, fakeSession("attach", "accepted-attach", endpoint));

    const stop = service.stopRuntime();
    await Promise.resolve();

    assert.strictEqual(service.phase(), "connected");
    assert.strictEqual(service.operationState()?.kind, "remote_disconnect");
    const reconnect = await service.connectRemote(endpoint, "Remote PLC");
    assert.strictEqual(reconnect.ok, false);
    assert.strictEqual(service.operationState()?.kind, "remote_disconnect");

    stopGate.resolve(undefined);
    const stopped = await stop;
    assert.strictEqual(stopped.ok, true);
    assert.strictEqual(service.phase(), "stopped");
    assert.strictEqual(service.operationState(), undefined);
  });

  test("managed Stop propagates a failed attached-session disconnect", async () => {
    const expected: RuntimeLifecycleResult = {
      ok: false,
      failure: {
        kind: "stale_runtime",
        message:
          "The managed process stopped, but the debug session stayed attached.",
      },
    };
    let stopCalls = 0;
    const actual = await disconnectManagedRuntimeAfterStop(
      "cell-b",
      {
        ok: true,
        status: "stopped",
        controlEndpoint: "tcp://127.0.0.1:9902",
      },
      "managed-stop-attempt",
      {
        kind: "managed",
        id: "cell-b",
        endpoint: "tcp://127.0.0.1:9902",
      },
      {
        snapshot: async () =>
          ({
            status: {
              runtimeMode: "online",
              runtimeState: "connected",
              endpoint: "tcp://127.0.0.1:9902",
              targetLabel: "cell-a (this computer)",
            },
            activeTarget: {
              kind: "managed",
              id: "cell-a",
              endpoint: "tcp://127.0.0.1:9902",
            },
          }) as RuntimeLifecycleSnapshot,
        stopRuntime: async (operationId) => {
          stopCalls += 1;
          assert.strictEqual(operationId, "managed-stop-attempt");
          return expected;
        },
      },
    );

    assert.strictEqual(actual, expected);
    assert.strictEqual(stopCalls, 1);
  });

  test("managed attach session carries an explicit runtime identity", () => {
    const session = fakeSession(
      "attach",
      "managed-attach",
      "tcp://127.0.0.1:9902",
    );
    session.configuration[MANAGED_RUNTIME_ID_FIELD] = "cell-a";
    session.configuration.targetLabel = "cell-b (this computer)";

    assert.deepStrictEqual(runtimeTargetForSession(session), {
      kind: "managed",
      id: "cell-a",
      endpoint: "tcp://127.0.0.1:9902",
    });
  });

  test("accepted attach conflicts with local Start and accepted launch blocks remote mutation", async () => {
    const attached = new RuntimeLifecycleService();
    const attachedSession = fakeSession("attach", "accepted-attach");
    acceptSession(attached, attachedSession);
    assert.deepStrictEqual(attached.activeTarget(), {
      kind: "remote",
      endpoint: "tcp://remote.test:5680",
      label: "Remote PLC",
    });
    let validatorCalls = 0;
    const local = await attached.startLocalSimulator(async () => {
      validatorCalls += 1;
      return VALID;
    });
    assert.strictEqual(local.ok, false);
    assert.strictEqual(validatorCalls, 0);

    const launched = new RuntimeLifecycleService();
    acceptSession(launched, fakeSession("launch", "accepted-launch"));
    const remote = await launched.connectRemote(
      "tcp://different-remote.test:5680",
      "Different remote",
    );
    assert.strictEqual(remote.ok, false);
    assert.strictEqual(launched.phase(), "running");

    const source = fs.readFileSync(
      path.join(
        path.resolve(__dirname, "../../.."),
        "src",
        "runtimeLifecycle.ts",
      ),
      "utf8",
    );
    const connect = source.slice(
      source.indexOf("async connectRemote("),
      source.indexOf("private async settleOwnedTransition("),
    );
    assert.ok(
      connect.indexOf("const accepted = this.acceptedLifecycleSession()") <
        connect.indexOf('config.update("runtime.controlEndpoint"'),
      "remote Connect must reject an owned-session conflict before any config mutation",
    );
  });

  test("accepted session request is authoritative over stale persisted mode and target", () => {
    const status = fs.readFileSync(
      path.join(
        path.resolve(__dirname, "../../.."),
        "src",
        "io-panel",
        "status.ts",
      ),
      "utf8",
    );
    const lifecycle = fs.readFileSync(
      path.join(
        path.resolve(__dirname, "../../.."),
        "src",
        "runtimeLifecycle.ts",
      ),
      "utf8",
    );
    const events = fs.readFileSync(
      path.join(
        path.resolve(__dirname, "../../.."),
        "src",
        "runtimeLifecycleEvents.ts",
      ),
      "utf8",
    );
    const sessionAuthority = fs.readFileSync(
      path.join(
        path.resolve(__dirname, "../../.."),
        "src",
        "runtimeSessionAuthority.ts",
      ),
      "utf8",
    );
    const ownedAcceptance = lifecycle.slice(
      lifecycle.indexOf("private async settleOwnedTransition("),
      lifecycle.indexOf("async stopRuntime("),
    );
    const externalAcceptance = lifecycle.slice(
      lifecycle.indexOf("private async acceptExternalSession("),
      lifecycle.indexOf("private async waitForAttachedSessionReady("),
    );
    assert.ok(
      status.includes(
        'runtimeMode = request === "attach" ? "online" : "simulate"',
      ) && status.includes("endpointConfigured = endpoint.length > 0"),
      "accepted launch/attach must override stale settings and expose the exact session endpoint",
    );
    assert.ok(
      ownedAcceptance.indexOf(
        "await this.selectAcceptedSessionTarget(ownedSession)",
      ) >= 0 &&
        ownedAcceptance.indexOf(
          "await this.selectAcceptedSessionTarget(ownedSession)",
        ) < ownedAcceptance.indexOf("this.acceptedSessions.add(key)") &&
        ownedAcceptance.includes(
          "this.sessions.get(key) !== ownedSession",
        ) &&
        externalAcceptance.indexOf(
          "await this.selectAcceptedSessionTarget(session)",
        ) >= 0 &&
        externalAcceptance.indexOf(
          "await this.selectAcceptedSessionTarget(session)",
        ) < externalAcceptance.indexOf("this.acceptedSessions.add(key)") &&
        externalAcceptance.includes("this.sessions.get(key) !== session") &&
        sessionAuthority.includes("setSelectedRuntimeId") &&
        events.includes("deps.acceptExternal(active)"),
      "owned and direct-F5 acceptance must persist the target and recheck exact identity before publishing acceptance",
    );
  });
});
