// The inspector when nothing is selected (§4.6, §4.9, §8.7): a page's title and its place in the
// navigation, a popup's title, size and pin, and a faceplate's description and fields.

import { useState } from "react";

import type { HmiPageModel } from "../../drawing/model";
import { Field } from "./Field";

/** The kinds of value a field can take, as the catalogue names them (§5.6); none takes any. */
const KINDS = ["number", "boolean", "text", "time", "enumeration", "structure", "array", "value"];

/** The navigation icons truST ships (§4.6). */
const ICONS = [
    "overview",
    "process",
    "trend",
    "alarm",
    "settings",
    "pump",
    "valve",
    "tank",
    "motor",
    "heat",
    "flow",
    "power",
    "list",
    "info",
];

interface PageSectionProps {
    page: HmiPageModel;
    readOnly: boolean;
    onCommit: (page: HmiPageModel) => void;
}

/** The page with a key set, or removed when the value is empty. */
function withKey(page: HmiPageModel, key: keyof HmiPageModel, value: unknown): HmiPageModel {
    const next = { ...page } as Record<string, unknown>;
    if (value === undefined || value === "" || value === false) delete next[key];
    else next[key] = value;
    return next as unknown as HmiPageModel;
}

/** A whole number from `minimum`, or undefined. */
function whole(text: string, minimum: number): number | undefined {
    const value = Number(text);
    return text === "" || !Number.isInteger(value) || value < minimum ? undefined : value;
}

export function PageSection({ page, readOnly, onCommit }: PageSectionProps) {
    const set = (key: keyof HmiPageModel, value: unknown): void => onCommit(withKey(page, key, value));
    const popup = page.size !== undefined;
    return (
        <>
            <Field
                name="title"
                label="Title"
                value={page.title}
                readOnly={readOnly}
                onCommit={(title) => title && onCommit({ ...page, title })}
            />
            {popup ? (
                <section className="hb-section" data-part="popup">
                    <h2>Size</h2>
                    <div className="hb-pair">
                        <Field
                            name="width"
                            label="Width"
                            value={page.size?.[0]}
                            numeric
                            readOnly={readOnly}
                            onCommit={(text) => {
                                const width = whole(text, 1);
                                if (width && page.size) set("size", [width, page.size[1]]);
                            }}
                        />
                        <Field
                            name="height"
                            label="Height"
                            value={page.size?.[1]}
                            numeric
                            readOnly={readOnly}
                            onCommit={(text) => {
                                const height = whole(text, 1);
                                if (height && page.size) set("size", [page.size[0], height]);
                            }}
                        />
                    </div>
                    <label className="hb-check">
                        <input
                            type="checkbox"
                            name="pinnable"
                            checked={page.pinnable === true}
                            disabled={readOnly}
                            onChange={(event) => set("pinnable", event.target.checked)}
                        />
                        <span>Can be pinned open beside the page</span>
                    </label>
                </section>
            ) : null}
            {page.fields !== undefined && (!page.panel || Object.keys(page.fields).length > 0) ? (
                <FaceplateSection page={page} readOnly={readOnly} onCommit={onCommit} />
            ) : null}
            {popup ? null : (
                <section className="hb-section" data-part="navigation">
                    <h2>Navigation</h2>
                    <div className="hb-pair">
                        <Field
                            name="order"
                            label="Order"
                            value={page.order}
                            numeric
                            readOnly={readOnly}
                            onCommit={(text) => set("order", whole(text, 0))}
                        />
                        <label className="hb-field">
                            <span className="hb-field-label">Icon</span>
                            <select
                                name="icon"
                                value={page.icon ?? ""}
                                disabled={readOnly}
                                onChange={(event) => set("icon", event.target.value)}
                            >
                                <option value="">None</option>
                                {ICONS.map((icon) => (
                                    <option key={icon} value={icon}>
                                        {icon}
                                    </option>
                                ))}
                            </select>
                        </label>
                    </div>
                    <Field
                        name="group"
                        label="Group"
                        value={page.group}
                        readOnly={readOnly}
                        onCommit={(text) => set("group", text)}
                    />
                    <label className="hb-check">
                        <input
                            type="checkbox"
                            name="hidden"
                            checked={page.hidden === true}
                            disabled={readOnly}
                            onChange={(event) => set("hidden", event.target.checked)}
                        />
                        <span>Hidden from the navigation</span>
                    </label>
                </section>
            )}
            <p className="hb-hint">Select an object to change it, or add one from Components.</p>
        </>
    );
}

/** A faceplate's description and fields (§8.7): a field in use by a binding stays. */
function FaceplateSection({ page, readOnly, onCommit }: PageSectionProps) {
    const [name, setName] = useState("");
    const [kind, setKind] = useState("");
    const fields = page.fields ?? {};
    const bound = new Set(page.object.flatMap((object) => Object.values(object.bind).map((binding) => binding.path)));
    const valid = /^[A-Za-z_][A-Za-z0-9_]*$/.test(name) && !(name in fields);
    return (
        <section className="hb-section" data-part="faceplate">
            <h2>Faceplate</h2>
            <Field
                name="description"
                label="Description"
                value={page.description}
                readOnly={readOnly}
                onCommit={(text) => onCommit(withKey(page, "description", text))}
            />
            <span className="hb-field-label">Fields</span>
            <ul className="hb-fields">
                {Object.entries(fields).map(([field, definition]) => {
                    const used = bound.has(`$${field}`);
                    return (
                        <li key={field} data-field={field}>
                            <code>${field}</code>
                            <span className="hb-hint">
                                {definition.accepts.length > 0 ? definition.accepts.join(", ") : "any value"}
                            </span>
                            <button
                                type="button"
                                className="hb-secondary"
                                data-action="remove-field"
                                disabled={readOnly || used}
                                title={used ? `A binding uses $${field}` : `Remove $${field}`}
                                onClick={() => {
                                    const next = { ...fields };
                                    delete next[field];
                                    onCommit({ ...page, fields: next });
                                }}
                            >
                                ✕
                            </button>
                        </li>
                    );
                })}
            </ul>
            <div className="hb-pair">
                <label className="hb-field">
                    <span className="hb-field-label">New field</span>
                    <input
                        name="new-field"
                        value={name}
                        disabled={readOnly}
                        spellCheck={false}
                        onChange={(event) => setName(event.target.value.trim())}
                    />
                </label>
                <label className="hb-field">
                    <span className="hb-field-label">Takes</span>
                    <select
                        name="new-field-kind"
                        value={kind}
                        disabled={readOnly}
                        onChange={(event) => setKind(event.target.value)}
                    >
                        <option value="">Any value</option>
                        {KINDS.map((option) => (
                            <option key={option} value={option}>
                                {option}
                            </option>
                        ))}
                    </select>
                </label>
            </div>
            <button
                type="button"
                data-action="add-field"
                disabled={readOnly || !valid}
                onClick={() => {
                    onCommit({ ...page, fields: { ...fields, [name]: { accepts: kind ? [kind] : [] } } });
                    setName("");
                    setKind("");
                }}
            >
                Add field
            </button>
            <p className="hb-hint">
                Objects bind a field as $Name; the faceplate's pair maps it to the equipment's member.
            </p>
        </section>
    );
}
