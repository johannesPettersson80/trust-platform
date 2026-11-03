import React, { useRef, useState } from "react";
import { ActionMapping, AskDialog, ReadValue } from "./types";

type ActionMappings = Record<string, ActionMapping>;

interface ActionMappingsPanelProps {
  actionMappings: ActionMappings;
  /** Actions the chart names that no mapping defines (the language server's action report). */
  unmappedActions: string[];
  /** Mappings no state or transition names (the language server's action report). */
  unusedMappings: string[];
  onUpdateActionMappings: (
    mappings: ActionMappings | ((current: ActionMappings) => ActionMappings)
  ) => void;
  /** Asks the language server for the value of typed text (a mapping's value). */
  readValue: ReadValue;
  /** Asks the host for a new mapping's name or a deletion's confirmation, in VS Code's dialogs. */
  askDialog: AskDialog;
}

export const ActionMappingsPanel: React.FC<ActionMappingsPanelProps> = ({
  actionMappings,
  unmappedActions,
  unusedMappings,
  onUpdateActionMappings,
  readValue,
  askDialog,
}) => {
  const [isCollapsed, setIsCollapsed] = useState(true);
  const [selectedAction, setSelectedAction] = useState<string | null>(null);

  // The host asks for the name in VS Code and the language server adds the mapping or says why not
  // (specification 25 §11.4).
  const handleAddMapping = async () => {
    const answer = await askDialog({ dialog: "addActionMapping", mappings: actionMappings });
    if (!answer.mappings || !answer.name) return;
    onUpdateActionMappings(answer.mappings);
    setSelectedAction(answer.name);
  };

  const handleDeleteMapping = async (name: string) => {
    const answer = await askDialog({ dialog: "deleteActionMapping", name });
    if (!answer.confirmed) return;
    onUpdateActionMappings((current) => {
      const { [name]: _, ...rest } = current;
      return rest;
    });
    if (selectedAction === name) {
      setSelectedAction(null);
    }
  };

  // Merged into the mappings as they are when the update lands: a value the language server
  // answers later does not undo another field's change.
  const handleUpdateMapping = (
    name: string,
    updates: Partial<ActionMapping>
  ) => {
    onUpdateActionMappings((current) => ({
      ...current,
      [name]: { ...current[name], ...updates },
    }));
  };

  return (
    <div
      style={{
        borderTop: "1px solid var(--trust-border)",
        display: "flex",
        flexDirection: "column",
        height: isCollapsed ? "auto" : "400px",
        minHeight: "40px",
      }}
    >
      {/* Header */}
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          padding: "10px 14px",
          backgroundColor: "transparent",
          borderBottom: isCollapsed
            ? "none"
            : "1px solid var(--trust-border)",
          cursor: "pointer",
          userSelect: "none",
        }}
        onClick={() => setIsCollapsed(!isCollapsed)}
      >
        <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
          <span
            style={{
              transform: isCollapsed ? "rotate(-90deg)" : "rotate(0deg)",
              transition: "transform 0.2s",
            }}
          >
            ▼
          </span>
          <span style={{ fontWeight: 600 }}>Action Mappings</span>
          {unmappedActions.length > 0 && (
            <span
              style={{
                backgroundColor: "var(--vscode-editorWarning-foreground)",
                color: "var(--trust-canvas)",
                padding: "2px 6px",
                borderRadius: "10px",
                fontSize: "11px",
                fontWeight: 600,
              }}
            >
              {unmappedActions.length}
            </span>
          )}
        </div>
        <button
          onClick={(e) => {
            e.stopPropagation();
            void handleAddMapping();
          }}
          style={{
            padding: "4px 8px",
            fontSize: "12px",
            cursor: "pointer",
            backgroundColor: "var(--trust-accent)",
            color: "var(--trust-on-accent)",
            border: "1px solid var(--trust-accent)",
            borderRadius: "var(--trust-radius)",
          }}
        >
          + Add
        </button>
      </div>

      {/* Content */}
      {!isCollapsed && (
        <div style={{ display: "flex", flex: 1, overflow: "hidden" }}>
          {/* List of mappings */}
          <div
            style={{
              width: "40%",
              borderRight: "1px solid var(--trust-border)",
              overflowY: "auto",
            }}
          >
            {/* Warnings for unmapped actions */}
            {unmappedActions.length > 0 && (
              <div
                style={{
                  padding: "8px",
                  backgroundColor: "var(--vscode-inputValidation-warningBackground)",
                  borderBottom: "1px solid var(--vscode-inputValidation-warningBorder)",
                  fontSize: "12px",
                }}
              >
                <div style={{ fontWeight: 600, marginBottom: "4px" }}>
                  ⚠️ Unmapped Actions
                </div>
                <div style={{ color: "var(--trust-text-muted)" }}>
                  The following actions are referenced but not mapped:
                </div>
                <ul style={{ margin: "4px 0", paddingLeft: "20px" }}>
                  {unmappedActions.map((action) => (
                    <li key={action}>
                      <code>{action}</code>
                    </li>
                  ))}
                </ul>
              </div>
            )}

            {/* Mapping list */}
            {Object.keys(actionMappings).length === 0 ? (
              <div
                style={{
                  padding: "16px",
                  textAlign: "center",
                  color: "var(--trust-text-muted)",
                  fontSize: "12px",
                }}
              >
                No action mappings defined.
                <br />
                Click "+ Add" to create one.
              </div>
            ) : (
              Object.entries(actionMappings).map(([name, mapping]) => {
                const isUnused = unusedMappings.includes(name);
                return (
                  <div
                    key={name}
                    onClick={() => setSelectedAction(name)}
                    style={{
                      padding: "8px 12px",
                      cursor: "pointer",
                      backgroundColor:
                        selectedAction === name
                          ? "var(--vscode-list-activeSelectionBackground)"
                          : "transparent",
                      color:
                        selectedAction === name
                          ? "var(--vscode-list-activeSelectionForeground)"
                          : isUnused
                          ? "var(--trust-text-muted)"
                          : "inherit",
                      borderBottom: "1px solid var(--trust-border)",
                      fontSize: "13px",
                    }}
                  >
                    <div style={{ fontWeight: 500 }}>{name}</div>
                    <div style={{ fontSize: "11px", opacity: 0.8 }}>
                      {mapping.action}
                      {mapping.address && ` → ${mapping.address}`}
                      {isUnused && " (unused)"}
                    </div>
                  </div>
                );
              })
            )}
          </div>

          {/* Mapping editor */}
          <div style={{ flex: 1, overflowY: "auto", padding: "12px" }}>
            {selectedAction && actionMappings[selectedAction] ? (
              <MappingEditor
                key={selectedAction}
                name={selectedAction}
                mapping={actionMappings[selectedAction]}
                onUpdate={(updates) =>
                  handleUpdateMapping(selectedAction, updates)
                }
                onDelete={() => void handleDeleteMapping(selectedAction)}
                readValue={readValue}
              />
            ) : (
              <div
                style={{
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "center",
                  height: "100%",
                  color: "var(--trust-text-muted)",
                  fontSize: "12px",
                }}
              >
                Select an action mapping to edit
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  );
};

/**
 * The value field of a mapping: the text as typed while it is typed, the stored value otherwise.
 * The language server reads each text (`mappingValue`: `true`/`false`, a number, or text) and the
 * mapping stores that value; only the answer to the latest text is stored.
 */
const MappingValueInput: React.FC<{
  stored: unknown;
  readValue: ReadValue;
  onValue: (value: unknown) => void;
}> = ({ stored, readValue, onValue }) => {
  const [draft, setDraft] = useState<string | null>(null);
  const request = useRef(0);
  return (
    <input
      type="text"
      value={draft ?? (stored === undefined || stored === null ? "" : String(stored))}
      onChange={(e) => {
        const text = e.target.value;
        setDraft(text);
        request.current += 1;
        const latest = request.current;
        void readValue("mappingValue", text).then((answer) => {
          if (latest === request.current && answer.ok) {
            onValue(answer.value);
          }
        });
      }}
      onBlur={() => setDraft(null)}
      placeholder="e.g., 42, true, 'hello'"
      style={{
        width: "100%",
        padding: "6px 8px",
        fontSize: "13px",
        backgroundColor: "var(--trust-input-bg)",
        color: "var(--vscode-input-foreground, var(--trust-text))",
        border: "1px solid var(--trust-input-border)",
        borderRadius: "var(--trust-radius)",
      }}
    />
  );
};

const MappingEditor: React.FC<{
  name: string;
  mapping: ActionMapping;
  onUpdate: (updates: Partial<ActionMapping>) => void;
  onDelete: () => void;
  readValue: ReadValue;
}> = ({ name, mapping, onUpdate, onDelete, readValue }) => {
  const actionTypes = [
    "WRITE_OUTPUT",
    "SET_MULTIPLE",
    "LOG",
    "WRITE_VARIABLE",
  ];

  // Generate address options for EL2008 (8 digital outputs)
  const addressOptions = Array.from({ length: 8 }, (_, i) => `%QX0.${i}`);

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "16px" }}>
      <div>
        <div
          style={{
            display: "flex",
            justifyContent: "space-between",
            alignItems: "center",
            marginBottom: "8px",
          }}
        >
          <label
            style={{
              display: "block",
              marginBottom: "4px",
              fontSize: "12px",
              fontWeight: 600,
            }}
          >
            Action Name
          </label>
          <button
            onClick={onDelete}
            style={{
              padding: "4px 8px",
              fontSize: "11px",
              cursor: "pointer",
              backgroundColor: "transparent",
              color: "var(--trust-danger)",
              border: "1px solid color-mix(in srgb, var(--trust-danger) 45%, transparent)",
              borderRadius: "var(--trust-radius)",
            }}
          >
            Delete
          </button>
        </div>
        <input
          type="text"
          value={name}
          readOnly
          style={{
            width: "100%",
            padding: "6px 8px",
            fontSize: "13px",
            backgroundColor: "var(--trust-input-bg)",
            color: "var(--vscode-input-foreground, var(--trust-text))",
            border: "1px solid var(--trust-input-border)",
            borderRadius: "var(--trust-radius)",
            opacity: 0.6,
            cursor: "not-allowed",
          }}
        />
        <div
          style={{
            fontSize: "11px",
            color: "var(--trust-text-muted)",
            marginTop: "4px",
          }}
        >
          Action names are defined in state entry/exit/transition actions
        </div>
      </div>

      <div>
        <label
          style={{
            display: "block",
            marginBottom: "4px",
            fontSize: "12px",
            fontWeight: 600,
          }}
        >
          Action Type
        </label>
        <select
          value={mapping.action}
          onChange={(e) => onUpdate({ action: e.target.value })}
          style={{
            width: "100%",
            padding: "6px 8px",
            fontSize: "13px",
            backgroundColor: "var(--trust-input-bg)",
            color: "var(--vscode-input-foreground, var(--trust-text))",
            border: "1px solid var(--trust-input-border)",
            borderRadius: "var(--trust-radius)",
          }}
        >
          {actionTypes.map((type) => (
            <option key={type} value={type}>
              {type}
            </option>
          ))}
        </select>
      </div>

      {mapping.action === "WRITE_OUTPUT" && (
        <>
          <div>
            <label
              style={{
                display: "block",
                marginBottom: "4px",
                fontSize: "12px",
                fontWeight: 600,
              }}
            >
              Hardware Address
            </label>
            <select
              value={mapping.address || ""}
              onChange={(e) => onUpdate({ address: e.target.value })}
              style={{
                width: "100%",
                padding: "6px 8px",
                fontSize: "13px",
                backgroundColor: "var(--trust-input-bg)",
                color: "var(--vscode-input-foreground, var(--trust-text))",
                border: "1px solid var(--trust-input-border)",
                borderRadius: "var(--trust-radius)",
              }}
            >
              <option value="">Select address...</option>
              {addressOptions.map((addr) => (
                <option key={addr} value={addr}>
                  {addr}
                </option>
              ))}
            </select>
            <div
              style={{
                fontSize: "11px",
                color: "var(--trust-text-muted)",
                marginTop: "4px",
              }}
            >
              IEC 61131-3 format: %QX0.0 to %QX0.7 (EL2008)
            </div>
          </div>

          <div>
            <label
              style={{
                display: "block",
                marginBottom: "4px",
                fontSize: "12px",
                fontWeight: 600,
              }}
            >
              Output Value
            </label>
            <div style={{ display: "flex", gap: "8px" }}>
              <label
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: "6px",
                  cursor: "pointer",
                  fontSize: "13px",
                }}
              >
                <input
                  type="radio"
                  checked={mapping.value === true}
                  onChange={() => onUpdate({ value: true })}
                />
                ON (true)
              </label>
              <label
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: "6px",
                  cursor: "pointer",
                  fontSize: "13px",
                }}
              >
                <input
                  type="radio"
                  checked={mapping.value === false}
                  onChange={() => onUpdate({ value: false })}
                />
                OFF (false)
              </label>
            </div>
          </div>
        </>
      )}

      {mapping.action === "LOG" && (
        <div>
          <label
            style={{
              display: "block",
              marginBottom: "4px",
              fontSize: "12px",
              fontWeight: 600,
            }}
          >
            Log Message
          </label>
          <input
            type="text"
            value={mapping.message || ""}
            onChange={(e) => onUpdate({ message: e.target.value })}
            placeholder="Enter log message..."
            style={{
              width: "100%",
              padding: "6px 8px",
              fontSize: "13px",
              backgroundColor: "var(--trust-input-bg)",
              color: "var(--vscode-input-foreground, var(--trust-text))",
              border: "1px solid var(--trust-input-border)",
              borderRadius: "var(--trust-radius)",
            }}
          />
        </div>
      )}

      {mapping.action === "WRITE_VARIABLE" && (
        <>
          <div>
            <label
              style={{
                display: "block",
                marginBottom: "4px",
                fontSize: "12px",
                fontWeight: 600,
              }}
            >
              Variable Name
            </label>
            <input
              type="text"
              value={mapping.variable || ""}
              onChange={(e) => onUpdate({ variable: e.target.value })}
              placeholder="e.g., counter, temperature"
              style={{
                width: "100%",
                padding: "6px 8px",
                fontSize: "13px",
                backgroundColor: "var(--trust-input-bg)",
                color: "var(--vscode-input-foreground, var(--trust-text))",
                border: "1px solid var(--trust-input-border)",
                borderRadius: "var(--trust-radius)",
              }}
            />
          </div>
          <div>
            <label
              style={{
                display: "block",
                marginBottom: "4px",
                fontSize: "12px",
                fontWeight: 600,
              }}
            >
              Value
            </label>
            <MappingValueInput
              stored={mapping.value}
              readValue={readValue}
              onValue={(value) => onUpdate({ value })}
            />
          </div>
        </>
      )}

      {mapping.action === "SET_MULTIPLE" && (
        <div>
          <label
            style={{
              display: "block",
              marginBottom: "4px",
              fontSize: "12px",
              fontWeight: 600,
            }}
          >
            Multiple Targets
          </label>
          <div
            style={{
              fontSize: "11px",
              color: "var(--trust-text-muted)",
              padding: "8px",
              backgroundColor: "var(--vscode-textBlockQuote-background)",
              border: "1px solid var(--vscode-textBlockQuote-border)",
              borderRadius: "var(--trust-radius)",
            }}
          >
            SET_MULTIPLE requires editing the targets array in JSON format.
            <br />
            Click the file icon to edit the .statechart.json file directly.
          </div>
        </div>
      )}
    </div>
  );
};
