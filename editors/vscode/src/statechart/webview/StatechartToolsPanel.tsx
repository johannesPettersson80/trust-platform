import React from "react";

interface StatechartToolsPanelProps {
  canDelete: boolean;
  onAddState: () => void;
  onAddInitialState: () => void;
  onAddFinalState: () => void;
  onOpenRuntimePanel: () => void;
  onDelete: () => void;
  onAutoLayout: () => void;
  onSave: () => void;
}

const buttonStyle: React.CSSProperties = {
  padding: "6px 8px",
  border: "1px solid var(--vscode-button-border, var(--vscode-panel-border, #2b2b2b))",
  borderRadius: "4px",
  background: "var(--vscode-button-secondaryBackground, #3a3d41)",
  color: "var(--vscode-button-secondaryForeground, var(--vscode-foreground, #cccccc))",
  fontSize: "12px",
  cursor: "pointer",
};

export const StatechartToolsPanel: React.FC<StatechartToolsPanelProps> = ({
  canDelete,
  onAddState,
  onAddInitialState,
  onAddFinalState,
  onOpenRuntimePanel,
  onDelete,
  onAutoLayout,
  onSave,
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
      aria-label="Statechart tools"
    >
      <div style={{ fontSize: "12px", fontWeight: 600 }}>Statechart Tools</div>
      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "6px" }}>
        <button type="button" style={buttonStyle} onClick={onAddState}>
          Add State
        </button>
        <button type="button" style={buttonStyle} onClick={onAddInitialState}>
          Add Initial
        </button>
        <button type="button" style={buttonStyle} onClick={onAddFinalState}>
          Add Final
        </button>
        <button type="button" style={buttonStyle} onClick={onAutoLayout}>
          Auto Layout
        </button>
      </div>
      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "6px" }}>
        <button
          type="button"
          style={{
            ...buttonStyle,
            opacity: canDelete ? 1 : 0.55,
            cursor: canDelete ? "pointer" : "not-allowed",
          }}
          onClick={onDelete}
          disabled={!canDelete}
        >
          Delete
        </button>
        <button type="button" style={buttonStyle} onClick={onSave}>
          Save
        </button>
      </div>
      <button type="button" style={buttonStyle} onClick={onOpenRuntimePanel}>
        Open Runtime Panel
      </button>
    </section>
  );
};
