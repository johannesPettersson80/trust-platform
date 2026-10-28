// The project's languages in the editor (specification 39, §14.5 items 4 and 7): the table each
// language draws with, and the rows of the Texts table with the texts that have no translation.

import type { AlarmMessage, ProjectTexts } from "./messages";
import type { LanguageTable } from "./quality";

/** Each project language with its table: the project's texts over truST's words (§14.5 item 4). */
export function languageTables(texts: ProjectTexts): LanguageTable[] {
  return texts.languages.map(({ tag, name }) => ({
    tag,
    name,
    texts: { ...(texts.builtin[tag] ?? {}), ...(texts.project[tag] ?? {}) },
  }));
}

/**
 * The project's texts with the messages its program gives its alarms (§14.5 items 4 and 8): each
 * a text of the project like those on its pages, shown at `alarm:<path>`.
 */
export function withAlarmMessages(texts: ProjectTexts, alarms: AlarmMessage[]): ProjectTexts {
  if (alarms.length === 0) return texts;
  const merged = texts.texts.map((entry) => ({ ...entry, places: [...entry.places] }));
  for (const alarm of alarms) {
    const place = `alarm:${alarm.path}`;
    const known = merged.find((entry) => entry.text === alarm.message);
    if (!known) merged.push({ text: alarm.message, places: [place] });
    else if (!known.places.includes(place)) known.places.push(place);
  }
  return { ...texts, texts: merged };
}

/** One language of a text: the project's own translation, truST's, and whether it has none. */
export interface TextCell {
  language: string;
  /** The project's own text for it, from its text file. */
  own?: string;
  /** truST's text for one of its words. */
  builtin?: string;
  /** Neither the project nor truST gives it: it shows as written. */
  missing: boolean;
}

/** A row of the Texts table: a text as written, or one of truST's words, in every language. */
export interface TextRow {
  text: string;
  /** One of truST's own words rather than a text of the project. */
  word: boolean;
  cells: TextCell[];
}

/**
 * The project's texts, then truST's words that the project does not show as its own texts, each
 * in every language. A project text needs no translation in the first language, where it is
 * written; truST's words need one in every language truST does not ship.
 */
export function textRows(texts: ProjectTexts): TextRow[] {
  const first = texts.languages[0]?.tag;
  const row = (text: string, word: boolean): TextRow => ({
    text,
    word,
    cells: texts.languages.map(({ tag }) => {
      const own = texts.project[tag]?.[text];
      const builtin = texts.builtin[tag]?.[text];
      const written = !word && tag === first;
      return { language: tag, own, builtin, missing: !written && own === undefined && builtin === undefined };
    }),
  });
  const shown = new Set(texts.texts.map((entry) => entry.text));
  return [
    ...texts.texts.map((entry) => row(entry.text, false)),
    ...texts.words.filter((word) => !shown.has(word)).map((word) => row(word, true)),
  ];
}
