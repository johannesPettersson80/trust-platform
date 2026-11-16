import { execFile } from "child_process";
import * as vscode from "vscode";
import { trustDevBinary } from "./binary";
import { projectRequest } from "./projectRequest";

/**
 * Keeps a project's generated declarations of its Rust POUs and its bindings of the Structured Text
 * its crate calls in step with the crate (specification 48 RU-24, RU-39): `trust-dev rust
 * declarations` runs for each open project once, and for a project again whenever a `.rs` file of
 * it is saved. What the project is and what the command writes are trust-dev's and the language
 * server's: the extension only starts the command and shows what it reports.
 */
export function registerRustDeclarations(context: vscode.ExtensionContext): void {
  const output = vscode.window.createOutputChannel("truST Rust declarations");
  context.subscriptions.push(output);
  // One run at a time for each project; a save during a run asks for one more after it.
  const running = new Map<string, Promise<void>>();
  const again = new Set<string>();

  const run = (root: string, notify: boolean): void => {
    if (running.has(root)) {
      again.add(root);
      return;
    }
    const done = declarations(context, root).then((failure) => {
      if (failure !== undefined) {
        output.appendLine(failure);
        if (notify) {
          void vscode.window
            .showWarningMessage(
              `The Rust declarations of ${root} are not written.`,
              "Show Output"
            )
            .then((choice) => {
              if (choice === "Show Output") {
                output.show(true);
              }
            });
        }
      }
      running.delete(root);
      if (again.delete(root)) {
        run(root, notify);
      }
    });
    running.set(root, done);
  };

  const folders = () => (vscode.workspace.workspaceFolders ?? []).map((folder) => folder.uri.fsPath);
  const projectOf = async (path: string | undefined): Promise<string | undefined> => {
    const answer = await projectRequest<{ root: string | null }>("trust-lsp.projectRoot", {
      ...(path ? { path } : {}),
      folders: folders(),
    });
    return answer.ok && answer.root ? answer.root : undefined;
  };

  context.subscriptions.push(
    vscode.workspace.onDidSaveTextDocument(async (document) => {
      if (document.uri.scheme !== "file" || !document.fileName.endsWith(".rs")) {
        return;
      }
      const root = await projectOf(document.uri.fsPath);
      if (root) {
        run(root, true);
      }
    })
  );
  void (async () => {
    for (const folder of folders()) {
      const root = await projectOf(folder);
      if (root) {
        run(root, false);
      }
    }
  })();
}

/** Runs `trust-dev rust declarations` for the project in `root`; gives what it reported when it failed. */
function declarations(context: vscode.ExtensionContext, root: string): Promise<string | undefined> {
  return new Promise((resolve) => {
    execFile(
      trustDevBinary(context),
      ["rust", "declarations", "--project", root],
      { cwd: root, timeout: 120_000, maxBuffer: 8 * 1024 * 1024, windowsHide: true },
      (error, stdout, stderr) => {
        resolve(error ? `${stdout}${stderr}`.trim() || error.message : undefined);
      }
    );
  });
}
