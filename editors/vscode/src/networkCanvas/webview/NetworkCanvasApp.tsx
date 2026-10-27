import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  type Edge,
  type Node,
  ReactFlowProvider,
  useEdgesState,
  useNodesState,
  useReactFlow,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import "../../webview/theme.css";
import { buildGraph } from "./layout";
import { visibleFaultsForValidationState } from "./faults";
import type { InspectorNode } from "./NodeInspector";
import { useEquipmentConnections } from "./EquipmentConnections";
import { useBrowseSession } from "./useBrowseSession";
import { useCanvasHostState } from "./useCanvasHostState";
import { useDiscoverActions, useDiscoverPaneLifecycle } from "./useDiscoverPane";
import type { DeviceDraft } from "./discoverPaneModel";
import type { AddSlotRequest } from "./editMode";
import { applyFilter, filterReport, protocolsInGraph } from "./filter";
import type { CommApplyResponse } from "../../communication/schemaForm";
import { LOCAL_RUNTIME_NODE_ID } from "./types";
import { CanvasToolbar } from "./CanvasToolbar";
import { CanvasGraphShell } from "./CanvasGraphShell";
import {
  activeDrawerWidth,
  CanvasDrawerRouter,
  type CanvasDrawerRoutes,
} from "./CanvasDrawerRouter";
import { useCanvasViewport } from "./useCanvasViewport";
import {
  createPlantDraft,
  createPlantEditDraft,
  plantDraftCancelTarget,
  PLANT_DRAG_MIME,
  plantDropPresentation,
  projectPlantDraft,
  resolvePlantDrop,
  type PlantDraft,
} from "../plant/addFlow";
import type { PlantSignalCatalog } from "../plant/signalCatalog";
import { receivePlantEditResult, type PlantEditorTables } from "../plant/editorSession";

interface VsCodeApi {
  postMessage(msg: unknown): void;
  getState(): unknown;
  setState(state: unknown): void;
}

interface PlantSaveConflict {
  currentRevision: string;
  currentPlant: Record<string, unknown> | null;
}
declare function acquireVsCodeApi(): VsCodeApi;

const vscode: VsCodeApi =
  typeof acquireVsCodeApi === "function"
    ? acquireVsCodeApi()
    : { postMessage() {}, getState: () => undefined, setState() {} };

function Canvas() {
  const post = useCallback((message: unknown) => vscode.postMessage(message), []);
  const equipment = useEquipmentConnections(post);
  const {
    open: discoverOpen,
    show: openDiscoverPane,
    close: closeDiscoverPane,
    toggle: toggleDiscoverPane,
    scanning: discoverScanning,
    progress: discoverProgress,
    results: discoverResults,
    error: discoverError,
    prepareReady: prepareDiscoveryReady,
    handleMessage: handleDiscoveryMessage,
    startScan: onDiscoverScan,
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
  const [plantDraft, setPlantDraft] = useState<PlantDraft | undefined>(undefined);
  const [plantSaving, setPlantSaving] = useState(false);
  const [plantSaveError, setPlantSaveError] = useState<string | undefined>(undefined);
  const [plantSaveConflict, setPlantSaveConflict] = useState<PlantSaveConflict | undefined>(undefined);
  const plantDraftOriginRef = useRef<{ selectedId?: string }>({});
  const plantDragSequence = useRef(0);
  const activePlantDragIdRef = useRef<string | undefined>(undefined);
  const [activePlantDragId, setActivePlantDragId] = useState<string | undefined>(undefined);
  const [plantDropTargetId, setPlantDropTargetId] = useState<string | undefined>(undefined);
  const [plantDropMessage, setPlantDropMessage] = useState<string | undefined>(undefined);
  const [selectedId, setSelectedId] = useState<string | undefined>(undefined);
  const [focusTargetId, setFocusTargetId] = useState<string | undefined>(undefined);
  const [keyboardFocusTargetId, setKeyboardFocusTargetId] = useState<string | undefined>(undefined);
  const clearFocusTarget = useCallback(() => setFocusTargetId(undefined), []);
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
  const [plantSignals, setPlantSignals] = useState<PlantSignalCatalog>({
    status: "compile_error",
    commands: [],
    feedbacks: [],
    error: "Compiled PLC signals are not available yet.",
  });
  const [plantEditorTables, setPlantEditorTables] = useState<PlantEditorTables>();
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
    adsImportLoading,
    adsImportResult,
    open: openBrowse,
    openNode: onBrowse,
    handleMessage: handleBrowseMessage,
    browseTarget: onBrowseTarget,
    trustCertificate: onTrustCertificate,
    createRoute: onCreateRoute,
    copy: onCopy,
    addTags: onAddTags,
    addAdsTags: onAddAdsTags,
    removeAdsTag: onRemoveAdsTag,
    close: closeBrowse,
  } = useBrowseSession(post, clearApplyResult);
  const onHostFocusNode = useCallback((nodeId: string) => {
    setSelectedId(nodeId);
    setFocusTargetId(nodeId);
  }, []);
  // The simulated plants' messages (specification 47): the compiled signal catalog rides on
  // `meta`, and a save or removal answers the panel that asked for it.
  const handlePlantMessage = useCallback((message: unknown) => {
    const msg = message as Record<string, unknown> | null | undefined;
    if (!msg) {
      return false;
    }
    if (receivePlantEditResult(msg)) {
      return true;
    }
    if (msg.type === "meta") {
      if (msg.plantSignals) setPlantSignals(msg.plantSignals as PlantSignalCatalog);
      if (msg.plantEditor) setPlantEditorTables(msg.plantEditor as PlantEditorTables);
      return false;
    }
    if (msg.type === "plantSaveResult") {
      setPlantSaving(false);
      if (msg.ok === true && typeof msg.nodeId === "string") {
        setPlantDraft(undefined);
        setPlantSaveError(undefined);
        setPlantSaveConflict(undefined);
        setSelectedId(msg.nodeId);
        setFocusTargetId(msg.nodeId);
        plantDraftOriginRef.current = {};
      } else {
        setPlantSaveError(typeof msg.message === "string" ? msg.message : "The simulated plant was not saved.");
        setPlantSaveConflict(
          msg.code === "revision_conflict" && typeof msg.currentRevision === "string"
            ? {
                currentRevision: msg.currentRevision,
                currentPlant:
                  msg.currentPlant && typeof msg.currentPlant === "object" && !Array.isArray(msg.currentPlant)
                    ? msg.currentPlant as Record<string, unknown>
                    : null,
              }
            : undefined
        );
      }
      return true;
    }
    if (msg.type === "plantRemoveResult") {
      setPlantSaving(false);
      if (msg.ok === true && typeof msg.runtimeId === "string") {
        setPlantDraft(undefined);
        setPlantSaveError(undefined);
        setPlantSaveConflict(undefined);
        setSelectedId(msg.runtimeId);
        setFocusTargetId(msg.runtimeId);
        setKeyboardFocusTargetId(msg.runtimeId);
        plantDraftOriginRef.current = {};
      } else {
        setPlantSaveError(
          typeof msg.message === "string" ? msg.message : "The simulated plant was not removed."
        );
      }
      return true;
    }
    return false;
  }, []);
  const { graph, schema, reachable, setupMessage, discoveredAds } = useCanvasHostState({
    handleDiscoveryMessage,
    handleBrowseMessage,
    handleMessage: handlePlantMessage,
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
  const runtimeAt = useCallback(
    (clientX: number, clientY: number) => {
      const pos = screenToFlowPosition({ x: clientX, y: clientY });
      return getIntersectingNodes({ x: pos.x, y: pos.y, width: 1, height: 1 }).find(
        (node) => node.type === "runtime"
      );
    },
    [screenToFlowPosition, getIntersectingNodes]
  );

  const onDragOver = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault();
      if (e.dataTransfer.types.includes(PLANT_DRAG_MIME)) {
        const runtime = runtimeAt(e.clientX, e.clientY);
        setPlantDropTargetId(runtime?.id);
        setPlantDropMessage(undefined);
        e.dataTransfer.dropEffect =
          runtime &&
          (runtime.data as { plantSimulationSupported?: boolean }).plantSimulationSupported === true
            ? "copy"
            : "none";
        return;
      }
      e.dataTransfer.dropEffect = "copy";
    },
    [runtimeAt]
  );

  const onDrop = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault();
      if (e.dataTransfer.types.includes(PLANT_DRAG_MIME)) {
        const runtime = runtimeAt(e.clientX, e.clientY);
        const result = resolvePlantDrop({
          raw: e.dataTransfer.getData(PLANT_DRAG_MIME),
          activeDragSessionId: activePlantDragIdRef.current ?? "",
          targetRuntimeId: runtime?.id,
          eligibleRuntimeIds:
            runtime &&
            (runtime.data as { plantSimulationSupported?: boolean }).plantSimulationSupported === true
              ? [runtime.id]
              : [],
        });
        activePlantDragIdRef.current = undefined;
        setActivePlantDragId(undefined);
        setPlantDropTargetId(undefined);
        if (!result.ok) {
          setPlantDropMessage(result.message);
          return;
        }
        const target = runtime;
        const next = createPlantDraft({
          id: result.runtimeId,
          name: String((target?.data as { label?: string } | undefined)?.label ?? "runtime"),
        });
        clearApplyResult();
        setDraft(undefined);
        setPlantDraft(next);
        setSelectedId(next.id);
        setFocusTargetId(next.id);
        setAddSlot(undefined);
        setPlantDropMessage(undefined);
        return;
      }
      const protocol = e.dataTransfer.getData("application/trust-protocol");
      if (!protocol) {
        return;
      }
      const runtime = runtimeAt(e.clientX, e.clientY);
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
    [runtimeAt, clearApplyResult]
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

  const protocols = useMemo(() => protocolsInGraph(graph), [graph]);
  const report = useMemo(() => filterReport(graph, hidden), [graph, hidden]);
  const editSlotsVisible =
    editMode && !draft && !plantDraft && !selectedId && !browseTags && !discoverOpen && !addSlot && !filterOpen;
  const built = useMemo(
    () => {
      const filtered = applyFilter(graph, hidden);
      const next = buildGraph(
        plantDraft && !plantDraft.persisted ? projectPlantDraft(filtered, plantDraft) : filtered,
        draft ? { runtimeId: draft.runtimeId, protocol: draft.protocol } : undefined,
        editSlotsVisible
      );
      const withDrop = !activePlantDragId
        ? next
        : {
        ...next,
        nodes: next.nodes.map((node) => {
          if (node.type !== "runtime") {
            return node;
          }
          const presentation = plantDropPresentation({
            runtimeId: node.id,
            eligible:
              (node.data as { plantSimulationSupported?: boolean }).plantSimulationSupported === true,
            activeTargetRuntimeId: plantDropTargetId,
          });
          return {
            ...node,
            data: {
              ...node.data,
              plantDropState: presentation.state,
              plantDropLabel: presentation.label,
            },
          };
        }),
      };
      return {
        ...withDrop,
        nodes: withDrop.nodes.map((node) => ({
          ...node,
          selected: node.id === selectedId,
        })),
      };
    },
    [graph, hidden, draft, plantDraft, editSlotsVisible, activePlantDragId, plantDropTargetId, selectedId]
  );

  useEffect(() => {
    if (!activePlantDragId) {
      return;
    }
    const cancelPlantDrag = (event: KeyboardEvent) => {
      if (event.key !== "Escape") {
        return;
      }
      activePlantDragIdRef.current = undefined;
      setActivePlantDragId(undefined);
      setPlantDropTargetId(undefined);
      setPlantDropMessage(undefined);
    };
    window.addEventListener("keydown", cancelPlantDrag);
    return () => window.removeEventListener("keydown", cancelPlantDrag);
  }, [activePlantDragId]);

  useEffect(() => {
    if (!keyboardFocusTargetId || plantDraft) {
      return;
    }
    const focusTarget = () => {
      const node = Array.from(
        document.querySelectorAll<HTMLElement>(".react-flow__node")
      ).find((candidate) => candidate.getAttribute("data-id") === keyboardFocusTargetId);
      const target = node ?? document.querySelector<HTMLElement>("[data-canvas-add]");
      if (target) {
        target.focus();
        setKeyboardFocusTargetId(undefined);
      }
    };
    // The draft removal changes the React Flow node set. Focus only after that
    // projection has committed so it is not lost with the discarded draft DOM.
    const retry = window.setTimeout(focusTarget, 180);
    return () => {
      window.clearTimeout(retry);
    };
  }, [keyboardFocusTargetId, plantDraft, nodes]);


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
    closeDiscoverPane();
    plantDraftOriginRef.current = { selectedId };
    setSelectedId(undefined);
    setDraft(undefined);
    setEditMode(false);
    clearApplyResult();
    setAddSlot({ kind: "device", targetId: toolbarAddTarget.id });
  }, [toolbarAddTarget, clearApplyResult, selectedId, closeDiscoverPane]);

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

  useEffect(() => {
    setSearchDraft(graph.searchQuery ?? "");
  }, [graph.searchQuery]);

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

  const {
    origins: discoverOrigins,
    protocols: discoverProtocols,
    add: onDiscoverAdd,
    isOnCanvas: isDiscoveredDeviceOnCanvas,
    disabledReason: discoveredDisabledReason,
    adopt: onDiscoverAdopt,
  } = useDiscoverActions({
    nodes: built.nodes,
    discoveredAds,
    schema,
    post,
    openBrowse,
    clearApplyResult,
    close: closeDiscoverPane,
    setSelectedId,
    setDraft,
    setEditMode,
  });
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
        if (slot.add === "device") {
          plantDraftOriginRef.current = { selectedId };
        }
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
    [editMode, clearApplyResult, selectedId, closeDiscoverPane]
  );

  const drawerRoutes: CanvasDrawerRoutes = {};
  if (equipment.visible && equipment.state) {
    drawerRoutes.equipment = {
      state: equipment.state,
      post,
      onClose: equipment.close,
      onBrowse: () => equipment.setVisible(false),
    };
  }
  if (addSlot?.kind === "device") {
    drawerRoutes.addConnection = {
      schema,
      target: {
        id: addSlot.targetId ?? "",
        name: String((built.nodes.find((node) => node.id === addSlot.targetId)?.data as { label?: string } | undefined)?.label ?? "runtime"),
      },
      plantOption: {
        eligible:
          (built.nodes.find((node) => node.id === addSlot.targetId)?.data as
            | { plantSimulationSupported?: boolean }
            | undefined)?.plantSimulationSupported === true,
        disabledReason:
          (built.nodes.find((node) => node.id === addSlot.targetId)?.data as
            | { plantSimulationSupported?: boolean }
            | undefined)?.plantSimulationSupported === true
            ? undefined
            : "This runtime does not advertise simulated-plant support.",
      },
      onBeginPlantDrag: () => {
        const id = `plant-drag-${++plantDragSequence.current}`;
        activePlantDragIdRef.current = id;
        setActivePlantDragId(id);
        setPlantDropTargetId(undefined);
        setPlantDropMessage(undefined);
        return id;
      },
      onEndPlantDrag: () => {
        activePlantDragIdRef.current = undefined;
        setActivePlantDragId(undefined);
        setPlantDropTargetId(undefined);
      },
      onChoosePlant: () => {
        const runtime = built.nodes.find((node) => node.id === addSlot.targetId);
        const next = createPlantDraft({
          id: addSlot.targetId ?? "",
          name: String((runtime?.data as { label?: string } | undefined)?.label ?? "runtime"),
        });
        clearApplyResult();
        setDraft(undefined);
        setPlantDraft(next);
        setSelectedId(next.id);
        setFocusTargetId(next.id);
        setAddSlot(undefined);
      },
      onChoose: (protocol) => {
        const runtime = built.nodes.find((node) => node.id === addSlot.targetId);
        clearApplyResult();
        setSelectedId(undefined);
        setDraft({
          runtimeId: addSlot.targetId ?? "",
          runtimeName: String((runtime?.data as { label?: string } | undefined)?.label ?? "runtime"),
          protocol,
        });
        setAddSlot(undefined);
      },
      onDiscover: () => {
        clearApplyResult();
        setAddSlot(undefined);
        openDiscoverPane();
      },
      onClose: () => {
        clearApplyResult();
        setAddSlot(undefined);
      },
    };
  }
  if (addSlot?.kind === "setup") {
    drawerRoutes.setupRuntime = {
      onConnect: () => setAddSlot({ kind: "host" }),
      onRunLocal: () => setAddSlot({ kind: "runtime-scaffold", targetId: addSlot.targetId }),
      onClose: () => setAddSlot(undefined),
    };
  }
  if (addSlot?.kind === "runtime-scaffold") {
    drawerRoutes.addRuntime = {
      post,
      onClose: () => {
        clearApplyResult();
        setAddSlot(undefined);
      },
    };
  }
  if (addSlot?.kind === "host") {
    drawerRoutes.addHost = {
      post,
      onSaved: () => setEditMode(false),
      onClose: () => {
        clearApplyResult();
        setAddSlot(undefined);
      },
    };
  }
  if (filterOpen) {
    drawerRoutes.filter = { protocols, hidden, report, onToggle: toggleHidden, onShowAll: showAllProtocols };
  }
  if (discoverOpen) {
    drawerRoutes.discover = {
      origins: discoverOrigins,
      discoverProtocols,
      scanning: discoverScanning,
      progress: discoverProgress,
      results: discoverResults,
      error: discoverError,
      onScan: onDiscoverScan,
      onAdd: onDiscoverAdd,
      isOnCanvas: isDiscoveredDeviceOnCanvas,
      disabledReason: discoveredDisabledReason,
      onAdopt: onDiscoverAdopt,
      onOpenAdsPortSettings: () => post({ type: "openAdsDiscoverySettings" }),
      onClose: closeDiscoverPane,
    };
  }
  if (draft) {
    drawerRoutes.addDevice = {
      schema,
      applyResult,
      reachable,
      setupMessage,
      target: { id: draft.runtimeId, name: draft.runtimeName },
      preselectProtocol: draft.protocol,
      preselectParams: draft.prefillParams,
      post,
      onValidationStale: () => setApplyResultLocallyStale(true),
      onSaved: (nodeId) => {
        setDraft(undefined);
        if (nodeId) {
          setSelectedId(nodeId);
          setFocusTargetId(nodeId);
          post({ type: "selectNode", nodeId });
        }
      },
      onClose: () => {
        clearApplyResult();
        setDraft(undefined);
      },
    };
  }
  if (plantDraft) {
    drawerRoutes.plantDraft = {
      draft: plantDraft,
      signalCatalog: plantSignals,
      tables: plantEditorTables,
      post,
      onChange: (next) => {
        setPlantSaveError(undefined);
        setPlantDraft({ ...next, dirty: true });
      },
      // Rust's opened draft: defaults made explicit, not a user change.
      onOpened: (next) => setPlantDraft(next),
      saving: plantSaving,
      saveError: plantSaveError,
      saveConflict: plantSaveConflict,
      onReloadConflict: () => {
        const nodeId = plantDraft.id;
        setPlantDraft(undefined);
        setPlantSaveError(undefined);
        setPlantSaveConflict(undefined);
        setSelectedId(nodeId);
        setFocusTargetId(nodeId);
        setKeyboardFocusTargetId(nodeId);
        post({ type: "selectNode", nodeId });
      },
      onSave: (candidate) => {
        setPlantSaving(true);
        setPlantSaveError(undefined);
        setPlantSaveConflict(undefined);
        post({
          type: "plantSave",
          runtimeId: plantDraft.runtimeId,
          candidate,
          ...(plantDraft.savedConfigurationRevision
            ? { expectedRevision: plantDraft.savedConfigurationRevision }
            : {}),
        });
      },
      onRemove: plantDraft.persisted
        ? () => {
            setPlantSaving(true);
            setPlantSaveError(undefined);
            post({
              type: "plantRemove",
              runtimeId: plantDraft.runtimeId,
              plantId: plantDraft.plantId,
              expectedRevision: plantDraft.savedConfigurationRevision,
            });
          }
        : undefined,
      onOpenControlStudio:
        plantDraft.persisted &&
        plantDraft.projectId &&
        plantDraft.savedConfigurationRevision
          ? () => post({
              type: "openControlStudio",
              projectId: plantDraft.projectId,
              runtimeId: plantDraft.runtimeId,
              plantId: plantDraft.plantId,
              savedConfigurationRevision: plantDraft.savedConfigurationRevision,
              ...(plantDraft.activeConfigurationDigest
                ? { activeConfigurationDigest: plantDraft.activeConfigurationDigest }
                : {}),
            })
          : undefined,
      onCancel: () => {
        const restoreId = plantDraftCancelTarget({
          previousSelectedId: plantDraftOriginRef.current.selectedId,
          runtimeId: plantDraft.runtimeId,
          availableNodeIds: built.nodes
            .filter((node) => node.id !== plantDraft.id)
            .map((node) => node.id),
        });
        setPlantDraft(undefined);
        setSelectedId(restoreId);
        setFocusTargetId(restoreId);
        setKeyboardFocusTargetId(restoreId ?? "__canvas_add__");
        plantDraftOriginRef.current = {};
      },
    };
  }
  if (selectedNode && !draft && !plantDraft) {
    drawerRoutes.inspectNode = {
      node: selectedNode,
      schema,
      params: selectedNode.data.params as Record<string, unknown> | undefined,
      reachable,
      applyResult,
      post,
      onFocus: focusNode,
      onBrowse,
      onClose: () => {
        clearApplyResult();
        setSelectedId(undefined);
      },
    };
  }
  if (browseTags?.protocol === "ads") {
    // An ADS PLC's tags are picked per AMS port (main 3abbbb6c3).
    drawerRoutes.adsBrowse = {
      targetLabel: browseTags.label,
      target: browseTags.target,
      tree: browseTree,
      routeMissing: browseRouteMissing,
      routePlan: browseRoutePlan,
      error: browseError,
      loading: browseLoading,
      importLoading: adsImportLoading,
      importResult: adsImportResult,
      onCreateRoute,
      onCopy,
      onBrowseTarget,
      onAddTags: onAddAdsTags,
      onRemoveTag: onRemoveAdsTag,
      onClose: closeBrowse,
      post,
    };
  } else if (browseTags) {
    drawerRoutes.browseTags = {
      title: browseTags.title,
      actionLabel: browseTags.actionLabel,
      targetLabel: browseTags.label,
      protocol: browseTags.protocol,
      target: browseTags.target,
      tree: browseTree,
      routeMissing: browseRouteMissing,
      routePlan: browseRoutePlan,
      error: browseError,
      loading: browseLoading,
      onCreateRoute,
      onTrustCertificate,
      onEditCredentials: onEditBrowseCredentials,
      onCopy,
      onBrowseTarget,
      onAddTags,
      onClose: closeBrowse,
      post,
    };
  }

  // Reserving the active drawer's own width keeps the graph beside the task
  // surface instead of underneath it, including when routes swap widths.
  const activeDrawerW = activeDrawerWidth(drawerRoutes);
  const drawerOpen = activeDrawerW > 0;
  const { flowWrapRef, onNodeDragStop } = useCanvasViewport({
    nodes,
    nextNodes: built.nodes,
    nextEdges: built.edges,
    setNodes,
    setEdges,
    fitView,
    focusTargetId,
    clearFocusTarget,
    editSlotsVisible,
    editMode,
    activeDrawerWidth: activeDrawerW,
    discoveryOpen: discoverOpen,
    discoveryResultCount: discoverResults.length,
    discoveryProgressCount: discoverProgress.length,
    discoveryScanning: discoverScanning,
    browseOpen: Boolean(browseTags),
    browseTreeSize: browseTree?.length ?? 0,
    browseLoading,
    applyResultSignature,
    builtNodeCount: built.nodes.length,
    stateStore: vscode,
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
  return (
    <div style={{ position: "absolute", inset: 0, display: "flex", flexDirection: "column" }}>
      {equipment.state && !equipment.visible ? (
        <button
          className="trust-button"
          style={{ alignSelf: "flex-start", margin: 6 }}
          onClick={() => equipment.setVisible(true)}
        >
          Connections for {equipment.state.instance}
        </button>
      ) : null}
      <CanvasToolbar
        searchDraft={searchDraft}
        onSearch={onSearch}
        onClearSearch={clearSearch}
        fieldIssueLabel={fieldIssueLabel}
        fieldIssueTitle={fieldIssueTitle}
        fault={fault}
        visibleFaultCount={visibleFaults.length}
        onFocusFault={focusNode}
        filterOpen={filterOpen}
        onToggleFilter={() => {
          setFilterOpen((value) => !value);
          setAddSlot(undefined);
          closeDiscoverPane();
          clearApplyResult();
        }}
        addTargetLabel={
          toolbarAddTarget
            ? String((toolbarAddTarget.data as { label?: string } | undefined)?.label ?? "runtime")
            : undefined
        }
        addOpen={addSlot?.kind === "device"}
        onAdd={() => {
          equipment.setVisible(false);
          openAddPicker();
        }}
        discoverOpen={discoverOpen}
        onToggleDiscover={() => {
          toggleDiscoverPane();
          setFilterOpen(false);
          setAddSlot(undefined);
          clearApplyResult();
        }}
        editMode={editMode}
        onToggleEdit={() => {
          setEditMode((value) => {
            if (value) {
              setAddSlot(undefined);
            }
            return !value;
          });
          setFilterOpen(false);
          clearApplyResult();
        }}
      />

      <div style={{ position: "relative", flex: 1, minHeight: 0, display: "flex", flexDirection: "row" }}>
        <CanvasGraphShell
          flowWrapRef={flowWrapRef}
          nodes={nodes}
          edges={edges}
          onNodesChange={onNodesChange}
          onEdgesChange={onEdgesChange}
          onNodeDragStop={onNodeDragStop}
          onDrop={onDrop}
          onDragOver={onDragOver}
          plantDropMessage={plantDropMessage}
          onNodeContextMenu={onNodeContextMenu}
          editMode={editModeValue}
          graph={graph}
          post={post}
          onSelectNode={(node) => {
            equipment.setVisible(false);
            clearApplyResult();
            setDraft(undefined); // selection and the add-flow share the right drawer
            const data = node.data as Record<string, unknown>;
            const configuration = data.configuration as {
              preservedConfiguration?: Record<string, unknown>;
              description?: string;
              parameters?: Record<string, number | number[] | number[][]>;
              initialState?: number[];
              enabledDefaulted?: boolean;
              initialStateDefaulted?: boolean;
              stepMs?: number;
              initialInput?: number[];
              inputScale?: number[];
              inputOffset?: number[];
              outputScale?: number[];
              outputOffset?: number[];
              actuator?: Record<string, number>;
              uncertainties?: Array<Record<string, unknown>>;
              scenarioEvents?: Array<Record<string, unknown>>;
            } | undefined;
            if (
              data.kind === "plant" &&
              data.draft !== true &&
              node.parentId &&
              typeof data.plantId === "string" &&
              typeof data.displayName === "string" &&
              typeof data.modelKind === "string" &&
              configuration?.parameters &&
              Array.isArray(configuration.initialState)
            ) {
              const runtime = built.nodes.find((candidate) => candidate.id === node.parentId);
              plantDraftOriginRef.current = { selectedId: node.id };
              setPlantDraft(createPlantEditDraft({
                ...configuration,
                parameters: configuration.parameters,
                initialState: configuration.initialState,
                id: node.id,
                projectId: String(data.projectId ?? ""),
                runtimeId: node.parentId,
                runtimeName: String(runtime?.data.label ?? "runtime"),
                plantId: data.plantId,
                displayName: data.displayName,
                description: configuration.description ?? "",
                enabled: data.enabled !== false,
                model: data.modelKind as import("../plant/editorModel").PlantModelKind,
                commandSources: Array.isArray(data.commandSources) ? data.commandSources as string[] : [],
                feedbackTargets: Array.isArray(data.feedbackTargets) ? data.feedbackTargets as string[] : [],
                savedConfigurationRevision: String(data.savedConfigurationRevision ?? ""),
                actuator: configuration.actuator,
                uncertainties: configuration.uncertainties as import("../plant/uncertaintyEditor").PlantUncertaintyDraft[] | undefined,
                scenarioEvents: configuration.scenarioEvents as import("../plant/scenarioEventEditor").PlantScenarioEventDraft[] | undefined,
              }));
            }
            setSelectedId(node.id);
            setFocusTargetId(node.id);
            post({ type: "selectNode", nodeId: node.id });
          }}
        />

        {/* Reserves the active drawer's width in the flex row so the canvas column narrows by exactly
            that much — the right-anchored panel lands in this gap and never covers a node. */}
        {drawerOpen && <div aria-hidden="true" style={{ width: activeDrawerW, flexShrink: 0 }} />}

        <CanvasDrawerRouter routes={drawerRoutes} />
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
