import React, { useCallback, useEffect, useState } from "react";
import {
  Background,
  BackgroundVariant,
  Controls,
  MarkerType,
  MiniMap,
  Panel,
  ReactFlow,
  type ReactFlowInstance,
  type XYPosition,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import "./sfcEditor.css";
import { ParallelNode } from "./ParallelNode";
import { PropertiesPanel } from "./PropertiesPanel";
import { SfcCodePanel } from "./SfcCodePanel";
import { SfcExecutionPanel } from "./SfcExecutionPanel";
import { StepNode } from "./StepNode";
import { SfcDragItemType, SfcToolsPanel } from "./SfcToolsPanel";
import { useSfc } from "./hooks/useSfc";
import {
  SfcExecutionState,
  SfcExtensionToWebviewMessage,
  SfcNode,
  SfcTransitionEdge,
  SfcWebviewToExtensionMessage,
} from "./types";
import { getVsCodeApi } from "../../visual/runtime/webview/vscodeApi";
import { useRightPaneResize } from "../../visual/runtime/webview/useRightPaneResize";
import {
  DEFAULT_RUNTIME_UI_STATE,
  type RightPaneView,
  type RuntimeUiState,
} from "../../visual/runtime/runtimeTypes";
import "../../visual/runtime/webview/rightPaneResize.css";

const vscode = getVsCodeApi();

const nodeTypes = {
  step: StepNode,
  parallelSplit: ParallelNode,
  parallelJoin: ParallelNode,
} as const;
const DRAG_MIME_TYPE = "application/x-trust-sfc-node";

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
  const [executionState, setExecutionState] = useState<SfcExecutionState | null>(null);
  const [runtimeState, setRuntimeState] = useState<RuntimeUiState>(
    DEFAULT_RUNTIME_UI_STATE
  );
  const [rightPaneView, setRightPaneView] = useState<RightPaneView>("io");
  const [showCodePanel, setShowCodePanel] = useState(false);
  const [generatedCode, setGeneratedCode] = useState<string | null>(null);
  const [codeErrors, setCodeErrors] = useState<string[]>([]);
  const [isGeneratingCode, setIsGeneratingCode] = useState(false);
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
            }
          } catch (error) {
            console.error("Failed to parse SFC workspace:", error);
            vscode.postMessage({
              type: "error",
              error: String(error),
            } as SfcWebviewToExtensionMessage);
          }
          break;

        case "executionState":
          setExecutionState(message.state);
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
          setExecutionState(null);
          highlightActiveSteps([]);
          break;

        case "runtime.state":
          setRuntimeState(message.state);
          if (!message.state.isExecuting) {
            setExecutionState(null);
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
  }, [addNodeAtPosition]);

  const handleAddParallelSplit = useCallback(() => {
    addNodeAtPosition("parallelSplit");
  }, [addNodeAtPosition]);

  const handleAddParallelJoin = useCallback(() => {
    addNodeAtPosition("parallelJoin");
  }, [addNodeAtPosition]);

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
  }, [autoLayout]);

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

  const handleDebugPause = useCallback(() => {
    vscode.postMessage({
      type: "debugPause",
    } as SfcWebviewToExtensionMessage);
  }, []);

  const handleDebugResume = useCallback(() => {
    vscode.postMessage({
      type: "debugResume",
    } as SfcWebviewToExtensionMessage);
  }, []);

  const handleDebugStepOver = useCallback(() => {
    vscode.postMessage({
      type: "debugStepOver",
    } as SfcWebviewToExtensionMessage);
  }, []);

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
  const stepCount = nodes.filter((node) => node.type === "step").length;

  return (
    <div className="sfc-editor" style={{ width: "100%", height: "100vh", display: "flex" }}>
      <div style={{ flex: 1, minWidth: 0, position: "relative" }}>
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
          fitView
          fitViewOptions={{
            padding: 0.2,
            minZoom: 0.5,
            maxZoom: 1,
          }}
          snapToGrid
          snapGrid={[15, 15]}
          defaultEdgeOptions={{
            type: "smoothstep",
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
              fill: "var(--vscode-editor-foreground)",
              fontSize: "11px",
              fontWeight: 600,
            },
            labelBgPadding: [7, 3],
            labelBgBorderRadius: 4,
            labelBgStyle: {
              fill: "var(--vscode-editor-background)",
              fillOpacity: 0.92,
              stroke: "var(--vscode-panel-border)",
              strokeWidth: 1,
            },
          }}
          style={{
            background: "var(--vscode-editor-background)",
          }}
        >
          <Background
            variant={BackgroundVariant.Dots}
            gap={20}
            size={1}
            color="var(--vscode-editorWidget-border)"
          />
          <Controls />
          <MiniMap
            nodeColor={(node) => {
              const data = node.data;
              if (data?.isActive) {
                return "#4caf50";
              }
              if (data?.isCurrentDebugStep) {
                return "#FFA500";
              }
              if (data?.type === "initial") {
                return "#2196f3";
              }
              if (
                data?.nodeType === "parallelSplit" ||
                data?.nodeType === "parallelJoin"
              ) {
                return "#9c27b0";
              }
              return "#757575";
            }}
            style={{
              backgroundColor: "var(--vscode-editor-background)",
              border: "1px solid var(--vscode-panel-border)",
            }}
          />

          <Panel
            position="bottom-right"
            style={{
              padding: "8px 12px",
              backgroundColor: "var(--vscode-editor-background)",
              border: "1px solid var(--vscode-panel-border)",
              borderRadius: "4px",
              fontSize: "12px",
            }}
          >
            <div>Steps: {stepCount}</div>
            <div>Transitions: {edges.length}</div>
            {nodes.length > stepCount && (
              <div>Parallel Nodes: {nodes.length - stepCount}</div>
            )}
            {selectedNode && <div>Selected: {selectedNode.data.label}</div>}
            {!selectedNode && selectedEdge && (
              <div>Selected Transition: {selectedEdge.data.label || selectedEdge.id}</div>
            )}
            {executionState && executionState.activeSteps.length > 0 && (
              <div style={{ color: "#4caf50", fontWeight: 600 }}>
                Active: {executionState.activeSteps.length}
              </div>
            )}
          </Panel>
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
          borderLeft: "1px solid var(--vscode-panel-border, #2b2b2b)",
          backgroundColor: "var(--vscode-sideBar-background, var(--vscode-editor-background, #1e1e1e))",
          display: "flex",
          flexDirection: "column",
          overflowY: "auto",
          overflowX: "hidden",
        }}
        className="right-pane-resizable"
      >
        <div
          style={{
            display: "grid",
            gridTemplateColumns: "repeat(3, minmax(0, 1fr))",
            gap: "6px",
            padding: "8px",
            borderBottom: "1px solid var(--vscode-panel-border, #2b2b2b)",
            position: "sticky",
            top: 0,
            zIndex: 3,
            background: "var(--vscode-sideBar-background, var(--vscode-editor-background, #1e1e1e))",
          }}
        >
          {(["io", "settings", "tools"] as RightPaneView[]).map((view) => (
            <button
              key={view}
              type="button"
              onClick={() => setRightPaneView(view)}
              style={{
                border: "1px solid var(--vscode-button-border, var(--vscode-panel-border, #2b2b2b))",
                borderRadius: "4px",
                background:
                  rightPaneView === view
                    ? "var(--vscode-button-background, #0e639c)"
                    : "var(--vscode-button-secondaryBackground, var(--vscode-button-background, #313131))",
                color:
                  rightPaneView === view
                    ? "var(--vscode-button-foreground, #ffffff)"
                    : "var(--vscode-button-secondaryForeground, var(--vscode-button-foreground, #cccccc))",
                borderColor:
                  rightPaneView === view
                    ? "var(--vscode-focusBorder, #007fd4)"
                    : "var(--vscode-button-border, var(--vscode-panel-border, #2b2b2b))",
                padding: "5px 8px",
                fontSize: "11px",
                fontWeight: 600,
                cursor: "pointer",
              }}
              aria-pressed={rightPaneView === view}
            >
              {view === "io" ? "I/O" : view === "settings" ? "Settings" : "Tools"}
            </button>
          ))}
        </div>

        {rightPaneView === "tools" ? (
          <>
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
              <div
                style={{
                  padding: "16px",
                  fontSize: "12px",
                  color: "var(--vscode-descriptionForeground, var(--vscode-foreground, #9d9d9d))",
                  textAlign: "center",
                }}
              >
                <div>Select a step, parallel node, or transition to view properties</div>
              </div>
            )}
          </>
        ) : (
          <SfcExecutionPanel
            activeRuntimeView={rightPaneView === "settings" ? "settings" : "io"}
            runtimeState={runtimeState}
            executionState={executionState}
            onViewChange={setRightPaneView}
            onDebugPause={handleDebugPause}
            onDebugResume={handleDebugResume}
            onDebugStepOver={handleDebugStepOver}
          />
        )}
      </div>
    </div>
  );
};
