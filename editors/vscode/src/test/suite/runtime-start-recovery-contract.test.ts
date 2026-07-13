import * as assert from "assert";

import { classifyRuntimeStartFailure } from "../../networkCanvas/runtimeFailures";
import { actionFailureMessage } from "../../trustHomeFailures";
import { startFailureChoices } from "../../trustHomeView";
import type { RuntimeStartFailure } from "../../runtimeLifecycle";
import type { SelectedRuntime } from "../../trustHomeModel";

function failure(kind: RuntimeStartFailure["kind"]): RuntimeStartFailure {
  return { kind, message: `${kind} safe message` };
}

suite("Simulator Start recovery action contract", () => {
  const selectedSimulator: SelectedRuntime = {
    id: "simulator",
    label: "Simulator",
    kind: "simulator",
    status: "stopped",
    statusLabel: "Stopped",
    primary: { action: "start", label: "Start", enabled: true },
  };

  test("each failure kind offers at most one matching recovery", () => {
    assert.deepStrictEqual(startFailureChoices(failure("configuration")), [
      "Open runtime.toml",
    ]);
    for (const kind of [
      "internal_startup",
      "port_conflict",
      "readiness_timeout",
      "failed_spawn",
      "missing_binary",
      "workspace_permission",
      "stale_runtime",
    ] as const) {
      assert.deepStrictEqual(
        startFailureChoices(failure(kind)),
        ["Open logs"],
        kind
      );
    }
  });

  test("classifier copy and recovery action agree for log-directed failures", () => {
    const cases = [
      "address already in use",
      "simulator readiness timed out",
      "unexpected adapter exit",
    ];
    for (const raw of cases) {
      const classified = classifyRuntimeStartFailure(raw);
      assert.deepStrictEqual(startFailureChoices(classified), ["Open logs"]);
      assert.match(
        classified.message,
        /logs|Debugger output|startup failed/i,
        raw
      );
      assert.doesNotMatch(classified.message, /tcp:\/\/|auth_token|os error/i);
    }
  });

  test("the sidebar does not repeat a Simulator failure that already names its subject", () => {
    const message =
      "Simulator could not start. The logs show what blocked startup.";
    assert.strictEqual(
      actionFailureMessage(selectedSimulator, {
        ok: false,
        failure: { kind: "stale_runtime", message },
      }),
      message,
    );
    assert.strictEqual(
      actionFailureMessage(selectedSimulator, {
        ok: false,
        failure: {
          kind: "missing_binary",
          message: "Required runtime binary was not found.",
        },
      }),
      "Could not start the simulator: Required runtime binary was not found.",
    );
  });
});
