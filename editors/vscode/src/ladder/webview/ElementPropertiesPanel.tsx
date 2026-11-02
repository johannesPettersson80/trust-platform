import React, { useEffect, useRef, useState } from "react";
import type { LadderElement } from "../ladderProgram.types";

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
    /** Sends a text typed or a choice made in a field to the language server (`setField`). */
    onSetField: (field: string, value: string) => void;
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

/**
 * A text field: it shows the stored text, and the text as typed while it is being typed, since
 * the language server stores each text (`setField`) and the stored one comes back after it.
 */
function TextFieldInput({
    id,
    stored,
    inputRef,
    onText,
}: {
    id: string;
    stored: string;
    inputRef?: React.RefObject<HTMLInputElement | null>;
    onText: (text: string) => void;
}) {
    const [draft, setDraft] = useState<string | null>(null);
    return (
        <input
            id={id}
            ref={inputRef}
            type="text"
            value={draft ?? stored}
            onChange={(event) => {
                setDraft(event.target.value);
                onText(event.target.value);
            }}
            onBlur={() => setDraft(null)}
        />
    );
}

export function ElementPropertiesPanel({
    selectedElement,
    selectedElementData,
    onSetNumber,
    numberFieldError,
    onSetField,
    onRemoveSelectedElement,
}: ElementPropertiesPanelProps) {
    const errorOf = (field: LadderNumberField): string | undefined =>
        numberFieldError?.field === field ? numberFieldError.message : undefined;
    const variableInputRef = useRef<HTMLInputElement | null>(null);
    const shouldFocusVariable =
        selectedElementData?.type === "contact" && selectedElementData.variable.trim().length === 0;

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
                                <TextFieldInput
                                    key={`${selectedElementData.id}:variable`}
                                    id="ladder-variable"
                                    inputRef={variableInputRef}
                                    stored={selectedElementData.variable}
                                    onText={(text) => onSetField("variable", text)}
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
                                    onChange={(event) => onSetField("contactType", event.target.value)}
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
                                    onChange={(event) => onSetField("coilType", event.target.value)}
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
                                        onChange={(event) => onSetField("timerType", event.target.value)}
                                    >
                                        <option value="TON">TON</option>
                                        <option value="TOF">TOF</option>
                                        <option value="TP">TP</option>
                                    </select>
                                </div>
                                <div className="property-row">
                                    <label htmlFor="ladder-timer-instance">Instance</label>
                                    <TextFieldInput
                                        key={`${selectedElementData.id}:instance`}
                                        id="ladder-timer-instance"
                                        stored={selectedElementData.instance}
                                        onText={(text) => onSetField("instance", text)}
                                    />
                                </div>
                                <div className="property-row">
                                    <label htmlFor="ladder-timer-input">Input Address</label>
                                    <TextFieldInput
                                        key={`${selectedElementData.id}:input`}
                                        id="ladder-timer-input"
                                        stored={selectedElementData.input ?? ""}
                                        onText={(text) => onSetField("input", text)}
                                    />
                                </div>
                                <div className="property-row">
                                    <label htmlFor="ladder-timer-q-output">Q Output</label>
                                    <TextFieldInput
                                        key={`${selectedElementData.id}:qOutput`}
                                        id="ladder-timer-q-output"
                                        stored={selectedElementData.qOutput}
                                        onText={(text) => onSetField("qOutput", text)}
                                    />
                                </div>
                                <div className="property-row">
                                    <label htmlFor="ladder-timer-et-output">ET Output</label>
                                    <TextFieldInput
                                        key={`${selectedElementData.id}:etOutput`}
                                        id="ladder-timer-et-output"
                                        stored={selectedElementData.etOutput}
                                        onText={(text) => onSetField("etOutput", text)}
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
                                        onChange={(event) => onSetField("counterType", event.target.value)}
                                    >
                                        <option value="CTU">CTU</option>
                                        <option value="CTD">CTD</option>
                                        <option value="CTUD">CTUD</option>
                                    </select>
                                </div>
                                <div className="property-row">
                                    <label htmlFor="ladder-counter-instance">Instance</label>
                                    <TextFieldInput
                                        key={`${selectedElementData.id}:instance`}
                                        id="ladder-counter-instance"
                                        stored={selectedElementData.instance}
                                        onText={(text) => onSetField("instance", text)}
                                    />
                                </div>
                                <div className="property-row">
                                    <label htmlFor="ladder-counter-input">Input Address</label>
                                    <TextFieldInput
                                        key={`${selectedElementData.id}:input`}
                                        id="ladder-counter-input"
                                        stored={selectedElementData.input ?? ""}
                                        onText={(text) => onSetField("input", text)}
                                    />
                                </div>
                                <div className="property-row">
                                    <label htmlFor="ladder-counter-q-output">Q Output</label>
                                    <TextFieldInput
                                        key={`${selectedElementData.id}:qOutput`}
                                        id="ladder-counter-q-output"
                                        stored={selectedElementData.qOutput}
                                        onText={(text) => onSetField("qOutput", text)}
                                    />
                                </div>
                                <div className="property-row">
                                    <label htmlFor="ladder-counter-cv-output">CV Output</label>
                                    <TextFieldInput
                                        key={`${selectedElementData.id}:cvOutput`}
                                        id="ladder-counter-cv-output"
                                        stored={selectedElementData.cvOutput}
                                        onText={(text) => onSetField("cvOutput", text)}
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
                                        onChange={(event) => onSetField("op", event.target.value)}
                                    >
                                        <option value="GT">GT</option>
                                        <option value="LT">LT</option>
                                        <option value="EQ">EQ</option>
                                    </select>
                                </div>
                                <div className="property-row">
                                    <label htmlFor="ladder-compare-left">IN1</label>
                                    <TextFieldInput
                                        key={`${selectedElementData.id}:left`}
                                        id="ladder-compare-left"
                                        stored={selectedElementData.left}
                                        onText={(text) => onSetField("left", text)}
                                    />
                                </div>
                                <div className="property-row">
                                    <label htmlFor="ladder-compare-right">IN2</label>
                                    <TextFieldInput
                                        key={`${selectedElementData.id}:right`}
                                        id="ladder-compare-right"
                                        stored={selectedElementData.right}
                                        onText={(text) => onSetField("right", text)}
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
                                        onChange={(event) => onSetField("op", event.target.value)}
                                    >
                                        <option value="ADD">ADD</option>
                                        <option value="SUB">SUB</option>
                                        <option value="MUL">MUL</option>
                                        <option value="DIV">DIV</option>
                                    </select>
                                </div>
                                <div className="property-row">
                                    <label htmlFor="ladder-math-left">IN1</label>
                                    <TextFieldInput
                                        key={`${selectedElementData.id}:left`}
                                        id="ladder-math-left"
                                        stored={selectedElementData.left}
                                        onText={(text) => onSetField("left", text)}
                                    />
                                </div>
                                <div className="property-row">
                                    <label htmlFor="ladder-math-right">IN2</label>
                                    <TextFieldInput
                                        key={`${selectedElementData.id}:right`}
                                        id="ladder-math-right"
                                        stored={selectedElementData.right}
                                        onText={(text) => onSetField("right", text)}
                                    />
                                </div>
                                <div className="property-row">
                                    <label htmlFor="ladder-math-output">Output Address</label>
                                    <TextFieldInput
                                        key={`${selectedElementData.id}:output`}
                                        id="ladder-math-output"
                                        stored={selectedElementData.output}
                                        onText={(text) => onSetField("output", text)}
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
