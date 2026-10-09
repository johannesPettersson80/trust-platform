// The bottom panel (§3.2), closed at first. Tags: the program's variables, to drag onto the page
// or onto a binding in the inspector (§7.1). Texts: every text in every language (§14.5 item 7).

import { useMemo, useState } from "react";
import type { MouseEvent as ReactMouseEvent } from "react";

import type { ProjectTexts, VariableNode } from "../messages";
import { matchesSearch, pickerTree } from "../variables";
import { TextsTable } from "./TextsTable";
import { VariableTree } from "./VariableTree";

export type BottomTab = "tags" | "texts";

interface BottomPanelProps {
    variables: VariableNode[] | undefined;
    problem: string | undefined;
    /** Open, and how tall, from the panel arrangement (§3.2 item 7). */
    open: boolean;
    height: number;
    /** The tab shown while open. */
    tab: BottomTab;
    /** A tab's button: it opens the panel on that tab, or closes it when that tab is shown. */
    onTab: (tab: BottomTab) => void;
    onResizeStart: (event: ReactMouseEvent) => void;
    onDragStart: (node: VariableNode, event: ReactMouseEvent) => void;
    /** The project's texts, once known. */
    texts?: ProjectTexts;
    onTranslate: (language: string, text: string, translation: string) => void;
}

export function BottomPanel({
    variables,
    problem,
    open,
    height,
    tab,
    onTab,
    onResizeStart,
    onDragStart,
    texts,
    onTranslate,
}: BottomPanelProps) {
    const [search, setSearch] = useState("");
    const searching = search.trim() !== "";
    const rows = useMemo(
        () => pickerTree(variables ?? [], (node) => !searching || matchesSearch(node, search)),
        [variables, search, searching],
    );
    let content;
    if (problem)
        content = (
            <p className="hb-hint" data-part="tags-problem">
                Variables appear when the program compiles. {problem}
            </p>
        );
    else if (variables === undefined) content = <p className="hb-hint">Reading the program's variables…</p>;
    else if (rows.length === 0)
        content = <p className="hb-hint">{searching ? "No variable matches." : "The program has no variables."}</p>;
    else content = <VariableTree rows={rows} unfolded={searching} onDragStart={onDragStart} />;
    return (
        <section className="hb-bottom" data-part="bottom-panel">
            {open ? (
                <div
                    className="hb-resize hb-resize-y"
                    data-resize="bottom"
                    title="Drag to resize"
                    onMouseDown={onResizeStart}
                />
            ) : null}
            <div className="hb-bottom-tabs" role="tablist">
                <button
                    type="button"
                    role="tab"
                    data-action="toggle-tags"
                    aria-selected={open && tab === "tags"}
                    aria-expanded={open && tab === "tags"}
                    onClick={() => onTab("tags")}
                >
                    Tags
                </button>
                {texts ? (
                    <button
                        type="button"
                        role="tab"
                        data-action="toggle-texts"
                        aria-selected={open && tab === "texts"}
                        aria-expanded={open && tab === "texts"}
                        onClick={() => onTab("texts")}
                    >
                        Texts
                    </button>
                ) : null}
            </div>
            {open && tab === "texts" && texts ? (
                <div className="hb-texts-panel" style={{ height }}>
                    <TextsTable texts={texts} onTranslate={onTranslate} />
                </div>
            ) : open ? (
                <div className="hb-tags" style={{ height }}>
                    <div className="hb-tags-head">
                        <input
                            className="hb-search"
                            name="tags-search"
                            placeholder="Search variables"
                            value={search}
                            onChange={(event) => setSearch(event.target.value)}
                        />
                        <span className="hb-hint">
                            Drag a variable onto the page, or onto a binding in the inspector.
                        </span>
                    </div>
                    {content}
                </div>
            ) : null}
        </section>
    );
}
