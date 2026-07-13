import * as assert from "assert";

import {
  runtimeOperationAllowed,
  runtimeOperationBlockReason,
  type RuntimeLockedAction,
} from "../../runtimeOperationPolicy";
import { runtimeNodeControls } from "../../networkCanvas/webview/runtimeNodeControls";
import { source } from "./runtime-operation-lock-contract-fixtures";

suite("Runtime operation lock contract", () => {
  test("local Simulator Start has a focused project-local coordinator", () => {
    const lifecycle = source("runtimeLifecycle.ts");
    const coordinator = source("localSimulatorStartCoordinator.ts");

    assert.ok(
      lifecycle.split(/\r?\n/).length < 1_000,
      "the lifecycle state authority must stay below the KISS large-file boundary",
    );
    assert.ok(lifecycle.includes("coordinateLocalSimulatorStart({"));
    assert.ok(!lifecycle.includes("saveDirtyProjectDocuments("));
    assert.ok(!lifecycle.includes("prepareLocalSimulatorProject("));
    assert.ok(
      coordinator.includes("saveDirtyProjectDocuments(dependencies.projectRoot)") &&
        coordinator.includes("vscode.workspace.getWorkspaceFolder(document.uri)") &&
        coordinator.includes("await document.save()") &&
        coordinator.includes("prepareLocalSimulatorProject(") &&
        coordinator.includes("dependencies.validateProject("),
      "the coordinator must save only the selected project before validation and DAP launch",
    );
    assert.ok(
      !coordinator.includes("projectValidationProof") &&
        !coordinator.includes("source_fingerprints") &&
        !coordinator.includes("config_fingerprints"),
      "the simulator lifecycle must not pull the unrelated Compile-attestation subsystem into this fix",
    );
  });

  test("the policy is action-specific across stopped, transition, launch, and attach phases", () => {
    const allActions: RuntimeLockedAction[] = [
      "compile",
      "apply_changes",
      "select_target",
      "set_run_target",
      "local_start",
      "local_stop",
      "remote_connect",
      "remote_disconnect",
      "managed_start",
      "managed_stop",
    ];

    assert.deepStrictEqual(
      allActions.filter((action) => runtimeOperationAllowed("stopped", action)),
      allActions.filter((action) => action !== "apply_changes"),
      "Stopped accepts fresh operations and harmless stale Stop/Disconnect, but cannot update a nonexistent running simulation",
    );
    assert.ok(
      allActions.every(
        (action) => !runtimeOperationAllowed("starting", action),
      ),
      "a transition owns the lifecycle until acceptance or failure",
    );
    assert.ok(
      allActions.every(
        (action) => !runtimeOperationAllowed("stopped", action, true),
      ),
      "a Compile/managed lease also locks stale clicks before phase changes",
    );
    assert.deepStrictEqual(
      allActions.filter((action) => runtimeOperationAllowed("running", action)),
      ["compile", "apply_changes", "local_stop", "managed_stop"],
      "a launched Simulator keeps Compile/Update and owned Stop, but blocks retargeting",
    );
    assert.deepStrictEqual(
      allActions.filter((action) =>
        runtimeOperationAllowed("connected", action),
      ),
      ["remote_disconnect", "managed_stop"],
      "an attached runtime keeps its owned Disconnect/Stop, but blocks retargeting",
    );
    assert.match(
      runtimeOperationBlockReason("starting", "compile") ?? "",
      /already in progress/i,
    );
    assert.match(
      runtimeOperationBlockReason("running", "select_target") ?? "",
      /Stop the Simulator/i,
    );
    assert.match(
      runtimeOperationBlockReason("connected", "set_run_target") ?? "",
      /Disconnect the remote runtime/i,
    );
  });

  test("runtime inspectors disable lifecycle and retarget clicks while diagnostics stay usable", () => {
    const startingRemote = runtimeNodeControls({
      isLocal: false,
      health: "stopped",
      attached: false,
      controlEndpoint: "tcp://pi:9902",
      logsAvailable: true,
      lifecyclePhase: "starting",
    });
    for (const action of ["runtimeConnect", "setAsRunTarget"] as const) {
      const control = startingRemote.find(
        (candidate) => candidate.action === action,
      );
      assert.strictEqual(
        control?.enabled,
        false,
        `${action} must be visibly disabled`,
      );
      assert.match(control?.disabledReason ?? "", /already in progress/i);
    }
    for (const action of ["openRuntimeLogs", "openRuntimeSettings"] as const) {
      assert.strictEqual(
        startingRemote.find((candidate) => candidate.action === action)
          ?.enabled,
        true,
        `${action} stays usable for diagnosis`,
      );
    }

    const attachedRemote = runtimeNodeControls({
      isLocal: false,
      health: "connected",
      attached: true,
      controlEndpoint: "tcp://pi:9902",
      lifecyclePhase: "connected",
    });
    assert.strictEqual(attachedRemote[0].action, "runtimeDisconnect");
    assert.strictEqual(
      attachedRemote[0].enabled,
      true,
      "the owned Disconnect remains available",
    );
    assert.strictEqual(
      attachedRemote.find((control) => control.action === "setAsRunTarget")
        ?.enabled,
      false,
      "the accepted attach cannot be hidden by selecting another target",
    );

    const ownedManaged = runtimeNodeControls({
      isLocal: false,
      managed: true,
      health: "connected",
      attached: true,
      lifecyclePhase: "connected",
    });
    const otherManaged = runtimeNodeControls({
      isLocal: false,
      managed: true,
      health: "connected",
      attached: false,
      lifecyclePhase: "connected",
    });
    assert.strictEqual(ownedManaged[0].action, "managedStop");
    assert.strictEqual(
      ownedManaged[0].enabled,
      true,
      "the owned managed Stop remains available",
    );
    assert.strictEqual(
      otherManaged[0].enabled,
      false,
      "another managed runtime cannot steal the session",
    );
  });
});
