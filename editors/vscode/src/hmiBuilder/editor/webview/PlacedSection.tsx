// A placed faceplate in the inspector (specification 39, §8.9): the faceplate it shows, which
// opens in its own editor, and its instance, chosen among the instances the faceplate fits.

import { useState } from "react";

import type { HmiObjectModel } from "../../drawing/model";
import { Field } from "./Field";

/** The Plant library's loop panel, whose loop Control Studio opens (§8.9); a cascade is two loops,
 * each with this panel. */
const CONTROL_STUDIO_PANELS = ["plant-pid"];

/** The instances a faceplate fits, as the host last answered. */
export interface FittingInstances {
    faceplate: string;
    instances: string[];
    problem?: string;
}

interface PlacedSectionProps {
    object: HmiObjectModel;
    readOnly: boolean;
    answer: FittingInstances | undefined;
    onAsk: (faceplate: string) => void;
    onOpen: (faceplate: string) => void;
    onStudio?: (id: string) => void;
    onChange: (object: HmiObjectModel) => void;
}

export function PlacedSection({ object, readOnly, answer, onAsk, onOpen, onStudio, onChange }: PlacedSectionProps) {
    const [picking, setPicking] = useState(false);
    const faceplate = object.faceplate ?? "";
    const fitting = answer?.faceplate === faceplate ? answer : undefined;
    const choose = (instance: string): void => {
        setPicking(false);
        if (instance && instance !== object.instance) onChange({ ...object, instance });
    };
    return (
        <section className="hb-section" data-part="placed">
            <h2>Faceplate</h2>
            <div className="hb-placed-name">
                <span className="hb-placed-faceplate">{faceplate}</span>
                <button
                    type="button"
                    className="hb-secondary"
                    data-action="open-faceplate"
                    disabled={!faceplate}
                    onClick={() => onOpen(faceplate)}
                >
                    Open
                </button>
            </div>
            {onStudio && CONTROL_STUDIO_PANELS.includes(faceplate) ? (
                <button
                    type="button"
                    className="hb-secondary"
                    disabled={!object.instance || readOnly}
                    onClick={() => onStudio(object.id)}
                >
                    Open in Control Studio
                </button>
            ) : null}
            <div className="hb-binding">
                <Field name="instance" label="Instance" value={object.instance} readOnly={readOnly} onCommit={choose} />
                <button
                    type="button"
                    className="hb-pick"
                    data-action="pick"
                    title="Choose an instance it fits"
                    aria-label="Choose an instance it fits"
                    aria-expanded={picking}
                    disabled={readOnly}
                    onClick={() => {
                        if (!picking) onAsk(faceplate);
                        setPicking(!picking);
                    }}
                >
                    ⋯
                </button>
                {picking ? (
                    <div className="hb-picker hb-instance-list" data-part="instance-list">
                        {fitting === undefined ? (
                            <p className="hb-hint">Finding the instances it fits…</p>
                        ) : fitting.instances.length === 0 ? (
                            <p className="hb-hint">
                                {fitting.problem ?? "No instance of the program fits this faceplate."}
                            </p>
                        ) : (
                            fitting.instances.map((instance) => (
                                <button
                                    type="button"
                                    key={instance}
                                    data-instance={instance}
                                    aria-pressed={instance === object.instance}
                                    onClick={() => choose(instance)}
                                >
                                    {instance}
                                </button>
                            ))
                        )}
                    </div>
                ) : null}
            </div>
        </section>
    );
}
