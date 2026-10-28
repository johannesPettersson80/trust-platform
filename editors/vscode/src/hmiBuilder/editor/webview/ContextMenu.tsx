// The canvas's context menu (specification 39, §3.3 item 8): cut, copy, paste, duplicate, delete,
// group, layer order, align, lock, and on a symbol, open its panel or place it on the page
// (§8.9 item 1). Each entry does what its
// key does; Escape or a press elsewhere closes it. Near the window's edge it opens inward.

import { useEffect, useLayoutEffect, useRef, useState } from "react";

import type { HmiPageModel } from "../../drawing/model";
import type { AlignEdge, DistributeAxis } from "../align";
import type { LayerMove } from "../layers";
import { panelOf } from "../placed";

export type EditAction =
    | "cut"
    | "copy"
    | "paste"
    | "duplicate"
    | "delete"
    | "group"
    | "ungroup"
    | LayerMove
    | `align-${AlignEdge}`
    | `distribute-${DistributeAxis}`
    | "lock"
    | "unlock"
    | "faceplate"
    | "place-faceplate";

type Entry = { action: EditAction; label: string; keys?: string } | "separator";

/** The entries for what is selected. */
export function menuEntries(page: HmiPageModel, selection: string[]): Entry[] {
    const objects = page.object.filter((object) => selection.includes(object.id));
    const entries: Entry[] = [];
    if (objects.length > 0) {
        entries.push(
            { action: "cut", label: "Cut", keys: "Ctrl+X" },
            { action: "copy", label: "Copy", keys: "Ctrl+C" },
        );
    }
    entries.push({ action: "paste", label: "Paste", keys: "Ctrl+V" });
    if (objects.length === 0) return entries;
    entries.push(
        { action: "duplicate", label: "Duplicate", keys: "Ctrl+D" },
        { action: "delete", label: "Delete", keys: "Del" },
        "separator",
    );
    if (objects.length > 1) entries.push({ action: "group", label: "Group", keys: "Ctrl+G" });
    if (objects.some((object) => object.group))
        entries.push({ action: "ungroup", label: "Ungroup", keys: "Ctrl+Shift+G" });
    entries.push(
        { action: "forward", label: "Bring forward", keys: "Ctrl+]" },
        { action: "backward", label: "Send backward", keys: "Ctrl+[" },
        { action: "front", label: "Bring to front", keys: "Ctrl+Shift+]" },
        { action: "back", label: "Send to back", keys: "Ctrl+Shift+[" },
    );
    if (objects.length > 1) {
        entries.push(
            "separator",
            { action: "align-left", label: "Align left" },
            { action: "align-centre", label: "Align centre" },
            { action: "align-right", label: "Align right" },
            { action: "align-top", label: "Align top" },
            { action: "align-middle", label: "Align middle" },
            { action: "align-bottom", label: "Align bottom" },
        );
        if (objects.length > 2)
            entries.push(
                { action: "distribute-horizontal", label: "Distribute horizontally" },
                { action: "distribute-vertical", label: "Distribute vertically" },
            );
    }
    entries.push(
        "separator",
        objects.every((object) => object.locked)
            ? { action: "unlock", label: "Unlock" }
            : { action: "lock", label: "Lock" },
    );
    if (objects.length === 1 && panelOf(objects[0])) {
        entries.push(
            { action: "faceplate", label: "Open panel" },
            { action: "place-faceplate", label: "Place panel on page" },
        );
    }
    return entries;
}

interface ContextMenuProps {
    at: [number, number];
    entries: Entry[];
    onAction: (action: EditAction) => void;
    onClose: () => void;
}

export function ContextMenu({ at, entries, onAction, onClose }: ContextMenuProps) {
    const element = useRef<HTMLDivElement>(null);
    const [place, setPlace] = useState(at);
    // The whole menu shows: it moves in from an edge it would cross, 4 px from it.
    useLayoutEffect(() => {
        const size = element.current?.getBoundingClientRect();
        if (!size) return;
        setPlace([
            Math.max(4, Math.min(at[0], window.innerWidth - size.width - 4)),
            Math.max(4, Math.min(at[1], window.innerHeight - size.height - 4)),
        ]);
    }, [at]);
    useEffect(() => {
        const onKey = (event: KeyboardEvent): void => {
            if (event.key === "Escape") onClose();
        };
        const onPress = (event: MouseEvent): void => {
            if (!element.current?.contains(event.target as Node)) onClose();
        };
        window.addEventListener("keydown", onKey);
        window.addEventListener("mousedown", onPress, { capture: true });
        return () => {
            window.removeEventListener("keydown", onKey);
            window.removeEventListener("mousedown", onPress, { capture: true });
        };
    }, [onClose]);
    return (
        <div
            ref={element}
            data-part="context-menu"
            className="hb-context-menu"
            role="menu"
            style={{ left: `${place[0]}px`, top: `${place[1]}px` }}
        >
            {entries.map((entry, index) =>
                entry === "separator" ? (
                    <hr key={`s${index}`} />
                ) : (
                    <button
                        key={entry.action}
                        type="button"
                        role="menuitem"
                        data-menu={entry.action}
                        onClick={() => {
                            onClose();
                            onAction(entry.action);
                        }}
                    >
                        <span>{entry.label}</span>
                        {entry.keys ? <kbd>{entry.keys}</kbd> : null}
                    </button>
                ),
            )}
        </div>
    );
}
