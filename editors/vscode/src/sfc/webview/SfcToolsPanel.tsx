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
    <section className="trust-section" aria-label="SFC tools">
      <div className="trust-section__title">Tools</div>
      <p className="trust-help">Drag tools into the canvas or click to add.</p>
      <p className="trust-help">Select a transition and press Delete/Backspace to remove it.</p>
      
      {/* Add Elements */}
      <div className="trust-button-grid" style={{ marginTop: 10 }}>
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

      {/* Actions */}
      <div className="trust-button-grid" style={{ marginTop: 7 }}>
        <button type="button" className="trust-button" onClick={onValidate} title="Validate SFC">
          Validate
        </button>
        <button type="button" className="trust-button" onClick={onGenerateST} title="Generate ST code">
          Generate
        </button>
        {onToggleCodePanel && (
          <button
            type="button"
            className={showCodePanel ? "trust-button trust-button--active" : "trust-button"}
            onClick={onToggleCodePanel}
            title={showCodePanel ? "Hide code panel" : "Show code panel"}
          >
            {showCodePanel ? "Hide Code" : "Show Code"}
          </button>
        )}
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

      {/* Save */}
      <div className="trust-button-grid trust-button-grid--single" style={{ marginTop: 7 }}>
        <button type="button" className="trust-button trust-button--primary" onClick={onSave} title="Save changes">
        Save
        </button>
      </div>
    </section>
  );
};
