import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";

import {
  mergeFleetTopologies,
  type FleetTopologyResponse,
} from "../../networkCanvas/fleetTopology";
import { buildCanvasGraph } from "../../networkCanvas/graphData";
import { buildNetworkCanvasModel } from "../../networkCanvas/model";
import { buildGraph } from "../../networkCanvas/webview/layout";

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

function topologyRuntimes(topology: FleetTopologyResponse): FleetRuntime[] {
  return topology.hosts.flatMap((host) => [
    ...host.runtimes,
    ...host.containers.flatMap((container) => container.runtimes),
  ]);
}

function cloneTopology(topology: FleetTopologyResponse): FleetTopologyResponse {
  return JSON.parse(JSON.stringify(topology)) as FleetTopologyResponse;
}

function sourceOf(value: unknown): string | undefined {
  return (value as { source?: string }).source;
}

function renderedIdentitySets(topology: FleetTopologyResponse): {
  nodeIds: string[];
  edgeIds: string[];
  meshHandleIds: string[];
} {
  const rendered = buildGraph(
    buildCanvasGraph(buildNetworkCanvasModel({ topology }), topology)
  );
  const bus = rendered.nodes.find((node) => node.id === "bus:mesh");
  const meshHandleIds = (
    (bus?.data as { handles?: Array<{ id: string }> } | undefined)?.handles ?? []
  ).map((handle) => handle.id);
  return {
    nodeIds: rendered.nodes.map((node) => node.id),
    edgeIds: rendered.edges.map((edge) => edge.id),
    meshHandleIds,
  };
}

suite("Network Canvas fleet identity", function () {
  test("legacy mesh identity normalization covers topology schemas v2 through v4", () => {
    for (const schemaVersion of [2, 3, 4] as const) {
      const first = legacyMeshSnapshot({
        schemaVersion,
        hostId: "host:line-a",
        runtimeId: "RESOURCE",
        peerAddress: "tcp/192.168.77.11:7447",
        status: "degraded",
        detail: "peer A degraded",
      });
      const second = legacyMeshSnapshot({
        schemaVersion,
        hostId: "host:line-b",
        runtimeId: "RESOURCE",
        peerAddress: "tcp/192.168.77.12:7447",
        status: "error",
        detail: "peer B failed",
      });
      const beforeFirst = cloneTopology(first);
      const beforeSecond = cloneTopology(second);

      const merged = mergeFleetTopologies([first, second]);
      const runtimes = topologyRuntimes(merged);
      const endpointIds = runtimes.flatMap((runtime) =>
        runtime.endpoints.map((endpoint) => endpoint.id)
      );
      const externalIds = merged.external.map((external) => external.id);
      assert.deepStrictEqual(first, beforeFirst, "normalization mutated the first input");
      assert.deepStrictEqual(second, beforeSecond, "normalization mutated the second input");
      assert.strictEqual(merged.hosts.length, 2, `schema ${schemaVersion}`);
      assert.strictEqual(runtimes.length, 2, `schema ${schemaVersion}`);
      assert.strictEqual(new Set(runtimes.map((runtime) => runtime.runtime_id)).size, 2);
      assert.strictEqual(new Set(endpointIds).size, 2);
      assert.strictEqual(externalIds.length, 2);
      assert.strictEqual(new Set(externalIds).size, 2);
      assert.strictEqual(merged.links.length, 2);
      assert.strictEqual(new Set(merged.links.map((link) => link.id)).size, 2);
      assert.ok(merged.links.every((link) => endpointIds.includes(link.from)));
      assert.ok(merged.links.every((link) => externalIds.includes(link.to)));
      assert.deepStrictEqual(
        runtimes.map((runtime) => runtime.name),
        ["RESOURCE", "RESOURCE"],
        `schema ${schemaVersion} changed runtime display names`
      );
      assert.deepStrictEqual(
        merged.external.map((external) => external.name).sort(),
        ["tcp/192.168.77.11:7447", "tcp/192.168.77.12:7447"]
      );
      assert.deepStrictEqual(
        merged.links
          .map((link) => ({
            status: link.status,
            detail: link.detail,
            secure: link.secure,
          }))
          .sort((left, right) =>
            String(left.detail).localeCompare(String(right.detail))
          ),
        [
          { status: "degraded", detail: "peer A degraded", secure: true },
          { status: "error", detail: "peer B failed", secure: true },
        ]
      );
      if (schemaVersion === 4) {
        assert.ok(merged.hosts.every((host) => sourceOf(host) === "self"));
        assert.ok(runtimes.every((runtime) => sourceOf(runtime) === "self"));
        assert.ok(
          runtimes.every((runtime) =>
            runtime.endpoints.every((endpoint) => sourceOf(endpoint) === "self")
          )
        );
        assert.ok(merged.external.every((external) => sourceOf(external) === "config"));
      }

      const rendered = buildGraph(
        buildCanvasGraph(buildNetworkCanvasModel({ topology: merged }), merged)
      );
      assert.strictEqual(
        new Set(rendered.nodes.map((node) => node.id)).size,
        rendered.nodes.length,
        `schema ${schemaVersion} rendered duplicate node IDs`
      );
      assert.strictEqual(
        new Set(rendered.edges.map((edge) => edge.id)).size,
        rendered.edges.length,
        `schema ${schemaVersion} rendered duplicate edge IDs`
      );
      for (const detail of ["peer A degraded", "peer B failed"]) {
        assert.ok(
          rendered.edges.some((edge) => edge.data?.detail === detail),
          `schema ${schemaVersion} lost ${detail}`
        );
      }
    }
  });

  test("same-host and container-owned legacy mesh peers keep lossless owner identity", () => {
    const scenarios: Array<{
      label: string;
      snapshots: FleetTopologyResponse[];
    }> = [
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
      const merged = mergeFleetTopologies(scenario.snapshots);
      const runtimes = topologyRuntimes(merged);
      const endpointIds = runtimes.flatMap((runtime) =>
        runtime.endpoints.map((endpoint) => endpoint.id)
      );
      const containerIds = merged.hosts.flatMap((host) =>
        host.containers.map((container) => container.container_id)
      );
      const externalIds = merged.external.map((external) => external.id);
      assert.strictEqual(runtimes.length, 2, scenario.label);
      assert.strictEqual(new Set(runtimes.map((runtime) => runtime.runtime_id)).size, 2);
      assert.strictEqual(new Set(endpointIds).size, 2);
      if (containerIds.length > 0) {
        assert.strictEqual(containerIds.length, 2);
        assert.strictEqual(new Set(containerIds).size, 2, scenario.label);
      }
      assert.strictEqual(externalIds.length, 2);
      assert.strictEqual(new Set(externalIds).size, 2);
      assert.strictEqual(merged.links.length, 2);
      assert.strictEqual(new Set(merged.links.map((link) => link.id)).size, 2);
      assert.ok(merged.links.every((link) => endpointIds.includes(link.from)));
      assert.ok(merged.links.every((link) => externalIds.includes(link.to)));
      assert.ok(merged.hosts.every((host) => sourceOf(host) === "self"));
      assert.ok(
        merged.hosts.every((host) =>
          host.containers.every((container) => sourceOf(container) === "self")
        )
      );
      assert.ok(runtimes.every((runtime) => sourceOf(runtime) === "self"));
      assert.ok(
        runtimes.every((runtime) =>
          runtime.endpoints.every((endpoint) => sourceOf(endpoint) === "self")
        )
      );
      assert.ok(merged.external.every((external) => sourceOf(external) === "config"));
      assert.deepStrictEqual(
        new Set(merged.shared[0]?.used_by ?? []),
        new Set(runtimes.map((runtime) => runtime.runtime_id))
      );
      const identities = renderedIdentitySets(merged);
      assert.strictEqual(new Set(identities.nodeIds).size, identities.nodeIds.length, scenario.label);
      assert.strictEqual(new Set(identities.edgeIds).size, identities.edgeIds.length, scenario.label);
      assert.strictEqual(
        new Set(identities.meshHandleIds).size,
        identities.meshHandleIds.length,
        scenario.label
      );
    }
  });

  test("a cross-runtime link resolves and scopes each endpoint independently", () => {
    const left = legacyMeshSnapshot({
      schemaVersion: 4,
      hostId: "host:cross-runtime",
      runtimeId: "LEFT",
      peerAddress: "tcp/192.168.77.51:7447",
      status: "connected",
      detail: "left ready",
    });
    const right = legacyMeshSnapshot({
      schemaVersion: 4,
      hostId: "host:cross-runtime",
      runtimeId: "RIGHT",
      peerAddress: "tcp/192.168.77.52:7447",
      status: "connected",
      detail: "right ready",
    });
    const rawFrom = left.hosts[0].runtimes[0].endpoints[0].id;
    const rawTo = right.hosts[0].runtimes[0].endpoints[0].id;
    const topology: FleetTopologyResponse = {
      schema_version: 4,
      hosts: [
        {
          ...left.hosts[0],
          runtimes: [left.hosts[0].runtimes[0], right.hosts[0].runtimes[0]],
        },
      ],
      links: [
        {
          id: "link:cross-runtime",
          from: rawFrom,
          to: rawTo,
          protocol: "mesh",
          role: "peer",
          direction: "bidirectional",
          same_host: true,
          status: "connected",
          secure: true,
          detail: "Cross-runtime mesh route ready.",
        },
      ],
      shared: [],
      external: [],
    };

    const normalized = mergeFleetTopologies([topology]);
    const endpointIds = topologyRuntimes(normalized).flatMap((runtime) =>
      runtime.endpoints.map((endpoint) => endpoint.id)
    );
    assert.strictEqual(normalized.links.length, 1);
    assert.ok(endpointIds.includes(normalized.links[0].from));
    assert.ok(endpointIds.includes(normalized.links[0].to));
    assert.notStrictEqual(normalized.links[0].from, rawFrom);
    assert.notStrictEqual(normalized.links[0].to, rawTo);
    assert.notStrictEqual(normalized.links[0].id, "link:cross-runtime");
  });

  test("a singleton legacy snapshot is normalized idempotently through display ingress", () => {
    const snapshot = legacyMeshSnapshot({
      schemaVersion: 4,
      hostId: "host:singleton",
      runtimeId: "RESOURCE",
      peerAddress: "tcp/192.168.77.40:7447",
      status: "degraded",
      detail: "singleton peer degraded",
    });
    const before = cloneTopology(snapshot);
    const once = mergeFleetTopologies([snapshot]);
    const twice = mergeFleetTopologies([once]);
    const inputRuntime = topologyRuntimes(snapshot)[0];
    const outputRuntime = topologyRuntimes(once)[0];

    assert.deepStrictEqual(snapshot, before, "normalization mutated its input");
    assert.deepStrictEqual(twice, once, "normalization must be idempotent");
    assert.notStrictEqual(once, snapshot);
    assert.notStrictEqual(once.hosts[0], snapshot.hosts[0]);
    assert.notStrictEqual(outputRuntime, inputRuntime);
    assert.notStrictEqual(outputRuntime.endpoints[0], inputRuntime.endpoints[0]);
    assert.notStrictEqual(once.links[0], snapshot.links[0]);
    assert.notStrictEqual(once.shared[0], snapshot.shared[0]);
    assert.notStrictEqual(once.shared[0].used_by, snapshot.shared[0].used_by);
    assert.notStrictEqual(once.external[0], snapshot.external[0]);
    assert.notStrictEqual(outputRuntime.runtime_id, "RESOURCE");
    assert.notStrictEqual(outputRuntime.endpoints[0]?.id, "endpoint:RESOURCE:mesh");
    assert.notStrictEqual(once.external[0]?.id, "external:mesh:0");
    assert.strictEqual(once.external[0]?.name, "tcp/192.168.77.40:7447");
    assert.strictEqual(once.links[0]?.detail, "singleton peer degraded");
    assert.strictEqual(once.links[0]?.secure, true);

    const refreshSource = fs.readFileSync(
      path.join(
        __dirname,
        "..",
        "..",
        "..",
        "src",
        "networkCanvas",
        "refreshData.ts"
      ),
      "utf8"
    );
    assert.match(
      refreshSource,
      /mergeFleetTopologies\(\[live\.topology, offline\.topology\]\)/,
      "a lone live topology must pass through the same normalization ingress as merged responses"
    );
  });

  test("repeated snapshots from one complete owner deduplicate", () => {
    const repeated = legacyMeshSnapshot({
      schemaVersion: 4,
      hostId: "host:repeat",
      runtimeId: "RESOURCE",
      peerAddress: "tcp/192.168.77.41:7447",
      status: "connected",
      detail: "peer ready",
    });
    const deduplicated = mergeFleetTopologies([repeated, cloneTopology(repeated)]);
    assert.strictEqual(topologyRuntimes(deduplicated).length, 1);
    assert.strictEqual(deduplicated.external.length, 1);
    assert.strictEqual(deduplicated.links.length, 1);
  });

  test("ambiguous ownership omits only affected references", () => {
    const ambiguous = legacyMeshSnapshot({
      schemaVersion: 4,
      hostId: "host:ambiguous",
      runtimeId: "RESOURCE",
      peerAddress: "tcp/192.168.77.42:7447",
      status: "degraded",
      detail: "ambiguous peer",
    });
    const originalRuntime = ambiguous.hosts[0].runtimes[0];
    const duplicateRuntime: FleetRuntime = {
      ...cloneTopology({
        schema_version: 4,
        hosts: [
          {
            ...ambiguous.hosts[0],
            containers: [],
            runtimes: [originalRuntime],
          },
        ],
        links: [],
        shared: [],
        external: [],
      }).hosts[0].runtimes[0],
      name: "Second RESOURCE",
    };
    const auxEndpointId = "endpoint:AUX:opcua";
    const auxExternalId = "external:opcua:aux";
    const auxRuntime: FleetRuntime = {
      runtime_id: "AUX",
      name: "Auxiliary runtime",
      mode: "online",
      cycle_ms: 10,
      health: "connected",
      detail: "Running.",
      source: "self",
      endpoints: [
        {
          id: auxEndpointId,
          kind: "service",
          protocol: "opcua_client",
          name: "OPC UA client",
          role: "client",
          health: "connected",
          detail: "Auxiliary OPC UA link ready.",
          owned: true,
          supports_test: false,
          source: "self",
        },
      ],
    };
    ambiguous.hosts[0].runtimes = [auxRuntime];
    ambiguous.hosts[0].containers = [
      {
        container_id: "Cell A",
        name: "Cell A",
        image: "runtime:latest",
        status: "running",
        runtimes: [originalRuntime],
        ...{ source: "self" },
      },
      {
        container_id: "Cell B",
        name: "Cell B",
        image: "runtime:latest",
        status: "running",
        runtimes: [duplicateRuntime],
        ...{ source: "self" },
      },
    ];
    ambiguous.links.push({
      id: "link:opcua:aux",
      from: auxEndpointId,
      to: auxExternalId,
      protocol: "opcua_client",
      role: "client",
      direction: "outbound",
      same_host: false,
      status: "connected",
      secure: true,
      detail: "Auxiliary OPC UA peer ready.",
    });
    ambiguous.external.push({
      id: auxExternalId,
      kind: "server",
      name: "opc.tcp://aux.example:4840",
      via_protocol: ["opcua_client"],
      direction: "outbound",
      ...{ source: "config" },
    });
    ambiguous.shared[0].used_by.push("AUX");
    const before = cloneTopology(ambiguous);

    const rejected = mergeFleetTopologies([ambiguous]);
    const runtimes = topologyRuntimes(rejected);
    const aux = runtimes.find((runtime) => runtime.name === "Auxiliary runtime");
    assert.deepStrictEqual(ambiguous, before, "fail-closed normalization mutated input");
    assert.strictEqual(runtimes.length, 3);
    assert.strictEqual(new Set(runtimes.map((runtime) => runtime.runtime_id)).size, 3);
    assert.strictEqual(rejected.links.length, 1, "the uniquely owned AUX link must survive");
    assert.strictEqual(rejected.links[0].protocol, "opcua_client");
    assert.strictEqual(rejected.external.length, 1, "the unrelated AUX external must survive");
    assert.strictEqual(rejected.external[0].name, "opc.tcp://aux.example:4840");
    assert.strictEqual(rejected.shared.length, 1, "the global shared system remains visible");
    assert.ok(aux, "the uniquely owned AUX runtime must survive");
    assert.deepStrictEqual(rejected.shared[0].used_by, [aux.runtime_id]);
  });
});
