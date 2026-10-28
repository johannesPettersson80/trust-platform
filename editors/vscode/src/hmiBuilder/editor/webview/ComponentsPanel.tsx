// The Components panel (§3.2): search, groups and a picture of each component, drawn by the
// shared drawing module. Click a component to place it at the centre of the view, or drag it onto
// the canvas. Its entries are the language server's (§21.3 item 1; in a P&ID what it may hold,
// §18 phase 4 item 1).

import { useMemo, useState } from "react";
import type { MouseEvent as ReactMouseEvent } from "react";

import { drawPage } from "../../drawing";
import { drawable } from "../../drawing/components";
import type { HmiObjectModel } from "../../drawing/model";
import { renderToString } from "../../drawing/svg";
import type { ComponentDefinition } from "../catalogue";
import type { FaceplateEntry } from "../messages";
import { componentName, GROUPS, inGroup } from "./names";

interface ComponentsPanelProps {
    /** The panel's entries as the language server gives them; none while it has not. */
    palette: ComponentDefinition[] | undefined;
    theme: "light" | "dark";
    onAdd: (component: ComponentDefinition) => void;
    onDragStart: (component: ComponentDefinition, event: ReactMouseEvent) => void;
    /** The project's and its libraries' faceplates, placed on the page for an instance (§8.9). */
    faceplates?: FaceplateEntry[];
    onPlaceFaceplate?: (faceplate: FaceplateEntry) => void;
    onFaceplateDragStart?: (faceplate: FaceplateEntry, event: ReactMouseEvent) => void;
}

/** Components whose picture shows words of their own rather than a sample value. */
const displayName = (component: ComponentDefinition): string =>
    component.equipment?.title ?? componentName(component.type);
const componentKey = (component: ComponentDefinition): string => component.equipment?.block ?? component.type;

const WORDED = ["text", "button", "pulse-button", "page-button"];

/** A component drawn on its own at its default size, bound to sample values. */
function preview(component: ComponentDefinition): string {
    const worded = WORDED.includes(component.type);
    const object: HmiObjectModel = {
        id: "preview",
        type: component.type,
        at: [0, 0],
        size: component.size,
        rotation: 0,
        bind: {},
        label: displayName(component),
        ...(worded ? { text: displayName(component) } : {}),
    };
    const page = { title: component.type, object: [object], pipe: [] };
    return renderToString(drawPage(page, { size: component.size, values: {} }));
}

export function ComponentsPanel({
    palette,
    theme,
    onAdd,
    onDragStart,
    faceplates = [],
    onPlaceFaceplate,
    onFaceplateDragStart,
}: ComponentsPanelProps) {
    const [search, setSearch] = useState("");
    const previews = useMemo(
        () => new Map((palette ?? []).map((component) => [componentKey(component), preview(component)])),
        [palette],
    );
    const wanted = search.trim().toLowerCase();
    const shown = (palette ?? []).filter(
        (component) => drawable(component.type) && (!wanted || displayName(component).toLowerCase().includes(wanted)),
    );
    const shownFaceplates = faceplates.filter(
        (faceplate) => !wanted || `${faceplate.title ?? ""} ${faceplate.name}`.toLowerCase().includes(wanted),
    );
    return (
        <aside className="hb-panel hb-components">
            <input
                className="hb-search"
                name="component-search"
                placeholder="Search components ( / )"
                value={search}
                onChange={(event) => setSearch(event.target.value)}
                onKeyDown={(event) => {
                    // Enter adds the first match; Escape leaves the search (§3.5).
                    const first = GROUPS.flatMap((group) => shown.filter((component) => inGroup(component, group)))[0];
                    if (event.key === "Enter" && first) onAdd(first);
                    if (event.key === "Escape") event.currentTarget.blur();
                }}
            />
            {palette ? null : <p className="hb-hint">Waiting for the truST language server…</p>}
            {GROUPS.map((group) => {
                const members = shown.filter((component) => inGroup(component, group));
                if (members.length === 0) return null;
                return (
                    <section key={group.id} data-component-group={group.id} className="hb-group">
                        <h2>{group.name}</h2>
                        <div className="hb-cards">
                            {members.map((component) => (
                                <button
                                    type="button"
                                    key={componentKey(component)}
                                    data-component={component.type}
                                    data-block={component.equipment?.block}
                                    className="hb-card"
                                    title={`Add ${displayName(component).toLowerCase()}`}
                                    onClick={() => onAdd(component)}
                                    onMouseDown={(event) => onDragStart(component, event)}
                                >
                                    <span
                                        className={`hb-card-preview hmi-theme-${theme}`}
                                        dangerouslySetInnerHTML={{
                                            __html: previews.get(componentKey(component)) ?? "",
                                        }}
                                    />
                                    <span className="hb-card-name">{displayName(component)}</span>
                                </button>
                            ))}
                        </div>
                    </section>
                );
            })}
            {shownFaceplates.length > 0 ? (
                <section data-component-group="faceplates" className="hb-group">
                    <h2>Panels</h2>
                    <div className="hb-faceplates">
                        {shownFaceplates.map((faceplate) => (
                            <button
                                type="button"
                                key={faceplate.name}
                                data-faceplate={faceplate.name}
                                className="hb-faceplate"
                                title={`Place ${faceplate.title ?? faceplate.name} on the page`}
                                onClick={() => onPlaceFaceplate?.(faceplate)}
                                onMouseDown={(event) => onFaceplateDragStart?.(faceplate, event)}
                            >
                                <span className="hb-faceplate-title">{faceplate.title ?? faceplate.name}</span>
                                <span className="hb-faceplate-name">{faceplate.name}</span>
                            </button>
                        ))}
                    </div>
                </section>
            ) : null}
        </aside>
    );
}
