// A project's own symbol, using the shared drawing and the normal HMI inspector (§14.4.7).
import { useEffect, useState } from "react";
import type { CustomSymbolModel } from "../../drawing/symbols/custom";
import { customSymbol } from "../../drawing/symbols/custom";
import type { Rule } from "../../drawing/symbols/rules";
import { applyRules } from "../../drawing/symbols/rules";
import type { HmiObjectModel, LiveValue } from "../../drawing/model";
import { el, findAll, renderToString } from "../../drawing/svg";
import { tokensCss } from "../../drawing/tokens";
import type { VsCodeApi } from "./vscodeApi";
import { Field } from "./Field";
import { SymbolFields } from "./SymbolFields";
import "./symbolEditor.css";

const DRIVES = [
    ["spin", "Turn while on", "boolean"],
    ["stroke", "Move to and fro while on", "boolean"],
    ["angle", "Angle", "number"],
    ["level", "Fill level", "number"],
    ["travel", "Move along its travel", "number"],
    ["equals", "Shown when equal to", "number"],
    ["show", "Show value", "any"],
    ["blink", "Blink while on", "boolean"],
    ["alarm", "Alarm colour", "boolean"],
    ["active", "Active colour", "boolean"],
] as const;
type Drive = (typeof DRIVES)[number][0];

export function SymbolEditor({ api, fontUrl }: { api: VsCodeApi; fontUrl?: string }) {
    const [model, setModel] = useState<CustomSymbolModel>();
    const [theme, setTheme] = useState<"dark" | "light">("dark");
    const [part, setPart] = useState<string>();
    const [drive, setDrive] = useState<Drive>("spin");
    const [values, setValues] = useState<Record<string, LiveValue>>({});
    const [errors, setErrors] = useState<string[]>([]);
    const [invalid, setInvalid] = useState(false);
    useEffect(() => {
        const receive = (event: MessageEvent) => {
            if (event.data.type === "symbol") {
                setModel(event.data.model);
                setTheme(event.data.theme ?? "dark");
                setErrors([]);
                setInvalid(false);
            } else if (event.data.type === "invalid" || event.data.type === "editRefused") {
                setErrors((event.data.errors ?? []).map((error: { message: string }) => error.message));
                setInvalid(event.data.type === "invalid");
            }
        };
        window.addEventListener("message", receive);
        api.postMessage({ type: "ready" });
        return () => window.removeEventListener("message", receive);
    }, [api]);
    const banner =
        errors.length > 0 ? (
            <div className="hb-banner" role="alert">
                <strong>{invalid ? "Fix the text to resume editing" : "The edit was not applied"}</strong>
                {errors.map((error, index) => (
                    <div key={index}>{error}</div>
                ))}
            </div>
        ) : null;
    const openText = (
        <button type="button" data-action="open-text" onClick={() => api.postMessage({ type: "openAsText" })}>
            Open as text
        </button>
    );
    if (!model)
        return (
            <div className="hb-app" data-view="symbol-editor">
                <header className="hb-topbar">
                    <strong className="hb-title">My Symbols</strong>
                    {openText}
                </header>
                {banner}
                <p className="hb-hint">{invalid ? "Open as text to fix the definition." : "Loading symbol…"}</p>
            </div>
        );
    const prefix = `my-${model.name}-`;
    const elements = findAll(model.svg, (node) => node.attrs.id?.startsWith(prefix)).map((node) =>
        node.attrs.id.slice(prefix.length),
    );
    const rule = model.parts.find((candidate) => candidate.part === part);
    const selectedProperty = rule && drive in rule ? (rule as unknown as Record<string, string>)[drive] : "";
    const selectPart = (id: string) => {
        setPart(id);
        const existing = model.parts.find((candidate) => candidate.part === id);
        setDrive(DRIVES.find(([key]) => existing && key in existing)?.[0] ?? "spin");
    };
    const setProperty = (property: string) => {
        if (invalid || !part || !property) return;
        // A state is shown at one number; a travel moves the part, upwards by default.
        const extra =
            drive === "equals"
                ? { value: 0 }
                : drive === "travel"
                  ? { range: [0, 100], move: [0, -Math.round(model.viewBox[3] / 4)] }
                  : {};
        const changed = { part, [drive]: property, ...extra } as Rule;
        const parts = model.parts.some((candidate) => candidate.part === part)
            ? model.parts.map((candidate) => (candidate.part === part ? changed : candidate))
            : [...model.parts, changed];
        api.postMessage({ type: "editSymbol", model: { ...model, parts } });
    };
    const definition = customSymbol(model);
    const object: HmiObjectModel = {
        id: model.name,
        type: `my:${model.name}`,
        at: [0, 0],
        size: model.size,
        rotation: 0,
        bind: {},
    };
    const parts = applyRules(model.parts, (name) => values[name], { size: model.size, values, nowMs: 0 }, object);
    const preview = el(
        "svg",
        { viewBox: `0 0 ${definition.size.join(" ")}`, class: "hmi-page", role: "img", "aria-label": model.title },
        definition.draw({ unknown: false, parts }),
    );
    for (const node of findAll(preview, (node) => node.attrs.id === `${prefix}${part}`))
        node.attrs = { ...node.attrs, "data-picked": "true" };
    const needs = DRIVES.find(([key]) => key === drive)?.[2];
    return (
        <div className="hb-app" data-view="symbol-editor">
            <style>{tokensCss({}, fontUrl)}</style>
            <header className="hb-topbar">
                <strong className="hb-title">{model.title}</strong>
                <span className="hb-hint">My Symbols</span>
                {openText}
            </header>
            {banner}
            <div className="hb-body hb-symbol-body">
                <aside className="hb-panel">
                    <div className="hb-panel-title">Drawing parts</div>
                    <p className="hb-hint">Select a part to give it a rule.</p>
                    {elements.map((id) => (
                        <button
                            key={id}
                            className="hb-symbol-part"
                            aria-pressed={part === id}
                            onClick={() => selectPart(id)}
                        >
                            {id}
                        </button>
                    ))}
                    <section className="hb-section">
                        <h2>Preview values</h2>
                        <p className="hb-hint">Try the motion here. These values are not saved.</p>
                        {Object.entries(model.properties).map(([name, kind]) =>
                            kind === "boolean" ? (
                                <label className="hb-field" key={name}>
                                    <span className="hb-field-label">{name}</span>
                                    <input
                                        name={`preview-${name}`}
                                        type="checkbox"
                                        checked={values[name]?.value === true}
                                        onChange={(event) =>
                                            setValues({
                                                ...values,
                                                [name]: { value: event.target.checked, state: "confirmed" },
                                            })
                                        }
                                    />
                                </label>
                            ) : (
                                <Field
                                    key={name}
                                    name={`preview-${name}`}
                                    label={name}
                                    numeric={kind === "number"}
                                    value={String(values[name]?.value ?? (kind === "number" ? 0 : ""))}
                                    onCommit={(value) => {
                                        const parsed = kind === "number" ? Number(value) : value;
                                        if (typeof parsed !== "number" || Number.isFinite(parsed))
                                            setValues({ ...values, [name]: { value: parsed, state: "confirmed" } });
                                    }}
                                />
                            ),
                        )}
                    </section>
                </aside>
                <main
                    className={`hb-symbol-preview hmi-theme-${theme}`}
                    onClick={(event) => {
                        const node = (event.target as Element).closest("[id]");
                        if (node?.id.startsWith(prefix)) selectPart(node.id.slice(prefix.length));
                    }}
                    dangerouslySetInnerHTML={{ __html: renderToString(preview) }}
                />
                <aside className="hb-panel hb-inspector">
                    <div className="hb-panel-title">{part ?? "Symbol"}</div>
                    {part ? (
                        <section className="hb-section">
                            <h2>Animation rule</h2>
                            <label className="hb-field">
                                <span className="hb-field-label">Behaviour</span>
                                <select
                                    name="drive"
                                    disabled={invalid}
                                    value={drive}
                                    onChange={(event) => setDrive(event.target.value as Drive)}
                                >
                                    {DRIVES.map(([key, title]) => (
                                        <option key={key} value={key}>
                                            {title}
                                        </option>
                                    ))}
                                </select>
                            </label>
                            <label className="hb-field">
                                <span className="hb-field-label">Binding</span>
                                <select
                                    name="property"
                                    disabled={invalid}
                                    value={selectedProperty}
                                    onChange={(event) => setProperty(event.target.value)}
                                >
                                    <option value="">Choose a binding</option>
                                    {Object.entries(model.properties)
                                        .filter(([, kind]) => needs === "any" || kind === needs)
                                        .map(([name]) => (
                                            <option key={name} value={name}>
                                                {name}
                                            </option>
                                        ))}
                                </select>
                            </label>
                            {drive === "equals" && rule && "equals" in rule && (
                                <Field
                                    name="equals-value"
                                    label="Shown at"
                                    numeric
                                    readOnly={invalid}
                                    value={rule.value}
                                    onCommit={(value) => {
                                        const shownAt = Number(value);
                                        if (invalid || !Number.isFinite(shownAt)) return;
                                        api.postMessage({
                                            type: "editSymbol",
                                            model: {
                                                ...model,
                                                parts: model.parts.map((candidate) =>
                                                    candidate === rule ? { ...rule, value: shownAt } : candidate,
                                                ),
                                            },
                                        });
                                    }}
                                />
                            )}
                            {drive === "travel" && rule && "travel" in rule && (
                                <div className="hb-pair">
                                    {([0, 1] as const).map((index) => (
                                        <Field
                                            key={index}
                                            name={index === 0 ? "move-x" : "move-y"}
                                            label={index === 0 ? "Moves by X" : "Moves by Y"}
                                            numeric
                                            readOnly={invalid}
                                            value={(rule.move ?? [0, 0])[index]}
                                            onCommit={(value) => {
                                                const move: [number, number] = [...(rule.move ?? [0, 0])];
                                                move[index] = Number(value);
                                                if (invalid || !move.every(Number.isFinite)) return;
                                                api.postMessage({
                                                    type: "editSymbol",
                                                    model: {
                                                        ...model,
                                                        parts: model.parts.map((candidate) =>
                                                            candidate === rule ? { ...rule, move } : candidate,
                                                        ),
                                                    },
                                                });
                                            }}
                                        />
                                    ))}
                                </div>
                            )}
                            {rule &&
                                (drive === "level" || drive === "travel") &&
                                ("level" in rule || "travel" in rule) && (
                                    <div className="hb-pair">
                                        {([0, 1] as const).map((index) => (
                                            <Field
                                                key={index}
                                                name={index === 0 ? "range-min" : "range-max"}
                                                label={index === 0 ? "Empty" : "Full"}
                                                numeric
                                                readOnly={invalid}
                                                value={(rule.range ?? [0, 100])[index]}
                                                onCommit={(value) => {
                                                    const range: [number, number] = [...(rule.range ?? [0, 100])];
                                                    range[index] = Number(value);
                                                    if (
                                                        invalid ||
                                                        !range.every(Number.isFinite) ||
                                                        range[0] >= range[1]
                                                    )
                                                        return;
                                                    api.postMessage({
                                                        type: "editSymbol",
                                                        model: {
                                                            ...model,
                                                            parts: model.parts.map((candidate) =>
                                                                candidate === rule ? { ...rule, range } : candidate,
                                                            ),
                                                        },
                                                    });
                                                }}
                                            />
                                        ))}
                                    </div>
                                )}
                            {rule && (
                                <button
                                    type="button"
                                    data-action="remove-rule"
                                    disabled={invalid}
                                    onClick={() =>
                                        api.postMessage({
                                            type: "editSymbol",
                                            model: {
                                                ...model,
                                                parts: model.parts.filter((candidate) => candidate.part !== part),
                                            },
                                        })
                                    }
                                >
                                    Remove rule
                                </button>
                            )}
                        </section>
                    ) : (
                        <p className="hb-hint">Select a part in the drawing or the list.</p>
                    )}
                    <SymbolFields
                        model={model}
                        disabled={invalid}
                        change={(next) => api.postMessage({ type: "editSymbol", model: next })}
                    />
                </aside>
            </div>
        </div>
    );
}
