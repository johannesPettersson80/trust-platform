import * as assert from "assert";
import {
  canonicalConnectorHealth,
  canonicalConnectorConfidence,
  canonicalConnectorState,
  mergeConnectorStatusSafely,
  mergeConnectorStatusIntoTopology,
} from "../../networkCanvas/connectorsStatus";
import type { FleetTopologyResponse } from "../../networkCanvas/fleetTopology";

function peerTopology(): FleetTopologyResponse {
  return {
    schema_version: 3,
    hosts: [
      {
        host_id: "peer-host",
        hostname: "peer-host",
        arch: "x86_64",
        os: "linux",
        ips: ["10.0.0.2"],
        containers: [],
        runtimes: [
          {
            runtime_id: "peer-runtime",
            name: "peer-runtime",
            mode: "production",
            cycle_ms: 10,
            health: "connected",
            detail: "reachable",
            endpoints: [
              {
                id: "mqtt",
                kind: "service",
                protocol: "mqtt",
                name: "mqtt",
                address: "mqtt://10.0.0.2",
                health: "connected",
                detail: "reachable",
                owned: true,
                supports_test: true,
              },
            ],
          },
        ],
      },
    ],
    links: [],
    shared: [],
    external: [],
  };
}

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
    const confidence = [
      "confirmed",
      "likely",
      "port_reachable",
      "unavailable",
    ] as const;
    assert.deepStrictEqual(states.map(canonicalConnectorState), [...states]);
    assert.deepStrictEqual(health.map(canonicalConnectorHealth), [...health]);
    assert.deepStrictEqual(confidence.map(canonicalConnectorConfidence), [
      ...confidence,
    ]);
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

  test("rejects unknown discovery confidence instead of projecting a peer", () => {
    assert.throws(
      () =>
        mergeConnectorStatusIntoTopology(peerTopology(), {
          schema_version: 1,
          connectors: [
            {
              connector_id: "peer-mqtt",
              protocol: "mqtt",
              state: "ready",
              health: "ok",
              confidence: "certainly_healthy",
            },
          ],
        }),
      /unknown connector confidence/
    );
  });

  test("keeps peer topology and reports invalid connector vocabulary", () => {
    const topology = peerTopology();
    const result = mergeConnectorStatusSafely(
      topology,
      {
        schema_version: 1,
        connectors: [
          {
            connector_id: "peer-mqtt",
            protocol: "mqtt",
            state: "ready",
            health: "ok",
            confidence: "certainly_healthy",
          },
        ],
      },
      "peer-a"
    );

    assert.strictEqual(result.topology, topology);
    assert.deepStrictEqual(result.errors, [
      "peer-a connector status: unknown connector confidence: certainly_healthy",
    ]);
  });
});
