// The requests of the visual editors to the truST language server (`trust-lsp.visual*`,
// specification 25 §11.4). Every rule about a Ladder, Blockly, Statechart or SFC program is
// answered there: its format and checks, the edits stored in it, its Structured Text companion and
// runtime entry, a new program's template and an import's target. The editors draw the answers.

import * as vscode from "vscode";

/** One problem the language server names. */
export interface VisualProblem {
  message: string;
  element?: string;
}

/** Every answer: `ok`, and the problems when it is not. */
export interface VisualAnswer {
  ok: boolean;
  errors?: VisualProblem[];
}

const SERVER_WAIT_MS = 15000;
const POLL_MS = 200;
const NOT_RUNNING = "The truST language server is not running yet.";

let serverCommandsSeen = false;

/** Waits, a bounded time, until the language client has registered the server's commands. */
async function serverOffers(command: string): Promise<boolean> {
  if (serverCommandsSeen) {
    return true;
  }
  const deadline = Date.now() + SERVER_WAIT_MS;
  for (;;) {
    if ((await vscode.commands.getCommands(true)).includes(command)) {
      serverCommandsSeen = true;
      return true;
    }
    if (Date.now() > deadline) {
      return false;
    }
    await new Promise((resolve) => setTimeout(resolve, POLL_MS));
  }
}

/** Sends one visual-program request; a server that is not running or fails answers `ok: false`. */
export async function visualRequest<T extends VisualAnswer = VisualAnswer>(
  command: string,
  argument: Record<string, unknown>
): Promise<T> {
  if (!(await serverOffers(command))) {
    return { ok: false, errors: [{ message: NOT_RUNNING }] } as T;
  }
  try {
    const answer = await vscode.commands.executeCommand<T | undefined>(command, argument);
    return answer ?? ({ ok: false, errors: [{ message: "The language server gave no answer." }] } as T);
  } catch (error) {
    return {
      ok: false,
      errors: [{ message: error instanceof Error ? error.message : String(error) }],
    } as T;
  }
}

/** The problems of an answer, one per line, for a message. */
export function problemText(answer: VisualAnswer): string {
  const lines = (answer.errors ?? []).map((problem) => problem.message);
  return lines.length > 0 ? lines.join("\n") : "The language server gave no answer.";
}
