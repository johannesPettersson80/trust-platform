import React from "react";
import { StRuntimePanel } from "../../visual/runtime/webview/StRuntimePanel";
import type { RightPaneView, RuntimeUiState } from "../../visual/runtime/runtimeTypes";

interface SfcExecutionState {
  activeSteps: string[];
  mode: "simulation" | "hardware";
  status?: "stopped" | "running" | "paused";
  breakpoints?: string[];
  currentStep?: string | null;
}

interface SfcExecutionPanelProps {
  activeRuntimeView: Extract<RightPaneView, "io" | "settings">;
  runtimeState: RuntimeUiState;
  executionState: SfcExecutionState | null;
  onViewChange: (view: RightPaneView) => void;
  onDebugPause?: () => void;
  onDebugResume?: () => void;
  onDebugStepOver?: () => void;
}

export const SfcExecutionPanel: React.FC<SfcExecutionPanelProps> = ({
  activeRuntimeView,
  runtimeState,
  executionState,
  onViewChange,
  onDebugPause,
  onDebugResume,
  onDebugStepOver,
}) => {
  const isRunning = runtimeState.isExecuting;
  const activeSteps = executionState?.activeSteps || [];
  const mode = executionState?.mode || "simulation";
  const status = executionState?.status || "stopped";
  const isPaused = status === "paused";
  const currentStep = executionState?.currentStep;
  const breakpointCount = executionState?.breakpoints?.length || 0;

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
              <div style={{ fontSize: "12px", fontWeight: 600 }}>Execution Mode</div>
              <div
                style={{
                  padding: "8px",
                  borderRadius: "4px",
                  border: "1px solid var(--vscode-panel-border, #2b2b2b)",
                  background: "var(--vscode-editor-background, #1e1e1e)",
                  fontSize: "12px",
                }}
              >
                <div>{mode === "simulation" ? "Simulation" : "Hardware"}</div>
              </div>

              <div style={{ fontSize: "12px", fontWeight: 600 }}>Execution Status</div>
              <div
                style={{
                  padding: "8px",
                  borderRadius: "4px",
                  border: "1px solid var(--vscode-panel-border, #2b2b2b)",
                  background: isPaused
                    ? "rgba(255, 165, 0, 0.1)"
                    : "var(--vscode-editor-background, #1e1e1e)",
                  fontSize: "12px",
                  display: "flex",
                  alignItems: "center",
                  gap: "8px",
                }}
              >
                <div>
                  {isPaused ? "Paused" : "Running"}
                  {breakpointCount > 0 && ` (${breakpointCount} breakpoint${breakpointCount > 1 ? "s" : ""})`}
                </div>
              </div>

              {/* Debug Controls */}
              {(onDebugPause || onDebugResume || onDebugStepOver) && (
                <>
                  <div style={{ fontSize: "12px", fontWeight: 600 }}>Debug Controls</div>
                  <div
                    style={{
                      display: "flex",
                      gap: "8px",
                    }}
                  >
                    {isPaused ? (
                      <>
                        <button
                          onClick={onDebugResume}
                          style={{
                            flex: 1,
                            padding: "6px 12px",
                            borderRadius: "4px",
                            border: "1px solid var(--vscode-button-border, var(--vscode-panel-border, #2b2b2b))",
                            background: "var(--vscode-button-background, #0e639c)",
                            color: "var(--vscode-button-foreground, #ffffff)",
                            cursor: "pointer",
                            fontSize: "11px",
                            fontWeight: 600,
                          }}
                          title="Resume execution"
                        >
                          Resume
                        </button>
                        <button
                          onClick={onDebugStepOver}
                          style={{
                            flex: 1,
                            padding: "6px 12px",
                            borderRadius: "4px",
                            border: "1px solid var(--vscode-button-border, var(--vscode-panel-border, #2b2b2b))",
                            background: "var(--vscode-button-secondaryBackground, var(--vscode-button-background, #313131))",
                            color: "var(--vscode-button-secondaryForeground, var(--vscode-button-foreground, #cccccc))",
                            cursor: "pointer",
                            fontSize: "11px",
                            fontWeight: 600,
                          }}
                          title="Execute one step and pause"
                        >
                          Step
                        </button>
                      </>
                    ) : (
                      <button
                        onClick={onDebugPause}
                        style={{
                          flex: 1,
                          padding: "6px 12px",
                          borderRadius: "4px",
                          border: "1px solid var(--vscode-button-border, var(--vscode-panel-border, #2b2b2b))",
                          background: "var(--vscode-button-background, #0e639c)",
                          color: "var(--vscode-button-foreground, #ffffff)",
                          cursor: "pointer",
                          fontSize: "11px",
                          fontWeight: 600,
                        }}
                        title="Pause execution"
                      >
                        Pause
                      </button>
                    )}
                  </div>
                </>
              )}

              {isPaused && currentStep && (
                <>
                  <div style={{ fontSize: "12px", fontWeight: 600 }}>Current Step</div>
                  <div
                    style={{
                      padding: "8px",
                      borderRadius: "4px",
                      border: "1px solid #FFA500",
                      background: "rgba(255, 165, 0, 0.1)",
                      fontSize: "12px",
                      color: "#FFA500",
                      fontWeight: 600,
                    }}
                  >
                    {currentStep}
                  </div>
                </>
              )}

              <div style={{ fontSize: "12px", fontWeight: 600 }}>Active Steps</div>
              <div
                style={{
                  padding: "8px",
                  borderRadius: "4px",
                  border: "1px solid var(--vscode-panel-border, #2b2b2b)",
                  background: "var(--vscode-editor-background, #1e1e1e)",
                  fontSize: "12px",
                  maxHeight: "120px",
                  overflowY: "auto",
                }}
              >
                {activeSteps.length > 0 ? (
                  <ul style={{ margin: 0, paddingLeft: "16px" }}>
                    {activeSteps.map((stepId) => (
                      <li key={stepId} style={{ color: "var(--vscode-terminal-ansiGreen)" }}>
                        {stepId}
                      </li>
                    ))}
                  </ul>
                ) : (
                  <div style={{ opacity: 0.75 }}>No active steps</div>
                )}
              </div>
            </>
          ) : (
            <div style={{ fontSize: "12px", opacity: 0.75 }}>
              Runtime is stopped. Click Start to execute the SFC.
            </div>
          )}
        </div>
      )}
    </div>
  );
};
