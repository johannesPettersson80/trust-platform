// How a panel or group rearranges its objects on a smaller screen (specification 39, §4 item 5):
// as designed, scaled with the page; or in a row, a column or a grid, with the gap between them.

import type { HmiObjectModel } from "../../drawing/model";
import { Field } from "./Field";

const ARRANGEMENTS: Array<[string, string]> = [
    ["", "As designed, scaled"],
    ["row", "Row"],
    ["column", "Column"],
    ["grid", "Grid"],
];

interface ArrangeSectionProps {
    object: HmiObjectModel;
    readOnly: boolean;
    onChange: (object: HmiObjectModel) => void;
}

export function ArrangeSection({ object, readOnly, onChange }: ArrangeSectionProps) {
    const arrange = (value: string): void => {
        const { arrange: _arrange, gap: _gap, ...rest } = object;
        onChange(value === "" ? rest : { ...object, arrange: value as HmiObjectModel["arrange"] });
    };
    const gap = (text: string): void => {
        const { gap: _gap, ...rest } = object;
        const value = Number(text);
        onChange(text === "" || !Number.isInteger(value) || value < 0 ? rest : { ...object, gap: value });
    };
    return (
        <section className="hb-section" data-part="arrange">
            <h2>Smaller screens</h2>
            <label className="hb-field">
                <span className="hb-field-label">Objects</span>
                <select
                    name="arrange"
                    value={object.arrange ?? ""}
                    disabled={readOnly}
                    onChange={(event) => arrange(event.target.value)}
                >
                    {ARRANGEMENTS.map(([value, name]) => (
                        <option key={value} value={value}>
                            {name}
                        </option>
                    ))}
                </select>
            </label>
            {object.arrange ? (
                <Field name="gap" label="Gap" value={object.gap ?? 16} numeric readOnly={readOnly} onCommit={gap} />
            ) : null}
        </section>
    );
}
