import React, { useEffect, useRef, useState } from "react";
import type {
  Coil as CoilType,
  CompareNode,
  Contact as ContactType,
  Counter as CounterType,
  LadderElement,
  MathNode,
  Timer as TimerType,
} from "../ladderProgram.types";

interface SelectedElementRef {
  rungIndex: number;
  elementIndex: number;
}

/** The number fields of the panel, by the program member each one stores. */
export type LadderNumberField = "presetMs" | "preset" | "x" | "y";

/** Why the language server did not store the text typed into a number field. */
export interface NumberFieldError {
  field: LadderNumberField;
  message: string;
}

interface ElementPropertiesPanelProps {
  selectedElement: SelectedElementRef | null;
  selectedElementData: LadderElement | null;
  /** Sends the text typed into a number field to the language server (`setNumber`). */
  onSetNumber: (field: LadderNumberField, text: string) => void;
  numberFieldError: NumberFieldError | null;
  onUpdateSelectedElement: (
    updater: (element: LadderElement) => LadderElement
  ) => void;
  onRemoveSelectedElement: () => void;
}

/**
 * A number field: it shows the stored number, and the text as typed while it is being typed. The
 * language server reads each text and stores its number, or answers why not (`error`).
 */
function NumberTextInput({
  id,
  stored,
  error,
  onText,
}: {
  id: string;
  stored: number;
  error?: string;
  onText: (text: string) => void;
}) {
  const [draft, setDraft] = useState<string | null>(null);
  return (
    <>
      <input
        id={id}
        type="number"
        value={draft ?? String(stored)}
        onChange={(event) => {
          setDraft(event.target.value);
          onText(event.target.value);
        }}
        onBlur={() => setDraft(null)}
      />
      {error && <p className="property-warning">{error}</p>}
    </>
  );
}

export function ElementPropertiesPanel({
  selectedElement,
  selectedElementData,
  onSetNumber,
  numberFieldError,
  onUpdateSelectedElement,
  onRemoveSelectedElement,
}: ElementPropertiesPanelProps) {
  const errorOf = (field: LadderNumberField): string | undefined =>
    numberFieldError?.field === field ? numberFieldError.message : undefined;
  const variableInputRef = useRef<HTMLInputElement | null>(null);
  const shouldFocusVariable =
    selectedElementData?.type === "contact" &&
    selectedElementData.variable.trim().length === 0;

  useEffect(() => {
    if (!shouldFocusVariable || !variableInputRef.current) {
      return;
    }
    variableInputRef.current.focus();
    variableInputRef.current.select();
  }, [shouldFocusVariable, selectedElementData?.id]);

  return (
    <aside className="properties-panel">
      {selectedElementData ? (
        <div className="properties-section">
          <h3>Properties</h3>
          <p className="property-hint">
            Rung {selectedElement!.rungIndex + 1} - {selectedElementData.type}
          </p>
          <>
            {"variable" in selectedElementData && (
              <div className="property-row">
                <label htmlFor="ladder-variable">Variable</label>
                <input
                  id="ladder-variable"
                  ref={variableInputRef}
                  type="text"
                  value={selectedElementData.variable}
                  onChange={(event) => {
                    const nextVariable = event.target.value;
                    onUpdateSelectedElement((element) => ({
                      ...element,
                      variable: nextVariable,
                    }));
                  }}
                />
                {selectedElementData.variable.trim().length === 0 && (
                  <p className="property-warning">Variable required.</p>
                )}
              </div>
            )}

            {selectedElementData.type === "contact" && (
              <div className="property-row">
                <label htmlFor="ladder-contact-type">Contact Type</label>
                <select
                  id="ladder-contact-type"
                  value={selectedElementData.contactType}
                  onChange={(event) => {
                    const nextType = event.target.value as ContactType["contactType"];
                    onUpdateSelectedElement((element) => ({
                      ...(element as ContactType),
                      contactType: nextType,
                    }));
                  }}
                >
                  <option value="NO">NO</option>
                  <option value="NC">NC</option>
                </select>
              </div>
            )}

            {selectedElementData.type === "coil" && (
              <div className="property-row">
                <label htmlFor="ladder-coil-type">Coil Type</label>
                <select
                  id="ladder-coil-type"
                  value={selectedElementData.coilType}
                  onChange={(event) => {
                    const nextType = event.target.value as CoilType["coilType"];
                    onUpdateSelectedElement((element) => ({
                      ...(element as CoilType),
                      coilType: nextType,
                    }));
                  }}
                >
                  <option value="NORMAL">NORMAL</option>
                  <option value="SET">SET</option>
                  <option value="RESET">RESET</option>
                  <option value="NEGATED">NEGATED (NOT)</option>
                </select>
              </div>
            )}

            {selectedElementData.type === "timer" && (
              <>
                <div className="property-row">
                  <label htmlFor="ladder-timer-type">Timer Type</label>
                  <select
                    id="ladder-timer-type"
                    value={selectedElementData.timerType}
                    onChange={(event) => {
                      const nextType = event.target.value as TimerType["timerType"];
                      onUpdateSelectedElement((element) => ({
                        ...(element as TimerType),
                        timerType: nextType,
                      }));
                    }}
                  >
                    <option value="TON">TON</option>
                    <option value="TOF">TOF</option>
                    <option value="TP">TP</option>
                  </select>
                </div>
                <div className="property-row">
                  <label htmlFor="ladder-timer-instance">Instance</label>
                  <input
                    id="ladder-timer-instance"
                    type="text"
                    value={selectedElementData.instance}
                    onChange={(event) => {
                      const next = event.target.value;
                      onUpdateSelectedElement((element) => ({
                        ...(element as TimerType),
                        instance: next,
                      }));
                    }}
                  />
                </div>
                <div className="property-row">
                  <label htmlFor="ladder-timer-input">Input Address</label>
                  <input
                    id="ladder-timer-input"
                    type="text"
                    value={selectedElementData.input ?? ""}
                    onChange={(event) => {
                      const next = event.target.value;
                      onUpdateSelectedElement((element) => ({
                        ...(element as TimerType),
                        input: next,
                      }));
                    }}
                  />
                </div>
                <div className="property-row">
                  <label htmlFor="ladder-timer-q-output">Q Output</label>
                  <input
                    id="ladder-timer-q-output"
                    type="text"
                    value={selectedElementData.qOutput}
                    onChange={(event) => {
                      const next = event.target.value;
                      onUpdateSelectedElement((element) => ({
                        ...(element as TimerType),
                        qOutput: next,
                      }));
                    }}
                  />
                </div>
                <div className="property-row">
                  <label htmlFor="ladder-timer-et-output">ET Output</label>
                  <input
                    id="ladder-timer-et-output"
                    type="text"
                    value={selectedElementData.etOutput}
                    onChange={(event) => {
                      const next = event.target.value;
                      onUpdateSelectedElement((element) => ({
                        ...(element as TimerType),
                        etOutput: next,
                      }));
                    }}
                  />
                </div>
                <div className="property-row">
                  <label htmlFor="ladder-timer-pt">Preset (ms)</label>
                  <NumberTextInput
                    key={`${selectedElementData.id}:presetMs`}
                    id="ladder-timer-pt"
                    stored={selectedElementData.presetMs}
                    error={errorOf("presetMs")}
                    onText={(text) => onSetNumber("presetMs", text)}
                  />
                </div>
              </>
            )}

            {selectedElementData.type === "counter" && (
              <>
                <div className="property-row">
                  <label htmlFor="ladder-counter-type">Counter Type</label>
                  <select
                    id="ladder-counter-type"
                    value={selectedElementData.counterType}
                    onChange={(event) => {
                      const nextType = event.target.value as CounterType["counterType"];
                      onUpdateSelectedElement((element) => ({
                        ...(element as CounterType),
                        counterType: nextType,
                      }));
                    }}
                  >
                    <option value="CTU">CTU</option>
                    <option value="CTD">CTD</option>
                    <option value="CTUD">CTUD</option>
                  </select>
                </div>
                <div className="property-row">
                  <label htmlFor="ladder-counter-instance">Instance</label>
                  <input
                    id="ladder-counter-instance"
                    type="text"
                    value={selectedElementData.instance}
                    onChange={(event) => {
                      const next = event.target.value;
                      onUpdateSelectedElement((element) => ({
                        ...(element as CounterType),
                        instance: next,
                      }));
                    }}
                  />
                </div>
                <div className="property-row">
                  <label htmlFor="ladder-counter-input">Input Address</label>
                  <input
                    id="ladder-counter-input"
                    type="text"
                    value={selectedElementData.input ?? ""}
                    onChange={(event) => {
                      const next = event.target.value;
                      onUpdateSelectedElement((element) => ({
                        ...(element as CounterType),
                        input: next,
                      }));
                    }}
                  />
                </div>
                <div className="property-row">
                  <label htmlFor="ladder-counter-q-output">Q Output</label>
                  <input
                    id="ladder-counter-q-output"
                    type="text"
                    value={selectedElementData.qOutput}
                    onChange={(event) => {
                      const next = event.target.value;
                      onUpdateSelectedElement((element) => ({
                        ...(element as CounterType),
                        qOutput: next,
                      }));
                    }}
                  />
                </div>
                <div className="property-row">
                  <label htmlFor="ladder-counter-cv-output">CV Output</label>
                  <input
                    id="ladder-counter-cv-output"
                    type="text"
                    value={selectedElementData.cvOutput}
                    onChange={(event) => {
                      const next = event.target.value;
                      onUpdateSelectedElement((element) => ({
                        ...(element as CounterType),
                        cvOutput: next,
                      }));
                    }}
                  />
                </div>
                <div className="property-row">
                  <label htmlFor="ladder-counter-preset">Preset</label>
                  <NumberTextInput
                    key={`${selectedElementData.id}:preset`}
                    id="ladder-counter-preset"
                    stored={selectedElementData.preset}
                    error={errorOf("preset")}
                    onText={(text) => onSetNumber("preset", text)}
                  />
                </div>
              </>
            )}

            {selectedElementData.type === "compare" && (
              <>
                <div className="property-row">
                  <label htmlFor="ladder-compare-op">Operator</label>
                  <select
                    id="ladder-compare-op"
                    value={selectedElementData.op}
                    onChange={(event) => {
                      const nextOp = event.target.value as CompareNode["op"];
                      onUpdateSelectedElement((element) => ({
                        ...(element as CompareNode),
                        op: nextOp,
                      }));
                    }}
                  >
                    <option value="GT">GT</option>
                    <option value="LT">LT</option>
                    <option value="EQ">EQ</option>
                  </select>
                </div>
                <div className="property-row">
                  <label htmlFor="ladder-compare-left">IN1</label>
                  <input
                    id="ladder-compare-left"
                    type="text"
                    value={selectedElementData.left}
                    onChange={(event) => {
                      const next = event.target.value;
                      onUpdateSelectedElement((element) => ({
                        ...(element as CompareNode),
                        left: next,
                      }));
                    }}
                  />
                </div>
                <div className="property-row">
                  <label htmlFor="ladder-compare-right">IN2</label>
                  <input
                    id="ladder-compare-right"
                    type="text"
                    value={selectedElementData.right}
                    onChange={(event) => {
                      const next = event.target.value;
                      onUpdateSelectedElement((element) => ({
                        ...(element as CompareNode),
                        right: next,
                      }));
                    }}
                  />
                </div>
              </>
            )}

            {selectedElementData.type === "math" && (
              <>
                <div className="property-row">
                  <label htmlFor="ladder-math-op">Operator</label>
                  <select
                    id="ladder-math-op"
                    value={selectedElementData.op}
                    onChange={(event) => {
                      const nextOp = event.target.value as MathNode["op"];
                      onUpdateSelectedElement((element) => ({
                        ...(element as MathNode),
                        op: nextOp,
                      }));
                    }}
                  >
                    <option value="ADD">ADD</option>
                    <option value="SUB">SUB</option>
                    <option value="MUL">MUL</option>
                    <option value="DIV">DIV</option>
                  </select>
                </div>
                <div className="property-row">
                  <label htmlFor="ladder-math-left">IN1</label>
                  <input
                    id="ladder-math-left"
                    type="text"
                    value={selectedElementData.left}
                    onChange={(event) => {
                      const next = event.target.value;
                      onUpdateSelectedElement((element) => ({
                        ...(element as MathNode),
                        left: next,
                      }));
                    }}
                  />
                </div>
                <div className="property-row">
                  <label htmlFor="ladder-math-right">IN2</label>
                  <input
                    id="ladder-math-right"
                    type="text"
                    value={selectedElementData.right}
                    onChange={(event) => {
                      const next = event.target.value;
                      onUpdateSelectedElement((element) => ({
                        ...(element as MathNode),
                        right: next,
                      }));
                    }}
                  />
                </div>
                <div className="property-row">
                  <label htmlFor="ladder-math-output">Output Address</label>
                  <input
                    id="ladder-math-output"
                    type="text"
                    value={selectedElementData.output}
                    onChange={(event) => {
                      const next = event.target.value;
                      onUpdateSelectedElement((element) => ({
                        ...(element as MathNode),
                        output: next,
                      }));
                    }}
                  />
                </div>
              </>
            )}

            <div className="property-row">
              <label htmlFor="ladder-pos-x">Position X</label>
              <NumberTextInput
                key={`${selectedElementData.id}:x`}
                id="ladder-pos-x"
                stored={selectedElementData.position.x}
                error={errorOf("x")}
                onText={(text) => onSetNumber("x", text)}
              />
            </div>

            <div className="property-row">
              <label htmlFor="ladder-pos-y">Position Y</label>
              <NumberTextInput
                key={`${selectedElementData.id}:y`}
                id="ladder-pos-y"
                stored={selectedElementData.position.y}
                error={errorOf("y")}
                onText={(text) => onSetNumber("y", text)}
              />
            </div>

            <button className="danger-button" onClick={onRemoveSelectedElement}>
              Delete Element
            </button>
          </>
        </div>
      ) : (
        <div className="trust-empty">
          <div>Select an element to edit properties.</div>
        </div>
      )}
    </aside>
  );
}
