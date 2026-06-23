import * as vscode from "vscode";

import {
  runtimeSourceOptionsForTarget,
  type RuntimeSourceOptions,
} from "../runtimeSourceOptions";
import {
  CompileIssue,
  CompileOptions,
  CompileResult,
} from "./types";

const DEBUG_TYPE = "structured-text";
const PRAGMA_SCAN_LINES = 20;

type CompileDeps = {
  getPanel: () => vscode.WebviewPanel | undefined;
  getStructuredTextSession: () => vscode.DebugSession | undefined;
  startDebugging: (programOverride?: string) => Promise<void>;
};

function diagnosticCodeLabel(
  code: string | number | { value: string | number; target?: vscode.Uri } | undefined
): string | undefined {
  if (code === undefined) {
    return undefined;
  }
  if (typeof code === "string" || typeof code === "number") {
    return String(code);
  }
  if (typeof code === "object" && "value" in code) {
    return String(code.value);
  }
  return undefined;
}

async function readStructuredText(
  uri: vscode.Uri
): Promise<string | undefined> {
  const openDoc = vscode.workspace.textDocuments.find(
    (doc) => doc.uri.toString() === uri.toString()
  );
  if (openDoc) {
    return openDoc.getText();
  }
  try {
    const data = await vscode.workspace.fs.readFile(uri);
    return new TextDecoder("utf-8").decode(data);
  } catch {
    return undefined;
  }
}

function containsConfiguration(source: string): boolean {
  return /\bCONFIGURATION\b/i.test(source);
}

async function sourcesContainConfiguration(
  uris: vscode.Uri[]
): Promise<boolean> {
  for (const uri of uris) {
    const text = await readStructuredText(uri);
    if (text && containsConfiguration(text)) {
      return true;
    }
  }
  return false;
}

async function collectRuntimeSources(
  targetDoc?: vscode.TextDocument
): Promise<vscode.Uri[]> {
  const runtimeOptions = runtimeSourceOptions(targetDoc?.uri);
  const includeGlobs = runtimeOptions.runtimeIncludeGlobs ?? [];
  const excludeGlobs = runtimeOptions.runtimeExcludeGlobs ?? [];
  const ignorePragmas = runtimeOptions.runtimeIgnorePragmas ?? [];
  const runtimeRoot =
    runtimeOptions.runtimeRoot ??
    (targetDoc
      ? vscode.workspace.getWorkspaceFolder(targetDoc.uri)?.uri.fsPath
      : vscode.workspace.workspaceFolders?.[0]?.uri.fsPath);
  if (!runtimeRoot) {
    return [];
  }

  const baseUri = vscode.Uri.file(runtimeRoot);
  const excludePattern = buildGlobAlternation(excludeGlobs);
  const exclude = excludePattern
    ? new vscode.RelativePattern(baseUri, excludePattern)
    : undefined;

  const candidates: vscode.Uri[] = [];
  for (const include of includeGlobs) {
    const pattern = new vscode.RelativePattern(baseUri, include);
    const matches = await vscode.workspace.findFiles(pattern, exclude);
    candidates.push(...matches);
  }

  const unique = new Map<string, vscode.Uri>();
  for (const candidate of candidates) {
    unique.set(candidate.fsPath, candidate);
  }
  if (targetDoc?.uri.fsPath) {
    unique.set(targetDoc.uri.fsPath, targetDoc.uri);
  }

  if (ignorePragmas.length === 0) {
    return Array.from(unique.values());
  }

  const filtered: vscode.Uri[] = [];
  for (const candidate of unique.values()) {
    if (targetDoc && candidate.fsPath === targetDoc.uri.fsPath) {
      filtered.push(candidate);
      continue;
    }
    if (await hasRuntimeIgnorePragma(candidate, ignorePragmas)) {
      continue;
    }
    filtered.push(candidate);
  }
  return filtered;
}

function buildGlobAlternation(globs: string[]): string | undefined {
  const normalized = globs.map((glob) => glob.trim()).filter(Boolean);
  if (normalized.length === 0) {
    return undefined;
  }
  if (normalized.length === 1) {
    return normalized[0];
  }
  return `{${normalized.join(",")}}`;
}

async function hasRuntimeIgnorePragma(
  uri: vscode.Uri,
  pragmas: string[]
): Promise<boolean> {
  if (pragmas.length === 0) {
    return false;
  }
  const text = await readStructuredText(uri);
  if (!text) {
    return false;
  }
  const lines = text.split(/\r?\n/).slice(0, PRAGMA_SCAN_LINES);
  for (const line of lines) {
    for (const pragma of pragmas) {
      if (pragma && line.includes(pragma)) {
        return true;
      }
    }
  }
  return false;
}

function runtimeSourceOptions(target?: vscode.Uri): RuntimeSourceOptions {
  return runtimeSourceOptionsForTarget(target);
}

function workspaceHasDirtyStructuredText(): boolean {
  return vscode.workspace.textDocuments.some(
    (doc) => doc.languageId === "structured-text" && doc.isDirty
  );
}

export async function compileActiveProgram(
  deps: CompileDeps,
  options: CompileOptions = {}
): Promise<void> {
  const panel = deps.getPanel();
  if (!panel) {
    return;
  }

  panel.webview.postMessage({
    type: "status",
    payload: "Compiling...",
  });

  const workspaceFolder = vscode.workspace.workspaceFolders?.[0];
  if (!workspaceFolder) {
    panel.webview.postMessage({
      type: "status",
      payload: "Open a workspace folder to compile.",
    });
    panel.webview.postMessage({
      type: "compileResult",
      payload: {
        target: "",
        dirty: false,
        errors: 0,
        warnings: 0,
        issues: [],
        runtimeStatus: "skipped",
        runtimeMessage: "No workspace folder open.",
      } satisfies CompileResult,
    });
    return;
  }

  const sourceUris = await collectRuntimeSources();
  const hasConfiguration = await sourcesContainConfiguration(sourceUris);
  if (sourceUris.length === 0) {
    panel.webview.postMessage({
      type: "status",
      payload: "No Structured Text files found in the workspace.",
    });
    panel.webview.postMessage({
      type: "compileResult",
      payload: {
        target: workspaceFolder.uri.fsPath,
        dirty: false,
        errors: 0,
        warnings: 0,
        issues: [],
        runtimeStatus: "skipped",
        runtimeMessage: "No Structured Text files found.",
      } satisfies CompileResult,
    });
    return;
  }

  let runtimeStatus: CompileResult["runtimeStatus"] = "skipped";
  let runtimeMessage: string | undefined;
  const session = deps.getStructuredTextSession();
  if (session) {
    const program =
      typeof session.configuration?.program === "string"
        ? session.configuration.program
        : undefined;
    if (!program) {
      runtimeStatus = "error";
      runtimeMessage = "Active debug session missing entry configuration.";
    } else {
      runtimeStatus = "ok";
      try {
        const runtimeOptions = runtimeSourceOptions(vscode.Uri.file(program));
        await session.customRequest("stReload", {
          program,
          ...runtimeOptions,
        });
        runtimeMessage = "Runtime reload succeeded.";
      } catch (err) {
        runtimeStatus = "error";
        const message = err instanceof Error ? err.message : String(err);
        runtimeMessage = `Runtime compile failed: ${message}`;
      }
    }
  }

  const issues: CompileIssue[] = [];
  for (const uri of sourceUris) {
    const fileDiagnostics = vscode.languages.getDiagnostics(uri);
    for (const diagnostic of fileDiagnostics) {
      if (
        diagnostic.severity !== vscode.DiagnosticSeverity.Error &&
        diagnostic.severity !== vscode.DiagnosticSeverity.Warning
      ) {
        continue;
      }
      issues.push({
        file: uri.fsPath,
        line: diagnostic.range.start.line + 1,
        column: diagnostic.range.start.character + 1,
        severity:
          diagnostic.severity === vscode.DiagnosticSeverity.Error
            ? "error"
            : "warning",
        message: diagnostic.message,
        code: diagnosticCodeLabel(diagnostic.code),
        source: diagnostic.source,
      });
    }
  }

  const errors = issues.filter((issue) => issue.severity === "error").length;
  const warnings = issues.filter((issue) => issue.severity === "warning").length;
  const dirty = workspaceHasDirtyStructuredText();
  const runtimeTarget =
    session && session.type === DEBUG_TYPE
      ? typeof session.configuration?.program === "string"
        ? session.configuration.program
        : undefined
      : undefined;

  panel.webview.postMessage({
    type: "compileResult",
    payload: {
      target: runtimeTarget ?? workspaceFolder.uri.fsPath,
      dirty,
      errors,
      warnings,
      issues,
      runtimeStatus,
      runtimeMessage:
        runtimeMessage ??
        (!hasConfiguration && runtimeStatus === "skipped"
          ? "No CONFIGURATION found. Debugging will prompt to create one."
          : undefined),
    } satisfies CompileResult,
  });

  let statusMessage = `Compile finished: ${errors} error(s), ${warnings} warning(s).`;
  if (runtimeStatus === "error" && runtimeMessage) {
    statusMessage = runtimeMessage;
  }
  if (options.startDebugAfter) {
    if (errors > 0) {
      statusMessage = `Compile blocked: ${errors} error(s). Fix errors before starting.`;
    } else if (dirty) {
      statusMessage = "Save all Structured Text files before starting the runtime.";
    } else {
      // No authoritative whole-project compile exists yet (phase 8) — this only aggregated diagnostics
      // + reload, so say what we actually know, not "Compile ok".
      statusMessage = "No known errors. Starting debug session...";
    }
  } else if (!hasConfiguration && runtimeStatus === "skipped" && errors === 0) {
    statusMessage +=
      " No CONFIGURATION found; debugging will prompt to create one.";
    const create = await vscode.window.showInformationMessage(
      "No CONFIGURATION found. Create one now?",
      "Create",
      "Not now"
    );
    if (create === "Create") {
      await vscode.commands.executeCommand(
        "trust-lsp.debug.ensureConfiguration"
      );
    }
  }
  panel.webview.postMessage({
    type: "status",
    payload: statusMessage,
  });

  if (options.startDebugAfter && errors === 0 && !dirty) {
    await deps.startDebugging();
  }
}
