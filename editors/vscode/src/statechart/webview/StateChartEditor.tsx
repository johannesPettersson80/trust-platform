import React, { useCallback, useEffect, useState } from "react";
import {
  Background,
  BackgroundVariant,
  Controls,
  ReactFlow,
  type ReactFlowInstance,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import { StateNode } from "./StateNode";
import { STATE_TRANSITION_EDGE, StateTransitionEdge } from "./StateTransitionEdge";
import { PropertiesPanel } from "./PropertiesPanel";
import { ActionMappingsPanel } from "./ActionMappingsPanel";
import { StatechartToolsPanel } from "./StatechartToolsPanel";
import { useStateChart } from "./hooks/useStateChart";
import {
  WebviewToExtensionMessage,
  ExtensionToWebviewMessage,
  StateChartNode,
  StateChartEdge,
} from "./types";
import { getVsCodeApi } from "../../visual/runtime/webview/vscodeApi";
import { useRightPaneResize } from "../../visual/runtime/webview/useRightPaneResize";
import "../../visual/runtime/webview/rightPaneResize.css";
import { t } from "../../webview/theme";

const vscode = getVsCodeApi();

const nodeTypes = {
  stateNode: StateNode,
} as any; // Type assertion to avoid @xyflow/react type inference issues
const edgeTypes = {
  [STATE_TRANSITION_EDGE]: StateTransitionEdge,
} as any;
const STATECHART_FIT_VIEW_OPTIONS = {
  padding: 0.18,
  minZoom: 0.3,
  maxZoom: 1,
} as const;

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
  const [parseError, setParseError] = useState<string | null>(null);
  const [reactFlowInstance, setReactFlowInstance] = useState<
    ReactFlowInstance<StateChartNode, StateChartEdge> | null
  >(null);
  const [fitViewRequest, setFitViewRequest] = useState(0);
  const {
    rightPaneStyle,
    resizeHandleClassName,
    resizeHandleProps,
  } = useRightPaneResize("statechart");

  const requestFitView = useCallback(() => {
    setFitViewRequest((value) => value + 1);
  }, []);

  useEffect(() => {
    if (!reactFlowInstance || fitViewRequest === 0) {
      return;
    }

    const timeout = window.setTimeout(() => {
      reactFlowInstance.fitView({
        ...STATECHART_FIT_VIEW_OPTIONS,
        duration: 150,
      });
    }, 50);

    return () => window.clearTimeout(timeout);
  }, [edges.length, fitViewRequest, nodes.length, reactFlowInstance]);

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
              requestFitView();
              setParseError(null);
            }
          } catch (error) {
            const detail = error instanceof Error ? error.message : String(error);
            console.error("Failed to parse StateChart config:", detail);
            setParseError(detail);
            vscode.postMessage({
              type: "error",
              error: "Could not open this statechart because the file is not valid JSON.",
            } as WebviewToExtensionMessage);
          }
          break;

        case "executionState":
          // Update active state indicator
          updateActiveState(message.state.currentState);
          break;

        case "executionStopped":
          // Clear active state indicators
          updateActiveState(null);
          break;

        case "runtime.state":
          if (!message.state.isExecuting) {
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
  }, [importFromXState, requestFitView]);

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

  const handleValidate = useCallback(() => {
    const config = exportToXState();
    const content = JSON.stringify(config, null, 2);
    vscode.postMessage({
      type: "validate",
      content,
    } as WebviewToExtensionMessage);
  }, [exportToXState]);

  const handleGenerateST = useCallback(() => {
    const config = exportToXState();
    const content = JSON.stringify(config, null, 2);
    vscode.postMessage({
      type: "generateST",
      content,
    } as WebviewToExtensionMessage);
  }, [exportToXState]);

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
    requestFitView();
  }, [addNewState, requestFitView]);

  const handleAddInitialState = useCallback(() => {
    addNewState("initial");
    requestFitView();
  }, [addNewState, requestFitView]);

  const handleAddFinalState = useCallback(() => {
    addNewState("final");
    requestFitView();
  }, [addNewState, requestFitView]);

  const handleDelete = useCallback(() => {
    deleteSelected();
    setSelectedNode(null);
    setSelectedEdge(null);
  }, [deleteSelected]);

  const handleAutoLayout = useCallback(() => {
    autoLayout();
    requestFitView();
  }, [autoLayout, requestFitView]);

  const handleOpenAsText = useCallback(() => {
    vscode.postMessage({ type: "openAsText" } as WebviewToExtensionMessage);
  }, []);

  return (
    <div className="trust-product-shell">
      <header className="trust-product-header" aria-label="Statechart editor header">
        <div className="trust-product-brand">
          tru<span className="trust-product-brand__accent">ST</span>
          <span className="trust-product-brand__separator">·</span>
          <span className="trust-product-brand__surface">Statechart editor</span>
        </div>
        <div className="trust-product-header__meta">State machine diagram</div>
      </header>

      <div className="trust-product-workspace">
      {/* Main editor area */}
      <div className="trust-canvas-pane">
        {parseError ? (
          <div
            role="alert"
            style={{
              height: "100%",
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              padding: "32px",
              background: t.canvas,
              color: t.text,
            }}
          >
            <div
              style={{
                maxWidth: 520,
                border: `1px solid ${t.danger}`,
                borderRadius: t.radius,
                padding: "18px 20px",
                background: t.surface,
                boxShadow: t.shadowOverlay,
              }}
            >
              <h2
                style={{
                  color: t.danger,
                  fontSize: 15,
                  margin: "0 0 8px",
                }}
              >
                Could not open this statechart
              </h2>
              <p
                style={{
                  color: t.text,
                  fontSize: 12,
                  lineHeight: 1.5,
                  margin: "0 0 10px",
                }}
              >
                The file is not valid JSON. Fix the JSON in the file, save it, and the visual editor will reload.
              </p>
              <button
                type="button"
                className="trust-button trust-button--primary"
                onClick={handleOpenAsText}
                title="Open this file in VS Code's default text editor"
                style={{ display: "block", marginBottom: 10 }}
              >
                Open as text
              </button>
              <code
                style={{
                  color: t.textMuted,
                  display: "block",
                  fontSize: 11,
                  whiteSpace: "pre-wrap",
                }}
              >
                {parseError}
              </code>
            </div>
          </div>
        ) : (
          <ReactFlow
            nodes={nodes}
            edges={edges}
            onNodesChange={onNodesChange}
            onEdgesChange={onEdgesChange}
            onConnect={onConnect}
            onSelectionChange={handleSelectionChange}
            nodeTypes={nodeTypes}
            edgeTypes={edgeTypes}
            onInit={setReactFlowInstance}
            fitView
            fitViewOptions={STATECHART_FIT_VIEW_OPTIONS}
            snapToGrid
            snapGrid={[15, 15]}
            defaultEdgeOptions={{
              type: STATE_TRANSITION_EDGE,
              style: {
                stroke: "var(--vscode-editorWidget-border)",
                strokeWidth: 2,
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
            <Controls showInteractive={false} />
          </ReactFlow>
        )}
      </div>

      {!parseError && (
        <>
          <div className={resizeHandleClassName} {...resizeHandleProps} />

          {/* Properties Panel (Sidebar) */}
          <div
            style={{
              ...rightPaneStyle,
            }}
            className="trust-inspector right-pane-resizable"
          >
            <div className="trust-inspector__header">
              <span className="trust-inspector__title">Statechart editor</span>
            </div>

            <StatechartToolsPanel
              canDelete={Boolean(selectedNode || selectedEdge)}
              onAddState={handleAddState}
              onAddInitialState={handleAddInitialState}
              onAddFinalState={handleAddFinalState}
              onDelete={handleDelete}
              onAutoLayout={handleAutoLayout}
              onFitView={requestFitView}
              onValidate={handleValidate}
              onGenerateST={handleGenerateST}
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
          </div>
        </>
      )}
      </div>
    </div>
  );
};
