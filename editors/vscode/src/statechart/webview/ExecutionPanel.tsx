import React, { useState } from "react";
import { StRuntimePanel } from "../../visual/runtime/webview/StRuntimePanel";
import type { RightPaneView, RuntimeUiState } from "../../visual/runtime/runtimeTypes";
import { ExecutionState } from "./types";

interface ExecutionPanelProps {
  activeRuntimeView: Extract<RightPaneView, "io" | "settings">;
  runtimeState: RuntimeUiState;
  executionState: ExecutionState | null;
  onSendEvent: (event: string) => void;
  onViewChange: (view: RightPaneView) => void;
}

export const ExecutionPanel: React.FC<ExecutionPanelProps> = ({
  activeRuntimeView,
  runtimeState,
  executionState,
  onSendEvent,
  onViewChange,
}) => {
  const [customEvent, setCustomEvent] = useState("");
  const isRunning = runtimeState.isExecuting;
  const currentState = executionState?.currentState;
  const previousState = executionState?.previousState;
  const availableEvents = executionState?.availableEvents || [];

  const sendCustomEvent = () => {
    const value = customEvent.trim();
    if (!value) {
      return;
    }
    onSendEvent(value);
    setCustomEvent("");
  };

  return (
    <div
      style={{
        borderBottom: "1px solid var(--vscode-panel-border, #2b2b2b)",
        display: "flex",
        flexDirection: "column",
      }}
    >
      <StRuntimePanel
        activeView={activeRuntimeView}
        onViewChange={onViewChange}
        showToolsShortcut
        toolsShortcutLabel="Tools"
      />

      {activeRuntimeView === "io" && (
        <div
          style={{
            padding: "12px",
            display: "flex",
            flexDirection: "column",
            gap: "10px",
          }}
        >
          {isRunning ? (
            <>
              <div style={{ fontSize: "12px", fontWeight: 600 }}>Current State</div>
              <div
                style={{
                  padding: "8px",
                  borderRadius: "4px",
                  border: "1px solid var(--vscode-panel-border, #2b2b2b)",
                  background: "var(--vscode-editor-background, #1e1e1e)",
                  fontSize: "12px",
                }}
              >
                <div>{currentState || "—"}</div>
                {previousState && (
                  <div style={{ opacity: 0.75, marginTop: "4px" }}>
                    Previous: {previousState}
                  </div>
                )}
              </div>

              <div style={{ fontSize: "12px", fontWeight: 600 }}>Events</div>
              <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "6px" }}>
                {availableEvents.length > 0 ? (
                  availableEvents.map((event) => (
                    <button
                      key={event}
                      style={eventButtonStyle}
                      onClick={() => onSendEvent(event)}
                      title={`Send ${event}`}
                    >
                      {event}
                    </button>
                  ))
                ) : (
                  <div style={{ fontSize: "12px", opacity: 0.75 }}>
                    No events available
                  </div>
                )}
              </div>

              <div style={{ display: "flex", gap: "6px" }}>
                <input
                  type="text"
                  value={customEvent}
                  onChange={(event) => setCustomEvent(event.target.value)}
                  onKeyDown={(event) => {
                    if (event.key === "Enter") {
                      sendCustomEvent();
                    }
                  }}
                  placeholder="Custom event"
                  style={{
                    flex: 1,
                    border: "1px solid var(--vscode-input-border, #3c3c3c)",
                    borderRadius: "4px",
                    background: "var(--vscode-input-background, #313131)",
                    color: "var(--vscode-input-foreground, var(--vscode-foreground, #cccccc))",
                    padding: "6px 8px",
                    fontSize: "12px",
                  }}
                />
                <button
                  onClick={sendCustomEvent}
                  disabled={!customEvent.trim()}
                  style={{
                    ...eventButtonStyle,
                    minWidth: "64px",
                  }}
                >
                  Send
                </button>
              </div>
            </>
          ) : (
            <div style={{ fontSize: "12px", opacity: 0.75 }}>
              Runtime is stopped.
            </div>
          )}
        </div>
      )}
    </div>
  );
};

const eventButtonStyle: React.CSSProperties = {
  border: "1px solid var(--vscode-button-border, var(--vscode-panel-border, #2b2b2b))",
  borderRadius: "4px",
  padding: "6px 8px",
  background: "var(--vscode-button-secondaryBackground, #3a3d41)",
  color: "var(--vscode-button-secondaryForeground, var(--vscode-foreground, #cccccc))",
  fontSize: "12px",
  cursor: "pointer",
};
