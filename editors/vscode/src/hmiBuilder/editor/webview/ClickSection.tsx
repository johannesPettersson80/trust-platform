// The inspector's click section (§4.9, §5.4, §8.7): an object goes to a page, opens a popup of
// the project, or opens a faceplate of the project or its libraries, for the instance the click
// names or else the one its bindings name.

import type { HmiObjectModel } from "../../drawing/model";
import { Field } from "./Field";

/** The project's pages and popups, and the faceplates of the project and its libraries. */
export interface ClickTargets {
    pages: string[];
    popups: string[];
    faceplates?: string[];
}

interface ClickSectionProps {
    object: HmiObjectModel;
    targets: ClickTargets;
    readOnly: boolean;
    onChange: (object: HmiObjectModel) => void;
}

export function ClickSection({ object, targets, readOnly, onChange }: ClickSectionProps) {
    const current = object.click ? Object.entries(object.click).find(([key]) => key !== "instance") : undefined;
    const kind = current?.[0] === "page" || current?.[0] === "popup" || current?.[0] === "faceplate" ? current[0] : "";
    const faceplates = targets.faceplates ?? [];
    const names =
        kind === "page" ? targets.pages : kind === "popup" ? targets.popups : kind === "faceplate" ? faceplates : [];
    // A target the lists do not have yet (a file not created) stays selectable.
    const listed = current && !names.includes(current[1]) ? [current[1], ...names] : names;
    const instance = object.instance ?? object.click?.instance;
    const change = (
        nextKind: string,
        name: string | undefined,
        nextInstance = nextKind === "faceplate" ? instance : undefined,
    ): void => {
        const next = { ...object } as Record<string, unknown>;
        if (!nextKind || !name) delete next.click;
        else next.click = { [nextKind]: name, ...(nextInstance ? { instance: nextInstance } : {}) };
        onChange(next as unknown as HmiObjectModel);
    };
    const first = (next: string): string | undefined =>
        next === "page"
            ? targets.pages[0]
            : next === "popup"
              ? targets.popups[0]
              : next === "faceplate"
                ? (object.faceplate ?? faceplates[0])
                : undefined;
    return (
        <section className="hb-section" data-part="click">
            <h2>On click</h2>
            <label className="hb-field">
                <span className="hb-field-label">Does</span>
                <select
                    name="click-kind"
                    value={kind}
                    disabled={readOnly}
                    onChange={(event) => {
                        const next = event.target.value;
                        change(next, first(next));
                    }}
                >
                    <option value="">Nothing</option>
                    <option value="page">Go to a page</option>
                    <option value="popup">Open a popup</option>
                    <option value="faceplate">Open equipment panel</option>
                </select>
            </label>
            {kind ? (
                <label className="hb-field">
                    <span className="hb-field-label">
                        {kind === "page" ? "Page" : kind === "popup" ? "Panel" : "Equipment panel"}
                    </span>
                    <select
                        name="click-target"
                        value={current?.[1] ?? ""}
                        disabled={readOnly}
                        onChange={(event) => change(kind, event.target.value)}
                    >
                        {listed.map((name) => (
                            <option key={name} value={name}>
                                {name}
                            </option>
                        ))}
                    </select>
                </label>
            ) : null}
            {kind === "faceplate" && current && !object.block ? (
                <>
                    <Field
                        name="click-instance"
                        label="Instance"
                        value={instance}
                        readOnly={readOnly}
                        onCommit={(text) => change(kind, current[1], text || undefined)}
                    />
                    <p className="hb-hint">Empty: the instance the object's bindings name.</p>
                </>
            ) : null}
        </section>
    );
}
