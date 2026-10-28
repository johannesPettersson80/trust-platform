// A trend pen's own scale, edited as one binding change (§5.9).
import { useState } from "react";
import type { HmiObjectModel } from "../../drawing/model";
import { NOT_PENS } from "../../drawing/components/trend";

interface Props {
    object: HmiObjectModel;
    readOnly: boolean;
    onChange: (object: HmiObjectModel) => void;
}

function PenScale({ object, name, readOnly, onChange }: Props & { name: string }) {
    const binding = object.bind[name];
    const [fixedDraft, setFixedDraft] = useState(false);
    const [problem, setProblem] = useState("");
    const fixed = fixedDraft || binding.range !== undefined;
    const change = (range?: [number, number]) => {
        if (readOnly) return;
        const updated = { ...binding };
        if (range) updated.range = range;
        else delete updated.range;
        onChange({ ...object, bind: { ...object.bind, [name]: updated } });
        setFixedDraft(false);
    };
    return (
        <div className="hb-section" data-pen={name}>
            <label className="hb-field">
                <span className="hb-field-label">{name}</span>
                <select
                    data-pen-scale={name}
                    value={fixed ? "fixed" : "auto"}
                    disabled={readOnly}
                    onChange={(event) => {
                        setProblem("");
                        if (event.target.value === "fixed") setFixedDraft(true);
                        else change();
                    }}
                >
                    <option value="auto">
                        {object.min !== undefined && object.max !== undefined
                            ? `Shared range (${object.min} – ${object.max})`
                            : "Automatic"}
                    </option>
                    <option value="fixed">Fixed</option>
                </select>
            </label>
            {fixed && (
                <form
                    key={JSON.stringify(binding.range)}
                    onSubmit={(event) => {
                        event.preventDefault();
                        const data = new FormData(event.currentTarget);
                        const range: [number, number] = [Number(data.get("pen-min")), Number(data.get("pen-max"))];
                        if (!range.every(Number.isFinite) || range[0] >= range[1]) {
                            setProblem("Maximum must be greater than minimum.");
                            return;
                        }
                        setProblem("");
                        change(range);
                    }}
                >
                    <div className="hb-pair">
                        <label className="hb-field">
                            <span className="hb-field-label">Minimum</span>
                            <input
                                name="pen-min"
                                type="number"
                                step="any"
                                required
                                defaultValue={binding.range?.[0] ?? object.min ?? 0}
                                disabled={readOnly}
                            />
                        </label>
                        <label className="hb-field">
                            <span className="hb-field-label">Maximum</span>
                            <input
                                name="pen-max"
                                type="number"
                                step="any"
                                required
                                defaultValue={binding.range?.[1] ?? object.max ?? 100}
                                disabled={readOnly}
                            />
                        </label>
                    </div>
                    <button type="submit" data-action="apply-pen-scale" disabled={readOnly}>
                        Apply scale
                    </button>
                </form>
            )}
            {problem && (
                <p role="alert" className="hb-hint">
                    {problem}
                </p>
            )}
        </div>
    );
}

export function TrendScalesSection(props: Props) {
    return (
        <section className="hb-section">
            <h2>Pen scales</h2>
            <p className="hb-hint">Each pen scales separately. The axis follows the first visible pen.</p>
            {Object.keys(props.object.bind)
                .filter((name) => !NOT_PENS.includes(name))
                .sort()
                .map((name) => (
                    <PenScale key={`${props.object.id}:${name}`} {...props} name={name} />
                ))}
        </section>
    );
}
