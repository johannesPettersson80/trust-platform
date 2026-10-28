// Canvas edits the SFC webview asks its host for. The host forwards each one to the language server
// (`trust-lsp.visualEdit`, kind `sfc`), which owns the SFC rules, and answers with `editResult`.

import { getVsCodeApi } from "../../visual/runtime/webview/vscodeApi";
import type { SfcEdit, SfcEditAnswer, SfcGraph } from "./types";

const vscode = getVsCodeApi();
const pending = new Map<number, (answer: SfcEditAnswer) => void>();
let lastRequestId = 0;

/** Asks the host for the edit `edit` of the canvas graph `graph`. */
export function requestEdit(graph: SfcGraph, edit: SfcEdit): Promise<SfcEditAnswer> {
  const requestId = ++lastRequestId;
  return new Promise((resolve) => {
    pending.set(requestId, resolve);
    vscode.postMessage({ type: "edit", requestId, graph, edit });
  });
}

/** Settles the edit request `requestId` with the host's answer. */
export function settleEdit(requestId: number, answer: SfcEditAnswer): void {
  const resolve = pending.get(requestId);
  pending.delete(requestId);
  resolve?.(answer);
}

/** The time stamp new node and transition ids are made of. */
export function stamp(): string {
  return String(Date.now());
}
