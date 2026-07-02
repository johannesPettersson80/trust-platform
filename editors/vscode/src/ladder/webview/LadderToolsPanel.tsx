import React from "react";

export const LADDER_TOOL_DRAG_MIME = "application/x-trust-ladder-tool";
export type LadderToolId =
  | "contact"
  | "coil"
  | "timer"
  | "counter"
  | "compare"
  | "math"
  | "branchSplit"
  | "branchMerge"
  | "junction";

interface LadderToolsPanelProps {
  selectedTool: LadderToolId | null;
  onToolSelect: (tool: LadderToolId | null) => void;
  onDeleteSelection: () => void;
  onAddRung: () => void;
  onRemoveRung: () => void;
  onAddParallelContact: () => void;
  onClearWiring: () => void;
  onUndo: () => void;
  onRedo: () => void;
  onCopy: () => void;
  onPaste: () => void;
  onSearchReplace: () => void;
  onAutoRoute: () => void;
  onValidate: () => void;
  onGenerateST: () => void;
  onSave: () => void;
  onToggleLinkMode: () => void;
  linkModeEnabled: boolean;
  linkSourceLabel?: string;
  linkFeedback?: string | null;
  canUndo: boolean;
  canRedo: boolean;
  canPaste: boolean;
  canDeleteSelection: boolean;
  canRemoveRung: boolean;
  canAddParallelContact: boolean;
  canClearWiring: boolean;
}

const LOGIC_TOOL_OPTIONS: Array<{ id: LadderToolId; label: string; title: string }> = [
  { id: "contact", label: "Contact", title: "Add Contact (NO/NC)" },
  {
    id: "coil",
    label: "Coil",
    title: "Add Coil (NORMAL/SET/RESET/NEGATED per IEC Table 76)",
  },
  { id: "timer", label: "Timer", title: "Add Timer (TON/TOF/TP)" },
  { id: "counter", label: "Counter", title: "Add Counter (CTU/CTD/CTUD)" },
  { id: "compare", label: "Compare", title: "Add Compare block (GT/LT/EQ)" },
  { id: "math", label: "Math", title: "Add Math block (ADD/SUB/MUL/DIV)" },
];

const TOPOLOGY_TOOL_OPTIONS: Array<{ id: LadderToolId; label: string; title: string }> = [
  {
    id: "branchSplit",
    label: "Split",
    title: "Add branch split node for parallel legs",
  },
  {
    id: "junction",
    label: "Junction",
    title: "Add branch junction node",
  },
  {
    id: "branchMerge",
    label: "Merge",
    title: "Add branch merge node",
  },
];

export function LadderToolsPanel({
  selectedTool,
  onToolSelect,
  onDeleteSelection,
  onAddRung,
  onRemoveRung,
  onAddParallelContact,
  onClearWiring,
  onUndo,
  onRedo,
  onCopy,
  onPaste,
  onSearchReplace,
  onAutoRoute,
  onValidate,
  onGenerateST,
  onSave,
  onToggleLinkMode,
  linkModeEnabled,
  linkSourceLabel,
  linkFeedback,
  canUndo,
  canRedo,
  canPaste,
  canDeleteSelection,
  canRemoveRung,
  canAddParallelContact,
  canClearWiring,
}: LadderToolsPanelProps) {
  const handleToolDragStart = (
    event: React.DragEvent<HTMLButtonElement>,
    toolId: LadderToolId
  ) => {
    event.dataTransfer.setData(LADDER_TOOL_DRAG_MIME, toolId);
    event.dataTransfer.effectAllowed = "copy";
  };

  return (
    <section className="trust-section" aria-label="Ladder tools">
      <div className="trust-section__title">Tools</div>
      <div className="trust-button-grid">
        <button type="button" className="trust-button" onClick={onValidate} title="Validate ladder program">
          Validate
        </button>
        <button type="button" className="trust-button" onClick={onGenerateST} title="Generate Structured Text companion">
          Generate ST
        </button>
        <button
          type="button"
          className="trust-button trust-button--primary"
          onClick={onSave}
          title="Save program"
        >
          Save
        </button>
      </div>
      <div className="trust-section__title" style={{ marginTop: 10 }}>Elements</div>
      <div className="trust-button-grid">
        {LOGIC_TOOL_OPTIONS.map((tool) => (
          <button
            key={tool.id}
            type="button"
            className={`trust-button ladder-tools-panel__tool ${
              selectedTool === tool.id ? "trust-button--active" : ""
            }`}
            draggable
            onDragStart={(event) => handleToolDragStart(event, tool.id)}
            onClick={() => onToolSelect(selectedTool === tool.id ? null : tool.id)}
            title={tool.title}
          >
            {tool.label}
          </button>
        ))}
      </div>
      <div className="trust-section__title" style={{ marginTop: 10 }}>Rungs</div>
      <div className="trust-button-grid">
        <button
          type="button"
          className="trust-button"
          onClick={onAddRung}
          title="Add new rung"
        >
          Add Rung
        </button>
        <button
          type="button"
          className="trust-button"
          onClick={onRemoveRung}
          title="Remove selected rung"
          disabled={!canRemoveRung}
        >
          Remove Rung
        </button>
      </div>
      <details className="ladder-tools-panel__details">
        <summary>More tools</summary>
        <div className="trust-section__title">Selection</div>
        <div className="trust-button-grid">
          <button
            type="button"
            className="trust-button"
            onClick={onDeleteSelection}
            disabled={!canDeleteSelection}
            title="Delete selected element"
          >
            Delete
          </button>
          <button
            type="button"
            className="trust-button"
            onClick={onCopy}
            title="Copy selected element or active rung"
          >
            Copy
          </button>
          <button
            type="button"
            className="trust-button"
            onClick={onPaste}
            disabled={!canPaste}
            title="Paste copied element or rung"
          >
            Paste
          </button>
          <button
            type="button"
            className="trust-button"
            onClick={onAddParallelContact}
            disabled={!canAddParallelContact}
            title="Auto-create a parallel branch from selected contact"
          >
            Parallel
          </button>
        </div>

        <div className="trust-section__title" style={{ marginTop: 10 }}>Topology</div>
        <div className="trust-button-grid">
          {TOPOLOGY_TOOL_OPTIONS.map((tool) => (
            <button
              key={tool.id}
              type="button"
              className={`trust-button ladder-tools-panel__tool ${
                selectedTool === tool.id ? "trust-button--active" : ""
              }`}
              draggable
              onDragStart={(event) => handleToolDragStart(event, tool.id)}
              onClick={() => onToolSelect(selectedTool === tool.id ? null : tool.id)}
              title={tool.title}
            >
              {tool.label}
            </button>
          ))}
        </div>
        <button
          type="button"
          className={`trust-button ${linkModeEnabled ? "trust-button--active" : ""}`}
          onClick={onToggleLinkMode}
          title="Wire mode: click source then target, or drag from source and release on target"
          style={{ width: "100%", marginTop: 7 }}
        >
          {linkModeEnabled ? "Wire Mode: On" : "Wire Mode"}
        </button>
        {linkModeEnabled && (
          <div className="trust-help" style={{ marginTop: 6 }}>
            {linkSourceLabel
              ? `Source: ${linkSourceLabel}. Click/drag to target node.`
              : "Click a source node, then click or drag to the target node."}
          </div>
        )}
        {linkModeEnabled && linkFeedback && (
          <div className="trust-help" style={{ marginTop: 6 }}>{linkFeedback}</div>
        )}
        <button
          type="button"
          className="trust-button"
          onClick={onClearWiring}
          disabled={!canClearWiring}
          title="Remove explicit wiring from active rung"
          style={{ width: "100%", marginTop: 7 }}
        >
          Clear Wiring
        </button>

        <div className="trust-section__title" style={{ marginTop: 10 }}>Edit</div>
        <div className="trust-button-grid">
          <button
            type="button"
            className="trust-button"
            onClick={onUndo}
            disabled={!canUndo}
            title="Undo (Ctrl/Cmd+Z)"
          >
            Undo
          </button>
          <button
            type="button"
            className="trust-button"
            onClick={onRedo}
            disabled={!canRedo}
            title="Redo (Ctrl/Cmd+Y or Shift+Ctrl/Cmd+Z)"
          >
            Redo
          </button>
          <button
            type="button"
            className="trust-button"
            onClick={onSearchReplace}
            title="Search and replace ladder symbols"
          >
            Replace
          </button>
          <button
            type="button"
            className="trust-button"
            onClick={onAutoRoute}
            title="Auto-route rung wiring"
          >
            Auto-route
          </button>
        </div>
      </details>
    </section>
  );
}
