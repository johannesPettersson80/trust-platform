import React, { useState } from "react";
import type {
  ParallelNodeData,
  SfcNode,
  SfcParallelNode,
  SfcStepNode,
  SfcTransitionEdge,
  SfcAction,
  ActionQualifier,
  SfcVariable,
  TransitionData,
} from "./types";

interface PropertiesPanelProps {
  selectedNode: SfcNode | null;
  selectedEdge: SfcTransitionEdge | null;
  variables: SfcVariable[] | undefined;
  onUpdateStepNode: (nodeId: string, updates: Partial<SfcStepNode["data"]>) => void;
  onUpdateParallelNode: (
    nodeId: string,
    updates: Partial<ParallelNodeData>
  ) => void;
  onUpdateEdge: (edgeId: string, updates: Partial<TransitionData>) => void;
  onAddAction: (stepId: string, action: SfcAction) => void;
  onUpdateAction: (
    stepId: string,
    actionId: string,
    updates: Partial<SfcAction>
  ) => void;
  onDeleteAction: (stepId: string, actionId: string) => void;
  onUpdateVariables: (variables: SfcVariable[]) => void;
  onClose: () => void;
}

const MIN_PARALLEL_BRANCHES = 2;

function isSfcStepNode(node: SfcNode): node is SfcStepNode {
  return node.type === "step";
}

function isSfcParallelNode(node: SfcNode): node is SfcParallelNode {
  return node.type === "parallelSplit" || node.type === "parallelJoin";
}

const ACTION_QUALIFIERS: ActionQualifier[] = [
  "N", "S", "R", "L", "D", "P", "SD", "DS", "SL",
];

const QUALIFIER_DESCRIPTIONS: Record<ActionQualifier, string> = {
  N: "Non-stored (Normal)",
  S: "Set (Stored)",
  R: "Reset",
  L: "Time Limited",
  D: "Time Delayed",
  P: "Pulse",
  SD: "Stored and Delayed",
  DS: "Delayed and Stored",
  SL: "Stored and Limited",
};

/**
 * Properties panel for editing selected step or transition
 */
export const PropertiesPanel: React.FC<PropertiesPanelProps> = ({
  selectedNode,
  selectedEdge,
  variables,
  onUpdateStepNode,
  onUpdateParallelNode,
  onUpdateEdge,
  onAddAction,
  onUpdateAction,
  onDeleteAction,
  onUpdateVariables,
  onClose,
}) => {
  const [newActionName, setNewActionName] = useState("");
  const [newActionQualifier, setNewActionQualifier] =
    useState<ActionQualifier>("N");
  const selectedStepNode =
    selectedNode && isSfcStepNode(selectedNode) ? selectedNode : null;
  const selectedParallelNode =
    selectedNode && isSfcParallelNode(selectedNode) ? selectedNode : null;

  const panelStyle: React.CSSProperties = {
    width: "100%",
    backgroundColor: "transparent",
    borderTop: "1px solid var(--trust-border)",
    overflowY: "auto",
    padding: "12px 14px",
    fontFamily: "var(--vscode-font-family)",
    fontSize: "12px",
    color: "var(--trust-text)",
    boxSizing: "border-box",
  };

  const inputStyle: React.CSSProperties = {
    width: "100%",
    padding: "6px 8px",
    backgroundColor: "var(--trust-input-bg)",
    color: "var(--vscode-input-foreground, var(--trust-text))",
    border: "1px solid var(--trust-input-border)",
    borderRadius: "var(--trust-radius)",
    fontSize: "12px",
    fontFamily: "var(--vscode-font-family)",
  };

  const buttonStyle: React.CSSProperties = {
    padding: "7px 10px",
    backgroundColor: "var(--trust-accent)",
    color: "var(--trust-on-accent)",
    border: "1px solid var(--trust-accent)",
    borderRadius: "var(--trust-radius)",
    cursor: "pointer",
    fontSize: "12px",
    fontWeight: 650,
    marginTop: "8px",
  };

  const handleAddAction = () => {
    if (selectedStepNode && newActionName.trim()) {
      const action: SfcAction = {
        id: `action_${Date.now()}`,
        name: newActionName.trim(),
        qualifier: newActionQualifier,
        body: "",
      };
      onAddAction(selectedStepNode.id, action);
      setNewActionName("");
    }
  };

  return (
    <div style={panelStyle}>
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
          marginBottom: "16px",
        }}
      >
        <h3 style={{ margin: 0, fontSize: "13px", fontWeight: 700 }}>
          Properties
        </h3>
        <button
          onClick={onClose}
          style={{
            ...buttonStyle,
            padding: "4px 8px",
            marginTop: 0,
          }}
        >
          ✕
        </button>
      </div>

      {selectedStepNode && (
        <div>
          <h4 style={{ marginTop: 0, marginBottom: "12px", fontSize: "12px", color: "var(--trust-text-muted)" }}>
            Step: {selectedStepNode.data.label}
          </h4>

          <div style={{ marginBottom: "16px" }}>
            <label style={{ display: "block", marginBottom: "4px" }}>
              Name:
            </label>
            <input
              type="text"
              style={inputStyle}
              value={selectedStepNode.data.label}
              onChange={(e) =>
                onUpdateStepNode(selectedStepNode.id, { label: e.target.value })
              }
            />
          </div>

          <div style={{ marginBottom: "16px" }}>
            <label style={{ display: "block", marginBottom: "4px" }}>
              Type:
            </label>
            <select
              style={inputStyle}
              value={selectedStepNode.data.type}
              onChange={(e) =>
                onUpdateStepNode(selectedStepNode.id, {
                  type: e.target.value as "normal" | "initial" | "final",
                })
              }
            >
              <option value="normal">Normal</option>
              <option value="initial">Initial</option>
              <option value="final">Final</option>
            </select>
          </div>

          <div style={{ marginBottom: "16px" }}>
            <label style={{ display: "block", marginBottom: "4px" }}>
              Description:
            </label>
            <textarea
              style={{ ...inputStyle, minHeight: "60px" }}
              value={selectedStepNode.data.description || ""}
              onChange={(e) =>
                onUpdateStepNode(selectedStepNode.id, { description: e.target.value })
              }
              placeholder="Optional description..."
            />
          </div>

          <div style={{ marginTop: "20px" }}>
            <h4 style={{ marginBottom: "12px" }}>Actions</h4>

            {selectedStepNode.data.actions && selectedStepNode.data.actions.length > 0 ? (
              <div style={{ marginBottom: "12px" }}>
                {selectedStepNode.data.actions.map((action) => (
                  <div
                    key={action.id}
                    className="trust-card"
                    style={{ marginBottom: "8px" }}
                  >
                    <div
                      style={{
                        display: "flex",
                        justifyContent: "space-between",
                        alignItems: "center",
                        marginBottom: "8px",
                      }}
                    >
                      <strong>{action.name}</strong>
                      <button
                        onClick={() =>
                          onDeleteAction(selectedStepNode.id, action.id)
                        }
                        style={{
                          ...buttonStyle,
                          marginTop: 0,
                          padding: "2px 6px",
                          fontSize: "11px",
                        }}
                      >
                        Delete
                      </button>
                    </div>

                    <div style={{ marginBottom: "8px" }}>
                      <label
                        style={{ display: "block", marginBottom: "4px", fontSize: "11px" }}
                      >
                        Qualifier: {QUALIFIER_DESCRIPTIONS[action.qualifier]}
                      </label>
                      <select
                        style={{ ...inputStyle, fontSize: "12px" }}
                        value={action.qualifier}
                        onChange={(e) =>
                          onUpdateAction(selectedStepNode.id, action.id, {
                            qualifier: e.target.value as ActionQualifier,
                          })
                        }
                      >
                        {ACTION_QUALIFIERS.map((q) => (
                          <option key={q} value={q}>
                            {q} - {QUALIFIER_DESCRIPTIONS[q]}
                          </option>
                        ))}
                      </select>
                    </div>

                    <div>
                      <label
                        style={{ display: "block", marginBottom: "4px", fontSize: "11px" }}
                      >
                        Code:
                      </label>
                      <textarea
                        style={{ ...inputStyle, minHeight: "50px", fontSize: "12px" }}
                        value={action.body}
                        onChange={(e) =>
                          onUpdateAction(selectedStepNode.id, action.id, {
                            body: e.target.value,
                          })
                        }
                        placeholder="Action code..."
                      />
                    </div>
                  </div>
                ))}
              </div>
            ) : (
              <p style={{ fontSize: "12px", fontStyle: "italic", opacity: 0.7 }}>
                No actions defined
              </p>
            )}

            <div
              style={{
                borderTop: "1px solid var(--trust-border)",
                paddingTop: "12px",
              }}
            >
              <label style={{ display: "block", marginBottom: "4px", fontSize: "12px" }}>
                Add Action:
              </label>
              <input
                type="text"
                style={{ ...inputStyle, marginBottom: "8px" }}
                value={newActionName}
                onChange={(e) => setNewActionName(e.target.value)}
                placeholder="Action name..."
              />
              <select
                style={inputStyle}
                value={newActionQualifier}
                onChange={(e) =>
                  setNewActionQualifier(e.target.value as ActionQualifier)
                }
              >
                {ACTION_QUALIFIERS.map((q) => (
                  <option key={q} value={q}>
                    {q} - {QUALIFIER_DESCRIPTIONS[q]}
                  </option>
                ))}
              </select>
              <button style={buttonStyle} onClick={handleAddAction}>
                Add Action
              </button>
            </div>
          </div>
        </div>
      )}

      {selectedParallelNode && (
        <div>
          <h4 style={{ marginTop: 0, marginBottom: "12px" }}>
            {selectedParallelNode.type === "parallelSplit"
              ? "Parallel Split"
              : "Parallel Join"}
          </h4>

          <div style={{ marginBottom: "16px" }}>
            <label style={{ display: "block", marginBottom: "4px" }}>
              Name:
            </label>
            <input
              type="text"
              style={inputStyle}
              value={selectedParallelNode.data.label}
              onChange={(e) =>
                onUpdateParallelNode(selectedParallelNode.id, {
                  label: e.target.value,
                })
              }
            />
          </div>

          <div style={{ marginBottom: "16px" }}>
            <label style={{ display: "block", marginBottom: "4px" }}>
              Branches (IEC parallel requires at least 2):
            </label>
            <input
              type="number"
              min={MIN_PARALLEL_BRANCHES}
              step={1}
              style={inputStyle}
              value={Math.max(
                MIN_PARALLEL_BRANCHES,
                Math.floor(selectedParallelNode.data.branchCount ?? MIN_PARALLEL_BRANCHES)
              )}
              onChange={(e) =>
                onUpdateParallelNode(selectedParallelNode.id, {
                  branchCount: Math.max(
                    MIN_PARALLEL_BRANCHES,
                    Number.parseInt(e.target.value || "0", 10) ||
                      MIN_PARALLEL_BRANCHES
                  ),
                })
              }
            />
            <div style={{ marginTop: "8px", fontSize: "11px", opacity: 0.8 }}>
              Split exposes one input and one output handle per branch. Join exposes one
              input handle per branch and one output.
            </div>
          </div>
        </div>
      )}

      {selectedEdge && (
        <div>
          <h4 style={{ marginTop: 0, marginBottom: "12px" }}>Transition</h4>

          <div style={{ marginBottom: "16px" }}>
            <label style={{ display: "block", marginBottom: "4px" }}>
              Label:
            </label>
            <input
              type="text"
              style={inputStyle}
              value={selectedEdge.data?.label || ""}
              onChange={(e) =>
                onUpdateEdge(selectedEdge.id, { label: e.target.value })
              }
            />
          </div>

          <div style={{ marginBottom: "16px" }}>
            <label style={{ display: "block", marginBottom: "4px" }}>
              Condition:
            </label>
            <textarea
              style={{ ...inputStyle, minHeight: "80px" }}
              value={selectedEdge.data?.condition || ""}
              onChange={(e) =>
                onUpdateEdge(selectedEdge.id, {
                  condition: e.target.value,
                  label: e.target.value,
                })
              }
              placeholder="e.g., sensor1 = TRUE"
            />
          </div>

          <div style={{ marginBottom: "16px" }}>
            <label style={{ display: "block", marginBottom: "4px" }}>
              Description:
            </label>
            <textarea
              style={{ ...inputStyle, minHeight: "60px" }}
              value={selectedEdge.data?.description || ""}
              onChange={(e) =>
                onUpdateEdge(selectedEdge.id, { description: e.target.value })
              }
              placeholder="Optional description..."
            />
          </div>

          <div style={{ fontSize: "11px", opacity: 0.7, marginTop: "16px" }}>
            <div>From: <strong>{selectedEdge.source}</strong></div>
            <div>To: <strong>{selectedEdge.target}</strong></div>
          </div>
        </div>
      )}
    </div>
  );
};
