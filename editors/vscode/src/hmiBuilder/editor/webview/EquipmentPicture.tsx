// The picture of the customer's own equipment in the inspector (specification 39, §21.9 items 4
// and 5): which picture shows the block, the plain device or one of My Symbols, or a drawing
// imported now; and which of the block's values drives each part of the picture.

import type { EquipmentPicture as Picture } from "../equipmentDetails";

interface EquipmentPictureProps {
    picture: Picture;
    /** The pictures it may show: the plain device and My Symbols, by type with their names. */
    pictures: Array<{ type: string; title: string }>;
    disabled: boolean;
    onChoose: (symbol: string) => void;
    onBind: (property: string, member: string) => void;
    onImport?: () => void;
}

export function EquipmentPicture({ picture, pictures, disabled, onChoose, onBind, onImport }: EquipmentPictureProps) {
    const inputs = picture.inputs ?? [];
    return (
        <section className="hb-section" data-part="equipment-picture">
            <h2>Picture</h2>
            <label className="hb-field">
                <span className="hb-field-label">Shown as</span>
                <select
                    name="equipment-picture"
                    value={picture.symbol ?? ""}
                    disabled={disabled}
                    onChange={(event) => onChoose(event.target.value)}
                >
                    {pictures.map((entry) => (
                        <option key={entry.type} value={entry.type}>
                            {entry.title}
                        </option>
                    ))}
                </select>
            </label>
            {onImport ? (
                <button type="button" data-action="import-picture" disabled={disabled} onClick={onImport}>
                    Import SVG…
                </button>
            ) : null}
            {inputs.length > 0 ? (
                <>
                    <p className="hb-hint">Which of the block's values drives each part of the picture.</p>
                    {inputs.map((input) => (
                        <label className="hb-field" key={input.property}>
                            <span className="hb-field-label">{input.property}</span>
                            <select
                                name={`picture-input-${input.property}`}
                                value={input.member ?? ""}
                                disabled={disabled}
                                onChange={(event) => onBind(input.property, event.target.value)}
                            >
                                <option value="">Not shown</option>
                                {input.choices.map((member) => (
                                    <option key={member} value={member}>
                                        {member}
                                    </option>
                                ))}
                            </select>
                        </label>
                    ))}
                </>
            ) : null}
        </section>
    );
}
