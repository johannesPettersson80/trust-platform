import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  Background,
  BackgroundVariant,
  Controls,
  type Edge,
  type Node,
  ReactFlow,
  ReactFlowProvider,
  useEdgesState,
  useNodesState,
  useReactFlow,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import "../../webview/theme.css";
import { t, tint } from "./theme";
import { buildGraph } from "./layout";
import { visibleFaultsForValidationState } from "./faults";
import { nodeTypes } from "./nodes";
import { edgeTypes } from "./CasedEdge";
import type { NCGraph } from "./types";
import { AddDevicePanel } from "./AddDevicePanel";
import { NodeInspector, type InspectorNode } from "./NodeInspector";
import { AddPane } from "./AddPane";
import { AddHostPanel } from "./AddHostPanel";
import { AddRuntimePanel } from "./AddRuntimePanel";
import { SetUpRuntimePanel } from "./SetUpRuntimePanel";
import {
  DiscoverPane,
  type DiscoverOrigin,
  type DiscoverRequest,
  type DiscoverProgressRow,
} from "./DiscoverPane";
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
import { applyFilter, filterReport, protocolsInGraph } from "./filter";
import type { CommApplyResponse, CommSchemaResponse } from "../../communication/schemaForm";
import { LOCAL_RUNTIME_NODE_ID } from "./types";

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
  const [searchDraft, setSearchDraft] = useState("");
  const [draft, setDraft] = useState<{ runtimeId: string; runtimeName: string; protocol: string; prefillParams?: Record<string, unknown> } | undefined>(undefined);
  const [selectedId, setSelectedId] = useState<string | undefined>(undefined);
  const [focusTargetId, setFocusTargetId] = useState<string | undefined>(undefined);
  const [schema, setSchema] = useState<CommSchemaResponse | undefined>(undefined);
  const [applyResult, setApplyResult] = useState<CommApplyResponse | undefined>(undefined);
  const applyResultSignature = useMemo(
    () =>
      applyResult
        ? JSON.stringify({
            applied: applyResult.applied,
            lifecycle_effect: applyResult.lifecycle_effect,
            message: applyResult.message,
            field_errors: applyResult.field_errors ?? [],
          })
        : "",
    [applyResult]
  );
  const [applyResultLocallyStale, setApplyResultLocallyStale] = useState(false);
  const [reachable, setReachable] = useState(false);
  const [setupMessage, setSetupMessage] = useState<string | undefined>(undefined);
  const { fitView, screenToFlowPosition, getIntersectingNodes } = useReactFlow();
  const clearApplyResult = useCallback(() => {
    setApplyResult(undefined);
    setApplyResultLocallyStale(false);
    vscode.postMessage({ type: "clearApplyResult" });
  }, []);

  const focusNode = useCallback(
    (nodeId: string) => {
      vscode.postMessage({ type: "focus", nodeId });
      void fitView({ duration: 500, padding: 0.2, maxZoom: 1.2 });
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
      clearApplyResult();
      setSelectedId(undefined); // add-flow and read-only inspector are mutually exclusive
      setDraft({
        runtimeId: runtime.id,
        runtimeName: String((runtime.data as { label?: string }).label ?? "runtime"),
        protocol,
      });
    },
    [screenToFlowPosition, getIntersectingNodes, clearApplyResult]
  );

  // §4.2E: right-click a runtime → Add endpoint (opens the inspector setup form).
  const onNodeContextMenu = useCallback(
    (e: React.MouseEvent, node: Node) => {
      if (node.type !== "runtime") {
        return;
      }
      e.preventDefault();
      clearApplyResult();
      setSelectedId(undefined); // add-flow and read-only inspector are mutually exclusive
      setDraft({
        runtimeId: node.id,
        runtimeName: String((node.data as { label?: string }).label ?? "runtime"),
        protocol: schema?.protocols[0]?.id ?? "",
      });
    },
    [schema, clearApplyResult]
  );

  useEffect(() => {
    setApplyResultLocallyStale(false);
  }, [applyResultSignature]);

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
      if (msg && msg.type === "focusNode" && typeof msg.nodeId === "string") {
        setSelectedId(msg.nodeId);
        setFocusTargetId(msg.nodeId);
      }
    };
    window.addEventListener("message", onMessage);
    vscode.postMessage({ type: "ready" });
    return () => window.removeEventListener("message", onMessage);
  }, []);

  // Signature of the current top-level node IDs — drives a re-fit when the graph's STRUCTURE changes
  // (first paint, or a swap like offline→live topology where host/runtime ids change), but not on a plain
  // live-value poll (same ids). A ref the resize observer reads, so it never re-fits to an empty graph.
  const fitSigRef = useRef("");
  const flowWrapRef = useRef<HTMLDivElement>(null);
  const nodeCountRef = useRef(0);
  const positionsRef = useRef<Record<string, { x: number; y: number }>>(
    ((vscode.getState() as { positions?: Record<string, { x: number; y: number }> } | undefined)
      ?.positions) ?? {}
  );

  const protocols = useMemo(() => protocolsInGraph(graph), [graph]);
  const report = useMemo(() => filterReport(graph, hidden), [graph, hidden]);
  const editSlotsVisible =
    editMode && !draft && !selectedId && !browseTags && !discoverOpen && !addSlot && !filterOpen;
  const built = useMemo(
    () =>
      buildGraph(
        applyFilter(graph, hidden),
        draft ? { runtimeId: draft.runtimeId, protocol: draft.protocol } : undefined,
        editSlotsVisible
      ),
    [graph, hidden, draft, editSlotsVisible]
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
  const toolbarAddTarget = useMemo(() => {
    const selectedRuntime = selectedId
      ? built.nodes.find((node) => node.id === selectedId && node.type === "runtime")
      : undefined;
    return (
      selectedRuntime ??
      built.nodes.find((node) => node.id === LOCAL_RUNTIME_NODE_ID && node.type === "runtime") ??
      built.nodes.find((node) => node.type === "runtime")
    );
  }, [built.nodes, selectedId]);
  const selectedOwningRuntime = useMemo(() => {
    const selected = selectedId ? built.nodes.find((node) => node.id === selectedId) : undefined;
    if (!selected) {
      return toolbarAddTarget;
    }
    if (selected.type === "runtime") {
      return selected;
    }
    return (
      (selected.parentId
        ? built.nodes.find((node) => node.id === selected.parentId && node.type === "runtime")
        : undefined) ?? toolbarAddTarget
    );
  }, [built.nodes, selectedId, toolbarAddTarget]);
  const openAddPicker = useCallback(() => {
    if (!toolbarAddTarget) {
      return;
    }
    setFilterOpen(false);
    setDiscoverOpen(false);
    setSelectedId(undefined);
    setDraft(undefined);
    setEditMode(false);
    clearApplyResult();
    setAddSlot({ kind: "device", targetId: toolbarAddTarget.id });
  }, [toolbarAddTarget, clearApplyResult]);

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
  const showAllProtocols = useCallback(() => setHidden(new Set()), []);

  // Merge new graph data over current nodes: keep user-dragged / persisted positions for top-level
  // nodes so live polling never resets the canvas. BUT an Edit-mode toggle reflows the layout (the
  // host grows to hold the slots), so on that transition drop stale auto-positions and keep only
  // explicit user drags — otherwise externals stay at their pre-grow Y and overlap the host.
  const layoutModeRef = useRef(editSlotsVisible);
  useEffect(() => {
    const modeChanged = layoutModeRef.current !== editSlotsVisible;
    layoutModeRef.current = editSlotsVisible;
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
  }, [built, editSlotsVisible, setNodes, setEdges]);

  useEffect(() => {
    if (!focusTargetId || !nodes.some((node) => node.id === focusTargetId)) {
      return;
    }
    void fitView({ duration: 500, padding: 0.2, maxZoom: 1.2 });
    setFocusTargetId(undefined);
  }, [focusTargetId, nodes, fitView]);

  // Fit when the graph node IDENTITY changes — first paint, a structural host swap, or child endpoints
  // appearing/disappearing after Start/Stop. NOT on a plain live-value poll (same ids, positions
  // preserved) — re-fitting on every refresh would yank the viewport. Without this, a managed Start can
  // add endpoint children under an existing host while the viewport stays on the old layout, leaving the
  // graph empty-looking even though the DOM contains the new nodes.
  useEffect(() => {
    nodeCountRef.current = nodes.length;
    const sig = nodes
      .map((n) => n.id)
      .sort()
      .join("|");
    if (sig && sig !== fitSigRef.current) {
      fitSigRef.current = sig;
      if (focusTargetId) {
        return;
      }
      void fitView({ padding: 0.2, duration: 300 });
    }
  }, [nodes, fitView, focusTargetId]);

  // Re-fit when the canvas CONTAINER resizes — chiefly the Debug Console docking after a managed Start
  // (it shrinks the editor area, pushing the graph below the now-shorter viewport), but also window
  // resizes. Debounced so a burst collapses into one fit; only fits once nodes exist so a transient
  // 0-size never fits an empty graph.
  useEffect(() => {
    const el = flowWrapRef.current;
    if (!el || typeof ResizeObserver === "undefined") {
      return;
    }
    let timer: ReturnType<typeof setTimeout> | undefined;
    const ro = new ResizeObserver(() => {
      if (timer) {
        clearTimeout(timer);
      }
      timer = setTimeout(() => {
        if (nodeCountRef.current > 0) {
          void fitView({ padding: 0.2, duration: 200 });
        }
      }, 140);
    });
    ro.observe(el);
    return () => {
      if (timer) {
        clearTimeout(timer);
      }
      ro.disconnect();
    };
  }, [fitView]);

  // VS Code can place Live Values or another editor beside this webview. That changes the webview
  // viewport even when the graph data is unchanged, so re-fit on window-level visibility/resize too.
  useEffect(() => {
    let timer: ReturnType<typeof setTimeout> | undefined;
    const refit = () => {
      if (timer) {
        clearTimeout(timer);
      }
      timer = setTimeout(() => {
        if (nodeCountRef.current > 0) {
          void fitView({ padding: 0.2, duration: 220 });
        }
      }, 120);
    };
    const onVisibility = () => {
      if (!document.hidden) {
        refit();
      }
    };
    window.addEventListener("resize", refit);
    window.addEventListener("focus", refit);
    document.addEventListener("visibilitychange", onVisibility);
    return () => {
      if (timer) {
        clearTimeout(timer);
      }
      window.removeEventListener("resize", refit);
      window.removeEventListener("focus", refit);
      document.removeEventListener("visibilitychange", onVisibility);
    };
  }, [fitView]);

  useEffect(() => {
    const nodesAreVisible = () => {
      const root = flowWrapRef.current;
      if (!root) {
        return true;
      }
      const rootRect = root.getBoundingClientRect();
      const nodeEls = Array.from(root.querySelectorAll<HTMLElement>(".react-flow__node"));
      if (!nodeEls.length) {
        return true;
      }
      return nodeEls.some((nodeEl) => {
        const rect = nodeEl.getBoundingClientRect();
        return (
          rect.right > rootRect.left &&
          rect.left < rootRect.right &&
          rect.bottom > rootRect.top &&
          rect.top < rootRect.bottom
        );
      });
    };
    const id = window.setInterval(() => {
      if (nodeCountRef.current > 0 && !nodesAreVisible()) {
        void fitView({ padding: 0.2, duration: 220 });
      }
    }, 900);
    return () => window.clearInterval(id);
  }, [fitView]);

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

  useEffect(() => {
    setSearchDraft(graph.searchQuery ?? "");
  }, [graph.searchQuery]);

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
    (e: React.ChangeEvent<HTMLInputElement>) => {
      setSearchDraft(e.target.value);
      post({ type: "search", query: e.target.value });
    },
    [post]
  );
  const clearSearch = useCallback(() => {
    setSearchDraft("");
    post({ type: "search", query: "" });
  }, [post]);

  const discoverOrigins = useMemo(() => {
    const runtimes = built.nodes
      .filter((n) => n.type === "runtime")
      .map((n): DiscoverOrigin => {
        const data = n.data as { label?: string; health?: string; attached?: boolean };
        const label = String(data.label ?? n.id);
        const health = String(data.health ?? "");
        const runtimeDiscoveryReady =
          data.attached === true ||
          health === "connected" ||
          health === "running" ||
          health === "online";
        return {
          id: n.id,
          label,
          runtimeDiscoveryReady,
          runtimeDiscoveryDisabledReason: runtimeDiscoveryReady
            ? undefined
            : `Start or connect ${label} before scanning from it.`,
        };
      });
    return [
      {
        id: "this_host",
        label: "This computer",
        runtimeDiscoveryReady: false,
        runtimeDiscoveryDisabledReason: "Choose a running runtime for EtherCAT or GPIO scans.",
      },
      ...runtimes,
    ];
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
      clearApplyResult();
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
    [post, clearApplyResult]
  );
  const onDiscoverAdd = useCallback(
    (c: DiscoverCandidate) => {
      clearApplyResult();
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
    [built.nodes, openBrowse, clearApplyResult]
  );
  const onDiscoverAdopt = useCallback(
    (c: DiscoverCandidate) => {
      const endpoint = typeof c.params.control_endpoint === "string" ? c.params.control_endpoint : "";
      const label =
        typeof c.label === "string" && c.label.trim().length > 0
          ? c.label.trim()
          : typeof c.params.name === "string"
            ? c.params.name.trim()
            : "";
      if (endpoint) {
        post({ type: "addHost", endpoint, label });
      }
      setDiscoverOpen(false);
      setEditMode(false); // success state: show the adopted runtime, not the next edit-mode placeholders
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
        (protocol === "opcua_client" || protocol === "ads") && Array.isArray(connections) && connections.length > 0
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
  const onEditBrowseCredentials = useCallback(() => {
    if (!browseTags) {
      return;
    }
    clearApplyResult();
    setBrowseTags(undefined);
    setSelectedId(undefined);
    const runtime = selectedOwningRuntime;
    setDraft({
      runtimeId: runtime?.id ?? "",
      runtimeName: String((runtime?.data as { label?: string } | undefined)?.label ?? "runtime"),
      protocol: browseTags.protocol,
      prefillParams: browseTags.target,
    });
  }, [browseTags, clearApplyResult, selectedOwningRuntime]);
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
        } else if (browseTags.protocol === "ethercat") {
          const paths = nodes.map((n) => n.path);
          post({ type: "addEthercatChannels", target: browseTags.target, paths });
        } else {
          // ADS tags / expose globals are keyed by their symbol path.
          const type = browseTags.mode === "expose" ? "addExpose" : "addTags";
          const paths = nodes.map((n) => n.path);
          post({ type, protocol: browseTags.protocol, target: browseTags.target, paths, writable });
        }
      }
      setBrowseTags(undefined);
    },
    [post, browseTags, browseTree]
  );

  const visibleFaults = visibleFaultsForValidationState(
    graph.faults,
    applyResultLocallyStale
  );
  const fault = visibleFaults[0];
  const editModeValue = useMemo(
    () => ({
      editMode,
      onPickSlot: (slot: AddSlotRequest) => {
        setFilterOpen(false);
        setDiscoverOpen(false);
        setSelectedId(undefined);
        setDraft(undefined);
        clearApplyResult();
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
    [editMode, clearApplyResult]
  );

  // Every drawer (inspector, add/setup/discover/filter, browse) opens on the RIGHT — opposite the
  // Explorer, beside the top-right toolbar that triggers most of them. A spacer of the drawer's OWN
  // width reserves space in the flex row, so React Flow's viewport genuinely narrows (not a racy calc)
  // and we re-fit into it: the panel sits beside the graph, never over a node, at any webview width
  // (Explorer open or not). Keep these widths in sync with each panel's PANEL width.
  const activeDrawerW = draft
    ? 360 // AddDevicePanel
    : selectedId
      ? 340 // NodeInspector
      : browseTags
        ? 340 // BrowseTagsPanel
        : discoverOpen
          ? 290 // DiscoverPane
          : addSlot?.kind === "setup"
            ? 252 // SetUpRuntimePanel
            : addSlot?.kind === "host"
              ? 300 // AddHostPanel / Connect existing runtime
            : addSlot?.kind === "device"
              ? 360 // AddPane
            : addSlot?.kind === "runtime-scaffold"
              ? 232 // AddRuntimePanel
            : addSlot
              ? 232
              : filterOpen
                ? 184 // FilterPanel
                : 0;
  const drawerOpen = activeDrawerW > 0;
  // Re-fit whenever the reserved width CHANGES — opening (0→W), closing (W→0), or swapping between two
  // drawers of different widths (e.g. the 360px add picker → the 360px config form). Tracking the
  // width (not just open/closed) is what keeps a node from slipping under a drawer that grew.
  const prevWidthRef = useRef(0);
  useEffect(() => {
    if (prevWidthRef.current === activeDrawerW) {
      return;
    }
    prevWidthRef.current = activeDrawerW;
    // The spacer reflows the canvas column to its new width first; once it has, fit the WHOLE graph into
    // the now-visible width so no node — or its parent host — can sit under the drawer. (Zoom = Focus button.)
    const id = setTimeout(() => void fitView({ padding: 0.2, duration: 320 }), 80);
    return () => clearTimeout(id);
  }, [activeDrawerW, fitView]);

  const toolbarBtn = (
    active: boolean,
    variant: "default" | "primary" = "default",
    disabled = false
  ): React.CSSProperties => ({
    border: `1px solid ${active || variant === "primary" ? t.accent : t.border}`,
    background:
      variant === "primary"
        ? t.accent
        : active
          ? tint(t.accent, 0.14)
          : "transparent",
    color: disabled ? t.textSubtle : variant === "primary" ? t.onAccent : t.text,
    borderRadius: t.radius,
    padding: "6px 12px",
    fontSize: 12,
    fontWeight: variant === "primary" ? 650 : 500,
    cursor: disabled ? "not-allowed" : "pointer",
    opacity: disabled ? 0.62 : 1,
    whiteSpace: "nowrap",
    transition: `background ${t.ease}, border-color ${t.ease}`,
  });
  const fieldIssueCount = applyResultLocallyStale ? 0 : applyResult?.field_errors?.length ?? 0;
  const fieldIssueLabel =
    fieldIssueCount > 0
      ? `${fieldIssueCount} field issue${fieldIssueCount === 1 ? "" : "s"} · fix highlighted fields`
      : undefined;
  const fieldIssueTitle =
    fieldIssueCount > 0
      ? applyResult?.message || "Fix the highlighted fields and try again."
      : undefined;
  const issuePillStyle: React.CSSProperties = {
    border: `1px solid ${tint(t.danger, 0.5)}`,
    background: tint(t.danger, 0.12),
    color: t.danger,
    borderRadius: t.radius,
    padding: "6px 10px",
    fontSize: 11,
    fontWeight: 600,
    whiteSpace: "nowrap",
    maxWidth: 360,
    overflow: "hidden",
    textOverflow: "ellipsis",
  };

  return (
    <div style={{ position: "absolute", inset: 0, display: "flex", flexDirection: "column" }}>
      <header
        style={{
          display: "flex",
          alignItems: "center",
          gap: 12,
          padding: "10px 16px",
          borderBottom: `1px solid ${t.border}`,
          background: t.surface,
          zIndex: 5,
        }}
      >
        <div
          aria-label="truST"
          title="truST"
          style={{ fontWeight: 700, fontSize: 14, whiteSpace: "nowrap", color: t.text, letterSpacing: 0.2 }}
        >
          tru<span style={{ color: t.accent }}>ST</span>
        </div>
        <input
          onChange={onSearch}
          value={searchDraft}
          placeholder="Search nodes, links, faults"
          style={{
            flex: "1 1 240px",
            minWidth: 0,
            background: t.inputBg,
            border: `1px solid ${t.inputBorder}`,
            borderRadius: t.radius,
            color: t.text,
            padding: "6px 10px",
            fontSize: 12,
          }}
        />
        {searchDraft.trim().length > 0 && (
          <button
            onClick={clearSearch}
            title="Clear search"
            style={toolbarBtn(false)}
          >
            Clear search
          </button>
        )}
        {fieldIssueLabel ? (
          <span
            style={issuePillStyle}
            title={fieldIssueTitle}
          >
            {fieldIssueLabel}
          </span>
        ) : fault && (
          <button
            onClick={() => focusNode(fault.targetNodeId)}
            style={{
              ...issuePillStyle,
              cursor: "pointer",
            }}
            title={fault.label}
          >
            {visibleFaults.length} issue{visibleFaults.length === 1 ? "" : "s"} · {fault.label}
          </button>
        )}
        <button
          onClick={() => {
            setFilterOpen((v) => !v);
            setAddSlot(undefined);
            setDiscoverOpen(false);
            clearApplyResult();
          }}
          title="Filter connections by protocol"
          style={toolbarBtn(filterOpen)}
        >
          Filter
        </button>
        <button
          onClick={openAddPicker}
          disabled={!toolbarAddTarget}
          title={
            toolbarAddTarget
              ? `Add a device or connection to ${String((toolbarAddTarget.data as { label?: string } | undefined)?.label ?? "runtime")}`
              : "Open or set up a runtime before adding a device or connection"
          }
          style={toolbarBtn(addSlot?.kind === "device", "primary", !toolbarAddTarget)}
        >
          + Add
        </button>
        <button
          onClick={() => {
            setDiscoverOpen((v) => !v);
            setFilterOpen(false);
            setAddSlot(undefined);
            clearApplyResult();
          }}
          title="Find devices on the network"
          style={toolbarBtn(discoverOpen)}
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
            clearApplyResult();
          }}
          title="Edit mode: shows + on each runtime to add a device or service"
          style={toolbarBtn(editMode)}
        >
          {editMode ? "Done" : "Edit"}
        </button>
      </header>

      <div style={{ position: "relative", flex: 1, minHeight: 0, display: "flex", flexDirection: "row" }}>
        <div ref={flowWrapRef} style={{ position: "relative", flex: 1, minWidth: 0 }}>
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
          style={{ width: "100%", height: "100%" }}
          proOptions={{ hideAttribution: true }}
          onNodeClick={(_, node) => {
            clearApplyResult();
            setDraft(undefined); // selection and the add-flow share the right drawer
            setSelectedId(node.id);
            setFocusTargetId(node.id);
            post({ type: "selectNode", nodeId: node.id });
          }}
        >
          <Background variant={BackgroundVariant.Dots} gap={26} size={1} color="var(--trust-grid-line)" />
          <Controls showInteractive={false} />
        </ReactFlow>
        </EditModeContext.Provider>

        {nodes.length === 0 && (
          <div
            className="trust-loading"
            style={{ position: "absolute", inset: 0, display: "flex", flexDirection: "column", alignItems: "center", justifyContent: "center", gap: 12, pointerEvents: "none" }}
          >
            <svg width="38" height="38" viewBox="0 0 24 24" fill="none" stroke={t.textSubtle} strokeWidth={1.4} strokeLinecap="round" strokeLinejoin="round">
              <rect x="3" y="4.5" width="18" height="6" rx="1.5" />
              <rect x="3" y="13.5" width="18" height="6" rx="1.5" />
              <circle cx="6.6" cy="7.5" r="1" fill={t.textSubtle} stroke="none" />
              <circle cx="6.6" cy="16.5" r="1" fill={t.textSubtle} stroke="none" />
            </svg>
            <div style={{ fontSize: 13.5, fontWeight: 600, color: t.textMuted }}>Loading your devices…</div>
            <div style={{ fontSize: 12, color: t.textSubtle, maxWidth: 300, textAlign: "center" }}>Reading the project's runtime and connections.</div>
          </div>
        )}

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
              background: t.overlay,
              border: `1px solid ${graph.banner.kind === "info" ? t.border : tint(t.danger, 0.5)}`,
              borderRadius: t.radiusLg,
              padding: "8px 12px",
              boxShadow: t.shadowOverlay,
              zIndex: 6,
            }}
          >
            <span
              style={{
                color: graph.banner.kind === "info" ? t.text : t.danger,
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
                  border: `1px solid ${t.border}`,
                  background: "transparent",
                  color: t.text,
                  borderRadius: t.radiusSm,
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
          <div className="trust-canvas-summary">
            {graph.summary}
          </div>
        )}
        </div>

        {/* Reserves the active drawer's width in the flex row so the canvas column narrows by exactly
            that much — the right-anchored panel lands in this gap and never covers a node. */}
        {drawerOpen && <div aria-hidden="true" style={{ width: activeDrawerW, flexShrink: 0 }} />}

        {addSlot?.kind === "device" && (
          <AddPane
            schema={schema}
            target={{
              id: addSlot.targetId ?? "",
              name: String((built.nodes.find((n) => n.id === addSlot.targetId)?.data as { label?: string } | undefined)?.label ?? "runtime"),
            }}
            onChoose={(protocol) => {
              const rt = built.nodes.find((n) => n.id === addSlot.targetId);
              clearApplyResult();
              setSelectedId(undefined);
              setDraft({
                runtimeId: addSlot.targetId ?? "",
                runtimeName: String((rt?.data as { label?: string } | undefined)?.label ?? "runtime"),
                protocol,
              });
              setAddSlot(undefined);
            }}
            onDiscover={() => {
              clearApplyResult();
              setAddSlot(undefined);
              setDiscoverOpen(true);
            }}
            onClose={() => {
              clearApplyResult();
              setAddSlot(undefined);
            }}
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
          <AddRuntimePanel post={post} onClose={() => {
            clearApplyResult();
            setAddSlot(undefined);
          }} />
        )}

        {addSlot?.kind === "host" && (
          <AddHostPanel
            post={post}
            onSaved={() => setEditMode(false)}
            onClose={() => {
              clearApplyResult();
              setAddSlot(undefined);
            }}
          />
        )}

        {filterOpen && (
          <FilterPanel
            protocols={protocols}
            hidden={hidden}
            report={report}
            onToggle={toggleHidden}
            onShowAll={showAllProtocols}
          />
        )}

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
            onValidationStale={() => setApplyResultLocallyStale(true)}
            onSaved={(nodeId) => {
              setDraft(undefined);
              if (nodeId) {
                setSelectedId(nodeId);
                setFocusTargetId(nodeId);
                post({ type: "selectNode", nodeId });
              }
            }}
            onClose={() => {
              clearApplyResult();
              setDraft(undefined);
            }}
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
            onClose={() => {
              clearApplyResult();
              setSelectedId(undefined);
            }}
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
            onEditCredentials={onEditBrowseCredentials}
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
