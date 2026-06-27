import React, { memo } from "react";
import { Handle, Position, NodeProps } from "@xyflow/react";
import { StateNodeData } from "./types";
import { t, tint } from "../../webview/theme";

export const STATE_TARGET_TOP = "target-top";
export const STATE_TARGET_LEFT = "target-left";
export const STATE_TARGET_RIGHT = "target-right";
export const STATE_SOURCE_BOTTOM = "source-bottom";
export const STATE_SOURCE_LEFT = "source-left";
export const STATE_SOURCE_RIGHT = "source-right";

/**
 * Visual component for a state node in the StateChart  * Renders different styles based on state type (normal, initial, final, compound)
 */
export const StateNode: React.FC<NodeProps> = memo(
  ({ data, selected }) => {
    const stateData = data as StateNodeData;

    const getNodeStyle = () => {
      const baseStyle: React.CSSProperties = {
        padding: "12px 16px",
        borderRadius: `${t.radiusLg}px`,
        border: `2px solid ${selected ? t.accent : t.border}`,
        backgroundColor: t.canvas,
        minWidth: "120px",
        fontSize: "13px",
        fontFamily: "var(--vscode-font-family)",
        color: t.text,
        boxShadow: selected ? `0 0 0 2px ${t.accent}` : t.shadow,
        transition: `border-color ${t.ease}, box-shadow ${t.ease}, filter ${t.ease}`,
      };

      if (stateData.isActive) {
        return {
          ...baseStyle,
          backgroundColor: tint(t.ok, 0.22),
          borderColor: t.ok,
          borderWidth: "3px",
          boxShadow: `0 0 0 2px ${tint(t.ok, 0.28)}, ${t.shadow}`,
          color: t.onAccent,
          fontWeight: 600,
        };
      }

      // Style variations by state type
      switch (stateData.type) {
        case "initial":
          return {
            ...baseStyle,
            borderColor: t.ok,
            borderWidth: "3px",
          };
        case "final":
          return {
            ...baseStyle,
            borderColor: t.danger,
            borderWidth: "3px",
            background: "var(--vscode-editor-inactiveSelectionBackground)",
          };
        case "compound":
          return {
            ...baseStyle,
            borderColor: t.accent,
            borderStyle: "dashed",
          };
        default:
          return baseStyle;
      }
    };

    const hasEntry = stateData.entry && stateData.entry.length > 0;
    const hasExit = stateData.exit && stateData.exit.length > 0;
    const handleStyle: React.CSSProperties = {
      background: "var(--vscode-button-background)",
      width: "10px",
      height: "10px",
      border: `2px solid ${t.canvas}`,
    };

    return (
      <div style={getNodeStyle()}>
        {/* Input handle for incoming transitions */}
        <Handle
          id={STATE_TARGET_TOP}
          type="target"
          position={Position.Top}
          style={handleStyle}
        />
        <Handle
          id={STATE_TARGET_LEFT}
          type="target"
          position={Position.Left}
          style={handleStyle}
        />
        <Handle
          id={STATE_TARGET_RIGHT}
          type="target"
          position={Position.Right}
          style={handleStyle}
        />

        {/* State label and type indicator */}
        <div style={{ marginBottom: hasEntry || hasExit ? "8px" : 0 }}>
          <div
            style={{
              fontWeight: stateData.isActive ? 700 : 600,
              fontSize: stateData.isActive ? "15px" : "14px",
              marginBottom: "4px",
              color: stateData.isActive ? t.onAccent : "inherit",
            }}
          >
            {stateData.isActive && "▶ "}
            {stateData.label}
          </div>
          {stateData.type !== "normal" && (
            <div
              style={{
                fontSize: "11px",
                opacity: stateData.isActive ? 0.9 : 0.7,
                textTransform: "uppercase",
                color: stateData.isActive ? t.onAccent : "inherit",
              }}
            >
              {stateData.type}
            </div>
          )}
        </div>

        {/* Entry actions */}
        {hasEntry && (
          <div
            style={{
              fontSize: "11px",
              marginTop: "6px",
              paddingTop: "6px",
              borderTop: `1px solid ${stateData.isActive ? tint(t.onAccent, 0.3) : t.border}`,
              color: stateData.isActive ? t.onAccent : "inherit",
            }}
          >
            <div style={{ opacity: stateData.isActive ? 0.9 : 0.7, marginBottom: "2px" }}>entry /</div>
            {stateData.entry!.map((action: string, idx: number) => (
              <div
                key={idx}
                style={{
                  paddingLeft: "8px",
                  fontFamily: "var(--vscode-editor-font-family)",
                  color: stateData.isActive ? t.onAccent : "inherit",
                }}
              >
                {action}
              </div>
            ))}
          </div>
        )}

        {/* Exit actions */}
        {hasExit && (
          <div
            style={{
              fontSize: "11px",
              marginTop: "6px",
              paddingTop: "6px",
              borderTop: `1px solid ${stateData.isActive ? tint(t.onAccent, 0.3) : t.border}`,
              color: stateData.isActive ? t.onAccent : "inherit",
            }}
          >
            <div style={{ opacity: stateData.isActive ? 0.9 : 0.7, marginBottom: "2px" }}>exit /</div>
            {stateData.exit!.map((action: string, idx: number) => (
              <div
                key={idx}
                style={{
                  paddingLeft: "8px",
                  fontFamily: "var(--vscode-editor-font-family)",
                  color: stateData.isActive ? t.onAccent : "inherit",
                }}
              >
                {action}
              </div>
            ))}
          </div>
        )}

        {/* Active state indicator - enhanced pulse animation */}
        {stateData.isActive && (
          <>
            <div
              style={{
                position: "absolute",
                top: "-8px",
                right: "-8px",
                width: "20px",
                height: "20px",
                borderRadius: "50%",
                backgroundColor: t.ok,
                border: `3px solid ${t.canvas}`,
                animation: "pulse 1.8s infinite",
                zIndex: 10,
              }}
            />
            {/* Outer glow ring */}
            <div
              style={{
                position: "absolute",
                top: "-14px",
                right: "-14px",
                width: "32px",
                height: "32px",
                borderRadius: "50%",
                border: `2px solid ${t.ok}`,
                animation: "ripple 2s infinite",
                zIndex: 9,
              }}
            />
          </>
        )}

        {/* Output handle for outgoing transitions */}
        <Handle
          id={STATE_SOURCE_BOTTOM}
          type="source"
          position={Position.Bottom}
          style={handleStyle}
        />
        <Handle
          id={STATE_SOURCE_LEFT}
          type="source"
          position={Position.Left}
          style={handleStyle}
        />
        <Handle
          id={STATE_SOURCE_RIGHT}
          type="source"
          position={Position.Right}
          style={handleStyle}
        />

        <style>{`
          @keyframes pulse {
            0%, 100% { 
              transform: scale(1);
              opacity: 1;
            }
            50% { 
              transform: scale(1.2);
              opacity: 0.8;
            }
          }
          
          @keyframes ripple {
            0% {
              transform: scale(1);
              opacity: 1;
            }
            100% {
              transform: scale(1.8);
              opacity: 0;
            }
          }
        `}</style>
      </div>
    );
  }
);

StateNode.displayName = "StateNode";
