// The runtime display's users from the editor (specification 39, §12.8): Screen settings list who
// may log in, and add or remove a user. The language server (`trust-lsp.hmiUsers`) checks the name
// and the password's length and writes the user; the password is stored only as the hash
// `trust-runtime hmi-password` makes, in the project's runtime.toml, and never reaches the language
// server or the file.

import { execFile } from "child_process";
import * as vscode from "vscode";
import { editRuntimeToml } from "./runtimeToml";
import { projectWithHmi } from "./symbolImport";

export const ADD_USER_COMMAND = "trust-lsp.hmiBuilder.addUser";
export const REMOVE_USER_COMMAND = "trust-lsp.hmiBuilder.removeUser";

export type HmiRole = "viewer" | "operator" | "engineer";

export interface HmiUserEntry {
  name: string;
  role: string;
}

interface UsersAnswer {
  ok?: boolean;
  error?: string;
  users?: HmiUserEntry[];
  problem?: string | null;
  text?: string;
}

const NO_ANSWER = "The truST language server is not running.";

async function users(argument: Record<string, unknown>): Promise<UsersAnswer> {
  const answer = await vscode.commands.executeCommand<UsersAnswer | undefined>("trust-lsp.hmiUsers", argument);
  if (answer?.ok !== true) throw new Error(answer?.error ?? NO_ANSWER);
  return answer;
}

/** Why the name or the password cannot be taken, or undefined. */
async function problem(argument: Record<string, unknown>): Promise<string | undefined> {
  try {
    return (await users(argument)).problem ?? undefined;
  } catch (error) {
    return error instanceof Error ? error.message : String(error);
  }
}

/** The stored hash of a password, made by `trust-runtime hmi-password` from standard input. */
function hashOf(binary: string, password: string, cwd: string): Promise<string> {
  return new Promise((resolve, reject) => {
    const child = execFile(binary, ["hmi-password"], { cwd, timeout: 60_000 }, (error, stdout, stderr) => {
      const hash = stdout.trim();
      if (error || hash === "") reject(new Error(stderr.trim() || error?.message || "no hash"));
      else resolve(hash);
    });
    child.stdin?.end(password);
  });
}

async function runtimeText(root: vscode.Uri): Promise<string> {
  try {
    return (await vscode.workspace.openTextDocument(vscode.Uri.joinPath(root, "runtime.toml"))).getText();
  } catch {
    return "";
  }
}

export interface AddUserArgs {
  /** The project; by default the open project with an HMI. */
  project?: string;
  /** Given by tests and agents; the user is asked for each one missing. */
  name?: string;
  role?: HmiRole;
  password?: string;
}

const ROLES: Array<vscode.QuickPickItem & { role: HmiRole }> = [
  { label: "Operator", description: "Operates equipment and acknowledges alarms", role: "operator" },
  { label: "Engineer", description: "Also changes settings and takes equipment out of service", role: "engineer" },
  { label: "Viewer", description: "Watches only", role: "viewer" },
];

/** Asks the name, the role and the password, and adds the user; answers the name added. */
async function addUser(binary: string, args: AddUserArgs): Promise<string | undefined> {
  const root = args.project ? vscode.Uri.parse(args.project) : await projectWithHmi();
  if (!root) {
    void vscode.window.showErrorMessage("Open a project with an HMI (hmi/screen.toml) to add a user to.");
    return undefined;
  }
  const runtimeToml = await runtimeText(root);
  const nameProblem = (name: string) => problem({ action: "checkName", runtimeToml, name });
  const name = (
    args.name ??
    (await vscode.window.showInputBox({
      title: "Add HMI user",
      prompt: "The name the user logs in with on the runtime display",
      validateInput: nameProblem,
    }))
  )?.trim();
  if (name === undefined || (await nameProblem(name)) !== undefined) return undefined;
  const role = args.role ?? (await vscode.window.showQuickPick(ROLES, { title: `What ${name} may do` }))?.role;
  if (!role) return undefined;
  // Only the password's length in characters goes to the language server, never the password.
  const passwordProblem = (value: string) => problem({ action: "checkPassword", characters: [...value].length });
  const password =
    args.password ??
    (await vscode.window.showInputBox({
      title: `Password of ${name}`,
      password: true,
      validateInput: passwordProblem,
    }));
  if (password === undefined || (await passwordProblem(password)) !== undefined) return undefined;
  try {
    const hash = await hashOf(binary, password, root.fsPath);
    await editRuntimeToml(
      root,
      async (text) => (await users({ action: "add", runtimeToml: text, name, role, hash })).text ?? text,
    );
  } catch (error) {
    void vscode.window.showErrorMessage(
      `${name} was not added: ${error instanceof Error ? error.message : String(error)}`,
    );
    return undefined;
  }
  return name;
}

async function removeUser(args: { project: string; name: string }): Promise<boolean> {
  return editRuntimeToml(
    vscode.Uri.parse(args.project),
    async (text) => (await users({ action: "remove", runtimeToml: text, name: args.name })).text ?? text,
  );
}

/** The users of a project, for Screen settings; none while the language server cannot read them. */
export async function projectUsers(root: vscode.Uri): Promise<HmiUserEntry[]> {
  try {
    return (await users({ action: "list", runtimeToml: await runtimeText(root) })).users ?? [];
  } catch {
    return [];
  }
}

export function registerHmiUsers(binary: () => string): vscode.Disposable {
  return vscode.Disposable.from(
    vscode.commands.registerCommand(ADD_USER_COMMAND, (args?: AddUserArgs) => addUser(binary(), args ?? {})),
    vscode.commands.registerCommand(REMOVE_USER_COMMAND, (args: { project: string; name: string }) => removeUser(args)),
  );
}
