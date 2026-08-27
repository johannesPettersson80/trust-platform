import React from "react";
import {
  Background,
  BackgroundVariant,
  Controls,
  type Edge,
  type Node,
  type NodeMouseHandler,
  type OnEdgesChange,
  type OnNodesChange,
  ReactFlow,
} from "@xyflow/react";

import { CasedEdge } from "./CasedEdge";
import { EditModeContext, type EditModeValue } from "./editMode";
import { nodeTypes } from "./nodes";
import { t, tint } from "./theme";
import type { NCGraph } from "./types";

const edgeTypes = { cased: CasedEdge };

interface CanvasGraphShellProps {
  flowWrapRef: React.RefObject<HTMLDivElement | null>;
  nodes: Node[];
  edges: Edge[];
  onNodesChange: OnNodesChange<Node>;
  onEdgesChange: OnEdgesChange<Edge>;
  onNodeDragStop: NodeMouseHandler<Node>;
  onDrop: React.DragEventHandler;
  onDragOver: React.DragEventHandler;
  plantDropMessage?: string;
  onNodeContextMenu: NodeMouseHandler<Node>;
  editMode: EditModeValue;
  graph: NCGraph;
  onSelectNode: (node: Node) => void;
  post: (message: unknown) => void;
}

// This component owns only the graph viewport and its honest empty/banner
// presentation. Selection policy, drawers, persistence and host transport stay
// in their dedicated owners.
export function CanvasGraphShell({
  flowWrapRef,
  nodes,
  edges,
  onNodesChange,
  onEdgesChange,
  onNodeDragStop,
  onDrop,
  onDragOver,
  plantDropMessage,
  onNodeContextMenu,
  editMode,
  graph,
  onSelectNode,
  post,
}: CanvasGraphShellProps) {
  return (
    <div ref={flowWrapRef} style={{ position: "relative", flex: 1, minWidth: 0 }}>
      <EditModeContext.Provider value={editMode}>
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
          onNodeClick={(_, node) => onSelectNode(node)}
        >
          <Background variant={BackgroundVariant.Dots} gap={26} size={1} color="var(--trust-grid-line)" />
          <Controls showInteractive={false} />
        </ReactFlow>
      </EditModeContext.Provider>

      {nodes.length === 0 && <LoadingState />}

      {plantDropMessage && (
        <div
          data-plant-drop-message
          role="status"
          aria-live="polite"
          style={{
            position: "absolute",
            bottom: 16,
            left: "50%",
            transform: "translateX(-50%)",
            maxWidth: "min(520px, calc(100% - 32px))",
            background: t.overlay,
            border: `1px solid ${t.border}`,
            borderRadius: t.radius,
            color: t.text,
            padding: "7px 10px",
            fontSize: 11.5,
            zIndex: 6,
          }}
        >
          {plantDropMessage}
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
          {graph.banner.actions.map((action) => (
            <button
              key={action.action}
              onClick={() => post({ type: "action", action: action.action })}
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
              {action.label}
            </button>
          ))}
        </div>
      )}

      {graph.summary && <div className="trust-canvas-summary">{graph.summary}</div>}
    </div>
  );
}

function LoadingState() {
  return (
    <div
      className="trust-loading"
      style={{
        position: "absolute",
        inset: 0,
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        justifyContent: "center",
        gap: 12,
        pointerEvents: "none",
      }}
    >
      <svg
        width="38"
        height="38"
        viewBox="0 0 24 24"
        fill="none"
        stroke={t.textSubtle}
        strokeWidth={1.4}
        strokeLinecap="round"
        strokeLinejoin="round"
      >
        <rect x="3" y="4.5" width="18" height="6" rx="1.5" />
        <rect x="3" y="13.5" width="18" height="6" rx="1.5" />
        <circle cx="6.6" cy="7.5" r="1" fill={t.textSubtle} stroke="none" />
        <circle cx="6.6" cy="16.5" r="1" fill={t.textSubtle} stroke="none" />
      </svg>
      <div style={{ fontSize: 13.5, fontWeight: 600, color: t.textMuted }}>
        Loading your devices…
      </div>
      <div style={{ fontSize: 12, color: t.textSubtle, maxWidth: 300, textAlign: "center" }}>
        Reading the project's runtime and connections.
      </div>
    </div>
  );
}
