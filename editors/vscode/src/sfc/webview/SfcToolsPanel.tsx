import React from "react";

export type SfcDragItemType = "step" | "parallelSplit" | "parallelJoin";

interface SfcToolsPanelProps {
  onAddStep: () => void;
  onAddParallelSplit?: () => void;
  onAddParallelJoin?: () => void;
  onToolDragStart?: (
    event: React.DragEvent<HTMLButtonElement>,
    itemType: SfcDragItemType
  ) => void;
  onDelete: () => void;
  onValidate: () => void;
  onGenerateST: () => void;
  onAutoLayout: () => void;
  onSave: () => void;
  onToggleCodePanel?: () => void;
  showCodePanel?: boolean;
  hasSelection: boolean;
}

const buttonStyle: React.CSSProperties = {
  padding: "7px 10px",
  border: "1px solid var(--vscode-button-border, var(--vscode-panel-border, #2b2b2b))",
  borderRadius: "4px",
  background: "var(--vscode-button-secondaryBackground, var(--vscode-button-background, #313131))",
  color: "var(--vscode-button-secondaryForeground, var(--vscode-button-foreground, #cccccc))",
  fontSize: "12px",
  fontWeight: 600,
  lineHeight: 1.25,
  whiteSpace: "nowrap",
  cursor: "pointer",
};

/**
 * SFC Tools Panel - appears in Tools tab
 */
export const SfcToolsPanel: React.FC<SfcToolsPanelProps> = ({
  onAddStep,
  onAddParallelSplit,
  onAddParallelJoin,
  onToolDragStart,
  onDelete,
  onValidate,
  onGenerateST,
  onAutoLayout,
  onSave,
  onToggleCodePanel,
  showCodePanel = false,
  hasSelection,
}) => {
  return (
    <section
      style={{
        display: "flex",
        flexDirection: "column",
        gap: "8px",
        padding: "10px 8px",
        borderBottom: "1px solid var(--vscode-panel-border, #2b2b2b)",
      }}
      aria-label="SFC tools"
    >
      <div style={{ fontSize: "12px", fontWeight: 600 }}>SFC Tools</div>
      <div
        style={{
          fontSize: "11px",
          color: "var(--vscode-descriptionForeground, var(--vscode-foreground, #9d9d9d))",
          opacity: 0.9,
        }}
      >
        Drag tools into the canvas or click to add.
      </div>
      <div
        style={{
          fontSize: "11px",
          color: "var(--vscode-descriptionForeground, var(--vscode-foreground, #9d9d9d))",
          opacity: 0.8,
        }}
      >
        Select a transition and press Delete/Backspace to remove it.
      </div>
      
      {/* Add Elements */}
      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "6px" }}>
        <button
          type="button"
          style={buttonStyle}
          onClick={onAddStep}
          title="Add new step"
          draggable={Boolean(onToolDragStart)}
          onDragStart={(event) => onToolDragStart?.(event, "step")}
        >
          Add Step
        </button>
        {onAddParallelSplit && (
          <button
            type="button"
            style={buttonStyle}
            onClick={onAddParallelSplit}
            title="Add parallel split"
            draggable={Boolean(onToolDragStart)}
            onDragStart={(event) => onToolDragStart?.(event, "parallelSplit")}
          >
            Split
          </button>
        )}
        {onAddParallelJoin && (
          <button
            type="button"
            style={buttonStyle}
            onClick={onAddParallelJoin}
            title="Add parallel join"
            draggable={Boolean(onToolDragStart)}
            onDragStart={(event) => onToolDragStart?.(event, "parallelJoin")}
          >
            Join
          </button>
        )}
        <button type="button" style={buttonStyle} onClick={onAutoLayout} title="Auto arrange steps">
          Layout
        </button>
      </div>

      {/* Actions */}
      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "6px" }}>
        <button type="button" style={buttonStyle} onClick={onValidate} title="Validate SFC">
          Validate
        </button>
        <button type="button" style={buttonStyle} onClick={onGenerateST} title="Generate ST code">
          Generate
        </button>
        {onToggleCodePanel && (
          <button
            type="button"
            style={{
              ...buttonStyle,
              background: showCodePanel
                ? "var(--vscode-button-background, #0e639c)"
                : "var(--vscode-button-secondaryBackground, var(--vscode-button-background, #313131))",
              color: showCodePanel
                ? "var(--vscode-button-foreground, #ffffff)"
                : "var(--vscode-button-secondaryForeground, var(--vscode-button-foreground, #cccccc))",
              borderColor: showCodePanel ? "var(--vscode-focusBorder, #007fd4)" : "var(--vscode-button-border, var(--vscode-panel-border, #2b2b2b))",
            }}
            onClick={onToggleCodePanel}
            title={showCodePanel ? "Hide code panel" : "Show code panel"}
          >
            {showCodePanel ? "Hide Code" : "Show Code"}
          </button>
        )}
        <button
          type="button"
          style={{
            ...buttonStyle,
            opacity: hasSelection ? 1 : 0.55,
            cursor: hasSelection ? "pointer" : "not-allowed",
          }}
          onClick={onDelete}
          disabled={!hasSelection}
          title="Delete selected element"
        >
          Delete
        </button>
      </div>

      {/* Save */}
      <button type="button" style={buttonStyle} onClick={onSave} title="Save changes">
        Save
      </button>
    </section>
  );
};
