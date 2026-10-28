// The project's languages as tick boxes (specification 39, §14.5 item 2): the languages truST
// ships its words in, by their own names, and any other language the project lists, with the one
// its texts are written in marked Default. The list it gives has the default first, then the
// others in the order shown here.

import { useState } from "react";

import type { LanguageEntry } from "../messages";
import "./languages.css";

/** A BCP 47 language tag such as `fi` or `pt-BR`. */
const TAG = /^[A-Za-z]{2,3}(-[A-Za-z0-9]{2,8})*$/;

interface LanguageChoicesProps {
    /** The languages truST ships, each with its own name. */
    shipped: LanguageEntry[];
    /** The project's languages, the default first. */
    chosen: string[];
    onChange: (languages: string[]) => void;
    disabled?: boolean;
}

/** The project's languages after a change: the default first, then the others as listed. */
function ordered(rows: string[], chosen: Set<string>, preferred: string | undefined): string[] {
    const first = preferred && chosen.has(preferred) ? preferred : rows.find((tag) => chosen.has(tag));
    return first ? [first, ...rows.filter((tag) => tag !== first && chosen.has(tag))] : [];
}

export function LanguageChoices({ shipped, chosen, onChange, disabled = false }: LanguageChoicesProps) {
    const [other, setOther] = useState("");
    const names = new Map(shipped.map((entry) => [entry.tag, entry.name]));
    // A language the project lists but truST does not ship keeps its row, after truST's.
    const rows = [...shipped.map((entry) => entry.tag), ...chosen.filter((tag) => !names.has(tag))];
    const picked = new Set(chosen);
    const fallback = chosen[0];
    const toggle = (tag: string, on: boolean): void => {
        const next = new Set(picked);
        if (on) next.add(tag);
        else next.delete(tag);
        onChange(ordered(rows, next, fallback));
    };
    const otherTag = other.trim();
    const otherValid = TAG.test(otherTag) && !rows.some((tag) => tag.toLowerCase() === otherTag.toLowerCase());
    return (
        <div className="hb-language-choices" data-part="language-choices">
            {rows.map((tag) => {
                const on = picked.has(tag);
                return (
                    <div className="hb-language-row" key={tag} data-language={tag}>
                        <label className="hb-language-name">
                            <input
                                type="checkbox"
                                checked={on}
                                // The last language stays: a project has at least one.
                                disabled={disabled || (on && picked.size === 1)}
                                onChange={(event) => toggle(tag, event.target.checked)}
                            />
                            <span>{names.get(tag) ?? tag}</span>
                            {names.has(tag) ? null : <span className="hb-language-tag">{tag}</span>}
                        </label>
                        <label className="hb-language-default" title="The language your texts are written in">
                            <input
                                type="radio"
                                name="language-default"
                                checked={tag === fallback}
                                disabled={disabled || !on}
                                onChange={() => onChange(ordered(rows, picked, tag))}
                            />
                            Default
                        </label>
                    </div>
                );
            })}
            <form
                className="hb-language-other"
                onSubmit={(event) => {
                    event.preventDefault();
                    if (!otherValid) return;
                    onChange([...chosen, otherTag]);
                    setOther("");
                }}
            >
                <input
                    name="other-language"
                    placeholder="Other language, as a code such as fi"
                    value={other}
                    disabled={disabled}
                    spellCheck={false}
                    onChange={(event) => setOther(event.target.value)}
                />
                <button type="submit" disabled={disabled || !otherValid}>
                    Add
                </button>
            </form>
        </div>
    );
}
