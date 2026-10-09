// The texts a drawing cut to fit their object (specification 39, §14.5 item 6), for the editor's
// warnings. Only a text is counted, a word or a project text the drawing said, never a value.

/** A text cut to fit, and the object it belongs to. */
export interface CutText {
  object: string;
  /** The whole text, before it was cut. */
  text: string;
}

interface Collecting {
  cuts: CutText[];
  /** Every text said while drawing, to tell a text from a value. */
  said: Set<string>;
  /** The object being drawn. */
  object?: string;
}

let collecting: Collecting | undefined;

/** Draws, and gives every text cut to fit with the object it belongs to. */
export function collectCuts<T>(draw: () => T): { drawn: T; cuts: CutText[] } {
  const before = collecting;
  const now: Collecting = { cuts: [], said: new Set() };
  collecting = now;
  try {
    return { drawn: draw(), cuts: now.cuts };
  } finally {
    collecting = before;
  }
}

/** Draws one object: the texts cut meanwhile belong to it. */
export function drawingObject<T>(id: string, draw: () => T): T {
  const now = collecting;
  if (!now) return draw();
  const before = now.object;
  now.object = id;
  try {
    return draw();
  } finally {
    now.object = before;
  }
}

/** A text the drawing said: one of truST's words or a project text. */
export function heard(text: string): string {
  collecting?.said.add(text);
  return text;
}

/** A text was cut to fit; a value cut is not counted. */
export function noteCut(text: string): void {
  const now = collecting;
  if (now?.object === undefined || !now.said.has(text)) return;
  if (!now.cuts.some((cut) => cut.object === now.object && cut.text === text))
    now.cuts.push({ object: now.object, text });
}
