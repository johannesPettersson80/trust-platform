import React from "react";

interface ToolbarProps {
  selectedTool: string | null;
  onToolSelect: (tool: string | null) => void;
  onAddRung: () => void;
  onRemoveRung: () => void;
  canRemoveRung: boolean;
  onSave: () => void;
  onAutoRoute: () => void;
  onSearchReplace: () => void;
  onUndo: () => void;
  onRedo: () => void;
  onCopy: () => void;
  onPaste: () => void;
  canUndo: boolean;
  canRedo: boolean;
  canPaste: boolean;
}

export function Toolbar({
  selectedTool,
  onToolSelect,
  onAddRung,
  onRemoveRung,
  canRemoveRung,
  onSave,
  onAutoRoute,
  onSearchReplace,
  onUndo,
  onRedo,
  onCopy,
  onPaste,
  canUndo,
  canRedo,
  canPaste,
}: ToolbarProps) {
  return (
    <section className="trust-section" aria-label="Ladder tools">
      <div className="trust-section__title">Elements</div>
      <div className="trust-button-grid">
        <button
          className={`trust-button ${selectedTool === 'contact' ? 'trust-button--active' : ''}`}
          onClick={() => onToolSelect(selectedTool === 'contact' ? null : 'contact')}
          title="Add Contact (NO/NC)"
        >
          Contact
        </button>
        <button
          className={`trust-button ${selectedTool === 'coil' ? 'trust-button--active' : ''}`}
          onClick={() => onToolSelect(selectedTool === 'coil' ? null : 'coil')}
          title="Add Coil"
        >
          Coil
        </button>
        <button
          className={`trust-button ${selectedTool === 'timer' ? 'trust-button--active' : ''}`}
          onClick={() => onToolSelect(selectedTool === 'timer' ? null : 'timer')}
          title="Add Timer"
        >
          Timer
        </button>
        <button
          className={`trust-button ${selectedTool === 'counter' ? 'trust-button--active' : ''}`}
          onClick={() => onToolSelect(selectedTool === 'counter' ? null : 'counter')}
          title="Add Counter"
        >
          Counter
        </button>
        <button
          className={`trust-button ${selectedTool === 'compare' ? 'trust-button--active' : ''}`}
          onClick={() => onToolSelect(selectedTool === 'compare' ? null : 'compare')}
          title="Add Comparator"
        >
          Compare
        </button>
        <button
          className={`trust-button ${selectedTool === 'math' ? 'trust-button--active' : ''}`}
          onClick={() => onToolSelect(selectedTool === 'math' ? null : 'math')}
          title="Add Math Block"
        >
          Math
        </button>
      </div>

      <div className="trust-section__title" style={{ marginTop: 10 }}>Rungs</div>
      <div className="trust-button-grid">
        <button
          className="trust-button"
          onClick={onAddRung}
          title="Add new rung"
        >
          Add Rung
        </button>
        <button
          className="trust-button"
          onClick={onRemoveRung}
          title="Remove selected rung"
          disabled={!canRemoveRung}
        >
          Remove Rung
        </button>
      </div>

      <div className="trust-section__title" style={{ marginTop: 10 }}>Edit</div>
      <div className="trust-button-grid">
        <button
          className="trust-button"
          onClick={onUndo}
          disabled={!canUndo}
          title="Undo (Ctrl/Cmd+Z)"
        >
          Undo
        </button>
        <button
          className="trust-button"
          onClick={onRedo}
          disabled={!canRedo}
          title="Redo (Ctrl/Cmd+Y or Shift+Ctrl/Cmd+Z)"
        >
          Redo
        </button>
        <button
          className="trust-button"
          onClick={onCopy}
          title="Copy selected element or active rung (Ctrl/Cmd+C)"
        >
          Copy
        </button>
        <button
          className="trust-button"
          onClick={onPaste}
          disabled={!canPaste}
          title="Paste copied element/rung (Ctrl/Cmd+V)"
        >
          Paste
        </button>
        <button
          className="trust-button"
          onClick={onSearchReplace}
          title="Search/replace ladder symbols"
        >
          Replace
        </button>
        <button
          className="trust-button"
          onClick={onAutoRoute}
          title="Auto-route rung wires"
        >
          Auto-route
        </button>
        <button
          className="trust-button trust-button--primary"
          onClick={onSave}
          title="Save program"
        >
          Save
        </button>
      </div>
    </section>
  );
}
