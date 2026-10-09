// The runtime's write policy from the editor (specification 39, §12.2): a control whose point no
// rule covers gets its rule in the project's runtime.toml in one action, with the control's own
// role and limits, so the policy grows by exactly what the page asks and never wider.

import * as vscode from "vscode";
import type { HmiObjectModel } from "../drawing/model";
import { notWritable } from "./policyProblems";
import { appended, editRuntimeToml } from "./runtimeToml";

export const ALLOW_WRITING_COMMAND = "trust-lsp.hmiBuilder.allowWriting";

export interface WriteRule {
  path: string;
  role: "operator" | "engineer";
  min?: number;
  max?: number;
}

/** The rule a control needs to write a point: its role, Engineer when it names none (§12.3), and
 * its limits. */
export function ruleFor(object: Pick<HmiObjectModel, "role" | "min" | "max">, point: string): WriteRule {
  return {
    path: point,
    role: object.role ?? "engineer",
    ...(typeof object.min === "number" ? { min: object.min } : {}),
    ...(typeof object.max === "number" ? { max: object.max } : {}),
  };
}

/** The points runtime.toml's `[[runtime.web.hmi.write]]` rules name, in lower case; undefined
 * when it lists its rules inline, where a new table would not load. */
export function ruledPoints(text: string): Set<string> | undefined {
  const points = new Set<string>();
  let section = "";
  for (const line of text.split(/\r?\n/)) {
    const header = /^\s*(\[\[?)\s*([^\]]+?)\s*\]\]?\s*(#.*)?$/.exec(line);
    if (header) {
      section = `${header[1]}${header[2]}`;
      continue;
    }
    if (section === "[runtime.web.hmi" && /^\s*write\s*=/.test(line)) return undefined;
    const point = /^\s*path\s*=\s*"([^"]*)"/.exec(line);
    if (section === "[[runtime.web.hmi.write" && point) points.add(point[1].toLowerCase());
  }
  return points;
}

/** The TOML of rules, appended to runtime.toml. */
export function rulesText(rules: WriteRule[]): string {
  const number = (value: number) => (Number.isInteger(value) ? value.toFixed(1) : String(value));
  return rules
    .map((rule) =>
      [
        "",
        "[[runtime.web.hmi.write]]",
        `path = ${JSON.stringify(rule.path)}`,
        `role = "${rule.role}"`,
        ...(rule.min !== undefined ? [`min = ${number(rule.min)}`] : []),
        ...(rule.max !== undefined ? [`max = ${number(rule.max)}`] : []),
      ].join("\n"),
    )
    .join("\n")
    .concat("\n");
}

/** Adds to the project's runtime.toml the rules for the points it does not name yet, and saves it
 * unless it had unsaved changes of the user's own; answers the points added. */
export async function allowWriting(root: vscode.Uri, rules: WriteRule[]): Promise<string[]> {
  let added: WriteRule[] = [];
  await editRuntimeToml(root, (text) => {
    const named = ruledPoints(text);
    if (!named)
      throw new Error("runtime.toml lists its write rules inline under [runtime.web.hmi]; add the rule there by hand.");
    added = rules.filter(
      (rule, index) =>
        !named.has(rule.path.toLowerCase()) &&
        rules.findIndex((other) => other.path.toLowerCase() === rule.path.toLowerCase()) === index,
    );
    return added.length > 0 ? appended(text, rulesText(added)) : text;
  });
  return added.map((rule) => rule.path);
}

/** The project of a page, popup or panel file, the folder that holds its `hmi/`, and the file's
 * kind as the language server reads it. */
function placeOf(file: vscode.Uri): { root: vscode.Uri; kind: string } | undefined {
  const parts = file.path.split("/");
  const hmi = parts.lastIndexOf("hmi");
  const folder = parts[hmi + 1];
  if (hmi < 1 || !["pages", "popups", "panels"].includes(folder)) return undefined;
  return { root: file.with({ path: parts.slice(0, hmi).join("/") }), kind: folder.replace(/s$/, "") };
}

/** The rules a page's controls need, one per point they write, or for `point` only. Several
 * controls writing one point give the lowest role they ask and the widest limits, so none of them
 * asks more than the rule allows. */
export async function pageRules(page: vscode.Uri, point?: string): Promise<{ root: vscode.Uri; rules: WriteRule[] }> {
  const place = placeOf(page);
  if (!place) throw new Error("The page is not in a project's hmi folder.");
  const text = (await vscode.workspace.openTextDocument(page)).getText();
  const parsed = await vscode.commands.executeCommand<{ ok?: boolean; model?: { object?: HmiObjectModel[] } }>(
    "trust-lsp.hmiFileParse",
    { kind: place.kind, text, file: page.path },
  );
  const byPoint = new Map<string, WriteRule[]>();
  for (const object of parsed?.model?.object ?? []) {
    for (const binding of Object.values(object.bind ?? {})) {
      if (binding.direction === "read" || (point && binding.path.toLowerCase() !== point.toLowerCase())) continue;
      const key = binding.path.toLowerCase();
      byPoint.set(key, [...(byPoint.get(key) ?? []), ruleFor(object, binding.path)]);
    }
  }
  if (point && byPoint.size === 0) byPoint.set(point.toLowerCase(), [{ path: point, role: "engineer" }]);
  const rules = [...byPoint.values()].map((found): WriteRule => {
    const mins = found.map((rule) => rule.min);
    const maxes = found.map((rule) => rule.max);
    return {
      path: found[0].path,
      role: found.some((rule) => rule.role === "operator") ? "operator" : "engineer",
      ...(mins.every((value) => value !== undefined) ? { min: Math.min(...(mins as number[])) } : {}),
      ...(maxes.every((value) => value !== undefined) ? { max: Math.max(...(maxes as number[])) } : {}),
    };
  });
  return { root: place.root, rules };
}

export interface AllowWritingArgs {
  /** The page, popup or panel file whose control writes the point. */
  page: string;
  /** The point to allow; every point the page's controls write when absent. */
  point?: string;
}

/** The quick fix of a write-policy problem in a page file (§12.2.3). */
class AllowWritingActions implements vscode.CodeActionProvider {
  provideCodeActions(document: vscode.TextDocument, _range: vscode.Range, context: vscode.CodeActionContext) {
    return context.diagnostics.flatMap((diagnostic) => {
      const point = notWritable(diagnostic.message);
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
export function registerWritePolicy(): vscode.Disposable {
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
      new AllowWritingActions(),
      { providedCodeActionKinds: [vscode.CodeActionKind.QuickFix] },
    ),
  );
}
