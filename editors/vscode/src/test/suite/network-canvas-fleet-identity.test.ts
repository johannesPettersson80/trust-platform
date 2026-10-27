import * as assert from "assert";
import * as vscode from "vscode";

import {
  DEVICES_TOPOLOGY_COMMAND,
  requestCanvasTopology,
  type CanvasRuntimeReport,
} from "../../networkCanvas/canvasCommands";
import type { FleetTopologyResponse } from "../../networkCanvas/fleetTopology";
import { canvasGraph } from "../../networkCanvas/graphData";
import { buildGraph } from "../../networkCanvas/webview/layout";
import type { NCGraph } from "../../networkCanvas/webview/types";

// The owner-scoped identities (specification 25 §5.6) are the language server's: the Rust tests in
// crates/trust-devices/src/topology/tests/identity.rs port this suite's identity assertions
// (`equal_raw_ids_under_different_owners_stay_distinct`, `cross_runtime_link_scopes_each_end`,
// `normalization_is_idempotent_and_leaves_its_input_unchanged`, `ambiguous_reference_drops_only_its_link`
// and the others). These tests prove that the canvas draws what the server answers: every node and
// edge once, with each link's status and detail.

type LegacyMeshStatus = "connected" | "degraded" | "error";
type FleetRuntime = FleetTopologyResponse["hosts"][number]["runtimes"][number];

function legacyFleetIdPart(value: string): string {
  const sanitized = [...value]
    .map((character) =>
      /[A-Za-z0-9]/.test(character) ? character.toLowerCase() : "-"
    )
    .join("")
    .replace(/^-+|-+$/g, "");
  return sanitized || "unknown";
}

function legacyMeshSnapshot(options: {
  schemaVersion: 2 | 3 | 4;
  hostId: string;
  runtimeId: string;
  peerAddress: string;
  status: LegacyMeshStatus;
  detail: string;
  containerId?: string;
}): FleetTopologyResponse {
  const externalId = "external:mesh:0";
  const endpointId = `endpoint:${options.runtimeId}:mesh`;
  const runtime: FleetRuntime = {
    runtime_id: options.runtimeId,
    name: options.runtimeId,
    mode: "online",
    cycle_ms: 10,
    health: "connected",
    detail: "Running.",
    endpoints: [
      {
        id: endpointId,
        kind: "peer",
        protocol: "mesh",
        name: "Mesh / Zenoh",
        health: options.status,
        detail: options.detail,
        owned: true,
        supports_test: false,
        ...(options.schemaVersion === 4
          ? { role: "peer", source: "self" }
          : {}),
      },
    ],
    ...(options.schemaVersion === 4 ? { source: "self" } : {}),
  };
  const link: FleetTopologyResponse["links"][number] = {
    from: endpointId,
    to: externalId,
    protocol: "mesh",
    direction: "outbound",
    same_host: false,
    status: options.status,
    secure: true,
    detail: options.detail,
    ...(options.schemaVersion >= 3
      ? {
          id: `link:mesh:peer:${legacyFleetIdPart(endpointId)}:${legacyFleetIdPart(
            externalId
          )}`,
          role: "peer",
        }
      : {}),
  };
  const containers = options.containerId
    ? [
        {
          container_id: options.containerId,
          name: options.containerId,
          image: "runtime:latest",
          status: "running",
          runtimes: [runtime],
          ...(options.schemaVersion === 4 ? { source: "self" } : {}),
        },
      ]
    : [];
  return {
    schema_version: options.schemaVersion,
    hosts: [
      {
        host_id: options.hostId,
        hostname: options.hostId,
        arch: "x86_64",
        os: "linux",
        ips: [],
        containers,
        runtimes: options.containerId ? [] : [runtime],
        ...(options.schemaVersion === 4 ? { source: "self" } : {}),
      },
    ],
    links: [link],
    shared: [
      {
        id: "shared:mqtt:broker",
        kind: "broker",
        name: "MQTT broker",
        address: "127.0.0.1:1883",
        used_by: [options.runtimeId],
      },
    ],
    external: [
      {
        id: externalId,
        kind: "peer",
        name: options.peerAddress,
        via_protocol: ["mesh"],
        direction: "outbound",
        ...(options.schemaVersion === 4 ? { source: "config" } : {}),
      },
    ],
  };
}

async function languageServerReady(): Promise<void> {
  const deadline = Date.now() + 30000;
  while (!(await vscode.commands.getCommands(true)).includes(DEVICES_TOPOLOGY_COMMAND)) {
    if (Date.now() > deadline) assert.fail("the language server's devices commands are not available");
    await new Promise((resolve) => setTimeout(resolve, 200));
  }
}

/**
 * The canvas drawn from `snapshots`, each the answer of its own configured runtime. Nothing reports the
 * project's runtime, so the simulator stands for it on this computer (specification 25 §5.6); `peers`
 * leaves it out.
 */
async function drawn(snapshots: readonly FleetTopologyResponse[]) {
  const peers: CanvasRuntimeReport[] = snapshots.map((snapshot, index) => ({
    endpoint: `tcp://10.0.0.${index + 1}:5680`,
    label: `runtime ${index + 1}`,
    reach: "reachable",
    topology: { ok: snapshot },
    connectors: { ok: { connectors: [] } },
  }));
  const answer = await requestCanvasTopology({
    peers,
    run_mode: "online",
    search_query: "",
    discovered: [],
  });
  assert.strictEqual(answer.topology_error, undefined, "every snapshot reads");
  const graph = canvasGraph(answer.fleet);
  const rendered = buildGraph(graph);
  const bus = rendered.nodes.find((node) => node.id === "bus:mesh");
  const meshHandleIds = (
    (bus?.data as { handles?: Array<{ id: string }> } | undefined)?.handles ?? []
  ).map((handle) => handle.id);
  return { graph, rendered, meshHandleIds, peers: peerHosts(graph) };
}

/** The computers of the configured runtimes: every computer but the one the simulator is drawn on. */
function peerHosts(graph: NCGraph) {
  return graph.hosts.filter((host) => host.id !== "host:this-computer");
}

function assertDrawnOnce(label: string, drawing: Awaited<ReturnType<typeof drawn>>): void {
  const nodeIds = drawing.rendered.nodes.map((node) => node.id);
  const edgeIds = drawing.rendered.edges.map((edge) => edge.id);
  assert.strictEqual(new Set(nodeIds).size, nodeIds.length, `${label}: duplicate node IDs`);
  assert.strictEqual(new Set(edgeIds).size, edgeIds.length, `${label}: duplicate edge IDs`);
  assert.strictEqual(
    new Set(drawing.meshHandleIds).size,
    drawing.meshHandleIds.length,
    `${label}: duplicate mesh handles`
  );
}

suite("Network Canvas fleet identity", function () {
  this.timeout(60000);
  suiteSetup(languageServerReady);

  test("legacy mesh snapshots of every schema draw each runtime, peer and link once with its detail", async () => {
    for (const schemaVersion of [2, 3, 4] as const) {
      const drawing = await drawn([
        legacyMeshSnapshot({
          schemaVersion,
          hostId: "host:line-a",
          runtimeId: "RESOURCE",
          peerAddress: "tcp/192.168.77.11:7447",
          status: "degraded",
          detail: "peer A degraded",
        }),
        legacyMeshSnapshot({
          schemaVersion,
          hostId: "host:line-b",
          runtimeId: "RESOURCE",
          peerAddress: "tcp/192.168.77.12:7447",
          status: "error",
          detail: "peer B failed",
        }),
      ]);
      assertDrawnOnce(`schema ${schemaVersion}`, drawing);
      assert.strictEqual(drawing.peers.length, 2, `schema ${schemaVersion}`);
      assert.deepStrictEqual(
        drawing.graph.external
          .filter((external) => external.kind === "peer")
          .map((external) => external.name)
          .sort(),
        ["tcp/192.168.77.11:7447", "tcp/192.168.77.12:7447"]
      );
      for (const detail of ["peer A degraded", "peer B failed"]) {
        assert.ok(
          drawing.rendered.edges.some((edge) => edge.data?.detail === detail),
          `schema ${schemaVersion} lost ${detail}`
        );
      }
    }
  });

  test("same-host and container-owned runtimes with colliding raw IDs stay distinct on the canvas", async () => {
    const scenarios: Array<{ label: string; snapshots: FleetTopologyResponse[] }> = [
      {
        label: "same host, runtime IDs that collide after display sanitization",
        snapshots: [
          legacyMeshSnapshot({
            schemaVersion: 4,
            hostId: "host:line",
            runtimeId: "Line_A",
            peerAddress: "tcp/192.168.77.21:7447",
            status: "degraded",
            detail: "line A degraded",
          }),
          legacyMeshSnapshot({
            schemaVersion: 4,
            hostId: "host:line",
            runtimeId: "line-a",
            peerAddress: "tcp/192.168.77.22:7447",
            status: "error",
            detail: "line B failed",
          }),
        ],
      },
      {
        label: "same host, separate containers with the same runtime ID",
        snapshots: [
          legacyMeshSnapshot({
            schemaVersion: 4,
            hostId: "host:containers",
            containerId: "Cell_A",
            runtimeId: "RESOURCE",
            peerAddress: "tcp/192.168.77.31:7447",
            status: "degraded",
            detail: "container A degraded",
          }),
          legacyMeshSnapshot({
            schemaVersion: 4,
            hostId: "host:containers",
            containerId: "cell-a",
            runtimeId: "RESOURCE",
            peerAddress: "tcp/192.168.77.32:7447",
            status: "error",
            detail: "container B failed",
          }),
        ],
      },
      {
        label: "different hosts with the same container and runtime IDs",
        snapshots: [
          legacyMeshSnapshot({
            schemaVersion: 4,
            hostId: "host:container-a",
            containerId: "Cell",
            runtimeId: "RESOURCE",
            peerAddress: "tcp/192.168.77.33:7447",
            status: "degraded",
            detail: "host A container degraded",
          }),
          legacyMeshSnapshot({
            schemaVersion: 4,
            hostId: "host:container-b",
            containerId: "Cell",
            runtimeId: "RESOURCE",
            peerAddress: "tcp/192.168.77.34:7447",
            status: "error",
            detail: "host B container failed",
          }),
        ],
      },
    ];

    for (const scenario of scenarios) {
      const drawing = await drawn(scenario.snapshots);
      assertDrawnOnce(scenario.label, drawing);
      const runtimes = drawing.peers.flatMap((host) => [
        ...host.runtimes,
        ...host.containers.flatMap((container) => container.runtimes),
      ]);
      assert.strictEqual(runtimes.length, 2, scenario.label);
      assert.strictEqual(new Set(runtimes.map((runtime) => runtime.id)).size, 2, scenario.label);
      const links = drawing.graph.links;
      assert.strictEqual(links.length, 2, scenario.label);
      const ids = new Set([
        ...runtimes.flatMap((runtime) => runtime.endpoints.map((endpoint) => endpoint.id)),
        ...drawing.graph.external.map((external) => external.id),
      ]);
      assert.ok(
        links.every((link) => ids.has(link.from) && ids.has(link.to)),
        `${scenario.label}: every link names a node of the graph`
      );
    }
  });
});
