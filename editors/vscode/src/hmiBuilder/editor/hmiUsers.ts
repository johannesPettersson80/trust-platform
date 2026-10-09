// The runtime display's users from the editor (specification 39, §12.8): Screen settings list who
// may log in, and add or remove a user. The password is stored only as the hash
// `trust-runtime hmi-password` makes, in the project's runtime.toml; the password itself is never
// written.

import { execFile } from "child_process";
import * as vscode from "vscode";
import { appended, editRuntimeToml, sectionsOf } from "./runtimeToml";
import { projectWithHmi } from "./symbolImport";

export const ADD_USER_COMMAND = "trust-lsp.hmiBuilder.addUser";
export const REMOVE_USER_COMMAND = "trust-lsp.hmiBuilder.removeUser";

export type HmiRole = "viewer" | "operator" | "engineer";

export interface HmiUserEntry {
  name: string;
  role: string;
}

const USER = "[[runtime.web.hmi.user]]";

/** The users runtime.toml lists, in its order. */
export function usersIn(text: string): HmiUserEntry[] {
  const lines = text.split(/\r?\n/);
  return sectionsOf(lines)
    .filter((section) => section.header === USER)
    .map((section) => {
      const user: HmiUserEntry = { name: "", role: "" };
      for (const line of lines.slice(section.start + 1, section.end)) {
        const value = /^\s*(name|role)\s*=\s*"([^"]*)"/.exec(line);
        if (value) user[value[1] as "name" | "role"] = value[2];
      }
      return user;
    });
}

/** A user's table, appended to runtime.toml. */
export function userText(name: string, role: HmiRole, hash: string): string {
  return `\n${USER}\nname = ${JSON.stringify(name)}\nrole = "${role}"\npassword = ${JSON.stringify(hash)}\n`;
}

/** runtime.toml without the user's table, with the blank lines that end it. */
export function withoutUser(text: string, name: string): string {
  const lines = text.split("\n");
  const section = sectionsOf(lines)
    .filter((candidate) => candidate.header === USER)
    .find((candidate) =>
      lines
        .slice(candidate.start + 1, candidate.end)
        .some((line) => (/^\s*name\s*=\s*"([^"]*)"/.exec(line)?.[1] ?? "").toLowerCase() === name.toLowerCase()),
    );
  if (!section) return text;
  return [...lines.slice(0, section.start), ...lines.slice(section.end)].join("\n");
}

/** The stored hash of a password, made by `trust-runtime hmi-password` from standard input. */
function hashOf(binary: string, password: string, cwd: string): Promise<string> {
  return new Promise((resolve, reject) => {
    const child = execFile(binary, ["hmi-password"], { cwd, timeout: 60_000 }, (error, stdout, stderr) => {
      const hash = stdout.trim();
      if (error || !hash.startsWith("pbkdf2-sha256:")) reject(new Error(stderr.trim() || error?.message || "no hash"));
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
  const taken = usersIn(await runtimeText(root)).map((user) => user.name.toLowerCase());
  const name = (
    args.name ??
    (await vscode.window.showInputBox({
      title: "Add HMI user",
      prompt: "The name the user logs in with on the runtime display",
      validateInput: (value) =>
        !value.trim()
          ? "Give the user a name"
          : taken.includes(value.trim().toLowerCase())
            ? "A user of this name exists already"
            : undefined,
    }))
  )?.trim();
  if (!name || taken.includes(name.toLowerCase())) return undefined;
  const role = args.role ?? (await vscode.window.showQuickPick(ROLES, { title: `What ${name} may do` }))?.role;
  if (!role) return undefined;
  const password =
    args.password ??
    (await vscode.window.showInputBox({
      title: `Password of ${name}`,
      password: true,
      validateInput: (value) => (value.length < 8 ? "Use at least 8 characters" : undefined),
    }));
  if (password === undefined || password.length < 8) return undefined;
  try {
    const hash = await hashOf(binary, password, root.fsPath);
    await editRuntimeToml(root, (text) => appended(text, userText(name, role, hash)));
  } catch (error) {
    void vscode.window.showErrorMessage(
      `${name} was not added: ${error instanceof Error ? error.message : String(error)}`,
    );
    return undefined;
  }
  return name;
}

async function removeUser(args: { project: string; name: string }): Promise<boolean> {
  return editRuntimeToml(vscode.Uri.parse(args.project), (text) => withoutUser(text, args.name));
}

/** The users of a project, for Screen settings. */
export async function projectUsers(root: vscode.Uri): Promise<HmiUserEntry[]> {
  return usersIn(await runtimeText(root));
}

export function registerHmiUsers(binary: () => string): vscode.Disposable {
  return vscode.Disposable.from(
    vscode.commands.registerCommand(ADD_USER_COMMAND, (args?: AddUserArgs) => addUser(binary(), args ?? {})),
    vscode.commands.registerCommand(REMOVE_USER_COMMAND, (args: { project: string; name: string }) => removeUser(args)),
  );
}
