import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  Background,
  BackgroundVariant,
  Controls,
  type Edge,
  MiniMap,
  type Node,
  ReactFlow,
  ReactFlowProvider,
  useEdgesState,
  useNodesState,
  useReactFlow,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import { buildGraph } from "./layout";
import { nodeTypes } from "./nodes";
import { edgeTypes } from "./CasedEdge";
import type { NCGraph } from "./types";
import { AddDevicePanel } from "./AddDevicePanel";
import { NodeInspector, type InspectorNode } from "./NodeInspector";
import { Palette } from "./Palette";
import { FilterPanel } from "./FilterPanel";
import { applyFilter, protocolsInGraph } from "./filter";
import type { CommApplyResponse, CommSchemaResponse } from "../../communication/schemaForm";

interface VsCodeApi {
  postMessage(msg: unknown): void;
  getState(): unknown;
  setState(state: unknown): void;
}
declare function acquireVsCodeApi(): VsCodeApi;

const vscode: VsCodeApi =
  typeof acquireVsCodeApi === "function"
    ? acquireVsCodeApi()
    : { postMessage() {}, getState: () => undefined, setState() {} };

const EMPTY: NCGraph = {
  kind: "graph",
  title: "Network Canvas",
  summary: "",
  hosts: [],
  links: [],
  external: [],
  faults: [],
};

function Canvas() {
  const [graph, setGraph] = useState<NCGraph>(
    () => (typeof window !== "undefined" && (window as { __NC__?: NCGraph }).__NC__) || EMPTY
  );
  const [nodes, setNodes, onNodesChange] = useNodesState<Node>([]);
  const [edges, setEdges, onEdgesChange] = useEdgesState<Edge>([]);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [filterOpen, setFilterOpen] = useState(false);
  const [hidden, setHidden] = useState<ReadonlySet<string>>(new Set());
  const [draft, setDraft] = useState<{ runtimeId: string; runtimeName: string; protocol: string } | undefined>(undefined);
  const [selectedId, setSelectedId] = useState<string | undefined>(undefined);
  const [schema, setSchema] = useState<CommSchemaResponse | undefined>(undefined);
  const [applyResult, setApplyResult] = useState<CommApplyResponse | undefined>(undefined);
  const [reachable, setReachable] = useState(false);
  const [setupMessage, setSetupMessage] = useState<string | undefined>(undefined);
  const { fitView, screenToFlowPosition, getIntersectingNodes } = useReactFlow();

  const focusNode = useCallback(
    (nodeId: string) => {
      vscode.postMessage({ type: "focus", nodeId });
      void fitView({ nodes: [{ id: nodeId }], duration: 500, padding: 0.6, maxZoom: 1.4 });
    },
    [fitView]
  );

  // §6.1: drop a palette item onto a runtime → that runtime owns the new endpoint.
  const onDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.dataTransfer.dropEffect = "copy";
  }, []);

  const onDrop = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault();
      const protocol = e.dataTransfer.getData("application/trust-protocol");
      if (!protocol) {
        return;
      }
      const pos = screenToFlowPosition({ x: e.clientX, y: e.clientY });
      const runtime = getIntersectingNodes({ x: pos.x, y: pos.y, width: 1, height: 1 }).find(
        (n) => n.type === "runtime"
      );
      if (!runtime) {
        return; // only runtimes own endpoints (§6.1 drop target decides ownership)
      }
      setSelectedId(undefined); // add-flow and read-only inspector are mutually exclusive
      setDraft({
        runtimeId: runtime.id,
        runtimeName: String((runtime.data as { label?: string }).label ?? "runtime"),
        protocol,
      });
    },
    [screenToFlowPosition, getIntersectingNodes]
  );

  // §4.2E: right-click a runtime → Add endpoint (opens the inspector setup form).
  const onNodeContextMenu = useCallback(
    (e: React.MouseEvent, node: Node) => {
      if (node.type !== "runtime") {
        return;
      }
      e.preventDefault();
      setSelectedId(undefined); // add-flow and read-only inspector are mutually exclusive
      setDraft({
        runtimeId: node.id,
        runtimeName: String((node.data as { label?: string }).label ?? "runtime"),
        protocol: schema?.protocols[0]?.id ?? "",
      });
    },
    [schema]
  );

  useEffect(() => {
    const onMessage = (event: MessageEvent) => {
      const msg = event.data;
      if (msg && msg.type === "graph" && msg.graph) {
        setGraph(msg.graph as NCGraph);
      }
      if (msg && msg.type === "meta") {
        setSchema(msg.schema as CommSchemaResponse | undefined);
        setApplyResult(msg.applyResult as CommApplyResponse | undefined);
        setReachable(Boolean(msg.reachable));
        setSetupMessage(typeof msg.setupMessage === "string" ? msg.setupMessage : undefined);
      }
    };
    window.addEventListener("message", onMessage);
    vscode.postMessage({ type: "ready" });
    return () => window.removeEventListener("message", onMessage);
  }, []);

  const fittedRef = useRef(false);
  const positionsRef = useRef<Record<string, { x: number; y: number }>>(
    ((vscode.getState() as { positions?: Record<string, { x: number; y: number }> } | undefined)
      ?.positions) ?? {}
  );

  const protocols = useMemo(() => protocolsInGraph(graph), [graph]);
  const built = useMemo(
    () =>
      buildGraph(
        applyFilter(graph, hidden),
        draft ? { runtimeId: draft.runtimeId, protocol: draft.protocol } : undefined
      ),
    [graph, hidden, draft]
  );
  // Resolve the selected node from the freshly-built graph so the inspector reflects
  // live polls and auto-closes if the node is filtered out / disappears (vs a stale snapshot).
  const selectedNode = useMemo<InspectorNode | undefined>(() => {
    if (!selectedId) {
      return undefined;
    }
    const n = built.nodes.find((node) => node.id === selectedId);
    return n ? { id: n.id, type: n.type, data: n.data as Record<string, unknown> } : undefined;
  }, [selectedId, built.nodes]);

  const toggleHidden = useCallback((protocol: string) => {
    setHidden((prev) => {
      const next = new Set(prev);
      if (next.has(protocol)) {
        next.delete(protocol);
      } else {
        next.add(protocol);
      }
      return next;
    });
  }, []);

  // Merge new graph data over current nodes: keep user-dragged / persisted
  // positions for top-level nodes so live polling never resets the canvas.
  useEffect(() => {
    setNodes((prev) => {
      const prevPos = new Map(prev.map((n) => [n.id, n.position]));
      return built.nodes.map((n) => {
        if (n.parentId) {
          return n;
        }
        const pos = prevPos.get(n.id) ?? positionsRef.current[n.id] ?? n.position;
        return { ...n, position: pos };
      });
    });
    setEdges(built.edges);
  }, [built, setNodes, setEdges]);

  // Fit the view once, when nodes first appear (not on every live update).
  useEffect(() => {
    if (!fittedRef.current && nodes.length > 0) {
      fittedRef.current = true;
      void fitView({ padding: 0.2, duration: 300 });
    }
  }, [nodes, fitView]);

  const onNodeDragStop = useCallback((_evt: React.MouseEvent, node: Node) => {
    if (node.parentId) {
      return;
    }
    positionsRef.current = { ...positionsRef.current, [node.id]: node.position };
    const state = (vscode.getState() as Record<string, unknown>) ?? {};
    vscode.setState({ ...state, positions: positionsRef.current });
  }, []);

  const post = useCallback((message: unknown) => vscode.postMessage(message), []);

  const onSearch = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => post({ type: "search", query: e.target.value }),
    [post]
  );

  const fault = graph.faults[0];

  return (
    <div style={{ position: "absolute", inset: 0, display: "flex", flexDirection: "column" }}>
      <header
        style={{
          display: "flex",
          alignItems: "center",
          gap: 12,
          padding: "10px 16px",
          borderBottom: "1px solid #2a2f3a",
          background: "rgba(18,21,27,.85)",
          zIndex: 5,
        }}
      >
        <div style={{ fontWeight: 800, fontSize: 15, whiteSpace: "nowrap" }}>
          tru<span style={{ color: "#5aa9ff" }}>ST</span> · Network Canvas
        </div>
        <input
          onChange={onSearch}
          defaultValue={graph.searchQuery ?? ""}
          placeholder="Search nodes, links, faults"
          style={{
            flex: "1 1 240px",
            minWidth: 0,
            background: "#10141b",
            border: "1px solid #343b47",
            borderRadius: 8,
            color: "#eef1f5",
            padding: "6px 10px",
            fontSize: 12,
          }}
        />
        {fault && (
          <button
            onClick={() => focusNode(fault.targetNodeId)}
            style={{
              border: "1px solid rgba(255,92,84,.45)",
              background: "rgba(255,92,84,.12)",
              color: "#ffcfcb",
              borderRadius: 8,
              padding: "6px 10px",
              fontSize: 11,
              fontWeight: 750,
              cursor: "pointer",
              whiteSpace: "nowrap",
              maxWidth: 360,
              overflow: "hidden",
              textOverflow: "ellipsis",
            }}
            title={fault.label}
          >
            {graph.faults.length} issue{graph.faults.length === 1 ? "" : "s"} · {fault.label}
          </button>
        )}
        <button
          onClick={() => {
            setFilterOpen((v) => !v);
            setPaletteOpen(false);
          }}
          title="Filter connections by protocol"
          style={{
            border: filterOpen ? "1px solid #2f81f7" : "1px solid #343b47",
            background: filterOpen ? "rgba(47,129,247,.16)" : "transparent",
            color: "#eef1f5",
            borderRadius: 8,
            padding: "6px 12px",
            fontSize: 12,
            cursor: "pointer",
            whiteSpace: "nowrap",
          }}
        >
          Filter
        </button>
        <button
          onClick={() => {
            setPaletteOpen((v) => !v);
            setFilterOpen(false);
          }}
          title="Show the device palette, then drag onto a runtime"
          style={{
            border: paletteOpen ? "1px solid #2f81f7" : "1px solid #343b47",
            background: paletteOpen ? "rgba(47,129,247,.16)" : "transparent",
            color: "#eef1f5",
            borderRadius: 8,
            padding: "6px 12px",
            fontSize: 12,
            cursor: "pointer",
            whiteSpace: "nowrap",
          }}
        >
          + Add device
        </button>
      </header>

      <div style={{ position: "relative", flex: 1, minHeight: 0 }}>
        <ReactFlow
          nodes={nodes}
          edges={edges}
          onNodesChange={onNodesChange}
          onEdgesChange={onEdgesChange}
          onNodeDragStop={onNodeDragStop}
          onDrop={onDrop}
          onDragOver={onDragOver}
          onNodeContextMenu={onNodeContextMenu}
          nodeTypes={nodeTypes}
          edgeTypes={edgeTypes}
          minZoom={0.2}
          maxZoom={1.75}
          proOptions={{ hideAttribution: true }}
          onNodeClick={(_, node) => {
            setDraft(undefined); // selection and the add-flow share the right drawer
            setSelectedId(node.id);
            post({ type: "selectNode", nodeId: node.id });
          }}
        >
          <Background variant={BackgroundVariant.Dots} gap={26} size={1} color="#ffffff14" />
          <Controls showInteractive={false} />
          <MiniMap pannable zoomable style={{ background: "#11151c" }} maskColor="rgba(8,10,14,.6)" />
        </ReactFlow>

        {graph.banner && (
          <div
            style={{
              position: "absolute",
              top: 12,
              left: "50%",
              transform: "translateX(-50%)",
              display: "flex",
              alignItems: "center",
              gap: 12,
              background: "rgba(31,20,20,.96)",
              border: "1px solid rgba(255,92,84,.5)",
              borderRadius: 8,
              padding: "8px 12px",
              zIndex: 6,
            }}
          >
            <span style={{ color: "#ffcfcb", fontSize: 12, fontWeight: 600 }}>{graph.banner.text}</span>
            {graph.banner.actions.map((a) => (
              <button
                key={a.action}
                onClick={() => post({ type: "action", action: a.action })}
                style={{
                  border: "1px solid #343b47",
                  background: "transparent",
                  color: "#cfd6e0",
                  borderRadius: 6,
                  padding: "4px 10px",
                  fontSize: 11,
                  cursor: "pointer",
                }}
              >
                {a.label}
              </button>
            ))}
          </div>
        )}

        {graph.summary && (
          <div
            style={{
              position: "absolute",
              left: 16,
              bottom: 14,
              padding: "8px 12px",
              borderRadius: 8,
              border: "1px solid #343b47",
              background: "rgba(15,18,24,.82)",
              color: "#949cab",
              fontSize: 11,
              pointerEvents: "none",
            }}
          >
            {graph.summary}
          </div>
        )}

        {paletteOpen && <Palette schema={schema} reachable={reachable} />}

        {filterOpen && <FilterPanel protocols={protocols} hidden={hidden} onToggle={toggleHidden} />}

        {draft && (
          <AddDevicePanel
            schema={schema}
            applyResult={applyResult}
            reachable={reachable}
            setupMessage={setupMessage}
            target={{ id: draft.runtimeId, name: draft.runtimeName }}
            preselectProtocol={draft.protocol}
            post={post}
            onClose={() => setDraft(undefined)}
          />
        )}

        {selectedNode && !draft && (
          <NodeInspector
            node={selectedNode}
            onFocus={focusNode}
            onClose={() => setSelectedId(undefined)}
          />
        )}
      </div>
    </div>
  );
}

export function NetworkCanvasApp() {
  return (
    <ReactFlowProvider>
      <Canvas />
    </ReactFlowProvider>
  );
}
