// An instrument's attachment in the inspector (§6 item 5): the equipment or the pipe it
// measures, or nothing.

import type { HmiObjectModel, HmiPageModel } from "../../drawing/model";
import { withAttach } from "../attachments";
import type { Catalogue } from "../catalogue";

interface AttachSectionProps {
    object: HmiObjectModel;
    page: HmiPageModel;
    catalogue: Catalogue | undefined;
    readOnly: boolean;
    onChange: (object: HmiObjectModel) => void;
}

/** What an instrument can attach to: the page's equipment that does not attach itself, and its
 * pipes, each as the operator reads it. */
export function attachTargets(
    page: HmiPageModel,
    catalogue: Catalogue | undefined,
    own: string,
): Array<[string, string]> {
    const definition = (type: string) => catalogue?.components.find((component) => component.type === type);
    const name = (id: string): string => {
        const found = page.object.find((candidate) => candidate.id === id);
        return found?.tag ?? id;
    };
    const end = (text: string): string =>
        `${name(text.slice(0, text.lastIndexOf(".")))} ${text.slice(text.lastIndexOf(".") + 1)}`;
    const equipment = page.object
        .filter(
            (candidate) =>
                candidate.id !== own &&
                definition(candidate.type)?.category === "equipment" &&
                !definition(candidate.type)?.attaches,
        )
        .map((candidate): [string, string] => [candidate.id, name(candidate.id)]);
    const pipes = page.pipe.map((pipe, index): [string, string] => [
        `pipe:${index}`,
        `Pipe ${end(pipe.from)} → ${end(pipe.to)}`,
    ]);
    return [...equipment, ...pipes];
}

export function AttachSection({ object, page, catalogue, readOnly, onChange }: AttachSectionProps) {
    const targets = attachTargets(page, catalogue, object.id);
    return (
        <section className="hb-section" data-part="attach-section">
            <h2>Measures</h2>
            <label className="hb-field">
                <span className="hb-field-label">Attached to</span>
                <select
                    name="attach"
                    value={object.attach ?? ""}
                    disabled={readOnly}
                    onChange={(event) => onChange(withAttach(object, event.target.value || undefined))}
                >
                    <option value="">Nothing</option>
                    {targets.map(([value, label]) => (
                        <option key={value} value={value}>
                            {label}
                        </option>
                    ))}
                </select>
            </label>
            <p className="hb-hint">Or drag from the dot under its bubble onto the equipment or the pipe.</p>
        </section>
    );
}
