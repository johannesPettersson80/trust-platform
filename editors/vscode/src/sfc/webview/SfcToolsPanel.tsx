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
  onFitView: () => void;
  onSave: () => void;
  onToggleCodePanel?: () => void;
  showCodePanel?: boolean;
  hasSelection: boolean;
}

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
  onFitView,
  onSave,
  onToggleCodePanel,
  showCodePanel = false,
  hasSelection,
}) => {
  return (
    <section className="trust-section" aria-label="SFC tools">
      <div className="trust-section__title">Tools</div>
      <p className="trust-help">Drag tools into the canvas or click to add.</p>
      <p className="trust-help">Select a transition and press Delete/Backspace to remove it.</p>

      {/* Actions */}
      <div className="trust-button-grid" style={{ marginTop: 10 }}>
        <button type="button" className="trust-button" onClick={onValidate} title="Validate SFC">
          Validate
        </button>
        <button type="button" className="trust-button" onClick={onGenerateST} title="Write generated ST companion file and open it beside the editor">
          Generate ST
        </button>
        <button type="button" className="trust-button trust-button--primary" onClick={onSave} title="Save changes">
          Save
        </button>
      </div>

      <div className="trust-section__title" style={{ marginTop: 10 }}>Edit</div>
      <div className="trust-button-grid">
        <button
          type="button"
          className="trust-button"
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
            className="trust-button"
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
            className="trust-button"
            onClick={onAddParallelJoin}
            title="Add parallel join"
            draggable={Boolean(onToolDragStart)}
            onDragStart={(event) => onToolDragStart?.(event, "parallelJoin")}
          >
            Join
          </button>
        )}
        <button type="button" className="trust-button" onClick={onAutoLayout} title="Auto arrange steps">
          Layout
        </button>
      </div>

      <div className="trust-button-grid" style={{ marginTop: 7 }}>
        <button
          type="button"
          className="trust-button trust-button--danger"
          onClick={onDelete}
          disabled={!hasSelection}
          title="Delete selected element"
        >
          Delete
        </button>
      </div>

      <div className="trust-section__title" style={{ marginTop: 10 }}>View</div>
      <div className="trust-button-grid">
        <button type="button" className="trust-button" onClick={onFitView} title="Fit the full SFC diagram in the canvas">
          Fit View
        </button>
        {onToggleCodePanel && (
          <button
            type="button"
            className={showCodePanel ? "trust-button trust-button--active" : "trust-button"}
            onClick={onToggleCodePanel}
            title={showCodePanel ? "Hide generated ST preview" : "Preview generated ST without saving the companion file"}
          >
            {showCodePanel ? "Hide Preview" : "Preview ST"}
          </button>
        )}
      </div>
    </section>
  );
};
