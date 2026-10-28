// Display thresholds are edited together so a half-entered range never reaches the document.
import { useState } from "react";
import type { HmiObjectModel } from "../../drawing/model";
import { LIMIT_NAMES } from "../../drawing/alarmRanges";

export function LimitsSection({
    object,
    readOnly,
    onChange,
}: {
    object: HmiObjectModel;
    readOnly: boolean;
    onChange: (object: HmiObjectModel) => void;
}) {
    const [problem, setProblem] = useState("");
    const names = { "low-low": "Low low", low: "Low", high: "High", "high-high": "High high" };
    return (
        <section className="hb-section" data-part="display-limits">
            <h2>Alarm limits</h2>
            <p className="hb-hint">Marks on the scale. Configure alarm behaviour in the program.</p>
            <form
                key={JSON.stringify(object.limits)}
                onSubmit={(event) => {
                    event.preventDefault();
                    if (readOnly) return;
                    const data = new FormData(event.currentTarget);
                    const limits: NonNullable<HmiObjectModel["limits"]> = {};
                    let previous = -Infinity;
                    for (const name of LIMIT_NAMES) {
                        const text = String(data.get(`limit-${name}`) ?? "").trim();
                        if (!text) continue;
                        const value = Number(text);
                        if (!Number.isFinite(value) || value < previous) {
                            setProblem("Limits must be finite and ordered from Low low to High high.");
                            return;
                        }
                        limits[name] = value;
                        previous = value;
                    }
                    setProblem("");
                    const updated = { ...object };
                    if (Object.keys(limits).length) updated.limits = limits;
                    else delete updated.limits;
                    onChange(updated);
                }}
            >
                <div className="hb-pair">
                    {LIMIT_NAMES.map((name) => (
                        <label key={name} className="hb-field">
                            <span className="hb-field-label">{names[name]}</span>
                            <input
                                type="number"
                                step="any"
                                name={`limit-${name}`}
                                defaultValue={object.limits?.[name] ?? ""}
                                disabled={readOnly}
                            />
                        </label>
                    ))}
                </div>
                <button type="submit" data-action="apply-limits" disabled={readOnly}>
                    Apply limits
                </button>
                {problem ? (
                    <p role="alert" className="hb-hint">
                        {problem}
                    </p>
                ) : null}
            </form>
        </section>
    );
}
