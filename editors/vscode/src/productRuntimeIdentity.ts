import * as vscode from "vscode";

import { getBinaryPath } from "./binary";

export interface ProductRuntimeIdentity {
  readonly schemaVersion: 1;
  readonly extensionMode: vscode.ExtensionMode;
  readonly extensionPath: string;
  readonly extensionVersion: string;
  readonly binaries: {
    readonly languageServer: string;
    readonly debugAdapter: string;
    readonly runtime: string;
  };
}

type BinaryResolver = (
  context: vscode.ExtensionContext,
  binaryName: string,
  configKey: string
) => string;

export function productRuntimeIdentity(
  context: vscode.ExtensionContext,
  resolveBinary: BinaryResolver = getBinaryPath,
  activeLanguageServer = resolveBinary(context, "trust-lsp", "server.path")
): ProductRuntimeIdentity {
  return {
    schemaVersion: 1,
    extensionMode: context.extensionMode,
    extensionPath: context.extensionPath,
    extensionVersion: String(context.extension.packageJSON.version ?? ""),
    binaries: {
      languageServer: activeLanguageServer,
      debugAdapter: resolveBinary(context, "trust-debug", "debug.adapter.path"),
      runtime: resolveBinary(context, "trust-runtime", "runtime.cli.path"),
    },
  };
}

export function createProductRuntimeIdentityApi(
  context: vscode.ExtensionContext,
  activeLanguageServer: string
) {
  return {
    getProductRuntimeIdentity: () =>
      productRuntimeIdentity(context, getBinaryPath, activeLanguageServer),
  };
}
