import React from "react";
import { StateChartNode, StateChartEdge, StateType } from "./types";

interface PropertiesPanelProps {
  selectedNode?: StateChartNode | null;
  selectedEdge?: StateChartEdge | null;
  onUpdateNode: (id: string, data: Partial<StateChartNode["data"]>) => void;
  onUpdateEdge: (id: string, data: Partial<StateChartEdge["data"]>) => void;
}

export const PropertiesPanel: React.FC<PropertiesPanelProps> = ({
  selectedNode,
  selectedEdge,
  onUpdateNode,
  onUpdateEdge,
}) => {
  const [isCollapsed, setIsCollapsed] = React.useState(false);

  const hasSelection = selectedNode || selectedEdge;

  return (
    <div
      style={{
        borderTop: "1px solid var(--trust-border)",
        borderBottom: "1px solid var(--trust-border)",
        display: 'flex',
        flexDirection: 'column',
        height: isCollapsed ? 'auto' : '250px',
        minHeight: '40px',
        overflow: isCollapsed ? 'visible' : 'auto',
      }}
    >
      {/* Collapsible Header */}
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          padding: "10px 14px",
          backgroundColor: "transparent",
          borderBottom: isCollapsed ? "none" : "1px solid var(--trust-border)",
          cursor: 'pointer',
          userSelect: 'none',
        }}
        onClick={() => setIsCollapsed(!isCollapsed)}
      >
        <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
          <span
            style={{
              transform: isCollapsed ? 'rotate(-90deg)' : 'rotate(0deg)',
              transition: 'transform 0.2s',
            }}
          >
            ▼
          </span>
          <span style={{ fontWeight: 600 }}>Properties</span>
          {hasSelection && (
            <span
              style={{
                fontSize: '11px',
                color: "var(--trust-text-muted)",
              }}
            >
              {selectedNode ? '● Node' : '● Edge'}
            </span>
          )}
        </div>
      </div>

      {/* Panel Content */}
      {!isCollapsed && (
        <div style={{ padding: "12px 14px", flex: 1, overflow: "auto" }}>
          {!hasSelection ? (
            <div
              style={{
                display: "flex",
                height: "100%",
                alignItems: "center",
                justifyContent: "center",
                color: "var(--trust-text-muted)",
                fontSize: '12px',
              }}
            >
              Select a node or edge to edit properties
            </div>
          ) : selectedNode ? (
            <NodeProperties node={selectedNode} onUpdate={onUpdateNode} />
          ) : selectedEdge ? (
            <EdgeProperties edge={selectedEdge} onUpdate={onUpdateEdge} />
          ) : null}
        </div>
      )}
    </div>
  );
};

const NodeProperties: React.FC<{
  node: StateChartNode;
  onUpdate: (id: string, data: Partial<StateChartNode["data"]>) => void;
}> = ({ node, onUpdate }) => {
  const handleLabelChange = (label: string) => {
    onUpdate(node.id, { label });
  };

  const handleTypeChange = (type: StateType) => {
    onUpdate(node.id, { type });
  };

  const handleDescriptionChange = (description: string) => {
    onUpdate(node.id, { description });
  };

  const handleArrayChange = (
    field: "entry" | "exit",
    index: number,
    value: string
  ) => {
    const current = node.data[field] || [];
    const updated = [...current];
    updated[index] = value;
    onUpdate(node.id, { [field]: updated });
  };

  const handleAddToArray = (field: "entry" | "exit") => {
    const current = node.data[field] || [];
    onUpdate(node.id, { [field]: [...current, ""] });
  };

  const handleRemoveFromArray = (field: "entry" | "exit", index: number) => {
    const current = node.data[field] || [];
    onUpdate(node.id, { [field]: current.filter((_, i) => i !== index) });
  };

  return (
    <div style={{ padding: "16px", overflow: "auto", height: "100%" }}>
      <div style={cardStyle}>
        <h3 style={headerStyle}>State Properties</h3>
        <div style={{ display: "flex", flexDirection: "column", gap: "16px" }}>
          <div style={fieldStyle}>
            <label style={labelStyle}>Label</label>
            <input
              style={inputStyle}
              value={node.data.label}
              onChange={(e) => handleLabelChange(e.target.value)}
            />
          </div>

          <div style={fieldStyle}>
            <label style={labelStyle}>Type</label>
            <select
              style={selectStyle}
              value={node.data.type}
              onChange={(e) => handleTypeChange(e.target.value as StateType)}
            >
              <option value="normal">Normal</option>
              <option value="initial">Initial</option>
              <option value="final">Final</option>
              <option value="compound">Compound</option>
            </select>
          </div>

          <div style={fieldStyle}>
            <label style={labelStyle}>Description</label>
            <textarea
              style={textareaStyle}
              value={node.data.description || ""}
              onChange={(e) => handleDescriptionChange(e.target.value)}
              placeholder="State description..."
              rows={3}
            />
          </div>

          <ActionArrayEditor
            label="Entry Actions"
            items={node.data.entry || []}
            onChange={(index, value) => handleArrayChange("entry", index, value)}
            onAdd={() => handleAddToArray("entry")}
            onRemove={(index) => handleRemoveFromArray("entry", index)}
          />

          <ActionArrayEditor
            label="Exit Actions"
            items={node.data.exit || []}
            onChange={(index, value) => handleArrayChange("exit", index, value)}
            onAdd={() => handleAddToArray("exit")}
            onRemove={(index) => handleRemoveFromArray("exit", index)}
          />
        </div>
      </div>
    </div>
  );
};

const EdgeProperties: React.FC<{
  edge: StateChartEdge;
  onUpdate: (id: string, data: Partial<StateChartEdge["data"]>) => void;
}> = ({ edge, onUpdate }) => {
  const data = edge.data || {};
  
  // Local state for the timer input to allow free typing
  const [timerValue, setTimerValue] = React.useState<string>(
    data.after !== undefined ? String(data.after) : ""
  );

  // Update local state when edge changes (e.g., different edge selected)
  React.useEffect(() => {
    setTimerValue(data.after !== undefined ? String(data.after) : "");
  }, [edge.id, data.after]);

  const handleEventChange = (event: string) => {
    onUpdate(edge.id, { ...data, event });
  };

  const handleGuardChange = (guard: string) => {
    onUpdate(edge.id, { ...data, guard });
  };

  const handleAfterChange = (value: string) => {
    // Update local state immediately for responsive UI
    setTimerValue(value);
    
    // Update edge data with parsed value
    if (value === "" || value === null || value === undefined) {
      onUpdate(edge.id, { ...data, after: undefined });
    } else {
      const parsed = parseInt(value, 10);
      if (!isNaN(parsed) && parsed >= 0) {
        onUpdate(edge.id, { ...data, after: parsed });
      }
    }
  };

  const handleDescriptionChange = (description: string) => {
    onUpdate(edge.id, { ...data, description });
  };

  const handleActionChange = (index: number, value: string) => {
    const actions = [...(data.actions || [])];
    actions[index] = value;
    onUpdate(edge.id, { ...data, actions });
  };

  const handleAddAction = () => {
    onUpdate(edge.id, { ...data, actions: [...(data.actions || []), ""] });
  };

  const handleRemoveAction = (index: number) => {
    const actions = (data.actions || []).filter((_, i) => i !== index);
    onUpdate(edge.id, { ...data, actions });
  };

  return (
    <div style={{ padding: "16px", overflow: "auto", height: "100%" }}>
      <div style={cardStyle}>
        <h3 style={headerStyle}>Transition Properties</h3>
        <div style={{ display: "flex", flexDirection: "column", gap: "16px" }}>
          <div style={fieldStyle}>
            <label style={labelStyle}>Event</label>
            <input
              style={inputStyle}
              value={data.event || ""}
              onChange={(e) => handleEventChange(e.target.value)}
              placeholder="EVENT_NAME"
            />
          </div>

          <div style={fieldStyle}>
            <label style={labelStyle}>Guard</label>
            <input
              style={inputStyle}
              value={data.guard || ""}
              onChange={(e) => handleGuardChange(e.target.value)}
              placeholder="condition"
            />
          </div>

          <div style={fieldStyle}>
            <label style={labelStyle}>
              Auto-Transition Timer (ms)
              <span style={{ fontSize: "11px", color: "var(--trust-text-muted)", marginLeft: "8px" }}>
                optional
              </span>
            </label>
            <input
              type="number"
              style={inputStyle}
              value={timerValue}
              onChange={(e) => handleAfterChange(e.target.value)}
              placeholder="e.g., 1000 for 1 second"
              min="0"
              step="1"
            />
            <div style={{ fontSize: "11px", color: "var(--trust-text-muted)", marginTop: "4px" }}>
              Leave empty for manual events. Set delay for automatic transitions.
            </div>
          </div>

          <div style={fieldStyle}>
            <label style={labelStyle}>Description</label>
            <textarea
              style={textareaStyle}
              value={data.description || ""}
              onChange={(e) => handleDescriptionChange(e.target.value)}
              placeholder="Transition description..."
              rows={3}
            />
          </div>

          <ActionArrayEditor
            label="Actions"
            items={data.actions || []}
            onChange={handleActionChange}
            onAdd={handleAddAction}
            onRemove={handleRemoveAction}
          />
        </div>
      </div>
    </div>
  );
};

const ActionArrayEditor: React.FC<{
  label: string;
  items: string[];
  onChange: (index: number, value: string) => void;
  onAdd: () => void;
  onRemove: (index: number) => void;
}> = ({ label, items, onChange, onAdd, onRemove }) => {
  return (
    <div style={fieldStyle}>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
        <label style={labelStyle}>{label}</label>
        <button style={smallButtonStyle} onClick={onAdd} title="Add action">
          ➕
        </button>
      </div>
      <div style={{ display: "flex", flexDirection: "column", gap: "8px" }}>
        {items.length === 0 && (
          <div style={{ fontSize: "12px", color: "var(--trust-text-muted)" }}>
            No actions defined
          </div>
        )}
        {items.map((item, index) => (
          <div key={index} style={{ display: "flex", gap: "8px" }}>
            <input
              style={{ ...inputStyle, flex: 1 }}
              value={item}
              onChange={(e) => onChange(index, e.target.value)}
              placeholder="action name"
            />
            <button
              style={deleteButtonStyle}
              onClick={() => onRemove(index)}
              title="Remove action"
            >
              🗑️
            </button>
          </div>
        ))}
      </div>
    </div>
  );
};

// Styles
const cardStyle: React.CSSProperties = {
  padding: "12px",
  backgroundColor: "var(--trust-surface)",
  border: "1px solid var(--trust-border)",
  borderRadius: "var(--trust-radius-lg)",
};

const headerStyle: React.CSSProperties = {
  fontSize: "13px",
  fontWeight: "600",
  marginBottom: "12px",
  color: "var(--trust-text)",
};

const fieldStyle: React.CSSProperties = {
  display: "flex",
  flexDirection: "column",
  gap: "6px",
};

const labelStyle: React.CSSProperties = {
  fontSize: "11.5px",
  fontWeight: "500",
  color: "var(--trust-text-muted)",
};

const inputStyle: React.CSSProperties = {
  padding: "6px 8px",
  fontSize: "12px",
  backgroundColor: "var(--trust-input-bg)",
  color: "var(--vscode-input-foreground, var(--trust-text))",
  border: "1px solid var(--trust-input-border)",
  borderRadius: "var(--trust-radius)",
  outline: "none",
};

const selectStyle: React.CSSProperties = {
  ...inputStyle,
};

const textareaStyle: React.CSSProperties = {
  ...inputStyle,
  fontFamily: "var(--vscode-font-family)",
  resize: "vertical",
};

const smallButtonStyle: React.CSSProperties = {
  padding: "4px 8px",
  fontSize: "12px",
  backgroundColor: "transparent",
  color: "var(--trust-text)",
  border: "1px solid var(--trust-border)",
  borderRadius: "var(--trust-radius)",
  cursor: "pointer",
};

const deleteButtonStyle: React.CSSProperties = {
  padding: "6px 10px",
  fontSize: "12px",
  backgroundColor: "transparent",
  color: "var(--trust-danger)",
  border: "1px solid color-mix(in srgb, var(--trust-danger) 45%, transparent)",
  borderRadius: "var(--trust-radius)",
  cursor: "pointer",
};
