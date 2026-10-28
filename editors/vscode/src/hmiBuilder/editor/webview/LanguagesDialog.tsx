// The Languages dialog of the top bar (specification 39, §14.5 item 2): the languages the
// project's screens offer, chosen as tick boxes. Operators switch between them with the language
// button of the running screen. Escape or Cancel changes nothing.

import { useEffect, useState } from "react";

import type { LanguageEntry } from "../messages";
import { LanguageChoices } from "./LanguageChoices";

interface LanguagesDialogProps {
    shipped: LanguageEntry[];
    /** The project's languages, the default first. */
    languages: string[];
    onApply: (languages: string[]) => void;
    onCancel: () => void;
}

export function LanguagesDialog({ shipped, languages, onApply, onCancel }: LanguagesDialogProps) {
    const [draft, setDraft] = useState(languages);
    useEffect(() => {
        const onKey = (event: KeyboardEvent): void => {
            if (event.key === "Escape") onCancel();
        };
        window.addEventListener("keydown", onKey);
        return () => window.removeEventListener("keydown", onKey);
    }, [onCancel]);
    const changed = draft.join("|") !== languages.join("|");
    return (
        <div
            className="hb-chooser-backdrop"
            onMouseDown={(event) => event.target === event.currentTarget && onCancel()}
        >
            <section
                className="hb-chooser hb-languages-dialog"
                data-part="languages-dialog"
                role="dialog"
                aria-label="Languages"
            >
                <h2>Languages</h2>
                <p className="hb-chooser-hint">
                    The languages your screens offer. Operators switch between them with the language button on the
                    running screen; your texts are written in the default.
                </p>
                <LanguageChoices shipped={shipped} chosen={draft} onChange={setDraft} />
                <div className="hb-chooser-actions">
                    <button type="button" className="hb-secondary" data-action="cancel" onClick={onCancel}>
                        Cancel
                    </button>
                    <button type="button" data-action="apply" disabled={!changed} onClick={() => onApply(draft)}>
                        Apply
                    </button>
                </div>
            </section>
        </div>
    );
}
