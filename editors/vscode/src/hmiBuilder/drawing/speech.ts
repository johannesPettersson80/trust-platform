// The language a drawing speaks (specification 39, §14.5): the screen's text table and language,
// set for the length of one drawing from its frame, or for the runtime display's own words.
// Every word truST shows, every project text and every number goes through here.

import { heard } from "./cuts";
import { textWidth } from "./measure";
import type { DrawFrame } from "./model";
import type { Word } from "./words";

interface Speech {
  texts: Record<string, string>;
  language: string;
  /** The "longest" preview: every text lengthened by 40 % (§14.5 item 7). */
  longest?: boolean;
}

const ENGLISH: Speech = { texts: {}, language: "en" };
let current: Speech = ENGLISH;
let base: Speech = ENGLISH;

/** Draws in the frame's language; the language before it comes back afterwards. */
export function withSpeech<T>(frame: Pick<DrawFrame, "texts" | "language" | "longest">, draw: () => T): T {
  const before = current;
  current = { texts: frame.texts ?? base.texts, language: frame.language ?? base.language, longest: frame.longest };
  try {
    return draw();
  } finally {
    current = before;
  }
}

/** The language of the runtime display's own words, outside a drawing. */
export function setSpeech(texts: Record<string, string>, language: string): void {
  base = { texts, language };
  current = base;
}

/** One of truST's own words in the language, its values put in where the text names them. */
export function say(word: Word, values: Record<string, string | number> = {}): string {
  return spoken(fill(current.texts[word] ?? word, values));
}

/** A project's own text in the language; a text without a translation shows as written. */
export function translate(text: string): string {
  return spoken(current.texts[text] ?? text);
}

/** A text the runtime says, in the language when the text table holds it and as written otherwise,
 * not lengthened: it goes into a frame that `say` lengthens. */
export function tabled(text: string): string {
  return current.texts[text] ?? text;
}

/** A text as the drawing shows it: lengthened in the "longest" preview. */
function spoken(text: string): string {
  return heard(current.longest ? lengthen(text) : text);
}

/** The "longest" preview of a text: the text and a run of dots, 40 % wider in all (§14.5 item 7). */
export function lengthen(text: string): string {
  if (text.trim() === "") return text;
  const goal = textWidth(text, 14) * 1.4;
  let longer = `${text} `;
  while (textWidth(longer, 14) < goal) longer += "·";
  return longer;
}

/** The runtime display's text table and language, for the frames it draws (§14.5). */
export function displaySpeech(): { texts: Record<string, string>; language: string } {
  return { texts: base.texts, language: base.language };
}

/** The language spoken, a BCP 47 tag. */
export function language(): string {
  return current.language;
}

function fill(text: string, values: Record<string, string | number>): string {
  return text.replace(/\{([A-Za-z]+)\}/g, (whole, name: string) => (name in values ? String(values[name]) : whole));
}
