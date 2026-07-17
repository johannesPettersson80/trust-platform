import * as assert from "assert";
import {
  canonicalConnectorHealth,
  canonicalConnectorState,
} from "../../networkCanvas/connectorsStatus";

suite("connector status contract", () => {
  test("maps every canonical state and health without changing wire meaning", () => {
    const states = [
      "disabled",
      "configured",
      "starting",
      "ready",
      "degraded",
      "reconnecting",
      "stale",
      "not_ready",
      "faulted",
    ] as const;
    const health = ["ok", "degraded", "faulted", "unknown"] as const;
    assert.deepStrictEqual(states.map(canonicalConnectorState), [...states]);
    assert.deepStrictEqual(health.map(canonicalConnectorHealth), [...health]);
  });

  test("rejects unknown state and health instead of rendering healthy", () => {
    assert.throws(
      () => canonicalConnectorState("invented_healthy"),
      /unknown connector state/
    );
    assert.throws(
      () => canonicalConnectorHealth("excellent"),
      /unknown connector health/
    );
  });
});
