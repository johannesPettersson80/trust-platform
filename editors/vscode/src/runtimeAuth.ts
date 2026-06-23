import * as vscode from "vscode";

import { pickAuthToken } from "./runtimeAuthModel";

// §0.6.8 — runtime control auth tokens live in VS Code SecretStorage, keyed per control endpoint, NOT in
// plaintext settings. The legacy `trust-lsp.runtime.controlAuthToken` setting is read ONLY as a fallback
// for existing configs; new tokens are stored securely via the `trust-lsp.runtime.setAuthToken` command
// (registered programmatically, surfaced from the connect-auth-failure prompt — not the palette).

const SECRET_PREFIX = "trust.runtime.authToken:";
const LEGACY_SETTING = "runtime.controlAuthToken";

let secrets: vscode.SecretStorage | undefined;

function secretKey(endpoint: string): string {
  return `${SECRET_PREFIX}${endpoint.trim()}`;
}

function legacyToken(): string | undefined {
  const value = vscode.workspace
    .getConfiguration("trust-lsp")
    .get<string>(LEGACY_SETTING, "");
  return value.trim().length > 0 ? value.trim() : undefined;
}

/** SecretStorage first (per endpoint), then the legacy plaintext setting. */
export async function getControlAuthToken(
  endpoint: string | undefined
): Promise<string | undefined> {
  const ep = (endpoint ?? "").trim();
  let stored: string | undefined;
  if (secrets && ep) {
    stored = await secrets.get(secretKey(ep));
  }
  return pickAuthToken(stored, legacyToken());
}

export async function setControlAuthToken(
  endpoint: string,
  token: string
): Promise<void> {
  if (!secrets) {
    return;
  }
  const ep = endpoint.trim();
  if (!ep) {
    return;
  }
  if (token.trim().length > 0) {
    await secrets.store(secretKey(ep), token.trim());
  } else {
    await secrets.delete(secretKey(ep));
  }
}

export function registerRuntimeAuth(context: vscode.ExtensionContext): void {
  secrets = context.secrets;
  context.subscriptions.push(
    vscode.commands.registerCommand(
      "trust-lsp.runtime.setAuthToken",
      async (arg?: { endpoint?: string }) => {
        const configEndpoint = vscode.workspace
          .getConfiguration("trust-lsp")
          .get<string>("runtime.controlEndpoint", "")
          .trim();
        let endpoint = (arg?.endpoint ?? configEndpoint).trim();
        if (!endpoint) {
          endpoint =
            (
              await vscode.window.showInputBox({
                prompt: "Runtime control endpoint this token is for",
                placeHolder: "tcp://raspberrypi:5680",
                ignoreFocusOut: true,
              })
            )?.trim() ?? "";
        }
        if (!endpoint) {
          return;
        }
        const token = await vscode.window.showInputBox({
          prompt: `Auth token for ${endpoint}`,
          password: true,
          ignoreFocusOut: true,
        });
        if (token === undefined) {
          return; // cancelled
        }
        await setControlAuthToken(endpoint, token);
        void vscode.window.showInformationMessage(
          token.trim().length > 0
            ? "Runtime auth token saved to the OS secret store."
            : "Runtime auth token cleared."
        );
      }
    )
  );
}
