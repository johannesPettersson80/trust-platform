// The settings and connections of an equipment instance in the inspector (specification 39,
// §21.3 item 2): every setting of the definition with its real value and unit, the key ones first
// and the others under More settings, and the instances it depends on. A changed value is applied
// at once, as one undoable project edit.

import { useEffect, useState } from "react";

import type { EquipmentChanges, EquipmentDetails, SettingValue } from "../equipmentDetails";

export interface EquipmentEditing {
    state?: { instance: string; details?: EquipmentDetails; error?: string };
    result?: { instance: string; sequence: number; error?: string };
    ask: (instance: string, block?: string) => void;
    hardware?: (instance: string) => void;
    pick?: (request: import("./dependencyPick").DependencyPick) => void;
    apply: (instance: string, changes: EquipmentChanges, revision: number) => void;
    /** Another picture for the instance's definition, chosen members by input (§21.9). */
    picture?: (instance: string, block: string, symbol: string, bind?: Record<string, string>) => void;
    /** Import an SVG drawing into My Symbols. */
    importPicture?: () => void;
    /** Renames a declared instance with its program, pages and design, as Rename Symbol does. */
    rename?: (instance: string, name: string) => void;
}

/** The value a field shows. */
function shown(setting: SettingValue): string {
    return setting.value === null || setting.value === undefined ? "" : String(setting.value);
}

/** The value a field's text stands for, or why it stands for none. */
function parsed(setting: SettingValue, text: string): boolean | number | string {
    if (setting.kind === "text") return text;
    const value = Number(text.replace(",", "."));
    if (!text.trim() || !Number.isFinite(value)) throw new Error(`${setting.title} needs a number`);
    if (setting.kind === "integer" && !Number.isInteger(value))
        throw new Error(`${setting.title} needs a whole number`);
    if (setting.min !== null && setting.min !== undefined && value < setting.min)
        throw new Error(`${setting.title} is at least ${setting.min}`);
    if (setting.max !== null && setting.max !== undefined && value > setting.max)
        throw new Error(`${setting.title} is at most ${setting.max}`);
    return value;
}

/** One setting: a choice or a switch gives its value; a typed text is given as written. */
function SettingField({
    setting,
    disabled,
    onValue,
    onText,
}: {
    setting: SettingValue;
    disabled: boolean;
    onValue: (value: boolean | number | string) => void;
    onText: (text: string) => void;
}) {
    const name = `setting-${setting.member}`;
    const label = setting.unit ? `${setting.title} (${setting.unit})` : setting.title;
    if (setting.choices?.length) {
        const index = setting.choices.findIndex((choice) => choice.value === setting.value);
        return (
            <label className="hb-field" title={setting.help ?? undefined}>
                <span className="hb-field-label">{label}</span>
                <select
                    name={name}
                    value={index < 0 ? "" : String(index)}
                    disabled={disabled}
                    onChange={(event) => onValue(setting.choices![Number(event.target.value)].value)}
                >
                    {index < 0 ? <option value="">{shown(setting)}</option> : null}
                    {setting.choices.map((choice, at) => (
                        <option key={at} value={String(at)}>
                            {choice.title}
                        </option>
                    ))}
                </select>
            </label>
        );
    }
    if (setting.kind === "bool") {
        return (
            <label className="hb-field hb-check" title={setting.help ?? undefined}>
                <input
                    type="checkbox"
                    name={name}
                    checked={setting.value === true}
                    disabled={disabled}
                    onChange={(event) => onValue(event.target.checked)}
                />
                <span>{setting.title}</span>
            </label>
        );
    }
    const commit = (input: HTMLInputElement): void => {
        if (input.value.trim() !== shown(setting)) onText(input.value.trim());
    };
    return (
        <label className="hb-field" title={setting.help ?? undefined}>
            <span className="hb-field-label">{label}</span>
            <input
                key={shown(setting)}
                name={name}
                defaultValue={shown(setting)}
                inputMode={setting.kind === "text" ? undefined : "decimal"}
                readOnly={disabled || setting.kind === "other"}
                spellCheck={false}
                onKeyDown={(event) => {
                    if (event.key === "Enter") commit(event.currentTarget);
                    if (event.key === "Escape") event.currentTarget.value = shown(setting);
                }}
                onBlur={(event) => commit(event.currentTarget)}
            />
        </label>
    );
}

export function EquipmentConfiguration({
    instance,
    block,
    editing,
    readOnly,
}: {
    instance: string;
    block: string;
    editing: EquipmentEditing;
    readOnly: boolean;
}) {
    const details = editing.state?.instance === instance ? editing.state.details : undefined;
    const [busy, setBusy] = useState(false);
    const [problem, setProblem] = useState<string>();
    const { ask, result } = editing;
    useEffect(() => {
        setProblem(undefined);
        ask(instance, block);
    }, [ask, instance, block]);
    useEffect(() => {
        if (result?.instance !== instance) return;
        setBusy(false);
        setProblem(result.error);
    }, [result, instance]);
    if (!details)
        return (
            <p className="hb-hint">
                {(editing.state?.instance === instance && editing.state.error) || "Reading the equipment…"}
            </p>
        );
    const disabled = readOnly || busy || !details.owned;
    const apply = (changes: EquipmentChanges): void => {
        setProblem(undefined);
        setBusy(true);
        editing.apply(instance, changes, details.revision);
    };
    const field = (setting: SettingValue) => (
        <SettingField
            key={setting.member}
            setting={setting}
            disabled={disabled}
            onValue={(value) => apply({ settings: { [setting.member]: value } })}
            onText={(text) => {
                try {
                    apply({ settings: { [setting.member]: parsed(setting, text) } });
                } catch (error) {
                    setProblem(error instanceof Error ? error.message : String(error));
                }
            }}
        />
    );
    const key = details.settings.filter((setting) => setting.key);
    const more = details.settings.filter((setting) => !setting.key);
    return (
        <>
            {details.settings.length > 0 ? (
                <section className="hb-section" data-part="equipment-settings">
                    <h2>Settings</h2>
                    {!details.owned ? (
                        <p className="hb-hint">Declared in your PLC code: change these values there.</p>
                    ) : null}
                    {key.map(field)}
                    {more.length > 0 ? (
                        <details className="hb-more" data-part="equipment-more-settings">
                            <summary>More settings ({more.length})</summary>
                            {more.map(field)}
                        </details>
                    ) : null}
                </section>
            ) : null}
            {details.dependencies.length > 0 ? (
                <section className="hb-section" data-part="equipment-dependencies">
                    <h2>Connected equipment</h2>
                    {details.dependencies.map((dependency) => (
                        <label className="hb-field" key={dependency.member}>
                            <span className="hb-field-label">{dependency.title}</span>
                            <div className="hb-row">
                                <select
                                    name={`dependency-${dependency.member}`}
                                    value={dependency.value ?? ""}
                                    disabled={disabled}
                                    onChange={(event) =>
                                        apply({ dependencies: { [dependency.member]: event.target.value } })
                                    }
                                >
                                    <option value="">{dependency.required ? "Choose…" : "None"}</option>
                                    {dependency.choices.map((choice) => (
                                        <option key={choice.path} value={choice.path}>
                                            {choice.path.split(".").pop()}
                                        </option>
                                    ))}
                                </select>
                                {editing.pick ? (
                                    <button
                                        type="button"
                                        title="Pick it on the page"
                                        disabled={disabled || dependency.choices.length === 0}
                                        onClick={() =>
                                            editing.pick?.({
                                                title: dependency.title,
                                                choices: dependency.choices.map((choice) => choice.path),
                                                select: (path) =>
                                                    apply({ dependencies: { [dependency.member]: path } }),
                                            })
                                        }
                                    >
                                        Pick
                                    </button>
                                ) : null}
                            </div>
                        </label>
                    ))}
                </section>
            ) : null}
            {problem ? (
                <p role="alert" className="hb-error">
                    Not accepted: {problem}
                </p>
            ) : null}
        </>
    );
}
