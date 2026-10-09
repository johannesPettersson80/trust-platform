// A text's translation edited in place on the canvas (§14.5 item 7): in a language other than the
// first, a click on a text opens this field over it, with the text as written beside the language.
// Enter or leaving the field writes the translation; Escape leaves it as it was.

import { useEffect, useRef } from "react";
import type { KeyboardEvent } from "react";

interface TranslationFieldProps {
    /** The text as written, in the project's first language. */
    text: string;
    /** The project's own translation, if it has one. */
    translation: string | undefined;
    /** What shows without one: truST's word, or the text as written. */
    placeholder: string;
    languageName: string;
    /** Where the object lies on the canvas, in screen pixels. */
    box: { x: number; y: number; width: number };
    onCommit: (translation: string) => void;
    onCancel: () => void;
}

export function TranslationField({
    text,
    translation,
    placeholder,
    languageName,
    box,
    onCommit,
    onCancel,
}: TranslationFieldProps) {
    const input = useRef<HTMLInputElement>(null);
    const done = useRef(false);
    useEffect(() => {
        input.current?.focus();
        input.current?.select();
    }, []);
    const finish = (value: string | undefined): void => {
        if (done.current) return;
        done.current = true;
        if (value !== undefined && value !== (translation ?? "")) onCommit(value);
        else onCancel();
    };
    const onKeyDown = (event: KeyboardEvent<HTMLInputElement>): void => {
        // The canvas's own keys (Delete, arrows) stay out of the field.
        event.stopPropagation();
        if (event.key === "Enter") finish(event.currentTarget.value);
        if (event.key === "Escape") finish(undefined);
    };
    return (
        <div
            className="hb-translation"
            data-part="translation"
            style={{ left: box.x, top: box.y, minWidth: Math.max(240, box.width) }}
        >
            <div className="hb-translation-source">
                <span className="hb-translation-language">{languageName}</span>
                <span className="hb-translation-text">{text}</span>
            </div>
            <input
                ref={input}
                aria-label={`Translation of "${text}" in ${languageName}`}
                defaultValue={translation ?? ""}
                placeholder={placeholder}
                spellCheck={false}
                onKeyDown={onKeyDown}
                onBlur={(event) => finish(event.currentTarget.value)}
            />
        </div>
    );
}
