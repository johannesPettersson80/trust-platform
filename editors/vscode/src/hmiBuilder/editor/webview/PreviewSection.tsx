// The state switcher (§5.3 item 6): the selected object drawn in each state it can show, without
// a connected source. A state that needs a binding the object lacks says which; a preview is the
// editor's only and never saved.

import type { HmiObjectModel } from "../../drawing/model";
import { missingBindings, previewStates } from "../../drawing/previews";

interface PreviewSectionProps {
    object: HmiObjectModel;
    /** The component's bindings, which the data states need one of. */
    properties: string[];
    /** The state shown; the sample when undefined. */
    state: string | undefined;
    onPreview: (state: string | undefined) => void;
}

export function PreviewSection({ object, properties, state, onPreview }: PreviewSectionProps) {
    const states = previewStates(object.type, properties);
    return (
        <section className="hb-section">
            <h2>Preview</h2>
            <div className="hb-preview" data-part="preview" role="group" aria-label="Preview state">
                {states.map((candidate) => {
                    const missing = missingBindings(object, candidate);
                    return (
                        <button
                            key={candidate.id}
                            type="button"
                            data-state={candidate.id}
                            aria-pressed={candidate.id === (state ?? "sample")}
                            disabled={missing.length > 0}
                            title={missing.length > 0 ? `Bind ${missing.join(" and ")} to show this state` : undefined}
                            onClick={() => onPreview(candidate.id === "sample" ? undefined : candidate.id)}
                        >
                            {candidate.id === "sample" ? "Build" : candidate.name}
                        </button>
                    );
                })}
            </div>
            <p className="hb-hint">Shown here only; the page is not changed.</p>
        </section>
    );
}
