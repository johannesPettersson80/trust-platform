// The Texts table (§14.5 items 4 and 7): every text of the project and truST's own words in every
// project language, the ones without a translation marked; a cell writes its translation. A
// project text is changed on its page, so its first-language cell only shows it.

import { useMemo } from "react";
import type { KeyboardEvent } from "react";

import { textRows } from "../languages";
import type { ProjectTexts } from "../messages";

interface TextsTableProps {
    texts: ProjectTexts;
    onTranslate: (language: string, text: string, translation: string) => void;
}

/** A translation cell's field: it writes on Enter or when it loses focus, and Escape restores it. */
function TranslationInput({
    value,
    placeholder,
    label,
    onCommit,
}: {
    value: string;
    placeholder: string;
    label: string;
    onCommit: (value: string) => void;
}) {
    const commit = (input: HTMLInputElement): void => {
        if (input.value !== value) onCommit(input.value);
    };
    const onKeyDown = (event: KeyboardEvent<HTMLInputElement>): void => {
        if (event.key === "Enter") commit(event.currentTarget);
        if (event.key === "Escape") event.currentTarget.value = value;
    };
    return (
        <input
            key={value}
            defaultValue={value}
            placeholder={placeholder}
            aria-label={label}
            spellCheck={false}
            onKeyDown={onKeyDown}
            onBlur={(event) => commit(event.currentTarget)}
        />
    );
}

export function TextsTable({ texts, onTranslate }: TextsTableProps) {
    const rows = useMemo(() => textRows(texts), [texts]);
    const first = texts.languages[0]?.tag;
    const own = rows.filter((row) => !row.word).length;
    const missing = rows.reduce((sum, row) => sum + row.cells.filter((cell) => cell.missing).length, 0);
    const names = new Map(texts.languages.map((language) => [language.tag, language.name]));
    const firstWord = rows.findIndex((row) => row.word);
    return (
        <div className="hb-texts">
            <div className="hb-texts-head">
                <span className="hb-hint" data-part="texts-summary">
                    {own} {own === 1 ? "text" : "texts"} and {rows.length - own} of truST's words in{" "}
                    {texts.languages.length} {texts.languages.length === 1 ? "language" : "languages"}
                    {missing > 0 ? <strong className="hb-texts-missing"> · {missing} missing</strong> : null}
                </span>
                {texts.errors.map((error, index) => (
                    <span key={index} className="hb-texts-error" role="alert">
                        {error.file ? `${error.file}${error.line ? `:${error.line}` : ""}: ` : ""}
                        {error.message}
                    </span>
                ))}
            </div>
            <div className="hb-texts-scroll">
                <table data-part="texts-table">
                    <thead>
                        <tr>
                            {texts.languages.map((language) => (
                                <th key={language.tag} lang={language.tag}>
                                    {language.name}
                                </th>
                            ))}
                        </tr>
                    </thead>
                    <tbody>
                        {rows.map((row, index) => [
                            index === firstWord ? (
                                <tr key="words" className="hb-texts-section">
                                    <th colSpan={texts.languages.length}>truST's words</th>
                                </tr>
                            ) : null,
                            <tr key={`${row.word ? "word" : "text"}:${row.text}`} data-text={row.text}>
                                {row.cells.map((cell) =>
                                    cell.language === first && !row.word ? (
                                        <td
                                            key={cell.language}
                                            data-language={cell.language}
                                            className="hb-texts-written"
                                        >
                                            {row.text}
                                        </td>
                                    ) : (
                                        <td
                                            key={cell.language}
                                            data-language={cell.language}
                                            data-missing={cell.missing ? "true" : undefined}
                                            lang={cell.language}
                                        >
                                            <TranslationInput
                                                value={cell.own ?? ""}
                                                placeholder={cell.builtin ?? row.text}
                                                label={`"${row.text}" in ${names.get(cell.language) ?? cell.language}`}
                                                onCommit={(value) => onTranslate(cell.language, row.text, value)}
                                            />
                                        </td>
                                    ),
                                )}
                            </tr>,
                        ])}
                    </tbody>
                </table>
            </div>
        </div>
    );
}
