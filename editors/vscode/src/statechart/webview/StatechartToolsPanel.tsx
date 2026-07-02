import React from "react";

interface StatechartToolsPanelProps {
  canDelete: boolean;
  onAddState: () => void;
  onAddInitialState: () => void;
  onAddFinalState: () => void;
  onDelete: () => void;
  onAutoLayout: () => void;
  onValidate: () => void;
  onGenerateST: () => void;
  onSave: () => void;
}

export const StatechartToolsPanel: React.FC<StatechartToolsPanelProps> = ({
  canDelete,
  onAddState,
  onAddInitialState,
  onAddFinalState,
  onDelete,
  onAutoLayout,
  onValidate,
  onGenerateST,
  onSave,
}) => {
  return (
    <section className="trust-section" aria-label="Statechart tools">
      <div className="trust-section__title">Tools</div>
      <div className="trust-button-grid">
        <button type="button" className="trust-button" onClick={onValidate} title="Validate statechart">
          Validate
        </button>
        <button type="button" className="trust-button" onClick={onGenerateST} title="Generate Structured Text companion">
          Generate ST
        </button>
        <button type="button" className="trust-button trust-button--primary" onClick={onSave} title="Save statechart">
          Save
        </button>
      </div>
      <div className="trust-section__title" style={{ marginTop: 10 }}>Edit tools</div>
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
      </div>
    </section>
  );
};
