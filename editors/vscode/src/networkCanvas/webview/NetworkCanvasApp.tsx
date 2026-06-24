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
import { AddPane } from "./AddPane";
import { AddHostPanel } from "./AddHostPanel";
import { AddRuntimePanel } from "./AddRuntimePanel";
import { SetUpRuntimePanel } from "./SetUpRuntimePanel";
import { DiscoverPane, type DiscoverRequest, type DiscoverProgressRow } from "./DiscoverPane";
import { BrowseTagsPanel } from "./BrowseTagsPanel";
import { browseAction } from "./browseActions";
import type { DiscoverCandidate, RoutePlan, SymbolNode } from "../offlineComm";
import {
  buildOpcuaConnection,
  classifyOpcuaBrowseError,
  selectedLeaves,
  type OpcuaErrorView,
} from "./opcuaClientModel";
import { EditModeContext, type AddSlotRequest } from "./editMode";
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
  title: "Devices & Connections",
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
  const [editMode, setEditMode] = useState(false);
  const [addSlot, setAddSlot] = useState<
    | {
        kind: "device" | "setup" | "runtime-scaffold" | "host";
        targetId?: string;
      }
    | undefined
  >(undefined);
  const [filterOpen, setFilterOpen] = useState(false);
  const [discoverOpen, setDiscoverOpen] = useState(false);
  const [discoverScanning, setDiscoverScanning] = useState(false);
  const [discoverProgress, setDiscoverProgress] = useState<DiscoverProgressRow[]>([]);
  const [discoverResults, setDiscoverResults] = useState<DiscoverCandidate[]>([]);
  const [browseTags, setBrowseTags] = useState<{ label: string; protocol: string; target: Record<string, unknown>; title: string; actionLabel: string; mode: "tags" | "expose" } | undefined>(undefined);
  const [browseTree, setBrowseTree] = useState<SymbolNode[] | undefined>(undefined);
  const [browseRouteMissing, setBrowseRouteMissing] = useState(false);
  const [browseRoutePlan, setBrowseRoutePlan] = useState<RoutePlan | undefined>(undefined);
  // opcua_client: a structured browse failure mapped to one recovery action (esp. cert-trust).
  const [browseError, setBrowseError] = useState<OpcuaErrorView | undefined>(undefined);
  const [browseLoading, setBrowseLoading] = useState(false);
  const [hidden, setHidden] = useState<ReadonlySet<string>>(new Set());
  const [draft, setDraft] = useState<{ runtimeId: string; runtimeName: string; protocol: string; prefillParams?: Record<string, unknown> } | undefined>(undefined);
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
      if (msg && msg.type === "discoverProgress") {
        const status: "scanning" | "done" = msg.status === "done" ? "done" : "scanning";
        const row: DiscoverProgressRow = {
          protocol: String(msg.protocol ?? ""),
          label: String(msg.label ?? ""),
          status,
          count: typeof msg.count === "number" ? msg.count : undefined,
        };
        setDiscoverProgress((prev) => [...prev.filter((p) => p.label !== row.label), row]);
      }
      if (msg && msg.type === "discoverResults") {
        setDiscoverResults(Array.isArray(msg.candidates) ? (msg.candidates as DiscoverCandidate[]) : []);
        setDiscoverScanning(false);
      }
      if (msg && msg.type === "symbolTree") {
        setBrowseTree(Array.isArray(msg.tree) ? (msg.tree as SymbolNode[]) : []);
        setBrowseRouteMissing(Boolean(msg.routeMissing));
        setBrowseRoutePlan(msg.routePlan as RoutePlan | undefined);
        setBrowseError(
          msg.error
            ? classifyOpcuaBrowseError(msg.error as { code?: string; message?: string })
            : undefined
        );
        setBrowseLoading(false);
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
        draft ? { runtimeId: draft.runtimeId, protocol: draft.protocol } : undefined,
        editMode
      ),
    [graph, hidden, draft, editMode]
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

  // Merge new graph data over current nodes: keep user-dragged / persisted positions for top-level
  // nodes so live polling never resets the canvas. BUT an Edit-mode toggle reflows the layout (the
  // host grows to hold the slots), so on that transition drop stale auto-positions and keep only
  // explicit user drags — otherwise externals stay at their pre-grow Y and overlap the host.
  const layoutModeRef = useRef(editMode);
  useEffect(() => {
    const modeChanged = layoutModeRef.current !== editMode;
    layoutModeRef.current = editMode;
    setNodes((prev) => {
      const prevPos = new Map(prev.map((n) => [n.id, n.position]));
      return built.nodes.map((n) => {
        if (n.parentId) {
          return n;
        }
        const pos = modeChanged
          ? positionsRef.current[n.id] ?? n.position
          : prevPos.get(n.id) ?? positionsRef.current[n.id] ?? n.position;
        return { ...n, position: pos };
      });
    });
    setEdges(built.edges);
  }, [built, editMode, setNodes, setEdges]);

  // Fit the view once, when nodes first appear (not on every live update).
  useEffect(() => {
    if (!fittedRef.current && nodes.length > 0) {
      fittedRef.current = true;
      void fitView({ padding: 0.2, duration: 300 });
    }
  }, [nodes, fitView]);

  // Re-fit when Edit toggles — the empty slots widen the canvas, so reveal them.
  const prevEditRef = useRef(editMode);
  useEffect(() => {
    if (prevEditRef.current === editMode) {
      return;
    }
    prevEditRef.current = editMode;
    const t = setTimeout(() => void fitView({ padding: 0.2, duration: 300 }), 70);
    return () => clearTimeout(t);
  }, [editMode, fitView]);

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

  const discoverOrigins = useMemo(() => {
    const runtimes = built.nodes
      .filter((n) => n.type === "runtime")
      .map((n) => ({ id: n.id, label: String((n.data as { label?: string }).label ?? n.id) }));
    return [{ id: "this_host", label: "This computer" }, ...runtimes];
  }, [built.nodes]);

  // Discover-capable protocols come straight from the contract: comm.schema marks a protocol's
  // actions with "discover". The Discover pane offers exactly this set — so EtherCAT/GPIO appear the
  // moment the runtime advertises them, and nothing is ever offered that the backend can't scan.
  const discoverProtocols = useMemo(
    () =>
      new Set(
        (schema?.protocols ?? [])
          .filter((p) => p.actions.includes("discover"))
          .map((p) => p.id)
      ),
    [schema]
  );

  const onDiscoverScan = useCallback(
    (req: DiscoverRequest) => {
      setDiscoverScanning(true);
      setDiscoverProgress([]);
      setDiscoverResults([]);
      post({ type: "discover", request: req });
    },
    [post]
  );
  // Open the Browse-tags/channels flow for a protocol+target (shared by the inspector button and
  // discovery "+Add" — ADS is configured by picking its tags, not via a generic form).
  const openBrowse = useCallback(
    (protocol: string, target: Record<string, unknown>, label: string) => {
      const action = browseAction(protocol);
      if (!action) {
        return;
      }
      const tgt = action.local ? { local: true } : target;
      setBrowseTags({
        label,
        protocol,
        target: tgt,
        title: action.title,
        actionLabel: action.actionLabel,
        mode: action.mode,
      });
      setBrowseTree(undefined);
      setBrowseRouteMissing(false);
      setBrowseRoutePlan(undefined);
      setBrowseError(undefined);
      setBrowseLoading(true);
      post({ type: "browseSymbols", protocol, target: tgt, kind: action.kind });
    },
    [post]
  );
  const onDiscoverAdd = useCallback(
    (c: DiscoverCandidate) => {
      setDiscoverOpen(false);
      setSelectedId(undefined);
      // §0.5: a discovered ADS PLC is set up by picking its tags (browse → add tags), not via the
      // generic device form — route straight to Browse tags with the discovered connection.
      if (browseAction(c.protocol)?.mode === "tags") {
        openBrowse(c.protocol, c.params, c.label || c.protocol);
        return;
      }
      const rt = built.nodes.find((n) => n.type === "runtime");
      setDraft({
        runtimeId: rt?.id ?? "",
        runtimeName: String((rt?.data as { label?: string } | undefined)?.label ?? "runtime"),
        protocol: c.protocol,
        prefillParams: c.params,
      });
    },
    [built.nodes, openBrowse]
  );
  const onDiscoverAdopt = useCallback(
    (c: DiscoverCandidate) => {
      const endpoint = typeof c.params.control_endpoint === "string" ? c.params.control_endpoint : "";
      if (endpoint) {
        post({ type: "addHost", endpoint });
      }
      setDiscoverOpen(false);
    },
    [post]
  );
  const onBrowse = useCallback(
    (node: InspectorNode) => {
      const protocol = String(node.data.protocol ?? "");
      const params = (node.data.params as Record<string, unknown> | undefined) ?? {};
      // The opcua_client endpoint node carries the whole section ({ connections: [...] }), but the
      // remote browse needs ONE connection's endpoint settings (endpoint_url + security) as the
      // target — passing the section verbatim makes the backend reject it ("requires target endpoint
      // settings") and the tree comes back empty. Use the sole connection. (Multi-connection
      // disambiguation via a picker is a follow-up.)
      const connections = params.connections;
      const target =
        protocol === "opcua_client" && Array.isArray(connections) && connections.length > 0
          ? ((connections[0] as Record<string, unknown> | undefined) ?? params)
          : params;
      openBrowse(
        protocol,
        target,
        String(node.data.name ?? node.data.label ?? protocol)
      );
    },
    [openBrowse]
  );
  // opcua_client: explicit cert-trust path — re-browse the same endpoint with trust_server_certificate
  // set, and carry it into the saved connection. Never auto-trusts; only on the user's click.
  const onTrustCertificate = useCallback(() => {
    if (!browseTags) {
      return;
    }
    const target = { ...browseTags.target, trust_server_certificate: true };
    setBrowseTags({ ...browseTags, target });
    setBrowseTree(undefined);
    setBrowseError(undefined);
    setBrowseLoading(true);
    post({ type: "browseSymbols", protocol: browseTags.protocol, target, kind: "nodes" });
  }, [post, browseTags]);
  const onCreateRoute = useCallback(() => {
    if (browseTags) {
      post({ type: "createRoute", protocol: browseTags.protocol, target: browseTags.target });
    }
  }, [post, browseTags]);
  const onCopy = useCallback((text: string) => post({ type: "copyText", text }), [post]);
  const onAddTags = useCallback(
    (keys: string[], writable: boolean) => {
      if (browseTags && keys.length > 0) {
        // Resolve the stable selection keys back to leaves by node identity (not display path).
        const nodes = selectedLeaves(browseTree, new Set(keys));
        if (browseTags.protocol === "opcua_client") {
          // Map the selected leaves → a connection with points (var/node_id/type/access),
          // carrying the endpoint + chosen security/auth/trust from the browse target.
          const connection = buildOpcuaConnection(
            browseTags.target,
            browseTags.label,
            nodes,
            writable
          );
          if (connection) {
            post({ type: "addOpcuaConnection", connection });
          }
        } else {
          // ADS tags / EtherCAT channels / expose globals are keyed by their symbol path.
          const type = browseTags.mode === "expose" ? "addExpose" : "addTags";
          const paths = nodes.map((n) => n.path);
          post({ type, protocol: browseTags.protocol, target: browseTags.target, paths, writable });
        }
      }
      setBrowseTags(undefined);
    },
    [post, browseTags, browseTree]
  );

  const fault = graph.faults[0];
  const editModeValue = useMemo(
    () => ({
      editMode,
      onPickSlot: (slot: AddSlotRequest) => {
        setFilterOpen(false);
        setDiscoverOpen(false);
        setSelectedId(undefined);
        setDraft(undefined);
        if (slot.add === "device") {
          setAddSlot({ kind: "device", targetId: slot.targetId });
        } else if (slot.add === "runtime") {
          // The host runtime slot opens the gated "Set up runtime…" chooser (§0.6.0), not a raw add.
          setAddSlot({ kind: "setup", targetId: slot.targetId });
        } else {
          setAddSlot({ kind: "host" });
        }
      },
    }),
    [editMode]
  );

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
          tru<span style={{ color: "#5aa9ff" }}>ST</span> · Devices &amp; Connections
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
            setAddSlot(undefined);
            setDiscoverOpen(false);
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
            setDiscoverOpen((v) => !v);
            setFilterOpen(false);
            setAddSlot(undefined);
          }}
          title="Find devices on the network"
          style={{
            border: discoverOpen ? "1px solid #2f81f7" : "1px solid #343b47",
            background: discoverOpen ? "rgba(47,129,247,.16)" : "transparent",
            color: "#eef1f5",
            borderRadius: 8,
            padding: "6px 12px",
            fontSize: 12,
            cursor: "pointer",
            whiteSpace: "nowrap",
          }}
        >
          Discover
        </button>
        <button
          onClick={() => {
            setEditMode((v) => {
              if (v) {
                setAddSlot(undefined);
              }
              return !v;
            });
            setFilterOpen(false);
          }}
          title="Edit mode: shows + on each runtime to add a device or service"
          style={{
            border: editMode ? "1px solid #2f81f7" : "1px solid #343b47",
            background: editMode ? "rgba(47,129,247,.16)" : "transparent",
            color: "#eef1f5",
            borderRadius: 8,
            padding: "6px 12px",
            fontSize: 12,
            cursor: "pointer",
            whiteSpace: "nowrap",
          }}
        >
          {editMode ? "Done" : "Edit"}
        </button>
      </header>

      <div style={{ position: "relative", flex: 1, minHeight: 0 }}>
        <EditModeContext.Provider value={editModeValue}>
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
        </EditModeContext.Provider>

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
              background: graph.banner.kind === "info" ? "rgba(18,21,28,.96)" : "rgba(31,20,20,.96)",
              border:
                graph.banner.kind === "info"
                  ? "1px solid #343b47"
                  : "1px solid rgba(255,92,84,.5)",
              borderRadius: 8,
              padding: "8px 12px",
              zIndex: 6,
            }}
          >
            <span
              style={{
                color: graph.banner.kind === "info" ? "#cfd6e0" : "#ffcfcb",
                fontSize: 12,
                fontWeight: 600,
              }}
            >
              {graph.banner.text}
            </span>
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

        {addSlot?.kind === "device" && (
          <AddPane
            schema={schema}
            target={{
              id: addSlot.targetId ?? "",
              name: String((built.nodes.find((n) => n.id === addSlot.targetId)?.data as { label?: string } | undefined)?.label ?? "runtime"),
            }}
            onChoose={(protocol) => {
              const rt = built.nodes.find((n) => n.id === addSlot.targetId);
              setSelectedId(undefined);
              setDraft({
                runtimeId: addSlot.targetId ?? "",
                runtimeName: String((rt?.data as { label?: string } | undefined)?.label ?? "runtime"),
                protocol,
              });
              setAddSlot(undefined);
            }}
            onClose={() => setAddSlot(undefined)}
          />
        )}

        {addSlot?.kind === "setup" && (
          <SetUpRuntimePanel
            onConnect={() => setAddSlot({ kind: "host" })}
            onRunLocal={() =>
              setAddSlot({ kind: "runtime-scaffold", targetId: addSlot.targetId })
            }
            onClose={() => setAddSlot(undefined)}
          />
        )}

        {addSlot?.kind === "runtime-scaffold" && (
          <AddRuntimePanel post={post} onClose={() => setAddSlot(undefined)} />
        )}

        {addSlot?.kind === "host" && (
          <AddHostPanel post={post} onClose={() => setAddSlot(undefined)} />
        )}

        {filterOpen && <FilterPanel protocols={protocols} hidden={hidden} onToggle={toggleHidden} />}

        {discoverOpen && (
          <DiscoverPane
            origins={discoverOrigins}
            discoverProtocols={discoverProtocols}
            scanning={discoverScanning}
            progress={discoverProgress}
            results={discoverResults}
            onScan={onDiscoverScan}
            onAdd={onDiscoverAdd}
            onAdopt={onDiscoverAdopt}
            onClose={() => setDiscoverOpen(false)}
          />
        )}

        {draft && (
          <AddDevicePanel
            schema={schema}
            applyResult={applyResult}
            reachable={reachable}
            setupMessage={setupMessage}
            target={{ id: draft.runtimeId, name: draft.runtimeName }}
            preselectProtocol={draft.protocol}
            preselectParams={draft.prefillParams}
            post={post}
            onClose={() => setDraft(undefined)}
          />
        )}

        {selectedNode && !draft && (
          <NodeInspector
            node={selectedNode}
            schema={schema}
            params={selectedNode.data.params as Record<string, unknown> | undefined}
            reachable={reachable}
            applyResult={applyResult}
            post={post}
            onFocus={focusNode}
            onBrowse={onBrowse}
            onClose={() => setSelectedId(undefined)}
          />
        )}

        {browseTags && (
          <BrowseTagsPanel
            title={browseTags.title}
            actionLabel={browseTags.actionLabel}
            targetLabel={browseTags.label}
            tree={browseTree}
            routeMissing={browseRouteMissing}
            routePlan={browseRoutePlan}
            error={browseError}
            loading={browseLoading}
            onCreateRoute={onCreateRoute}
            onTrustCertificate={onTrustCertificate}
            onCopy={onCopy}
            onAddTags={onAddTags}
            onClose={() => setBrowseTags(undefined)}
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
