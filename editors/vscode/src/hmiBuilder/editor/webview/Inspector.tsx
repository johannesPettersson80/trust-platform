import { PanelSettings } from "./PanelSettings";
import { EquipmentSection } from "./EquipmentSection";
// The inspector (§3.2): the selected object's design and bindings; the page itself when nothing
// is selected. Every change is committed as one edit. A binding takes a variable typed, picked
// from the variables it can have, or dropped from the Tags panel (§7.1).

import { ArrangeSection } from "./ArrangeSection";
import { AlignSection } from "./AlignSection";
import { AttachSection } from "./AttachSection";
import { ColoursSection } from "./ColoursSection";
import type { QualityWarning } from "../quality";
import { useEffect, useState } from "react";
import type { MutableRefObject } from "react";

import type { HmiObjectModel, HmiPageModel } from "../../drawing/model";
import type { Catalogue, EquipmentDefinition } from "../catalogue";
import { deleteObjects, updateObject } from "../editing";
import type { VariableNode, ScreenModel } from "../messages";
import { deletePipes, pipeIndex } from "../pipes";
import { bound } from "../variables";
import { BindingPicker } from "./BindingPicker";
import { ClickSection, ClickTargets } from "./ClickSection";
import { ControlSection } from "./ControlSection";
import { Field } from "./Field";
import { componentName } from "./names";
import { PageSection } from "./PageSection";
import { PanelTitle } from "./PanelTitle";
import { VisibilitySection } from "./VisibilitySection";
import { FittingInstances, PlacedSection } from "./PlacedSection";
import { PipeSection } from "./PipeSection";
import { PreviewSection } from "./PreviewSection";
import { LimitsSection } from "./LimitsSection";
import { TrendScalesSection } from "./TrendScalesSection";
import { penName } from "../objectName";
import { useRust, withObject } from "./rust";

/** A variable being dragged from the Tags panel; a binding that takes the drop marks it handled. */
export interface VariableDrag {
    node: VariableNode;
    moved: boolean;
    handled: boolean;
}

interface InspectorProps {
    equipmentEditor?: import("./EquipmentConfiguration").EquipmentEditing;
    page: HmiPageModel;
    selection: string[];
    catalogue: Catalogue | undefined;
    /** The definition each equipment object shows, by the block it records (§8.5), as the language
     * server gives them. */
    definitions?: Record<string, EquipmentDefinition>;
    paused: boolean;
    variables: VariableNode[] | undefined;
    variablesProblem: string | undefined;
    variableDrag: MutableRefObject<VariableDrag | undefined>;
    /** The project's pages and popups, for click targets. */
    targets: ClickTargets;
    /** The selected object's quality warnings (§14.1 item 4). */
    warnings?: QualityWarning[];
    /** The selected object's binding problems (§7.6), and letting the runtime display write a
     * point the write policy does not cover yet (§12.2.3). */
    problems?: string[];
    /** The point each problem names that no write rule covers, as the language server names it. */
    points?: Record<string, string>;
    onAllowWriting?: (point: string) => void;
    /** The screen layout with the project's colours, and their warnings (§14.1 items 2 and 4). */
    screen: ScreenModel;
    colourWarnings: QualityWarning[];
    onColour: (theme: "dark" | "light", token: string, value: string | undefined) => void;
    onCommit: (page: HmiPageModel) => void;
    onSelect: (selection: string[]) => void;
    onNotice: (text: string) => void;
    onCollapse: () => void;
    /** The language the page's texts are written in, while the canvas shows another (§14.5 item 7). */
    writtenIn?: string;
    /** The state switcher in Design mode (§5.3 item 6): each object's previewed state, by id. */
    preview?: { states: Record<string, string>; onPreview: (id: string, state: string | undefined) => void };
    /** Live mode's test values set by hand (§11 item 2), by path. */
    /** For a placed faceplate: the instances its faceplate fits, asking for them, and opening its
     * faceplate (§8.9). */
    placing?: {
        studio?: (id: string) => void;
        answer: FittingInstances | undefined;
        ask: (faceplate: string) => void;
        open: (faceplate: string) => void;
    };
}

/** A binding's name as the inspector writes it; the visibility binding says what it does. */
function bindingLabel(name: string): string {
    return name === "visible" ? "Visible while" : componentName(name);
}

/** Text fields: an empty field removes the key. */
const TEXT_KEYS = ["tag", "label", "text", "unit"] as const;

/** A text field's name, with the language its text is written in while the canvas shows another. */
function writtenLabel(name: string, language: string | undefined): string {
    return language ? `${name} · ${language}` : name;
}
const NUMBER_KEYS = ["decimals", "min", "max", "span"] as const;
const TEXT_TYPES = ["text", "button", "pulse-button", "page-button"];

function withKey(object: HmiObjectModel, key: keyof HmiObjectModel, value: unknown): HmiObjectModel {
    const next = { ...object } as Record<string, unknown>;
    if (value === undefined || value === "") delete next[key];
    else next[key] = value;
    return next as unknown as HmiObjectModel;
}

/** The pictures the project's own equipment may show: the plain device and My Symbols (§21.9). */
function pictures(catalogue: Catalogue | undefined): Array<{ type: string; title: string }> {
    return (catalogue?.components ?? [])
        .filter((component) => component.type === "device" || component.group === "my-symbols")
        .map((component) => ({ type: component.type, title: componentName(component.type) }));
}

export function Inspector({
    page,
    selection,
    catalogue,
    definitions,
    paused,
    variables,
    variablesProblem,
    variableDrag,
    targets,
    warnings,
    problems,
    points,
    onAllowWriting,
    screen,
    colourWarnings,
    onColour,
    onCommit,
    onSelect,
    onNotice,
    onCollapse,
    writtenIn,
    preview,
    placing,
    equipmentEditor,
}: InspectorProps) {
    const replace = (object: HmiObjectModel): void =>
        onCommit({ ...page, object: page.object.map((item) => (item.id === object.id ? object : item)) });
    const [picking, setPicking] = useState<string>();
    const selected = selection.length === 1 ? selection[0] : undefined;
    useEffect(() => setPicking(undefined), [selected]);
    const { call, change } = useRust();

    if (selection.length === 0) {
        return (
            <aside className="hb-panel hb-inspector">
                <PanelTitle
                    title={page.panel ? "Shared panel" : page.size ? "Panel" : "Page"}
                    panel="inspector"
                    onCollapse={onCollapse}
                />
                <PanelSettings page={page} pages={targets.pages} readOnly={paused} onCommit={onCommit} />
                <PageSection page={page} readOnly={paused} onCommit={onCommit} />
                <ColoursSection screen={screen} warnings={colourWarnings} readOnly={paused} onColour={onColour} />
            </aside>
        );
    }
    if (selection.length > 1) {
        return (
            <aside className="hb-panel hb-inspector">
                <PanelTitle title={`${selection.length} objects selected`} panel="inspector" onCollapse={onCollapse} />
                <AlignSection page={page} selection={selection} readOnly={paused} onCommit={onCommit} />
                <VisibilitySection
                    page={page}
                    selection={selection}
                    catalogue={catalogue}
                    variables={variables}
                    problem={variablesProblem}
                    readOnly={paused}
                    onCommit={onCommit}
                />
                <button
                    type="button"
                    className="hb-danger"
                    disabled={paused}
                    onClick={() => {
                        onCommit(deleteObjects(deletePipes(page, selection), selection));
                        onSelect([]);
                    }}
                >
                    Delete
                </button>
            </aside>
        );
    }

    const pipe = pipeIndex(selection[0]);
    if (pipe !== undefined && page.pipe[pipe]) {
        return (
            <aside className="hb-panel hb-inspector">
                <PanelTitle title="Pipe" panel="inspector" onCollapse={onCollapse} />
                <PipeSection
                    page={page}
                    index={pipe}
                    paused={paused}
                    variables={variables}
                    variablesProblem={variablesProblem}
                    onCommit={onCommit}
                />
                <button
                    type="button"
                    className="hb-danger"
                    disabled={paused}
                    onClick={() => {
                        onCommit(deletePipes(page, selection));
                        onSelect([]);
                    }}
                >
                    Delete
                </button>
            </aside>
        );
    }
    const object = page.object.find((candidate) => candidate.id === selection[0]);
    if (!object) return <aside className="hb-panel hb-inspector" />;
    const component = catalogue?.components.find((candidate) => candidate.type === object.type);
    // A placed faceplate shows its faceplate's objects: it has a tag, a place and a size (§8.9).
    const placed = object.type === "faceplate";
    const equipment = object.block ? definitions?.[object.block] : undefined;
    const number = (text: string): number | undefined =>
        text === "" || Number.isNaN(Number(text)) ? undefined : Number(text);
    const position =
        (index: 0 | 1, key: "at" | "size") =>
        (text: string): void => {
            const value = number(text);
            if (value === undefined) return;
            const pair: [number, number] = [...object[key]] as [number, number];
            pair[index] = value;
            onCommit(updateObject(page, object.id, { [key]: pair }));
        };
    const bindingNames = [
        ...Object.keys(component?.properties ?? {}),
        ...Object.keys(object.bind).filter((name) => !component?.properties?.[name]),
    ];
    /** Binds a property to a typed or picked path, or to a variable dropped on it; the language
     * server gives the binding its direction and says why a variable cannot be bound (§7.1, §7.4). */
    const rebind = (name: string, given: { path: string } | { variable: VariableNode }): void =>
        change(async (current) => {
            const target = current.object.find((candidate) => candidate.id === object.id);
            if (!target) return undefined;
            const answer = await bound(call, component, target, name, given);
            if ("refused" in answer) {
                if (answer.refused) onNotice(answer.refused);
                return undefined;
            }
            return { page: withObject(current, answer.object) };
        });
    const bind =
        (name: string) =>
        (path: string): void =>
            rebind(name, { path });
    /** Adds a variable as a pen named after its last part, numbered when the name is taken (§5.7). */
    const addPen = (path: string): void => {
        const trimmed = path.trim();
        if (trimmed === "") return;
        // Named as its legend shows it (§5.9): the tag, and the member in words.
        const base = penName(trimmed);
        const taken = (candidate: string): boolean =>
            [...Object.keys(object.bind), ...Object.keys(component?.properties ?? {})].some(
                (key) => key.toLowerCase() === candidate.toLowerCase(),
            );
        let name = base;
        for (let number = 2; taken(name); number += 1) name = `${base} ${number}`;
        rebind(name, { path: trimmed });
    };
    /** A variable dragged from the Tags panel and released on a binding. */
    const drop = (name: string) => (): void => {
        const drag = variableDrag.current;
        if (!drag?.moved || paused) return;
        drag.handled = true;
        rebind(name, { variable: drag.node });
    };

    return (
        <aside className="hb-panel hb-inspector">
            <PanelTitle
                title={equipment?.title ?? componentName(object.type)}
                panel="inspector"
                onCollapse={onCollapse}
            />
            {equipment ? (
                <EquipmentSection
                    object={object}
                    definition={equipment}
                    variables={variables ?? []}
                    readOnly={paused}
                    editing={equipmentEditor}
                    pictures={pictures(catalogue)}
                />
            ) : null}
            {problems && problems.length > 0 ? (
                <section className="hb-section hb-quality hb-problems" data-part="problems">
                    <h2>Problems</h2>
                    {problems.map((problem, index) => {
                        const point = points?.[problem];
                        return (
                            <div key={index} className="hb-quality-line">
                                <span className="hb-quality-mark" aria-hidden="true">
                                    !
                                </span>
                                <span className="hb-quality-text">{problem}</span>
                                {point && onAllowWriting ? (
                                    <button
                                        type="button"
                                        className="hb-secondary"
                                        data-action="allow-writing"
                                        disabled={paused}
                                        title="Adds the rule to the project's runtime.toml, with this control's role and limits"
                                        onClick={() => onAllowWriting(point)}
                                    >
                                        Allow writing
                                    </button>
                                ) : null}
                            </div>
                        );
                    })}
                </section>
            ) : null}
            {warnings && warnings.length > 0 ? (
                <section className="hb-section hb-quality" data-part="quality">
                    <h2>Quality</h2>
                    {warnings.map((warning, index) => (
                        <div key={index} className="hb-quality-line">
                            <span className="hb-quality-mark" aria-hidden="true">
                                !
                            </span>
                            <span className="hb-quality-text">{warning.message}</span>
                            {warning.kind === "unit" ? (
                                <button
                                    type="button"
                                    className="hb-secondary"
                                    data-action="no-unit"
                                    disabled={paused}
                                    onClick={() => replace({ ...object, unit: "" })}
                                >
                                    No unit
                                </button>
                            ) : null}
                        </div>
                    ))}
                </section>
            ) : null}
            <section className="hb-section">
                <h2>General</h2>
                {!equipment ? <Field name="id" label="Id" value={object.id} readOnly /> : null}
                {TEXT_KEYS.filter((key) =>
                    equipment ? false : placed ? key === "tag" : key !== "text" || TEXT_TYPES.includes(object.type),
                ).map((key) => (
                    <Field
                        key={key}
                        name={key}
                        label={writtenLabel(
                            key === "tag" ? "Tag" : key === "label" ? "Label" : key === "text" ? "Text" : "Unit",
                            key === "label" || key === "text" ? writtenIn : undefined,
                        )}
                        value={object[key]}
                        readOnly={paused}
                        onCommit={(value) => replace(withKey(object, key, value))}
                    />
                ))}
                {component?.variants ? (
                    <label className="hb-field">
                        <span className="hb-field-label">Variant</span>
                        <select
                            name="variant"
                            value={object.variant ?? ""}
                            disabled={paused}
                            onChange={(event) => replace(withKey(object, "variant", event.target.value))}
                        >
                            <option value="">Standard</option>
                            {component.variants.map((variant) => (
                                <option key={variant} value={variant}>
                                    {variant}
                                </option>
                            ))}
                        </select>
                    </label>
                ) : null}
            </section>
            {placed && placing ? (
                <PlacedSection
                    object={object}
                    readOnly={paused}
                    answer={placing.answer}
                    onAsk={placing.ask}
                    onOpen={placing.open}
                    onStudio={placing.studio}
                    onChange={replace}
                />
            ) : null}
            <section className="hb-section">
                <h2>Position</h2>
                <div className="hb-pair">
                    <Field
                        name="x"
                        label="X"
                        value={object.at[0]}
                        numeric
                        readOnly={paused}
                        onCommit={position(0, "at")}
                    />
                    <Field
                        name="y"
                        label="Y"
                        value={object.at[1]}
                        numeric
                        readOnly={paused}
                        onCommit={position(1, "at")}
                    />
                </div>
                <div className="hb-pair">
                    <Field
                        name="width"
                        label="Width"
                        value={object.size[0]}
                        numeric
                        readOnly={paused}
                        onCommit={position(0, "size")}
                    />
                    <Field
                        name="height"
                        label="Height"
                        value={object.size[1]}
                        numeric
                        readOnly={paused}
                        onCommit={position(1, "size")}
                    />
                </div>
                <label className="hb-field">
                    <span className="hb-field-label">Rotation</span>
                    <select
                        name="rotation"
                        value={object.rotation}
                        disabled={paused}
                        onChange={(event) => replace({ ...object, rotation: Number(event.target.value) })}
                    >
                        {[0, 90, 180, 270].map((angle) => (
                            <option key={angle} value={angle}>
                                {angle}°
                            </option>
                        ))}
                    </select>
                </label>
            </section>
            {placed || equipment ? null : (
                <section className="hb-section">
                    <h2>Appearance</h2>
                    <div className="hb-pair">
                        {NUMBER_KEYS.filter((key) => key !== "span" || object.type === "trend").map((key) => (
                            <Field
                                key={key}
                                name={key}
                                label={key.charAt(0).toUpperCase() + key.slice(1)}
                                value={object[key]}
                                numeric
                                readOnly={paused}
                                onCommit={(value) => replace(withKey(object, key, number(value)))}
                            />
                        ))}
                    </div>
                </section>
            )}
            {["bar", "gauge", "split-bar"].includes(object.type) ? (
                <LimitsSection key={object.id} object={object} readOnly={paused} onChange={replace} />
            ) : null}
            {object.type === "trend" ? (
                <TrendScalesSection object={object} readOnly={paused} onChange={replace} />
            ) : null}
            {component?.attaches ? (
                <AttachSection object={object} page={page} catalogue={catalogue} readOnly={paused} onChange={replace} />
            ) : null}
            {object.type === "panel" || object.type === "group" ? (
                <ArrangeSection object={object} readOnly={paused} onChange={replace} />
            ) : null}
            {placed ? null : component?.category === "control" ? (
                <ControlSection object={object} readOnly={paused} onChange={replace} />
            ) : (
                <ClickSection object={object} targets={targets} readOnly={paused} onChange={replace} />
            )}
            {bindingNames.length > 0 && !equipment ? (
                <section className="hb-section">
                    <h2>Bindings</h2>
                    {bindingNames.map((name) => (
                        <div key={name} className="hb-binding" data-binding={name} onMouseUp={drop(name)}>
                            <Field
                                name={`bind.${name}`}
                                label={bindingLabel(name)}
                                value={object.bind[name]?.path}
                                readOnly={paused}
                                onCommit={bind(name)}
                            />
                            <button
                                type="button"
                                className="hb-pick"
                                data-action="pick"
                                data-binding={name}
                                title="Pick a variable"
                                aria-label={`Pick a variable for ${bindingLabel(name)}`}
                                aria-expanded={picking === name}
                                disabled={paused}
                                onClick={() => setPicking(picking === name ? undefined : name)}
                            >
                                ⋯
                            </button>
                            {picking === name ? (
                                <BindingPicker
                                    variables={variables}
                                    problem={variablesProblem}
                                    binding={{
                                        type: object.type,
                                        property: name,
                                        component,
                                        direction: object.bind[name]?.direction,
                                        fields: page.fields,
                                    }}
                                    onPick={(node) => {
                                        setPicking(undefined);
                                        bind(name)(node.path);
                                    }}
                                />
                            ) : null}
                        </div>
                    ))}
                    {component?.additional ? (
                        <div className="hb-binding" data-binding="add-pen">
                            <Field
                                name="add-pen"
                                label="Add pen"
                                value=""
                                readOnly={paused}
                                clearOnCommit
                                onCommit={addPen}
                            />
                            <button
                                type="button"
                                className="hb-pick"
                                data-action="pick"
                                data-binding="add-pen"
                                title="Pick a variable for a new pen"
                                aria-label="Pick a variable for a new pen"
                                aria-expanded={picking === "add-pen"}
                                disabled={paused}
                                onClick={() => setPicking(picking === "add-pen" ? undefined : "add-pen")}
                            >
                                ⋯
                            </button>
                            {picking === "add-pen" ? (
                                <BindingPicker
                                    variables={variables}
                                    problem={variablesProblem}
                                    binding={{ type: object.type, property: "pen", component, fields: page.fields }}
                                    onPick={(node) => {
                                        setPicking(undefined);
                                        addPen(node.path);
                                    }}
                                />
                            ) : null}
                        </div>
                    ) : null}
                </section>
            ) : null}
            {preview && !placed && bindingNames.length > 0 ? (
                <PreviewSection
                    object={object}
                    properties={Object.keys(component?.properties ?? {})}
                    state={preview.states[object.id]}
                    onPreview={(state) => preview.onPreview(object.id, state)}
                />
            ) : null}
            <button
                type="button"
                className="hb-danger"
                disabled={paused}
                onClick={() => {
                    onCommit(deleteObjects(page, [object.id]));
                    onSelect([]);
                }}
            >
                Delete
            </button>
        </aside>
    );
}
