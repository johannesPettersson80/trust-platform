import React, { useCallback, useEffect, useState } from "react";
import {
  Background,
  BackgroundVariant,
  Controls,
  MarkerType,
  ReactFlow,
  type ReactFlowInstance,
  type XYPosition,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import "./sfcEditor.css";
import { ParallelNode } from "./ParallelNode";
import { PropertiesPanel } from "./PropertiesPanel";
import { SfcCodePanel } from "./SfcCodePanel";
import { StepNode } from "./StepNode";
import { SfcDragItemType, SfcToolsPanel } from "./SfcToolsPanel";
import { TransitionEdge } from "./TransitionEdge";
import { useSfc } from "./hooks/useSfc";
import {
  SfcExtensionToWebviewMessage,
  SfcNode,
  SfcTransitionEdge,
  SfcWebviewToExtensionMessage,
} from "./types";
import { getVsCodeApi } from "../../visual/runtime/webview/vscodeApi";
import { useRightPaneResize } from "../../visual/runtime/webview/useRightPaneResize";
import "../../visual/runtime/webview/rightPaneResize.css";
import { t } from "../../webview/theme";

const vscode = getVsCodeApi();

const nodeTypes = {
  step: StepNode,
  parallelSplit: ParallelNode,
  parallelJoin: ParallelNode,
} as const;
const edgeTypes = {
  transition: TransitionEdge,
} as const;
const DRAG_MIME_TYPE = "application/x-trust-sfc-node";
const SFC_FIT_VIEW_OPTIONS = {
  padding: 0.2,
  minZoom: 0.5,
  maxZoom: 1,
} as const;

/**
 * Main SFC Editor Component
 */
export const SfcEditor: React.FC = () => {
  const {
    nodes,
    edges,
    variables,
    onNodesChange,
    onEdgesChange,
    onConnect,
    addNewStep,
    addParallelSplit,
    addParallelJoin,
    updateStepNodeData,
    updateParallelNodeData,
    updateEdgeData,
    addActionToStep,
    updateAction,
    deleteAction,
    deleteSelected,
    autoLayout,
    importFromJson,
    exportToJson,
    updateVariables,
    highlightActiveSteps,
    updateDebugState,
  } = useSfc();

  const [selectedNodeIds, setSelectedNodeIds] = useState<string[]>([]);
  const [selectedEdgeIds, setSelectedEdgeIds] = useState<string[]>([]);
  const [showCodePanel, setShowCodePanel] = useState(false);
  const [generatedCode, setGeneratedCode] = useState<string | null>(null);
  const [codeErrors, setCodeErrors] = useState<string[]>([]);
  const [isGeneratingCode, setIsGeneratingCode] = useState(false);
  const [fitViewRequest, setFitViewRequest] = useState(0);
  const [reactFlowInstance, setReactFlowInstance] = useState<
    ReactFlowInstance<SfcNode, SfcTransitionEdge> | null
  >(null);

  const {
    rightPaneStyle,
    resizeHandleClassName,
    resizeHandleProps,
  } = useRightPaneResize("sfc", { minWidth: 320, defaultWidth: 380 });

  const handleToggleBreakpoint = useCallback((stepId: string) => {
    vscode.postMessage({
      type: "toggleBreakpoint",
      stepId,
    } as SfcWebviewToExtensionMessage);
  }, []);

  const requestFitView = useCallback(() => {
    setFitViewRequest((value) => value + 1);
  }, []);

  useEffect(() => {
    if (!reactFlowInstance || fitViewRequest === 0) {
      return;
    }

    const timeout = window.setTimeout(() => {
      reactFlowInstance.fitView({
        ...SFC_FIT_VIEW_OPTIONS,
        duration: 150,
      });
    }, 80);

    return () => window.clearTimeout(timeout);
  }, [edges.length, fitViewRequest, nodes.length, reactFlowInstance]);

  // Handle messages from extension
  useEffect(() => {
    const handleMessage = (event: MessageEvent<SfcExtensionToWebviewMessage>) => {
      const message = event.data;

      switch (message.type) {
        case "init":
        case "update":
          try {
            if (message.content) {
              const workspace = JSON.parse(message.content);
              importFromJson(workspace);
              requestFitView();
            }
          } catch (error) {
            console.error("Failed to parse SFC workspace:", error);
            vscode.postMessage({
              type: "error",
              error: "Could not open this SFC because the file is not valid JSON.",
            } as SfcWebviewToExtensionMessage);
          }
          break;

        case "executionState":
          highlightActiveSteps(message.state.activeSteps || []);
          if (message.state.breakpoints !== undefined) {
            updateDebugState(
              message.state.breakpoints,
              message.state.currentStep || null,
              handleToggleBreakpoint
            );
          }
          break;

        case "executionStopped":
          highlightActiveSteps([]);
          break;

        case "runtime.state":
          if (!message.state.isExecuting) {
            highlightActiveSteps([]);
          }
          break;

        case "runtime.error":
          console.error("SFC runtime error:", message.message);
          break;

        case "validationResult":
          console.log("Validation result:", message.errors);
          if (message.errors.length === 0) {
            console.log("SFC validation passed");
          }
          break;

        case "codeGenerated":
          setIsGeneratingCode(false);
          setGeneratedCode(message.code ?? null);
          setCodeErrors(message.errors || []);
          setShowCodePanel(true);
          break;
      }
    };

    window.addEventListener("message", handleMessage);
    vscode.postMessage({ type: "ready" } as SfcWebviewToExtensionMessage);

    return () => window.removeEventListener("message", handleMessage);
  }, [
    handleToggleBreakpoint,
    highlightActiveSteps,
    importFromJson,
    requestFitView,
    updateDebugState,
  ]);

  const handleSave = useCallback(() => {
    const workspace = exportToJson();
    const content = JSON.stringify(workspace, null, 2);
    vscode.postMessage({
      type: "save",
      content,
    } as SfcWebviewToExtensionMessage);
  }, [exportToJson]);

  const clearSelection = useCallback(() => {
    setSelectedNodeIds([]);
    setSelectedEdgeIds([]);
  }, []);

  const handleSelectionChange = useCallback(
    ({
      nodes: currentNodes,
      edges: currentEdges,
    }: {
      nodes: SfcNode[];
      edges: SfcTransitionEdge[];
    }) => {
      setSelectedNodeIds(currentNodes.map((node) => node.id));
      setSelectedEdgeIds(currentEdges.map((edge) => edge.id));
    },
    []
  );

  const addNodeAtPosition = useCallback(
    (itemType: SfcDragItemType, position?: XYPosition) => {
      switch (itemType) {
        case "parallelSplit":
          addParallelSplit(position);
          break;
        case "parallelJoin":
          addParallelJoin(position);
          break;
        case "step":
        default:
          addNewStep("normal", position);
          break;
      }
    },
    [addNewStep, addParallelJoin, addParallelSplit]
  );

  const handleToolDragStart = useCallback(
    (event: React.DragEvent<HTMLButtonElement>, itemType: SfcDragItemType) => {
      event.dataTransfer.setData(DRAG_MIME_TYPE, itemType);
      event.dataTransfer.effectAllowed = "move";
    },
    []
  );

  const handleCanvasDragOver = useCallback((event: React.DragEvent) => {
    event.preventDefault();
    event.dataTransfer.dropEffect = "move";
  }, []);

  const handleCanvasDrop = useCallback(
    (event: React.DragEvent) => {
      event.preventDefault();
      const itemType = event.dataTransfer.getData(DRAG_MIME_TYPE) as SfcDragItemType;
      if (!reactFlowInstance) {
        return;
      }
      if (
        itemType !== "step" &&
        itemType !== "parallelSplit" &&
        itemType !== "parallelJoin"
      ) {
        return;
      }

      const position = reactFlowInstance.screenToFlowPosition({
        x: event.clientX,
        y: event.clientY,
      });
      addNodeAtPosition(itemType, position);
    },
    [addNodeAtPosition, reactFlowInstance]
  );

  const handleAddStep = useCallback(() => {
    addNodeAtPosition("step");
    requestFitView();
  }, [addNodeAtPosition, requestFitView]);

  const handleAddParallelSplit = useCallback(() => {
    addNodeAtPosition("parallelSplit");
    requestFitView();
  }, [addNodeAtPosition, requestFitView]);

  const handleAddParallelJoin = useCallback(() => {
    addNodeAtPosition("parallelJoin");
    requestFitView();
  }, [addNodeAtPosition, requestFitView]);

  const handleDelete = useCallback(() => {
    deleteSelected({
      nodeIds: selectedNodeIds,
      edgeIds: selectedEdgeIds,
    });
    clearSelection();
  }, [clearSelection, deleteSelected, selectedEdgeIds, selectedNodeIds]);

  const handleValidate = useCallback(() => {
    vscode.postMessage({
      type: "validate",
    } as SfcWebviewToExtensionMessage);
  }, []);

  const handleGenerateST = useCallback(() => {
    const workspace = exportToJson();
    const content = JSON.stringify(workspace, null, 2);
    setIsGeneratingCode(true);
    setCodeErrors([]);
    vscode.postMessage({
      type: "generateST",
      content,
    } as SfcWebviewToExtensionMessage);
  }, [exportToJson]);

  const handleAutoLayout = useCallback(() => {
    autoLayout();
    requestFitView();
  }, [autoLayout, requestFitView]);

  const handleToggleCodePanel = useCallback(() => {
    setShowCodePanel((prev) => {
      const next = !prev;
      if (next && !generatedCode && !isGeneratingCode) {
        handleGenerateST();
      }
      return next;
    });
  }, [generatedCode, handleGenerateST, isGeneratingCode]);

  const handleCopyCode = useCallback(() => {
    if (generatedCode) {
      navigator.clipboard.writeText(generatedCode);
    }
  }, [generatedCode]);

  const handleCloseProperties = useCallback(() => {
    clearSelection();
  }, [clearSelection]);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement | null;
      const tag = target?.tagName?.toLowerCase();
      if (target?.isContentEditable || tag === "input" || tag === "textarea") {
        return;
      }

      if (
        (event.key === "Delete" || event.key === "Backspace") &&
        (selectedNodeIds.length > 0 || selectedEdgeIds.length > 0)
      ) {
        event.preventDefault();
        deleteSelected({
          nodeIds: selectedNodeIds,
          edgeIds: selectedEdgeIds,
        });
        clearSelection();
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [clearSelection, deleteSelected, selectedEdgeIds, selectedNodeIds]);

  const selectedNode =
    selectedNodeIds.length === 1
      ? nodes.find((node) => node.id === selectedNodeIds[0]) || null
      : null;
  const selectedEdge =
    selectedEdgeIds.length === 1
      ? edges.find((edge) => edge.id === selectedEdgeIds[0]) || null
      : null;

  const hasSelection = selectedNodeIds.length > 0 || selectedEdgeIds.length > 0;

  return (
    <div className="sfc-editor trust-product-shell">
      <header className="trust-product-header" aria-label="SFC editor header">
        <div className="trust-product-brand">
          tru<span className="trust-product-brand__accent">ST</span>
          <span className="trust-product-brand__separator">·</span>
          <span className="trust-product-brand__surface">SFC editor</span>
        </div>
        <div className="trust-product-header__meta">Sequential function chart</div>
      </header>

      <div className="trust-product-workspace">
      <div className="trust-canvas-pane">
        <ReactFlow<SfcNode, SfcTransitionEdge>
          nodes={nodes}
          edges={edges}
          onNodesChange={onNodesChange}
          onEdgesChange={onEdgesChange}
          onConnect={onConnect}
          onPaneClick={clearSelection}
          onSelectionChange={handleSelectionChange}
          onInit={setReactFlowInstance}
          onDragOver={handleCanvasDragOver}
          onDrop={handleCanvasDrop}
          nodeTypes={nodeTypes}
          edgeTypes={edgeTypes}
          fitView
          fitViewOptions={SFC_FIT_VIEW_OPTIONS}
          snapToGrid
          snapGrid={[15, 15]}
          defaultEdgeOptions={{
            type: "transition",
            animated: true,
            markerEnd: {
              type: MarkerType.ArrowClosed,
              width: 20,
              height: 20,
            },
            style: {
              stroke: "var(--vscode-editorWidget-border)",
              strokeWidth: 2,
            },
            labelStyle: {
              fill: "var(--trust-text)",
              fontSize: "11px",
              fontWeight: 600,
            },
            labelBgPadding: [9, 4],
            labelBgBorderRadius: 6,
            labelBgStyle: {
              fill: "var(--trust-surface)",
              fillOpacity: 0.92,
              stroke: "var(--trust-border)",
              strokeWidth: 1,
            },
          }}
          style={{
            background: t.canvas,
          }}
        >
          <Background
            variant={BackgroundVariant.Dots}
            gap={20}
            size={1}
            color="var(--trust-grid-line)"
          />
          <Controls />
        </ReactFlow>

        {showCodePanel && (
          <SfcCodePanel
            code={generatedCode}
            errors={codeErrors}
            isGenerating={isGeneratingCode}
            onCopy={handleCopyCode}
          />
        )}
      </div>

      <div className={resizeHandleClassName} {...resizeHandleProps} />

      <div
        style={{
          ...rightPaneStyle,
        }}
        className="trust-inspector right-pane-resizable"
      >
        <div className="trust-inspector__header">
          <span className="trust-inspector__title">SFC editor</span>
        </div>

        <SfcToolsPanel
          onAddStep={handleAddStep}
          onAddParallelSplit={handleAddParallelSplit}
          onAddParallelJoin={handleAddParallelJoin}
          onToolDragStart={handleToolDragStart}
          onDelete={handleDelete}
          onValidate={handleValidate}
          onGenerateST={handleGenerateST}
          onAutoLayout={handleAutoLayout}
          onSave={handleSave}
          onToggleCodePanel={handleToggleCodePanel}
          showCodePanel={showCodePanel}
          hasSelection={hasSelection}
        />
        {(selectedNode || selectedEdge) && (
          <PropertiesPanel
            selectedNode={selectedNode}
            selectedEdge={selectedEdge}
            variables={variables}
            onUpdateStepNode={updateStepNodeData}
            onUpdateParallelNode={updateParallelNodeData}
            onUpdateEdge={updateEdgeData}
            onAddAction={addActionToStep}
            onUpdateAction={updateAction}
            onDeleteAction={deleteAction}
            onUpdateVariables={updateVariables}
            onClose={handleCloseProperties}
          />
        )}
        {!selectedNode && !selectedEdge && (
          <div className="trust-empty">
            <div>Select a step, parallel node, or transition to view properties</div>
          </div>
        )}
      </div>
      </div>
    </div>
  );
};
