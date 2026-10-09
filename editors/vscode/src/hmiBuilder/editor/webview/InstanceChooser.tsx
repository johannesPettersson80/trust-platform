// Placing a faceplate (specification 39, §8.9 item 1): it asks for its instance and offers only
// the instances whose block has every field the faceplate needs, as the language server finds
// them. Escape or Cancel places nothing.

import { useEffect } from "react";

interface InstanceChooserProps {
    title: string;
    /** The instances it fits, or undefined while they are being found. */
    instances: string[] | undefined;
    problem?: string;
    onChoose: (instance: string) => void;
    onCancel: () => void;
}

export function InstanceChooser({ title, instances, problem, onChoose, onCancel }: InstanceChooserProps) {
    useEffect(() => {
        const onKey = (event: KeyboardEvent): void => {
            if (event.key === "Escape") onCancel();
        };
        window.addEventListener("keydown", onKey);
        return () => window.removeEventListener("keydown", onKey);
    }, [onCancel]);
    return (
        <div
            className="hb-chooser-backdrop"
            onMouseDown={(event) => event.target === event.currentTarget && onCancel()}
        >
            <section className="hb-chooser" data-part="instance-chooser" role="dialog" aria-label={`Place ${title}`}>
                <h2>Place {title}</h2>
                <p className="hb-chooser-hint">Choose the instance it shows.</p>
                {instances === undefined ? (
                    <p className="hb-hint">Finding the instances it fits…</p>
                ) : instances.length === 0 ? (
                    <p className="hb-hint">
                        {problem ?? "No instance of the program fits this faceplate: none has every field it needs."}
                    </p>
                ) : (
                    <div className="hb-chooser-list">
                        {instances.map((instance) => (
                            <button
                                type="button"
                                key={instance}
                                data-instance={instance}
                                onClick={() => onChoose(instance)}
                            >
                                {instance}
                            </button>
                        ))}
                    </div>
                )}
                <div className="hb-chooser-actions">
                    <button type="button" className="hb-secondary" data-action="cancel" onClick={onCancel}>
                        Cancel
                    </button>
                </div>
            </section>
        </div>
    );
}
