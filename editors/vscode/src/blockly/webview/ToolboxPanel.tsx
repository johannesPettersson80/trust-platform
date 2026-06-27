import React from "react";

/**
 * Toolbox Panel - Shows available Blockly blocks organized by category
 */
export const ToolboxPanel: React.FC = () => {
  const categories = [
    { name: "Logic", icon: "🔀", color: "var(--trust-block-logic)" },
    { name: "Loops", icon: "🔁", color: "var(--trust-block-loop)" },
    { name: "Math", icon: "➕", color: "var(--trust-block-math)" },
    { name: "Variables", icon: "📦", color: "var(--trust-block-variables)" },
    { name: "Functions", icon: "⚙️", color: "var(--trust-block-functions)" },
    { name: "PLC I/O", icon: "🔌", color: "var(--trust-block-io)" },
    { name: "PLC Timers", icon: "⏱️", color: "var(--trust-block-timer)" },
    { name: "PLC Counters", icon: "🔢", color: "var(--trust-block-counter)" },
  ];

  return (
    <div className="toolbox-panel">
      <div className="toolbox-header">
        <h3>Blocks</h3>
      </div>
      <div className="toolbox-categories">
        {categories.map((category) => (
          <div
            key={category.name}
            className="toolbox-category"
            style={{ borderLeftColor: category.color }}
          >
            <span className="category-icon">{category.icon}</span>
            <span className="category-name">{category.name}</span>
          </div>
        ))}
      </div>
      <div className="toolbox-footer">
        <p className="toolbox-hint">
          Drag blocks to workspace
        </p>
      </div>
    </div>
  );
};
