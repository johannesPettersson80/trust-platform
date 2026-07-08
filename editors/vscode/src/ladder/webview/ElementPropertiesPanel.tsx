import React, { useEffect, useRef } from "react";
import type {
  Coil as CoilType,
  CompareNode,
  Contact as ContactType,
  Counter as CounterType,
  LadderElement,
  MathNode,
  Timer as TimerType,
} from "../ladderEngine.types";

interface SelectedElementRef {
  rungIndex: number;
  elementIndex: number;
}

interface ElementPropertiesPanelProps {
  selectedElement: SelectedElementRef | null;
  selectedElementData: LadderElement | null;
  gridSize: number;
  onUpdateSelectedElement: (
    updater: (element: LadderElement) => LadderElement
  ) => void;
  onRemoveSelectedElement: () => void;
}

export function ElementPropertiesPanel({
  selectedElement,
  selectedElementData,
  gridSize,
  onUpdateSelectedElement,
  onRemoveSelectedElement,
}: ElementPropertiesPanelProps) {
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
                  <input
                    id="ladder-timer-pt"
                    type="number"
                    value={selectedElementData.presetMs}
                    onChange={(event) => {
                      const next = Number(event.target.value);
                      if (!Number.isFinite(next)) {
                        return;
                      }
                      onUpdateSelectedElement((element) => ({
                        ...(element as TimerType),
                        presetMs: Math.max(0, Math.round(next)),
                      }));
                    }}
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
                  <input
                    id="ladder-counter-preset"
                    type="number"
                    value={selectedElementData.preset}
                    onChange={(event) => {
                      const next = Number(event.target.value);
                      if (!Number.isFinite(next)) {
                        return;
                      }
                      onUpdateSelectedElement((element) => ({
                        ...(element as CounterType),
                        preset: Math.round(next),
                      }));
                    }}
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
              <input
                id="ladder-pos-x"
                type="number"
                value={selectedElementData.position.x}
                onChange={(event) => {
                  const x = Number(event.target.value);
                  if (!Number.isFinite(x)) {
                    return;
                  }
                  onUpdateSelectedElement((element) => ({
                    ...element,
                    position: {
                      ...element.position,
                      x: Math.round(x / gridSize) * gridSize,
                    },
                  }));
                }}
              />
            </div>

            <div className="property-row">
              <label htmlFor="ladder-pos-y">Position Y</label>
              <input
                id="ladder-pos-y"
                type="number"
                value={selectedElementData.position.y}
                onChange={(event) => {
                  const y = Number(event.target.value);
                  if (!Number.isFinite(y)) {
                    return;
                  }
                  onUpdateSelectedElement((element) => ({
                    ...element,
                    position: {
                      ...element.position,
                      y: Math.round(y / gridSize) * gridSize,
                    },
                  }));
                }}
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
