import * as vscode from "vscode";

type LmToolResultCtor = new (parts: unknown[]) => unknown;
type LmTextPartCtor = new (value: string) => unknown;

const languageModelToolResultCtor = (
  vscode as unknown as { LanguageModelToolResult?: LmToolResultCtor }
).LanguageModelToolResult;

const languageModelTextPartCtor = (
  vscode as unknown as { LanguageModelTextPart?: LmTextPartCtor }
).LanguageModelTextPart;

type InvocationOptions<T> = {
  input: T;
};

type EmptyParams = Record<string, never>;

type DebugStartParams = {
  filePath?: string;
};

type CommandResult = {
  ok?: boolean;
  message?: string;
};

function textResult(text: string): unknown {
  if (languageModelToolResultCtor && languageModelTextPartCtor) {
    return new languageModelToolResultCtor([new languageModelTextPartCtor(text)]);
  }
  return text;
}

function errorResult(message: string): unknown {
  return textResult(`Error: ${message}`);
}

function uriFromFilePath(filePath: string): vscode.Uri | undefined {
  const raw = String(filePath ?? "").trim();
  if (!raw) {
    return undefined;
  }
  if (/^[A-Za-z][A-Za-z0-9+.-]*:\/\//.test(raw)) {
    try {
      const parsed = vscode.Uri.parse(raw);
      return parsed.scheme ? parsed : undefined;
    } catch {
      return undefined;
    }
  }
  if (/^file:[^/]/i.test(raw)) {
    return undefined;
  }
  if (/^[A-Za-z]:/.test(raw) || raw.startsWith("/") || raw.startsWith("\\")) {
    return vscode.Uri.file(raw);
  }
  return undefined;
}

function optionalUriFromFilePath(
  filePath: string | undefined
): vscode.Uri | undefined {
  if (!filePath) {
    return undefined;
  }
  return uriFromFilePath(filePath);
}

export class STDebugStartTool {
  async invoke(
    options: InvocationOptions<DebugStartParams>,
    token: vscode.CancellationToken
  ): Promise<unknown> {
    if (token.isCancellationRequested) {
      return textResult("Cancelled.");
    }
    const uri = optionalUriFromFilePath(options.input.filePath);
    if (options.input.filePath && !uri) {
      return errorResult("filePath must be an absolute path or URI when provided.");
    }
    try {
      await vscode.commands.executeCommand("trust-lsp.debug.start", uri);
      return textResult("Debug start requested.");
    } catch (error) {
      return errorResult(`Failed to start debugging: ${String(error)}`);
    }
  }
}

export class STDebugAttachTool {
  async invoke(
    _options: InvocationOptions<EmptyParams>,
    token: vscode.CancellationToken
  ): Promise<unknown> {
    if (token.isCancellationRequested) {
      return textResult("Cancelled.");
    }
    try {
      await vscode.commands.executeCommand("trust-lsp.debug.attach");
      return textResult("Debug attach requested.");
    } catch (error) {
      return errorResult(`Failed to attach debugger: ${String(error)}`);
    }
  }
}

export class STDebugReloadTool {
  async invoke(
    _options: InvocationOptions<EmptyParams>,
    token: vscode.CancellationToken
  ): Promise<unknown> {
    if (token.isCancellationRequested) {
      return textResult("Cancelled.");
    }
    try {
      const result = await vscode.commands.executeCommand<CommandResult>(
        "trust-lsp.debug.reload"
      );
      if (result && result.ok === false) {
        return errorResult(
          `Failed to update running simulation: ${result.message ?? "Update did not report success."}`
        );
      }
      return textResult("Update running simulation requested.");
    } catch (error) {
      return errorResult(`Failed to update running simulation: ${String(error)}`);
    }
  }
}

export class STDebugOpenIoPanelTool {
  async invoke(
    _options: InvocationOptions<EmptyParams>,
    token: vscode.CancellationToken
  ): Promise<unknown> {
    if (token.isCancellationRequested) {
      return textResult("Cancelled.");
    }
    try {
      await vscode.commands.executeCommand("trust-lsp.debug.openIoPanel");
      return textResult("Opened I/O panel.");
    } catch (error) {
      return errorResult(`Failed to open I/O panel: ${String(error)}`);
    }
  }
}

export class STDebugEnsureConfigurationTool {
  async invoke(
    _options: InvocationOptions<EmptyParams>,
    token: vscode.CancellationToken
  ): Promise<unknown> {
    if (token.isCancellationRequested) {
      return textResult("Cancelled.");
    }
    try {
      await vscode.commands.executeCommand("trust-lsp.debug.ensureConfiguration");
      return textResult("Ensure configuration requested.");
    } catch (error) {
      return errorResult(`Failed to ensure configuration: ${String(error)}`);
    }
  }
}
