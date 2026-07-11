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
import { buildGraph } from "./layout";
import { visibleFaultsForValidationState } from "./faults";
import { nodeTypes } from "./nodes";
import { edgeTypes } from "./CasedEdge";
import { AddDevicePanel } from "./AddDevicePanel";
import { NodeInspector, type InspectorNode } from "./NodeInspector";
import { AddPane } from "./AddPane";
import { AddHostPanel } from "./AddHostPanel";
import { AddRuntimePanel } from "./AddRuntimePanel";
import { SetUpRuntimePanel } from "./SetUpRuntimePanel";
import { DiscoverPane } from "./DiscoverPane";
import { BrowseTagsPanel } from "./BrowseTagsPanel";
import { useBrowseSession } from "./useBrowseSession";
import { useCanvasHostState } from "./useCanvasHostState";
import { NetworkCanvasHeader } from "./NetworkCanvasHeader";
import { NetworkCanvasOverlays } from "./NetworkCanvasOverlays";
import { activeDrawerWidth } from "./networkCanvasStyles";
import {
  useDiscoverActions,
  useDiscoverPaneLifecycle,
} from "./useDiscoverPane";
import type { DeviceDraft } from "./discoverPaneModel";
import { EditModeContext, type AddSlotRequest } from "./editMode";
import { FilterPanel } from "./FilterPanel";
import { applyFilter, filterReport, protocolsInGraph } from "./filter";
import type { CommApplyResponse } from "../../communication/schemaForm";
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

function Canvas() {
  const post = useCallback((message: unknown) => vscode.postMessage(message), []);
  const {
    open: discoverOpen,
    show: openDiscoverPane,
    close: closeDiscoverPane,
    handoffToBrowse: handoffDiscoveryToBrowse,
    toggle: toggleDiscoverPane,
    reset: resetDiscoverPane,
    scanning: discoverScanning,
    progress: discoverProgress,
    results: discoverResults,
    adsServiceProbes,
    error: discoverError,
    errorCode: discoverErrorCode,
    sessionCurrent: discoverSessionCurrent,
    prepareReady: prepareDiscoveryReady,
    handleMessage: handleDiscoveryMessage,
    startScan: onDiscoverScan,
    probeAdsServices: onProbeAdsServices,
  } = useDiscoverPaneLifecycle(post);
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
  const [hidden, setHidden] = useState<ReadonlySet<string>>(new Set());
  const [searchDraft, setSearchDraft] = useState("");
  const [draft, setDraft] = useState<DeviceDraft | undefined>(undefined);
  const [selectedId, setSelectedId] = useState<string | undefined>(undefined);
  const [focusTargetId, setFocusTargetId] = useState<string | undefined>(undefined);
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
  const { fitView, screenToFlowPosition, getIntersectingNodes } = useReactFlow();
  const clearApplyResult = useCallback(() => {
    setApplyResult(undefined);
    setApplyResultLocallyStale(false);
    post({ type: "clearApplyResult" });
  }, [post]);
  const {
    panel: browseTags,
    tree: browseTree,
    routeMissing: browseRouteMissing,
    routePlan: browseRoutePlan,
    error: browseError,
    loading: browseLoading,
    open: openBrowse,
    openNode: onBrowse,
    handleMessage: handleBrowseMessage,
    browseTarget: onBrowseTarget,
    trustCertificate: onTrustCertificate,
    createRoute: onCreateRoute,
    copy: onCopy,
    addTags: onAddTags,
    close: closeBrowse,
  } = useBrowseSession(post, clearApplyResult);
  const onHostFocusNode = useCallback((nodeId: string) => {
    setSelectedId(nodeId);
    setFocusTargetId(nodeId);
  }, []);
  const { graph, schema, reachable, setupMessage } = useCanvasHostState({
    handleDiscoveryMessage,
    handleBrowseMessage,
    prepareDiscoveryReady,
    onFocusNode: onHostFocusNode,
    onApplyResult: setApplyResult,
  });

  const focusNode = useCallback(
    (nodeId: string) => {
      post({ type: "focus", nodeId });
      void fitView({ duration: 500, padding: 0.2, maxZoom: 1.2 });
    },
    [fitView, post]
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
  const toolbarAddTargetLabel = toolbarAddTarget
    ? String((toolbarAddTarget.data as { label?: string } | undefined)?.label ?? "runtime")
    : undefined;
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
  const {
    origins: discoverOrigins,
    protocols: discoverProtocols,
    add: onDiscoverAdd,
    adopt: onDiscoverAdopt,
  } = useDiscoverActions({
    nodes: built.nodes,
    schema,
    post,
    openBrowse,
    clearApplyResult,
    close: closeDiscoverPane,
    handoffToBrowse: handoffDiscoveryToBrowse,
    setSelectedId,
    setDraft,
    setEditMode,
  });
  const openAddPicker = useCallback(() => {
    if (!toolbarAddTarget) {
      return;
    }
    setFilterOpen(false);
    closeDiscoverPane();
    setSelectedId(undefined);
    setDraft(undefined);
    setEditMode(false);
    clearApplyResult();
    setAddSlot({ kind: "device", targetId: toolbarAddTarget.id });
  }, [toolbarAddTarget, clearApplyResult, closeDiscoverPane]);

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
    const timers = [0, 120, 360].map((delay) =>
      setTimeout(() => void fitView({ duration: 220, padding: 0.2, maxZoom: 1.2 }), delay)
    );
    const clearTimer = setTimeout(() => setFocusTargetId(undefined), 420);
    return () => {
      timers.forEach(clearTimeout);
      clearTimeout(clearTimer);
    };
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
      const timers = [0, 140, 420].map((delay) =>
        setTimeout(() => void fitView({ padding: 0.2, duration: 220 }), delay)
      );
      return () => timers.forEach(clearTimeout);
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
        return nodeCountRef.current === 0;
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

  const onSearch = useCallback(
    (query: string) => {
      setSearchDraft(query);
      post({ type: "search", query });
    },
    [post]
  );
  const clearSearch = useCallback(() => {
    setSearchDraft("");
    post({ type: "search", query: "" });
  }, [post]);
  const onCanvasAction = useCallback(
    (action: string) => post({ type: "action", action }),
    [post]
  );
  const onToggleFilter = useCallback(() => {
    setFilterOpen((value) => !value);
    setAddSlot(undefined);
    closeDiscoverPane();
    clearApplyResult();
  }, [clearApplyResult, closeDiscoverPane]);
  const onToggleDiscover = useCallback(() => {
    toggleDiscoverPane();
    setFilterOpen(false);
    setAddSlot(undefined);
    clearApplyResult();
  }, [clearApplyResult, toggleDiscoverPane]);
  const onToggleEdit = useCallback(() => {
    setEditMode((value) => {
      if (value) {
        setAddSlot(undefined);
      }
      return !value;
    });
    setFilterOpen(false);
    clearApplyResult();
  }, [clearApplyResult]);

  const onEditBrowseCredentials = useCallback(() => {
    if (!browseTags) {
      return;
    }
    clearApplyResult();
    closeBrowse();
    setSelectedId(undefined);
    const runtime = selectedOwningRuntime;
    setDraft({
      runtimeId: runtime?.id ?? "",
      runtimeName: String((runtime?.data as { label?: string } | undefined)?.label ?? "runtime"),
      protocol: browseTags.protocol,
      prefillParams: browseTags.target,
    });
  }, [browseTags, clearApplyResult, closeBrowse, selectedOwningRuntime]);

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
        closeDiscoverPane();
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
    [editMode, clearApplyResult, closeDiscoverPane]
  );

  // Every drawer (inspector, add/setup/discover/filter, browse) opens on the RIGHT — opposite the
  // Explorer, beside the top-right toolbar that triggers most of them. A spacer of the drawer's OWN
  // width reserves space in the flex row, so React Flow's viewport genuinely narrows (not a racy calc)
  // and we re-fit into it: the panel sits beside the graph, never over a node, at any webview width
  // (Explorer open or not). Keep these widths in sync with each panel's PANEL width.
  const activeDrawerW = activeDrawerWidth(
    Boolean(draft),
    Boolean(selectedId),
    Boolean(browseTags),
    discoverOpen,
    addSlot?.kind,
    filterOpen
  );
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

  // Discovery results can materially change the right drawer content while the reserved drawer
  // width is unchanged. Re-fit after those result/progress updates so the canvas cannot be left
  // showing only a populated summary over an empty viewport.
  useEffect(() => {
    if (!discoverOpen || nodeCountRef.current === 0) {
      return;
    }
    const id = setTimeout(() => void fitView({ padding: 0.2, duration: 220 }), 120);
    return () => clearTimeout(id);
  }, [discoverOpen, discoverResults.length, discoverProgress.length, discoverScanning, fitView]);

  // Browse/add-tag flows close the right drawer and then refresh the graph from persisted config.
  // Fit after both transitions so an imported endpoint cannot leave the summary populated while
  // the actual nodes are off-screen or hidden behind the previous drawer geometry.
  useEffect(() => {
    if (nodeCountRef.current === 0) {
      return;
    }
    const timers = [140, 460].map((delay) =>
      setTimeout(() => void fitView({ padding: 0.2, duration: 240 }), delay)
    );
    return () => timers.forEach(clearTimeout);
  }, [browseTags, browseTree?.length, browseLoading, applyResultSignature, built.nodes.length, fitView]);

  return (
    <div style={{ position: "absolute", inset: 0, display: "flex", flexDirection: "column" }}>
      <NetworkCanvasHeader
        searchValue={searchDraft}
        onSearchChange={onSearch}
        onClearSearch={clearSearch}
        fieldIssueCount={applyResultLocallyStale ? 0 : applyResult?.field_errors?.length ?? 0}
        fieldIssueMessage={applyResult?.message}
        fault={fault}
        faultCount={visibleFaults.length}
        onFocusFault={focusNode}
        filterActive={filterOpen}
        onToggleFilter={onToggleFilter}
        addActive={addSlot?.kind === "device"}
        addTargetLabel={toolbarAddTargetLabel}
        onAdd={openAddPicker}
        discoverActive={discoverOpen}
        onToggleDiscover={onToggleDiscover}
        editActive={editMode}
        onToggleEdit={onToggleEdit}
      />

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

        <NetworkCanvasOverlays
          empty={nodes.length === 0}
          banner={graph.banner}
          summary={graph.summary}
          onAction={onCanvasAction}
        />
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
              openDiscoverPane();
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
            adsServiceProbes={adsServiceProbes}
            error={discoverError}
            errorCode={discoverErrorCode}
            sessionCurrent={discoverSessionCurrent}
            onScan={onDiscoverScan}
            onProbeAdsServices={onProbeAdsServices}
            onReset={resetDiscoverPane}
            onAdd={onDiscoverAdd}
            onAdopt={onDiscoverAdopt}
            onClose={closeDiscoverPane}
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
            protocol={browseTags.protocol}
            target={browseTags.target}
            tree={browseTree}
            routeMissing={browseRouteMissing}
            routePlan={browseRoutePlan}
            error={browseError}
            loading={browseLoading}
            onCreateRoute={onCreateRoute}
            onTrustCertificate={onTrustCertificate}
            onEditCredentials={onEditBrowseCredentials}
            onCopy={onCopy}
            onBrowseTarget={onBrowseTarget}
            onAddTags={onAddTags}
            onClose={closeBrowse}
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
