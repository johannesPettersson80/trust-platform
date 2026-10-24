import { execFile, spawn } from "child_process";
import * as vscode from "vscode";

import { getBinaryPath } from "./binary";

// Specification 44 §4.6: the toolchain bundle that compiles programs. Rust owns where it lies,
// its download and its check (`trust-runtime toolchain status|install`); this module asks for the
// status before a debug launch, starts the install when none is installed, and shows its progress.

export const INSTALL_TOOLCHAIN_COMMAND = "trust-lsp.toolchain.install";
export const INSTALL_TOOLCHAIN_FROM_FILE_COMMAND =
  "trust-lsp.toolchain.installFromFile";

/** `trust-runtime toolchain status --json`. */
export interface ToolchainStatus {
  readonly installed: boolean;
  readonly folder: string | null;
  readonly source: string | null;
  readonly trust: string | null;
}

/** One line of `trust-runtime toolchain install --json`. */
type InstallEvent =
  | { readonly event: "download"; readonly bytes: number; readonly total: number | null }
  | { readonly event: "verify" }
  | { readonly event: "installed"; readonly folder: string }
  | { readonly event: "failed"; readonly error: string };

function runtimeBinary(context: vscode.ExtensionContext): string {
  return getBinaryPath(context, "trust-runtime", "runtime.cli.path");
}

export function registerToolchainCommands(context: vscode.ExtensionContext): void {
  context.subscriptions.push(
    vscode.commands.registerCommand(INSTALL_TOOLCHAIN_COMMAND, () =>
      installToolchain(context)
    ),
    vscode.commands.registerCommand(INSTALL_TOOLCHAIN_FROM_FILE_COMMAND, async () => {
      const picked = await vscode.window.showOpenDialog({
        canSelectMany: false,
        openLabel: "Install",
        filters: { "truST toolchain": ["gz"] },
      });
      return picked?.[0] ? installToolchain(context, picked[0].fsPath) : false;
    })
  );
}

/** The toolchain's status, or `undefined` when trust-runtime cannot answer. */
export function toolchainStatus(
  context: vscode.ExtensionContext
): Promise<ToolchainStatus | undefined> {
  return new Promise((resolve) => {
    execFile(
      runtimeBinary(context),
      ["toolchain", "status", "--json"],
      { timeout: 30000 },
      (error, stdout) => {
        if (error) {
          resolve(undefined);
          return;
        }
        try {
          resolve(JSON.parse(stdout) as ToolchainStatus);
        } catch {
          resolve(undefined);
        }
      }
    );
  });
}

/**
 * Before a build that needs the toolchain: installs it when none is installed, as specification 44
 * §4.6 has the extension do on the first build. Resolves whether a toolchain is there; a launch
 * goes on either way, without one on the bytecode engine (NX-16).
 */
export async function ensureToolchain(
  context: vscode.ExtensionContext
): Promise<boolean> {
  const status = await toolchainStatus(context);
  if (!status) {
    return false;
  }
  if (status.installed) {
    return true;
  }
  return installToolchain(context);
}

/**
 * Installs the toolchain with `trust-runtime toolchain install --json`, downloaded or from the
 * bundle file `from`, its progress in a notification. A failure offers the install from a file.
 */
export function installToolchain(
  context: vscode.ExtensionContext,
  from?: string
): Thenable<boolean> {
  const args = ["toolchain", "install", "--json"];
  if (from) {
    args.push("--from", from);
  }
  return vscode.window
    .withProgress(
      {
        location: vscode.ProgressLocation.Notification,
        title: "Installing the truST toolchain",
        cancellable: true,
      },
      (progress, token) =>
        new Promise<string | undefined>((resolve) => {
          const child = spawn(runtimeBinary(context), args);
          token.onCancellationRequested(() => child.kill());
          let failure: string | undefined = "the install stopped";
          let pending = "";
          let shown = 0;
          child.stdout.on("data", (chunk: Buffer) => {
            pending += chunk.toString();
            const lines = pending.split("\n");
            pending = lines.pop() ?? "";
            for (const line of lines) {
              let event: InstallEvent;
              try {
                event = JSON.parse(line) as InstallEvent;
              } catch {
                continue;
              }
              switch (event.event) {
                case "download": {
                  const mb = Math.floor(event.bytes / 1048576);
                  if (event.total) {
                    const percent = Math.floor((event.bytes * 100) / event.total);
                    progress.report({
                      message: `${mb} of ${Math.floor(event.total / 1048576)} MB`,
                      increment: percent - shown,
                    });
                    shown = percent;
                  } else {
                    progress.report({ message: `${mb} MB` });
                  }
                  break;
                }
                case "verify":
                  progress.report({ message: "checking the files" });
                  break;
                case "installed":
                  failure = undefined;
                  break;
                case "failed":
                  failure = event.error;
                  break;
              }
            }
          });
          child.on("error", (error) => resolve(error.message));
          child.on("close", (code) =>
            resolve(code === 0 ? failure : (failure ?? `exit ${code}`))
          );
        })
    )
    .then(async (failure) => {
      if (failure === undefined) {
        vscode.window.showInformationMessage("The truST toolchain is installed.");
        return true;
      }
      const choice = await vscode.window.showWarningMessage(
        `The truST toolchain was not installed: ${failure}`,
        "Install Toolchain from File"
      );
      if (choice) {
        await vscode.commands.executeCommand(INSTALL_TOOLCHAIN_FROM_FILE_COMMAND);
      }
      return false;
    });
}
