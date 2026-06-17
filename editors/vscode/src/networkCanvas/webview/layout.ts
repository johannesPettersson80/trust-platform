// Deterministic nested layout: Host > [Container] > Runtime > Endpoint.
// Computes sizes bottom-up and emits React Flow nodes (with parentId + relative
// positions) and edges. No overlap by construction; the canvas pans/zooms.
import { MarkerType, type Edge, type Node } from "@xyflow/react";
import { protocolColor, protocolName } from "./nodes";
import type { NCGraph, NCHost, NCLink, NCRuntime } from "./types";

// The peer's role is the mirror of our endpoint's role on that link. `link.role` is
// truST's own role, emitted directly by the runtime (fleet.topology schema_version 3,
// spec §10.1) — the canvas no longer guesses it from the protocol.
function counterpartRole(protocol: string, role: string): string {
  if (protocol === "mqtt") {
    return "broker";
  }
  switch (role) {
    case "server":
      return "client";
    case "client":
      return "server";
    case "master":
      return "slaves";
    case "advertise":
      return "peer";
    default:
      return role || "peer";
  }
}

function externalSub(extId: string, links: readonly NCLink[]): string {
  const link = links.find((l) => l.to === extId || l.from === extId);
  if (!link) {
    return "external system";
  }
  return `${protocolName(link.protocol)} ${counterpartRole(link.protocol, link.role)}`;
}

const EP_W = 84;
const EP_H = 78;
const EP_GAP = 10;
const RT_PAD = 12;
const RT_HEADER = 46;
const MIN_RT_W = 210;
const HOST_PAD = 16;
const HOST_HEADER = 58;
const CT_PAD = 14;
const CT_HEADER = 40;
const STACK_GAP = 18;
const HOST_GAP = 56;
const EXT_W = 196;
const EXT_H = 54;

interface Sized {
  w: number;
  h: number;
}

function sizeRuntime(rt: NCRuntime): Sized {
  const n = Math.max(rt.endpoints.length, 1);
  const innerW = n * EP_W + (n - 1) * EP_GAP;
  return {
    w: Math.max(innerW + 2 * RT_PAD, MIN_RT_W),
    h: RT_HEADER + EP_H + 2 * RT_PAD,
  };
}

function stackHeight(sizes: Sized[], header: number, pad: number): Sized {
  if (sizes.length === 0) {
    return { w: MIN_RT_W + 2 * pad, h: header + pad };
  }
  const w = Math.max(...sizes.map((s) => s.w)) + 2 * pad;
  const h =
    header + sizes.reduce((sum, s) => sum + s.h, 0) + STACK_GAP * (sizes.length - 1) + pad;
  return { w, h };
}

export interface DraftEndpoint {
  runtimeId: string;
  protocol: string;
}

function injectDraft(hosts: readonly NCHost[], draft: DraftEndpoint | undefined): readonly NCHost[] {
  if (!draft) {
    return hosts;
  }
  const add = (rt: NCRuntime): NCRuntime =>
    rt.id === draft.runtimeId
      ? {
          ...rt,
          endpoints: [
            ...rt.endpoints,
            {
              id: `draft:${draft.protocol}`,
              kind: "field",
              protocol: draft.protocol,
              name: "New device",
              role: "draft",
              health: "pending",
              detail: "Draft — configure and Apply",
              dimmed: false,
            },
          ],
        }
      : rt;
  return hosts.map((host) => ({
    ...host,
    runtimes: host.runtimes.map(add),
    containers: host.containers.map((c) => ({ ...c, runtimes: c.runtimes.map(add) })),
  }));
}

export function buildGraph(
  graph: NCGraph,
  draft?: DraftEndpoint
): { nodes: Node[]; edges: Edge[] } {
  const hostsWithDraft = injectDraft(graph.hosts, draft);
  const nodes: Node[] = [];
  const endpointParent = new Map<string, string>();
  const knownIds = new Set<string>();
  const endpointCx = new Map<string, number>(); // absolute centre-x, for bus drops

  // Lay out a runtime group + its endpoint child nodes at (ox, oy) relative to
  // its parent group. parentAbsX = the parent group's absolute x. Returns size.
  function emitRuntime(
    rt: NCRuntime,
    parentId: string,
    ox: number,
    oy: number,
    parentAbsX: number,
    containerTag?: string
  ): Sized {
    const size = sizeRuntime(rt);
    nodes.push({
      id: rt.id,
      type: "runtime",
      parentId,
      position: { x: ox, y: oy },
      data: {
        label: rt.name,
        mode: rt.mode,
        health: rt.health,
        detail: rt.detail,
        endpointCount: rt.endpoints.length,
        container: containerTag,
      },
      style: { width: size.w, height: size.h },
      draggable: false,
      selectable: true,
    });
    knownIds.add(rt.id);
    rt.endpoints.forEach((ep, i) => {
      endpointParent.set(ep.id, rt.id);
      knownIds.add(ep.id);
      const epX = RT_PAD + i * (EP_W + EP_GAP);
      endpointCx.set(ep.id, parentAbsX + ox + epX + EP_W / 2);
      nodes.push({
        id: ep.id,
        type: "endpoint",
        parentId: rt.id,
        extent: "parent",
        position: { x: epX, y: RT_HEADER + RT_PAD },
        data: {
          name: ep.name,
          protocol: ep.protocol,
          kind: ep.kind,
          role: ep.role ?? "",
          detail: ep.detail,
          health: ep.health,
          dimmed: Boolean(ep.dimmed),
        },
        style: { width: EP_W, height: EP_H },
        draggable: false,
      });
    });
    return size;
  }

  // Stack a list of runtimes vertically inside a group starting at headerY.
  function emitRuntimeStack(runtimes: NCRuntime[], parentId: string, headerY: number, pad: number, parentAbsX: number): void {
    let y = headerY;
    for (const rt of runtimes) {
      const s = emitRuntime(rt, parentId, pad, y, parentAbsX);
      y += s.h + STACK_GAP;
    }
  }

  function hostSize(host: NCHost): Sized {
    const sizes: Sized[] = [
      ...host.runtimes.map(sizeRuntime),
      // A single-runtime container collapses into its runtime (just a chip) — §4.1b.
      ...host.containers.map((c) =>
        c.runtimes.length === 1
          ? sizeRuntime(c.runtimes[0])
          : stackHeight(c.runtimes.map(sizeRuntime), CT_HEADER, CT_PAD)
      ),
    ];
    return stackHeight(sizes, HOST_HEADER, HOST_PAD);
  }

  // Place hosts left-to-right.
  let hostX = 0;
  let maxHostH = 0;
  for (const host of hostsWithDraft) {
    const size = hostSize(host);
    const hostRuntimes = [
      ...host.runtimes,
      ...host.containers.flatMap((c) => c.runtimes),
    ];
    const hostEndpointCount = hostRuntimes.reduce((sum, rt) => sum + rt.endpoints.length, 0);
    nodes.push({
      id: host.id,
      type: "host",
      position: { x: hostX, y: 0 },
      data: {
        label: host.hostname,
        sub: host.label,
        health: host.health,
        runtimeCount: hostRuntimes.length,
        endpointCount: hostEndpointCount,
      },
      style: { width: size.w, height: size.h },
      draggable: true,
      selectable: true,
    });
    knownIds.add(host.id);

    let y = HOST_HEADER;
    // Bare runtimes first.
    for (const rt of host.runtimes) {
      const s = emitRuntime(rt, host.id, HOST_PAD, y, hostX);
      y += s.h + STACK_GAP;
    }
    // Containers. §4.1b: ONE runtime per container → collapse into the runtime (a chip,
    // no second box). Only a real multi-runtime container gets a thin grouping frame.
    for (const c of host.containers) {
      if (c.runtimes.length === 1) {
        const s = emitRuntime(c.runtimes[0], host.id, HOST_PAD, y, hostX, c.name);
        y += s.h + STACK_GAP;
        continue;
      }
      const inner = stackHeight(c.runtimes.map(sizeRuntime), CT_HEADER, CT_PAD);
      nodes.push({
        id: c.id,
        type: "container",
        parentId: host.id,
        position: { x: HOST_PAD, y },
        data: { label: c.name, image: c.image, status: c.status },
        style: { width: inner.w, height: inner.h },
        draggable: false,
        selectable: true,
      });
      knownIds.add(c.id);
      emitRuntimeStack(c.runtimes, c.id, CT_HEADER, CT_PAD, hostX + HOST_PAD);
      y += inner.h + STACK_GAP;
    }

    hostX += size.w + HOST_GAP;
    maxHostH = Math.max(maxHostH, size.h);
  }

  // ---- Wiring channel: each wire gets its own lane (height) so the horizontal
  // runs never overlap; mesh peers share one fabric bus. ----
  const meshEndpoints: Array<{ id: string; cx: number }> = [];
  for (const host of hostsWithDraft) {
    const runtimes = [...host.runtimes, ...host.containers.flatMap((c) => c.runtimes)];
    for (const rt of runtimes) {
      for (const ep of rt.endpoints) {
        if (ep.protocol === "mesh" && endpointCx.has(ep.id)) {
          meshEndpoints.push({ id: ep.id, cx: endpointCx.get(ep.id)! });
        }
      }
    }
  }
  // Register external/device/peer ids up front so link filtering sees them.
  for (const ext of graph.external) {
    knownIds.add(ext.id);
  }
  const wireLinks = graph.links.filter(
    (l) => l.protocol !== "mesh" && knownIds.has(l.from) && knownIds.has(l.to)
  );
  // Left-to-right by source keeps adjacent lanes near their wires (fewer crossings).
  wireLinks.sort((a, b) => (endpointCx.get(a.from) ?? 0) - (endpointCx.get(b.from) ?? 0));

  const LANE_GAP = 9;
  const channelTop = maxHostH + 20;
  const laneBottom = channelTop + Math.max(0, wireLinks.length - 1) * LANE_GAP;
  const meshBusY = laneBottom + 16;
  const extY = (meshEndpoints.length > 0 ? meshBusY : laneBottom) + 28;

  // External / device / peer nodes in a row at the bottom.
  let extX = 0;
  for (const ext of graph.external) {
    nodes.push({
      id: ext.id,
      type: "external",
      position: { x: extX, y: extY },
      data: { label: ext.name, sub: externalSub(ext.id, graph.links) },
      style: { width: EXT_W, height: EXT_H },
      draggable: true,
      selectable: true,
    });
    knownIds.add(ext.id);
    extX += EXT_W + 28;
  }

  // One cased wire per link, each on its own lane (centerY).
  const edges: Edge[] = [];
  wireLinks.forEach((link, lane) => {
    const stroke = protocolColor(link.protocol);
    const marker = { type: MarkerType.ArrowClosed, color: stroke, width: 16, height: 16 };
    // truST as server → clients connect INBOUND (arrow points at truST); else outbound.
    const inbound = link.role === "server";
    edges.push({
      id: link.id,
      source: link.from,
      target: link.to,
      type: "cased",
      data: {
        color: stroke,
        dashed: link.status === "error" || link.status === "degraded",
        centerY: channelTop + lane * LANE_GAP,
      },
      markerStart: inbound ? marker : undefined,
      markerEnd: inbound ? undefined : marker,
    });
  });

  // Mesh fabric: every mesh peer drops straight down onto one shared bus-bar (§0.2/§4.4).
  if (meshEndpoints.length > 0) {
    const meshColor = protocolColor("mesh");
    const pad = 36;
    const minX = Math.min(...meshEndpoints.map((e) => e.cx)) - pad;
    const maxX = Math.max(...meshEndpoints.map((e) => e.cx)) + pad;
    const busId = "bus:mesh";
    nodes.push({
      id: busId,
      type: "bus",
      position: { x: minX, y: meshBusY },
      data: {
        label: "Mesh fabric",
        color: meshColor,
        handles: meshEndpoints.map((e) => ({ id: `h-${e.id}`, x: e.cx - minX })),
      },
      style: { width: maxX - minX, height: 8 },
      draggable: false,
      selectable: false,
    });
    for (const e of meshEndpoints) {
      edges.push({
        id: `mesh-${e.id}`,
        source: e.id,
        target: busId,
        targetHandle: `h-${e.id}`,
        type: "cased",
        data: { color: meshColor, dashed: false },
      });
    }
  }

  return { nodes, edges };
}
