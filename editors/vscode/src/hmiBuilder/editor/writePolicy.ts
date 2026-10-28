// The runtime's write policy from the editor (specification 39, §12.2): a control whose point no
// rule covers gets its rule in the project's runtime.toml in one action, with the control's own
// role and limits, so the policy grows by exactly what the page asks and never wider.

import type { HmiProblems } from "./problems";
import * as vscode from "vscode";
import { editRuntimeToml } from "./runtimeToml";

export const ALLOW_WRITING_COMMAND = "trust-lsp.hmiBuilder.allowWriting";

export interface WriteRule {
  path: string;
  role: "operator" | "engineer";
  min?: number;
  max?: number;
}

/** What `trust-lsp.hmiWriteRules` answers. */
interface WriteRulesAnswer {
  ok?: boolean;
  error?: string;
  root?: string;
  rules?: WriteRule[];
  text?: string;
  added?: string[];
}

const NO_ANSWER = "The truST language server is not running, so runtime.toml could not be changed.";

async function writeRules(argument: Record<string, unknown>): Promise<WriteRulesAnswer> {
  const answer = await vscode.commands.executeCommand<WriteRulesAnswer | undefined>(
    "trust-lsp.hmiWriteRules",
    argument,
  );
  if (answer?.ok !== true) throw new Error(answer?.error ?? NO_ANSWER);
  return answer;
}

/** Adds to the project's runtime.toml the rules for the points it does not name yet, and saves it
 * unless it had unsaved changes of the user's own; answers the points added. The language server
 * decides which rules are new and writes them (§12.2 item 3). */
export async function allowWriting(root: vscode.Uri, rules: WriteRule[]): Promise<string[]> {
  let added: string[] = [];
  await editRuntimeToml(root, async (text) => {
    const answer = await writeRules({ action: "add", runtimeToml: text, rules });
    added = answer.added ?? [];
    return answer.text ?? text;
  });
  return added;
}

/** The rules a page's controls need, one per point they write, or for `point` only, as the
 * language server derives them, and the project whose runtime.toml takes them. */
export async function pageRules(page: vscode.Uri, point?: string): Promise<{ root: vscode.Uri; rules: WriteRule[] }> {
  const text = (await vscode.workspace.openTextDocument(page)).getText();
  const answer = await writeRules({ action: "forPage", file: page.path, text, ...(point ? { point } : {}) });
  return { root: page.with({ path: answer.root ?? "" }), rules: answer.rules ?? [] };
}

export interface AllowWritingArgs {
  /** The page, popup or panel file whose control writes the point. */
  page: string;
  /** The point to allow; every point the page's controls write when absent. */
  point?: string;
}

/** The quick fix of a write-policy problem in a page file (§12.2.3). */
class AllowWritingActions implements vscode.CodeActionProvider {
  /** `pointOf` gives the point a problem names, as the language server named it. */
  constructor(private readonly pointOf: (uri: vscode.Uri, diagnostic: vscode.Diagnostic) => string | undefined) {}

  provideCodeActions(document: vscode.TextDocument, _range: vscode.Range, context: vscode.CodeActionContext) {
    return context.diagnostics.flatMap((diagnostic) => {
      const point = this.pointOf(document.uri, diagnostic);
      if (!point) return [];
      const action = new vscode.CodeAction(
        `Allow the runtime display to write ${point} (runtime.toml)`,
        vscode.CodeActionKind.QuickFix,
      );
      action.diagnostics = [diagnostic];
      action.isPreferred = true;
      action.command = {
        command: ALLOW_WRITING_COMMAND,
        title: action.title,
        arguments: [{ page: document.uri.toString(), point } satisfies AllowWritingArgs],
      };
      return [action];
    });
  }
}

/** The command, used by the builder's inspector and the quick fix, and the quick fix. Saving
 * runtime.toml checks the project's pages again (`HmiProblems`). */
export function registerWritePolicy(problems: HmiProblems): vscode.Disposable {
  return vscode.Disposable.from(
    vscode.commands.registerCommand(ALLOW_WRITING_COMMAND, async (args: AllowWritingArgs) => {
      try {
        const { root, rules } = await pageRules(vscode.Uri.parse(args.page), args.point);
        return await allowWriting(root, rules);
      } catch (error) {
        void vscode.window.showErrorMessage(error instanceof Error ? error.message : String(error));
        return [];
      }
    }),
    vscode.languages.registerCodeActionsProvider(
      { scheme: "file", pattern: "**/hmi/{pages,popups,panels}/*.toml" },
      new AllowWritingActions((uri, diagnostic) => problems.pointOf(uri, diagnostic)),
      { providedCodeActionKinds: [vscode.CodeActionKind.QuickFix] },
    ),
  );
}
