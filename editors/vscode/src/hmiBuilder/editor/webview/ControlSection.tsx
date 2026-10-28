// The inspector's control section (§5.8, §12.8): who may use the control, the question asked
// before its command, how long confirmation may take, a button's action, a pulse's length and a
// selector's options. An empty field removes its key.

import type { HmiObjectModel } from "../../drawing/model";
import { Field } from "./Field";

interface ControlSectionProps {
    object: HmiObjectModel;
    readOnly: boolean;
    onChange: (object: HmiObjectModel) => void;
}

/** The object with a key set, or removed when the value is empty. */
function withKey(object: HmiObjectModel, key: keyof HmiObjectModel, value: unknown): HmiObjectModel {
    const next = { ...object } as Record<string, unknown>;
    if (value === undefined || value === "" || (Array.isArray(value) && value.length === 0)) delete next[key];
    else next[key] = value;
    return next as unknown as HmiObjectModel;
}

/** A positive number of seconds, or undefined. */
function seconds(text: string): number | undefined {
    const value = Number(text.replace(",", "."));
    return text === "" || !Number.isFinite(value) || value <= 0 ? undefined : value;
}

/** Options typed as a list separated by commas. */
function optionList(text: string): string[] {
    return text
        .split(",")
        .map((option) => option.trim())
        .filter((option) => option !== "");
}

export function ControlSection({ object, readOnly, onChange }: ControlSectionProps) {
    const set = (key: keyof HmiObjectModel, value: unknown): void => onChange(withKey(object, key, value));
    const chooses = object.type === "selector" || object.type === "mode-selector";
    return (
        <section className="hb-section" data-part="control">
            <h2>Control</h2>
            <label className="hb-field">
                <span className="hb-field-label">Who may use it</span>
                <select
                    name="role"
                    value={object.role ?? ""}
                    disabled={readOnly}
                    onChange={(event) => set("role", event.target.value)}
                >
                    <option value="">As the runtime's write policy says</option>
                    <option value="operator">Operator or higher</option>
                    <option value="engineer">Engineer</option>
                </select>
            </label>
            {object.type === "button" ? (
                <label className="hb-field">
                    <span className="hb-field-label">Action</span>
                    <select
                        name="action"
                        value={object.action ?? "set"}
                        disabled={readOnly}
                        onChange={(event) =>
                            set("action", event.target.value === "set" ? undefined : event.target.value)
                        }
                    >
                        <option value="set">Set (TRUE)</option>
                        <option value="reset">Reset (FALSE)</option>
                        <option value="toggle">Toggle</option>
                    </select>
                </label>
            ) : null}
            {chooses ? (
                <Field
                    name="options"
                    label={
                        object.type === "mode-selector" ? "Modes (Manual, Auto, Track)" : "Options, separated by commas"
                    }
                    value={object.options?.join(", ")}
                    readOnly={readOnly}
                    onCommit={(text) => set("options", optionList(text))}
                />
            ) : null}
            <Field
                name="confirm"
                label="Question before sending"
                value={object.confirm}
                readOnly={readOnly}
                onCommit={(text) => set("confirm", text)}
            />
            <div className="hb-pair">
                {object.type === "pulse-button" ? (
                    <Field
                        name="pulse"
                        label="Pulse (s)"
                        value={object.pulse}
                        numeric
                        readOnly={readOnly}
                        onCommit={(text) => set("pulse", seconds(text))}
                    />
                ) : null}
                <Field
                    name="timeout"
                    label="Confirm within (s)"
                    value={object.timeout}
                    numeric
                    readOnly={readOnly}
                    onCommit={(text) => set("timeout", seconds(text))}
                />
            </div>
        </section>
    );
}
