import * as assert from "assert";

import { runTargetOf } from "../../hmiBuilder/editor/runTarget";

// Specification 39, §11 item 6 and §16.2 item 8: Live mode and Deploy talk to the simulator or
// runtime the extension runs, or to the configured online runtime when it answers.

suite("HMI Builder: the run target", () => {
  test("a running simulator session is the target", () => {
    assert.deepStrictEqual(
      runTargetOf({
        runtimeMode: "simulate",
        runtimeState: "running",
        endpoint: "tcp://127.0.0.1:9001",
        endpointReachable: false,
      }),
      { endpoint: "tcp://127.0.0.1:9001", label: "Simulator", simulator: true },
    );
  });

  test("a configured online runtime that answers is the target, with no debug session", () => {
    assert.deepStrictEqual(
      runTargetOf({
        runtimeMode: "online",
        runtimeState: "stopped",
        endpoint: "tcp://192.168.1.20:9000",
        endpointReachable: true,
      }),
      { endpoint: "tcp://192.168.1.20:9000", label: "tcp://192.168.1.20:9000", simulator: false },
    );
  });

  test("an online runtime that does not answer, and a stopped simulator, are no target", () => {
    assert.strictEqual(
      runTargetOf({
        runtimeMode: "online",
        runtimeState: "stopped",
        endpoint: "tcp://192.168.1.20:9000",
        endpointReachable: false,
      }),
      undefined,
    );
    assert.strictEqual(
      runTargetOf({ runtimeMode: "simulate", runtimeState: "stopped", endpoint: "", endpointReachable: false }),
      undefined,
    );
  });
});
