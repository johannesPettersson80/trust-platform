import * as fs from "fs";
import * as path from "path";

import type { NCGraph } from "../../networkCanvas/webview/types";
import type { RuntimeLifecycleSnapshot } from "../../runtimeLifecycleModel";

export function extensionRoot(): string {
  return path.resolve(__dirname, "../../..");
}
export function source(relativePath: string): string {
  return fs.readFileSync(
    path.join(extensionRoot(), "src", relativePath),
    "utf8",
  );
}

export function graphFixture(): NCGraph {
  return {
    kind: "graph",
    title: "Devices & Connections",
    summary: "fixture",
    hosts: [
      {
        id: "host:this-computer",
        hostname: "This computer",
        label: "this computer",
        health: "connected",
        containers: [],
        runtimes: [
          {
            id: "runtime:local",
            name: "Simulator",
            mode: "simulate",
            health: "stopped",
            detail: "Stopped.",
            runTarget: false,
            endpoints: [],
          },
          {
            id: "runtime:stale",
            name: "Stale selected remote",
            mode: "remote",
            health: "stopped",
            detail: "Not connected.",
            controlEndpoint: "tcp://stale:9902",
            runTarget: true,
            endpoints: [],
          },
        ],
      },
    ],
    links: [],
    external: [],
    faults: [],
  };
}

export function managedGraph(
  runtimes: ReadonlyArray<{
    readonly name: string;
    readonly endpoint: string;
  }>,
): NCGraph {
  const graph = graphFixture();
  graph.hosts[0].runtimes.push(
    ...runtimes.map((runtime) => ({
      id: `managed:${runtime.name}`,
      name: runtime.name,
      mode: "managed" as const,
      managed: true,
      managedName: runtime.name,
      controlEndpoint: runtime.endpoint,
      health: "stopped",
      detail: "Stopped.",
      endpoints: [],
    })),
  );
  return graph;
}

export function lifecycleSnapshot(
  over: Partial<RuntimeLifecycleSnapshot> = {},
): RuntimeLifecycleSnapshot {
  return {
    status: {
      running: false,
      inlineValuesEnabled: true,
      runtimeMode: "simulate",
      runtimeState: "stopped",
      endpoint: "",
      endpointConfigured: false,
      endpointEnabled: true,
      endpointReachable: false,
    },
    ioState: { inputs: [], outputs: [], memory: [] },
    adsState: { schemaVersion: 1, scan: 0, entries: [] },
    starting: false,
    ...over,
  };
}
