import React from "react";

interface StatechartToolsPanelProps {
  canDelete: boolean;
  onAddState: () => void;
  onAddInitialState: () => void;
  onAddFinalState: () => void;
  onDelete: () => void;
  onAutoLayout: () => void;
  onSave: () => void;
}

export const StatechartToolsPanel: React.FC<StatechartToolsPanelProps> = ({
  canDelete,
  onAddState,
  onAddInitialState,
  onAddFinalState,
  onDelete,
  onAutoLayout,
  onSave,
}) => {
  return (
    <section className="trust-section" aria-label="Statechart tools">
      <div className="trust-section__title">Tools</div>
      <div className="trust-button-grid">
        <button type="button" className="trust-button" onClick={onAddState}>
          Add State
        </button>
        <button type="button" className="trust-button" onClick={onAddInitialState}>
          Add Initial
        </button>
        <button type="button" className="trust-button" onClick={onAddFinalState}>
          Add Final
        </button>
        <button type="button" className="trust-button" onClick={onAutoLayout}>
          Auto Layout
        </button>
      </div>
      <div className="trust-button-grid" style={{ marginTop: 7 }}>
        <button
          type="button"
          className="trust-button trust-button--danger"
          onClick={onDelete}
          disabled={!canDelete}
        >
          Delete
        </button>
        <button type="button" className="trust-button trust-button--primary" onClick={onSave}>
          Save
        </button>
      </div>
    </section>
  );
};
