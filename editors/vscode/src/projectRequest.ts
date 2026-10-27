// The requests of the extension to the truST language server's project commands (`trust-lsp.runEntry`,
// `trust-lsp.compileGate`, `trust-lsp.projectLibraries`, …; specification 25). Every rule about a project's files,
// its simulator entry, its libraries, its tests, the sidebar's gates and the examples gallery is answered there;
// the extension shows the answer.

import * as vscode from "vscode";

const SERVER_WAIT_MS = 15000;
const POLL_MS = 200;
const NOT_RUNNING = "The truST language server is not running yet.";

/** An answer: `ok` with the command's fields, or `ok: false` with the reason. */
export type ProjectAnswer<T extends object> = ({ readonly ok: true } & T) | { readonly ok: false; readonly error: string };

let serverCommandsSeen = false;
/** Set once a wait ran out: a server that did not start in time is not waited for again, only looked for. */
let serverWaitSpent = false;

/** Whether the language client has registered the server's commands; with `wait`, waits a bounded time for them. */
async function serverOffers(command: string, wait: boolean): Promise<boolean> {
  if (serverCommandsSeen) {
    return true;
  }
  const waits = wait && !serverWaitSpent;
  const deadline = Date.now() + (waits ? SERVER_WAIT_MS : 0);
  for (;;) {
    if ((await vscode.commands.getCommands(true)).includes(command)) {
      serverCommandsSeen = true;
      return true;
    }
    if (Date.now() >= deadline) {
      if (waits) {
        serverWaitSpent = true;
      }
      return false;
    }
    await new Promise((resolve) => setTimeout(resolve, POLL_MS));
  }
}

/**
 * Sends one project request. A server that is not running answers `ok: false`: a user's action waits a bounded time
 * for it (`wait`, the default), a view that redraws often does not wait.
 */
export async function projectRequest<T extends object>(
  command: string,
  argument: object,
  options: { readonly wait?: boolean } = {}
): Promise<ProjectAnswer<T>> {
  if (!(await serverOffers(command, options.wait ?? true))) {
    return { ok: false, error: NOT_RUNNING };
  }
  try {
    const answer = await vscode.commands.executeCommand<ProjectAnswer<T> | undefined>(command, argument);
    return answer ?? { ok: false, error: "The language server gave no answer." };
  } catch (error) {
    return { ok: false, error: error instanceof Error ? error.message : String(error) };
  }
}
