// Align and distribute in the inspector of a multiple selection (specification 39, §3.3 item 8).

import type { HmiPageModel } from "../../drawing/model";
import { AlignEdge, alignObjects, DistributeAxis, distributeObjects } from "../align";

const EDGES: Array<[AlignEdge, string]> = [
    ["left", "Align left"],
    ["centre", "Align centre"],
    ["right", "Align right"],
    ["top", "Align top"],
    ["middle", "Align middle"],
    ["bottom", "Align bottom"],
];

const AXES: Array<[DistributeAxis, string]> = [
    ["horizontal", "Distribute horizontally"],
    ["vertical", "Distribute vertically"],
];

interface AlignSectionProps {
    page: HmiPageModel;
    selection: string[];
    readOnly: boolean;
    onCommit: (page: HmiPageModel) => void;
}

export function AlignSection({ page, selection, readOnly, onCommit }: AlignSectionProps) {
    const ids = selection.filter((id) => page.object.some((object) => object.id === id));
    if (ids.length < 2) return null;
    return (
        <section className="hb-section" data-part="align">
            <h2>Arrange</h2>
            <div className="hb-align">
                {EDGES.map(([edge, name]) => (
                    <button
                        key={edge}
                        type="button"
                        data-align={edge}
                        title={name}
                        aria-label={name}
                        disabled={readOnly}
                        onClick={() => onCommit(alignObjects(page, ids, edge))}
                    >
                        {name.replace("Align ", "")}
                    </button>
                ))}
            </div>
            {ids.length >= 3 ? (
                <div className="hb-align">
                    {AXES.map(([axis, name]) => (
                        <button
                            key={axis}
                            type="button"
                            data-distribute={axis}
                            title={name}
                            aria-label={name}
                            disabled={readOnly}
                            onClick={() => onCommit(distributeObjects(page, ids, axis))}
                        >
                            {name.replace("Distribute ", "")}
                        </button>
                    ))}
                </div>
            ) : null}
        </section>
    );
}
