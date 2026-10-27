// Copy, paste and duplicate (specification 39, §3.3 item 8). The language server makes the
// clipboard text of the copied objects and the page they are pasted on (`trust-lsp.hmiClipboard`):
// new ids, pipes between copied objects, attachments and smart numbering are its rules. The
// webview asks for them here and commits the page it answers.

import type { HmiPageModel } from "../drawing/model";
import type { Point } from "./editing";
import { RUST_RULES, type CallRust, type VariableNode } from "./messages";

/** Where a paste lands from the original. */
export const PASTE_STEP: Point = [16, 16];

/** A page with pasted objects, and their ids. */
export interface Pasted {
  page: HmiPageModel;
  ids: string[];
}

/** The objects of `ids`, with the pipes between them, as clipboard text; none when none of them is
 * on the page. */
export async function copyText(call: CallRust, page: HmiPageModel, ids: string[]): Promise<string | undefined> {
  const answer = await call<{ text?: string }>(RUST_RULES.clipboard, { action: "copy", page, ids });
  return answer.text;
}

/** The page with the clipboard's objects pasted `step` from where they were copied; none when the
 * text holds no objects. */
export async function pasted(
  call: CallRust,
  page: HmiPageModel,
  text: string,
  step: Point,
  variables: VariableNode[],
): Promise<Pasted | undefined> {
  return pastedOf(await call<Partial<Pasted>>(RUST_RULES.clipboard, { action: "paste", page, text, step, variables }));
}

/** The page with a copy of the objects of `ids` pasted at once, `step` from them. */
export async function duplicated(
  call: CallRust,
  page: HmiPageModel,
  ids: string[],
  step: Point,
  variables: VariableNode[],
): Promise<Pasted | undefined> {
  return pastedOf(
    await call<Partial<Pasted>>(RUST_RULES.clipboard, { action: "duplicate", page, ids, step, variables }),
  );
}

function pastedOf(answer: Partial<Pasted>): Pasted | undefined {
  return answer.page && answer.ids ? { page: answer.page, ids: answer.ids } : undefined;
}
