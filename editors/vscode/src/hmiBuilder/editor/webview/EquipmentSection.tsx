// The inspector of an equipment object (specification 39, §21.3 item 2): its tag, its hardware
// status with one button, then its settings and connected equipment. The instances and programs it
// offers, the instance a tag or a program gives and a loop's signal lines are the language server's
// rules (`trust-lsp.hmiEquipmentEdit`); this section shows them and asks for them.

import { useEffect } from "react";

import { EquipmentConfiguration, type EquipmentEditing } from "./EquipmentConfiguration";
import { EquipmentPicture } from "./EquipmentPicture";
import type { HmiObjectModel } from "../../drawing/model";
import type { EquipmentDefinition } from "../catalogue";
import { boundTo, instanceChoices, withLoopSignals } from "../equipment";
import { hardwareWords } from "../hardwareStatus";
import type { VariableNode } from "../messages";
import { Field } from "./Field";
import { useRust, useRustAnswer, withObject } from "./rust";

interface Props {
    object: HmiObjectModel;
    definition: EquipmentDefinition;
    variables: VariableNode[];
    readOnly: boolean;
    editing?: EquipmentEditing;
    /** The pictures the project's own equipment may show (§21.9). */
    pictures?: Array<{ type: string; title: string }>;
}

const NAME = /^[A-Za-z_][A-Za-z0-9_]*$/;

export function EquipmentSection({ object, definition, variables, readOnly, editing, pictures = [] }: Props) {
    const { call, change } = useRust();
    const choices = useRustAnswer(call, [variables, definition.block, object.instance], (ask) =>
        instanceChoices(ask, definition.block, object.instance, variables),
    );
    const programs = choices?.programs ?? [];
    const program = choices?.program ?? "";
    const name = choices?.name ?? "";
    const shortName = (path: string): string => (programs.length > 1 ? path : (path.split(".").pop() ?? path));
    /** Binds the object to an instance, or to a name in a program (without one, its own name or
     * the program's first free one); a name that is not one changes nothing. */
    const bind = (to: { instance: string } | { program: string; name?: string }): void =>
        change(async (current) => {
            const target = current.object.find((candidate) => candidate.id === object.id);
            if (!target) return undefined;
            const next = await boundTo(call, target, definition, to, variables, current);
            return next ? { page: withObject(current, next) } : undefined;
        });
    const details = editing?.state?.instance === object.instance ? editing?.state?.details : undefined;
    const hardware = details?.hardware;
    // A loop controller's signal lines follow its configured measurement and controlled equipment
    // (§6 item 5): set them in Connected equipment, or pick them on the page, and the lines follow.
    useEffect(() => {
        if (!details || readOnly || object.type !== "controller") return;
        change(async (current) => {
            const target = current.object.find((candidate) => candidate.id === object.id);
            if (!target) return undefined;
            const next = await withLoopSignals(call, target, current, details.dependencies);
            return next ? { page: withObject(current, next) } : undefined;
        });
        // Only a new reading of the details decides; the page's own edits do not.
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [details]);
    return (
        <>
            <section className="hb-section" data-part="equipment-instance">
                <h2>{definition.title}</h2>
                {!choices ? null : choices.current && editing?.rename ? (
                    // A declared instance is renamed with its program, pages and design.
                    <Field
                        name="equipment-tag"
                        label={programs.length > 1 ? `Tag in ${program}` : "Tag"}
                        value={name}
                        readOnly={readOnly}
                        onCommit={(value) => {
                            if (NAME.test(value) && value.toLowerCase() !== name.toLowerCase())
                                editing.rename?.(object.instance!, value);
                        }}
                    />
                ) : choices.current ? (
                    <p className="hb-tag" data-part="equipment-tag" title={object.instance}>
                        {shortName(object.instance ?? "")}
                    </p>
                ) : (
                    <>
                        {programs.length > 1 ? (
                            <label className="hb-field">
                                <span className="hb-field-label">Program</span>
                                <select
                                    name="equipment-program"
                                    value={program}
                                    disabled={readOnly}
                                    onChange={(event) => bind({ program: event.target.value })}
                                >
                                    <option value="">Choose…</option>
                                    {programs.map((node) => (
                                        <option key={node.path} value={node.path}>
                                            {node.name}
                                        </option>
                                    ))}
                                </select>
                            </label>
                        ) : null}
                        <Field
                            name="equipment-tag"
                            label="Tag"
                            value={name}
                            readOnly={readOnly || !program}
                            onCommit={(value) => bind({ program, name: value })}
                        />
                    </>
                )}
                {choices && choices.declared.length > 0 && !readOnly ? (
                    <label className="hb-field">
                        <span className="hb-field-label">Existing equipment</span>
                        <select
                            name="equipment-instance"
                            value={choices.current ? object.instance : ""}
                            onChange={(event) => {
                                if (event.target.value) bind({ instance: event.target.value });
                            }}
                        >
                            <option value="">{choices.current ? "—" : "New equipment"}</option>
                            {choices.declared.map((path) => (
                                <option key={path} value={path}>
                                    {shortName(path)}
                                </option>
                            ))}
                        </select>
                    </label>
                ) : null}
                {definition.io.length > 0 && object.instance ? (
                    <div className="hb-hardware" data-part="equipment-hardware" data-status={hardware ?? "unknown"}>
                        <span className="hb-field-label">Hardware</span>
                        <div className="hb-row">
                            <span className="hb-status">{hardware ? hardwareWords(hardware) : "…"}</span>
                            {editing?.hardware ? (
                                <button
                                    type="button"
                                    disabled={readOnly}
                                    onClick={() => editing.hardware?.(object.instance!)}
                                >
                                    {hardware === "not-connected" ? "Connect hardware" : "Edit hardware connections"}
                                </button>
                            ) : null}
                        </div>
                    </div>
                ) : null}
            </section>
            {object.instance && details?.picture?.editable && editing?.picture ? (
                <EquipmentPicture
                    picture={details.picture}
                    pictures={pictures}
                    disabled={readOnly}
                    onChoose={(symbol) => editing?.picture?.(object.instance!, definition.block, symbol)}
                    onBind={(property, member) =>
                        editing?.picture?.(
                            object.instance!,
                            definition.block,
                            details?.picture?.symbol ?? definition.symbol,
                            { [property]: member },
                        )
                    }
                    onImport={editing?.importPicture}
                />
            ) : null}
            {object.instance && editing ? (
                <EquipmentConfiguration
                    key={object.instance}
                    instance={object.instance}
                    block={definition.block}
                    editing={editing}
                    readOnly={readOnly}
                />
            ) : null}
        </>
    );
}
