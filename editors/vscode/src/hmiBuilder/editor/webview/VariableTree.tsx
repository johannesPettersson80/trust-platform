// A tree of the program's variables (§7.1): each name with its type, and its current value while a
// run target is connected (§3.3 item 8); programs, blocks, structures and arrays fold. Roots start
// unfolded.

import { useContext, useEffect, useId, useState } from "react";
import type { MouseEvent as ReactMouseEvent, ReactElement } from "react";

import type { VariableNode } from "../messages";
import type { PickerRow } from "../variables";
import { currentText, PickerValues } from "./pickerValues";

interface VariableTreeProps {
    rows: PickerRow[];
    /** Everything unfolded, as while searching. */
    unfolded?: boolean;
    /** Rows that can be picked; the others only hold them. */
    pickable?: (node: VariableNode) => boolean;
    onPick?: (node: VariableNode) => void;
    onDragStart?: (node: VariableNode, event: ReactMouseEvent) => void;
}

export function VariableTree({ rows, unfolded, pickable, onPick, onDragStart }: VariableTreeProps) {
    /** Rows folded or unfolded by hand, against the default (roots unfolded). */
    const [toggled, setToggled] = useState<Set<string>>(() => new Set());
    const values = useContext(PickerValues);
    const tree = useId();
    /** The variables this tree shows, whose values the host reads. */
    const shown: string[] = [];
    const toggle = (path: string): void =>
        setToggled((current) => {
            const next = new Set(current);
            if (!next.delete(path)) next.add(path);
            return next;
        });

    const render = (row: PickerRow, depth: number): ReactElement[] => {
        const { node, children } = row;
        const open = unfolded || (depth === 0) !== toggled.has(node.path);
        const canPick = onPick !== undefined && (pickable?.(node) ?? true);
        const value = node.kind === "value" ? currentText(values.update, node.path, node.type) : undefined;
        if (node.kind === "value") shown.push(node.path);
        return [
            <div
                key={node.path}
                className="hb-tree-row"
                data-variable={node.path}
                style={{ paddingLeft: 4 + depth * 16 }}
                title={node.path}
                role={canPick ? "button" : undefined}
                aria-disabled={onPick !== undefined && !canPick ? true : undefined}
                onMouseDown={onDragStart ? (event) => onDragStart(node, event) : undefined}
                onClick={canPick ? () => onPick?.(node) : undefined}
            >
                {children.length > 0 ? (
                    <button
                        type="button"
                        className="hb-tree-fold"
                        data-expand={node.path}
                        aria-label={open ? `Fold ${node.name}` : `Unfold ${node.name}`}
                        aria-expanded={open}
                        onMouseDown={(event) => event.stopPropagation()}
                        onClick={(event) => {
                            event.stopPropagation();
                            toggle(node.path);
                        }}
                    >
                        {open ? "▾" : "▸"}
                    </button>
                ) : (
                    <span className="hb-tree-fold" />
                )}
                <span className="hb-tree-name">{node.name}</span>
                <span className="hb-tree-type">{node.type}</span>
                {value !== undefined ? (
                    <span className="hb-tree-value" data-part="current-value">
                        {value}
                    </span>
                ) : null}
            </div>,
            ...(open ? children.flatMap((child) => render(child, depth + 1)) : []),
        ];
    };

    const drawn = rows.flatMap((row) => render(row, 0));
    const watched = shown.join("\n");
    const { show } = values;
    useEffect(() => show(tree, watched ? watched.split("\n") : []), [show, tree, watched]);
    useEffect(() => () => show(tree, []), [show, tree]);
    return <div className="hb-tree">{drawn}</div>;
}
