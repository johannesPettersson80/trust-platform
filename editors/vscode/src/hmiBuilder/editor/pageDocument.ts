// One page document in the HMI Builder (specification 39, §3.1, §15.2): its model is read from the
// text by the language server, and an edited model is written back by the language server's
// canonical writer and applied as the smallest text edit.

import type { HmiPageModel } from "../drawing/model";

/** A problem with a file, with its line when it has one. */
export interface FileError {
  file?: string;
  line?: number;
  message: string;
}

export type PageRead = { ok: true; model: HmiPageModel } | { ok: false; errors: FileError[] };
export type PageWrite = { ok: true; text: string } | { ok: false; errors: FileError[] };

/** The language server's page file requests. */
export interface PageFiles {
  read(text: string, file: string): Promise<PageRead>;
  write(model: HmiPageModel): Promise<PageWrite>;
}

/** Whole lines `startLine` up to (not including) `endLine` replaced by `text`. */
export interface LineReplacement {
  startLine: number;
  endLine: number;
  text: string;
}

/** A text as lines, each with its line break; line endings are compared as `\n`. */
function linesOf(text: string): string[] {
  const parts = text.replace(/\r\n/g, "\n").split("\n");
  const lines = parts.slice(0, -1).map((part) => `${part}\n`);
  const last = parts[parts.length - 1];
  return last === "" ? lines : [...lines, last];
}

/** The smallest whole-line replacement that turns `before` into `after`; none when they are equal
 * apart from line endings. */
export function smallestEdit(before: string, after: string): LineReplacement | undefined {
  const old = linesOf(before);
  const next = linesOf(after);
  let start = 0;
  while (start < old.length && start < next.length && old[start] === next[start]) start += 1;
  if (start === old.length && start === next.length) return undefined;
  let oldEnd = old.length;
  let nextEnd = next.length;
  while (oldEnd > start && nextEnd > start && old[oldEnd - 1] === next[nextEnd - 1]) {
    oldEnd -= 1;
    nextEnd -= 1;
  }
  return { startLine: start, endLine: oldEnd, text: next.slice(start, nextEnd).join("") };
}

/** Whether two models hold the same values, whatever the order of their keys. */
export function sameModel(left: unknown, right: unknown): boolean {
  if (left === right) return true;
  if (Array.isArray(left) || Array.isArray(right)) {
    return (
      Array.isArray(left) &&
      Array.isArray(right) &&
      left.length === right.length &&
      left.every((item, index) => sameModel(item, right[index]))
    );
  }
  if (typeof left !== "object" || typeof right !== "object" || left === null || right === null) return false;
  const leftKeys = Object.keys(left).filter((key) => (left as Record<string, unknown>)[key] !== undefined);
  const rightKeys = Object.keys(right).filter((key) => (right as Record<string, unknown>)[key] !== undefined);
  return (
    leftKeys.length === rightKeys.length &&
    leftKeys.every((key) => sameModel((left as Record<string, unknown>)[key], (right as Record<string, unknown>)[key]))
  );
}
