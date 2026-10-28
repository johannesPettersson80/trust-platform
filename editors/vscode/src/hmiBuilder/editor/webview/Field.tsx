// One inspector field: it commits on Enter or when it loses focus, and Escape restores it.

import type { KeyboardEvent } from "react";

interface FieldProps {
    name: string;
    label: string;
    value: string | number | undefined;
    numeric?: boolean;
    readOnly?: boolean;
    /** Empty the field once its entry is committed: a field that adds something, not one that
     * shows a value. */
    clearOnCommit?: boolean;
    onCommit?: (value: string) => void;
}

export function Field({ name, label, value, numeric, readOnly, clearOnCommit, onCommit }: FieldProps) {
    const shown = value === undefined ? "" : String(value);
    const commit = (input: HTMLInputElement): void => {
        if (input.value === shown) return;
        const entered = input.value.trim();
        // A field that adds (Add pen) empties at once, so leaving it does not add the entry again.
        if (clearOnCommit) input.value = shown;
        onCommit?.(entered);
    };
    const onKeyDown = (event: KeyboardEvent<HTMLInputElement>): void => {
        if (event.key === "Enter") commit(event.currentTarget);
        if (event.key === "Escape") event.currentTarget.value = shown;
    };
    return (
        <label className="hb-field">
            <span className="hb-field-label">{label}</span>
            <input
                key={shown}
                name={name}
                defaultValue={shown}
                inputMode={numeric ? "decimal" : undefined}
                readOnly={readOnly}
                spellCheck={false}
                onKeyDown={onKeyDown}
                onBlur={(event) => commit(event.currentTarget)}
            />
        </label>
    );
}
