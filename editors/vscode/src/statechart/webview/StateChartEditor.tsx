import React, { useCallback, useEffect, useState } from "react";
import {
  ReactFlow,
  Background,
  Controls,
  MiniMap,
  Panel,
  BackgroundVariant,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import { StateNode } from "./StateNode";
import { PropertiesPanel } from "./PropertiesPanel";
import { ExecutionPanel } from "./ExecutionPanel";
import { ActionMappingsPanel } from "./ActionMappingsPanel";
import { StatechartToolsPanel } from "./StatechartToolsPanel";
import { useStateChart } from "./hooks/useStateChart";
import {
  WebviewToExtensionMessage,
  ExtensionToWebviewMessage,
  StateChartNode,
  StateChartEdge,
  ExecutionState,
} from "./types";
import { runtimeMessage } from "../../visual/runtime/runtimeMessages";
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
  stateNode: StateNode,
} as any; // Type assertion to avoid @xyflow/react type inference issues

/**
 * Main StateChart Editor Component
 */
export const StateChartEditor: React.FC = () => {
  const {
    nodes,
    edges,
    actionMappings,
    onNodesChange,
    onEdgesChange,
    onConnect,
    addNewState,
    updateNodeData,
    updateEdgeData,
    updateActionMappings,
    deleteSelected,
    autoLayout,
    exportToXState,
    importFromXState,
    setNodes,
  } = useStateChart();

  const [selectedNode, setSelectedNode] = useState<StateChartNode | null>(null);
  const [selectedEdge, setSelectedEdge] = useState<StateChartEdge | null>(null);
  const [executionState, setExecutionState] = useState<ExecutionState | null>(null);
  const [runtimeState, setRuntimeState] = useState<RuntimeUiState>(
    DEFAULT_RUNTIME_UI_STATE
  );
  const [rightPaneView, setRightPaneView] = useState<RightPaneView>("io");
  const {
    rightPaneStyle,
    resizeHandleClassName,
    resizeHandleProps,
  } = useRightPaneResize("statechart");

  // Handle messages from extension
  useEffect(() => {
    const handleMessage = (event: MessageEvent<ExtensionToWebviewMessage>) => {
      const message = event.data;

      switch (message.type) {
        case "init":
        case "update":
          try {
            if (message.content) {
              const config = JSON.parse(message.content);
              importFromXState(config);
            }
          } catch (error) {
            console.error("Failed to parse StateChart config:", error);
            vscode.postMessage({
              type: "error",
              error: String(error),
            } as WebviewToExtensionMessage);
          }
          break;

        case "executionState":
          setExecutionState(message.state);
          // Update active state indicator
          updateActiveState(message.state.currentState);
          break;

        case "executionStopped":
          setExecutionState(null);
          // Clear active state indicators
          updateActiveState(null);
          break;

        case "runtime.state":
          setRuntimeState(message.state);
          if (!message.state.isExecuting) {
            setExecutionState(null);
            updateActiveState(null);
          }
          break;

        case "runtime.error":
          console.error("StateChart runtime error:", message.message);
          break;
      }
    };

    window.addEventListener("message", handleMessage);
    
    // Notify extension that webview is ready
    vscode.postMessage({ type: "ready" } as WebviewToExtensionMessage);

    return () => window.removeEventListener("message", handleMessage);
  }, [importFromXState]);

  // Update active state indicator on nodes
  const updateActiveState = useCallback(
    (activeStateName: string | null) => {
      setNodes((nds) =>
        nds.map((node) => ({
          ...node,
          data: {
            ...node.data,
            isActive: node.data.label === activeStateName,
          },
        }))
      );
    },
    [setNodes]
  );

  // Save changes to document
  const handleSave = useCallback(() => {
    const config = exportToXState();
    const content = JSON.stringify(config, null, 2);
    vscode.postMessage({
      type: "save",
      content,
    } as WebviewToExtensionMessage);
  }, [exportToXState]);

  const handleOpenRuntimePanel = useCallback(() => {
    vscode.postMessage(runtimeMessage.openPanel() as WebviewToExtensionMessage);
  }, []);

  const handleSendEvent = useCallback((event: string) => {
    vscode.postMessage({
      type: "sendEvent",
      event,
    } as WebviewToExtensionMessage);
  }, []);

  // Handle selection changes
  const handleSelectionChange = useCallback(
    ({ nodes: selectedNodes, edges: selectedEdges }: any) => {
      setSelectedNode(selectedNodes[0] || null);
      setSelectedEdge(selectedEdges[0] || null);
    },
    []
  );

  // Toolbar actions
  const handleAddState = useCallback(() => {
    addNewState("normal");
  }, [addNewState]);

  const handleAddInitialState = useCallback(() => {
    addNewState("initial");
  }, [addNewState]);

  const handleAddFinalState = useCallback(() => {
    addNewState("final");
  }, [addNewState]);

  const handleDelete = useCallback(() => {
    deleteSelected();
    setSelectedNode(null);
    setSelectedEdge(null);
  }, [deleteSelected]);

  return (
    <div style={{ width: "100%", height: "100vh", display: "flex" }}>
      {/* Main editor area */}
      <div style={{ flex: 1, minWidth: 0, position: "relative" }}>
        <ReactFlow
          nodes={nodes}
          edges={edges}
          onNodesChange={onNodesChange}
          onEdgesChange={onEdgesChange}
          onConnect={onConnect}
          onSelectionChange={handleSelectionChange}
          nodeTypes={nodeTypes}
          fitView
          snapToGrid
          snapGrid={[15, 15]}
          defaultEdgeOptions={{
            type: "smoothstep",
            animated: true,
            style: {
              stroke: "var(--vscode-editorWidget-border)",
              strokeWidth: 2,
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
              const data = node.data as any;
              switch (data?.type) {
                case "initial":
                  return "#4caf50";
                case "final":
                  return "#f44336";
                case "compound":
                  return "#2196f3";
                default:
                  return "#757575";
              }
            }}
            style={{
              backgroundColor: "var(--vscode-editor-background)",
              border: "1px solid var(--vscode-panel-border)",
            }}
          />

          {/* Info Panel */}
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
            <div>Nodes: {nodes.length}</div>
            <div>Transitions: {edges.length}</div>
            {selectedNode && <div>Selected: {selectedNode.data.label}</div>}
          </Panel>
        </ReactFlow>
      </div>

      <div className={resizeHandleClassName} {...resizeHandleProps} />

      {/* Properties Panel (Sidebar) */}
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
          style={
            {
              display: "grid",
              gridTemplateColumns: "repeat(3, minmax(0, 1fr))",
              gap: "6px",
              padding: "8px",
              borderBottom: "1px solid var(--vscode-panel-border, #2b2b2b)",
              position: "sticky",
              top: 0,
              zIndex: 3,
              background: "var(--vscode-sideBar-background, var(--vscode-editor-background, #1e1e1e))",
            }
          }
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
                    : "var(--vscode-button-secondaryBackground, #3a3d41)",
                color:
                  rightPaneView === view
                    ? "var(--vscode-button-foreground, #ffffff)"
                    : "var(--vscode-button-secondaryForeground, var(--vscode-foreground, #cccccc))",
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
            <StatechartToolsPanel
              canDelete={Boolean(selectedNode || selectedEdge)}
              onAddState={handleAddState}
              onAddInitialState={handleAddInitialState}
              onAddFinalState={handleAddFinalState}
              onOpenRuntimePanel={handleOpenRuntimePanel}
              onDelete={handleDelete}
              onAutoLayout={autoLayout}
              onSave={handleSave}
            />
            <PropertiesPanel
              selectedNode={selectedNode}
              selectedEdge={selectedEdge}
              onUpdateNode={updateNodeData}
              onUpdateEdge={updateEdgeData}
            />
            <ActionMappingsPanel
              actionMappings={actionMappings}
              nodes={nodes}
              onUpdateActionMappings={updateActionMappings}
            />
          </>
        ) : (
          <ExecutionPanel
            activeRuntimeView={rightPaneView === "settings" ? "settings" : "io"}
            runtimeState={runtimeState}
            executionState={executionState}
            onSendEvent={handleSendEvent}
            onViewChange={setRightPaneView}
          />
        )}
      </div>
    </div>
  );
};
