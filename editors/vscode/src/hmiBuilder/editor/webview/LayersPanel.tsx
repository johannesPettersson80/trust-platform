// The Layers panel (specification 39, §3.3 item 8): the page's objects front to back. A click
// selects; dragging a row onto another puts its object in front of that one; the eye hides an
// object in the editor only; the lock, saved in the page file, stops it being picked on the canvas.

import { useRef, useState } from "react";

import type { HmiPageModel } from "../../drawing/model";
import { moveInFrontOf, withLock } from "../layers";
import { objectName } from "../objectName";
import { componentName } from "./names";

interface LayersPanelProps {
    page: HmiPageModel;
    selection: string[];
    hidden: string[];
    readOnly: boolean;
    onSelect: (selection: string[]) => void;
    onHide: (id: string) => void;
    onCommit: (page: HmiPageModel) => void;
}

const EYE = "M1 8s2.5-5 7-5 7 5 7 5-2.5 5-7 5-7-5-7-5Zm7 2.5a2.5 2.5 0 1 0 0-5 2.5 2.5 0 0 0 0 5Z";
const LOCK = "M4 7V5a4 4 0 0 1 8 0v2h1v8H3V7h1Zm2 0h4V5a2 2 0 0 0-4 0v2Z";

export function LayersPanel({ page, selection, hidden, readOnly, onSelect, onHide, onCommit }: LayersPanelProps) {
    // The row being dragged: in a ref for the drop, which may come before the next render; in state for its look.
    const dragged = useRef<string>(undefined);
    const [dragging, setDragging] = useState<string>();
    const drag = (id: string | undefined): void => {
        dragged.current = id;
        setDragging(id);
    };
    const rows = [...page.object].reverse();
    return (
        <ol className="hb-layers" data-part="layers" onMouseLeave={() => drag(undefined)}>
            {rows.map((object) => {
                const isHidden = hidden.includes(object.id);
                const name = objectName(object);
                return (
                    <li
                        key={object.id}
                        data-layer={object.id}
                        data-selected={selection.includes(object.id) ? "true" : undefined}
                        data-dragging={dragging === object.id ? "true" : undefined}
                        className="hb-layer-row"
                        onMouseDown={(event) => {
                            if (event.button === 0 && !readOnly && !(event.target as Element).closest("button"))
                                drag(object.id);
                        }}
                        onMouseUp={() => {
                            const from = dragged.current;
                            if (from !== undefined && from !== object.id)
                                onCommit(moveInFrontOf(page, from, object.id));
                            drag(undefined);
                        }}
                        onClick={(event) => {
                            if (!(event.target as Element).closest("button")) onSelect([object.id]);
                        }}
                    >
                        <span className="hb-layer-name">
                            {name}
                            <span className="hb-layer-type">{componentName(object.type)}</span>
                        </span>
                        <button
                            type="button"
                            data-layer-eye
                            className="hb-icon-button"
                            aria-pressed={isHidden}
                            title={isHidden ? "Show in the editor" : "Hide in the editor"}
                            aria-label={isHidden ? "Show in the editor" : "Hide in the editor"}
                            onClick={() => onHide(object.id)}
                        >
                            <svg viewBox="0 0 16 16" aria-hidden="true">
                                <path d={EYE} fillRule="evenodd" opacity={isHidden ? 0.35 : 1} />
                            </svg>
                        </button>
                        <button
                            type="button"
                            data-layer-lock
                            className="hb-icon-button"
                            aria-pressed={object.locked === true}
                            disabled={readOnly}
                            title={object.locked ? "Unlock" : "Lock"}
                            aria-label={object.locked ? "Unlock" : "Lock"}
                            onClick={() => onCommit(withLock(page, object.id, !object.locked))}
                        >
                            <svg viewBox="0 0 16 16" aria-hidden="true">
                                <path d={LOCK} fillRule="evenodd" opacity={object.locked ? 1 : 0.35} />
                            </svg>
                        </button>
                    </li>
                );
            })}
        </ol>
    );
}
