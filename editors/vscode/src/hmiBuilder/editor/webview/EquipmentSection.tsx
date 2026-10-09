// The inspector of an equipment object (specification 39, §21.3 item 2): its tag, its hardware
// status with one button, then its settings and connected equipment.

import { useEffect } from "react";

import { EquipmentConfiguration, type EquipmentEditing } from "./EquipmentConfiguration";
import { EquipmentPicture } from "./EquipmentPicture";
import type { HmiObjectModel, HmiPageModel } from "../../drawing/model";
import type { EquipmentDefinition } from "../catalogue";
import { hardwareWords } from "../hardwareStatus";
import type { VariableNode } from "../messages";
import { bindEquipment, equipmentPrograms, freeEquipmentInstance, matchingInstances } from "../equipment";
import { Field } from "./Field";
import { withLoopSignals } from "../loopSignals";

interface Props {
    object: HmiObjectModel;
    /** The page, whose objects' instances a new name avoids. */
    page?: HmiPageModel;
    definition: EquipmentDefinition;
    variables: VariableNode[];
    readOnly: boolean;
    onChange: (object: HmiObjectModel) => void;
    editing?: EquipmentEditing;
    /** The pictures the project's own equipment may show (§21.9). */
    pictures?: Array<{ type: string; title: string }>;
}

const NAME = /^[A-Za-z_][A-Za-z0-9_]*$/;
const INSTANCE = /^[A-Za-z_][A-Za-z0-9_]*(\.[A-Za-z_][A-Za-z0-9_]*)+$/;

export function EquipmentSection({
    object,
    page,
    definition,
    variables,
    readOnly,
    onChange,
    editing,
    pictures = [],
}: Props) {
    const declared = matchingInstances(variables, definition.block).filter((node) => !node.pending);
    const isDeclared = declared.some((node) => node.path.toLowerCase() === object.instance?.toLowerCase());
    const programs = equipmentPrograms(variables);
    const program =
        object.instance?.split(".").slice(0, -1).join(".") ||
        (programs.length === 1 ? programs[0].path : programs.length === 0 ? "Main" : "");
    const name = object.instance?.split(".").pop() ?? "";
    // A program's first free name, so two new objects never share an instance.
    const firstName = (chosen: string): string =>
        (freeEquipmentInstance(definition, variables, page, chosen) ?? `${chosen}.Equipment1`).split(".").pop()!;
    const shortName = (path: string): string => (programs.length > 1 ? path : (path.split(".").pop() ?? path));
    const bind = (instance: string): void => {
        if (INSTANCE.test(instance)) onChange(bindEquipment(object, definition, instance));
    };
    const details = editing?.state?.instance === object.instance ? editing?.state?.details : undefined;
    const hardware = details?.hardware;
    // A loop controller's signal lines follow its configured measurement and controlled equipment
    // (§6 item 5): set them in Connected equipment, or pick them on the page, and the lines follow.
    useEffect(() => {
        if (!details || !page || readOnly || object.type !== "controller") return;
        const next = withLoopSignals(object, page, details.dependencies);
        if (next) onChange(next);
        // Only a new reading of the details decides; the page's own edits do not.
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [details]);
    return (
        <>
            <section className="hb-section" data-part="equipment-instance">
                <h2>{definition.title}</h2>
                {isDeclared && editing?.rename ? (
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
                ) : isDeclared ? (
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
                                    onChange={(event) =>
                                        bind(`${event.target.value}.${name || firstName(event.target.value)}`)
                                    }
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
                            onCommit={(value) => {
                                if (NAME.test(value)) bind(`${program}.${value}`);
                            }}
                        />
                    </>
                )}
                {declared.length > 0 && !readOnly ? (
                    <label className="hb-field">
                        <span className="hb-field-label">Existing equipment</span>
                        <select
                            name="equipment-instance"
                            value={isDeclared ? object.instance : ""}
                            onChange={(event) => {
                                if (event.target.value) bind(event.target.value);
                            }}
                        >
                            <option value="">{isDeclared ? "—" : "New equipment"}</option>
                            {declared.map((node) => (
                                <option key={node.path} value={node.path}>
                                    {shortName(node.path)}
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
