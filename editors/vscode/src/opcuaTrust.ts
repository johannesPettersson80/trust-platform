import { execFile } from "child_process";
import * as vscode from "vscode";

import { getBinaryPath } from "./binary";

// §0.5 OPC-UA client cert trust is "trust on first use" + EXPLICIT consent (never auto). This command
// is the reversible escape hatch: list/clear the trusted OPC-UA server certificates so a trust
// decision can be undone. Offline — reads the runtime's PKI dir, no running runtime needed.

interface TrustListResponse {
  trusted?: { file_name?: string; path?: string }[];
}
interface TrustClearResponse {
  cleared?: number;
}

type RunResult<T> = { ok: true; value: T } | { ok: false; error: string };

// Distinguish a real CLI failure from an empty trust store: a spawn/exit error or unparseable output
// must NOT be silently treated as "no certificates" (that would be a false success).
function runJson<T>(bin: string, args: string[]): Promise<RunResult<T>> {
  return new Promise((resolve) => {
    execFile(
      bin,
      args,
      { timeout: 15_000, maxBuffer: 8 * 1024 * 1024 },
      (error, stdout, stderr) => {
        if (error) {
          const detail = (stderr || error.message || String(error)).toString().trim();
          resolve({ ok: false, error: detail || "trust-runtime comm opcua-trust failed" });
          return;
        }
        try {
          resolve({ ok: true, value: JSON.parse(stdout) as T });
        } catch {
          resolve({ ok: false, error: "unexpected output from trust-runtime comm opcua-trust" });
        }
      }
    );
  });
}

export function registerOpcuaTrust(context: vscode.ExtensionContext): void {
  context.subscriptions.push(
    vscode.commands.registerCommand("trust-lsp.opcua.clearTrustedCertificates", async () => {
      const bin = getBinaryPath(context, "trust-runtime", "runtime.cli.path");
      const list = await runJson<TrustListResponse>(bin, ["comm", "opcua-trust", "list", "--json"]);
      if (!list.ok) {
        await vscode.window.showWarningMessage(
          `Could not read trusted OPC UA server certificates: ${list.error}`
        );
        return;
      }
      const trusted = Array.isArray(list.value.trusted) ? list.value.trusted : [];
      if (trusted.length === 0) {
        await vscode.window.showInformationMessage(
          "No trusted OPC UA server certificates to clear."
        );
        return;
      }
      const choice = await vscode.window.showWarningMessage(
        `Clear ${trusted.length} trusted OPC UA server certificate(s)? truST will require explicit trust again the next time it connects.`,
        { modal: true },
        "Clear"
      );
      if (choice !== "Clear") {
        return;
      }
      const result = await runJson<TrustClearResponse>(bin, ["comm", "opcua-trust", "clear", "--json"]);
      if (!result.ok) {
        await vscode.window.showWarningMessage(
          `Could not clear trusted OPC UA server certificates: ${result.error}`
        );
        return;
      }
      await vscode.window.showInformationMessage(
        `Cleared ${result.value.cleared ?? trusted.length} trusted OPC UA server certificate(s).`
      );
    })
  );
}
