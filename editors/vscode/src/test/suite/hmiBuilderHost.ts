// Test helpers for the HMI Builder editor's host side: a webview panel that records what the
// editor posts, and the editor opened on a page file through it.

import * as assert from "assert";
import * as vscode from "vscode";

import type { LiveOperatorDeps } from "../../hmiBuilder/editor/liveOperator";
import type { DeployDeps } from "../../hmiBuilder/editor/deploy";
import { HmiBuilderEditorProvider, LiveConnection, ProblemSource } from "../../hmiBuilder/editor/hmiBuilderEditor";
import type { HostToWebview, WebviewToHost } from "../../hmiBuilder/editor/messages";

export class FakeWebview {
  options: vscode.WebviewOptions = {};
  html = "";
  readonly cspSource = "vscode-webview://hmi-builder-test";
  readonly posted: HostToWebview[] = [];
  private readonly listeners: Array<(message: WebviewToHost) => void> = [];

  asWebviewUri(resource: vscode.Uri): vscode.Uri {
    return resource;
  }
  postMessage(message: HostToWebview): Thenable<boolean> {
    this.posted.push(message);
    return Promise.resolve(true);
  }
  onDidReceiveMessage(listener: (message: WebviewToHost) => void): vscode.Disposable {
    this.listeners.push(listener);
    return new vscode.Disposable(() => this.listeners.splice(this.listeners.indexOf(listener), 1));
  }
  send(message: WebviewToHost): void {
    this.listeners.forEach((listener) => listener(message));
  }
}

/** A webview panel; like VS Code's own, its webview cannot be reached once it is disposed. */
export class FakePanel {
  active = true;
  private readonly viewStates = new vscode.EventEmitter<vscode.WebviewPanelOnDidChangeViewStateEvent>();
  readonly onDidChangeViewState = this.viewStates.event;
  setActive(active: boolean): void {
    this.active = active;
    this.viewStates.fire({ webviewPanel: this as unknown as vscode.WebviewPanel });
  }
  private readonly view = new FakeWebview();
  private disposed = false;
  private readonly disposeListeners: Array<() => void> = [];
  get webview(): FakeWebview {
    if (this.disposed) throw new Error("Webview is disposed");
    return this.view;
  }
  onDidDispose(listener: () => void): vscode.Disposable {
    this.disposeListeners.push(listener);
    return new vscode.Disposable(() => undefined);
  }
  dispose(): void {
    this.disposed = true;
    this.disposeListeners.forEach((listener) => listener());
    this.viewStates.dispose();
  }
}

/** Waits until `read` gives something other than undefined or false. */
export async function until<T>(what: string, read: () => T | undefined | false, timeoutMs = 15000): Promise<T> {
  const deadline = Date.now() + timeoutMs;
  for (;;) {
    const value = read();
    if (value !== undefined && value !== false) return value;
    if (Date.now() > deadline) assert.fail(`timed out waiting for ${what}`);
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
}

export async function languageServerReady(): Promise<void> {
  const deadline = Date.now() + 30000;
  while (!(await vscode.commands.getCommands(true)).includes("trust-lsp.hmiFileParse")) {
    if (Date.now() > deadline) assert.fail("the language server's page commands are not available");
    await new Promise((resolve) => setTimeout(resolve, 200));
  }
}

/** The user's remembered state, shared by every editor the tests open, as VS Code's is. */
class FakeMemento {
  private readonly values = new Map<string, unknown>();
  keys(): readonly string[] {
    return [...this.values.keys()];
  }
  get<T>(key: string, fallback?: T): T | undefined {
    return this.values.has(key) ? (this.values.get(key) as T) : fallback;
  }
  update(key: string, value: unknown): Thenable<void> {
    this.values.set(key, value);
    return Promise.resolve();
  }
}

export const userState = new FakeMemento();

/** The editor resolved on a page file, with a recording panel and, optionally, its problems. */
export async function openBuilder(
  file: string,
  problems?: ProblemSource,
  live?: LiveConnection,
  deploy?: DeployDeps,
  operator?: LiveOperatorDeps,
): Promise<{ document: vscode.TextDocument; webview: FakeWebview; panel: FakePanel }> {
  const document = await vscode.workspace.openTextDocument(vscode.Uri.file(file));
  const panel = new FakePanel();
  const provider = new HmiBuilderEditorProvider(
    { extensionUri: vscode.Uri.file(process.cwd()), globalState: userState } as unknown as vscode.ExtensionContext,
    { problems, live, deploy, operator },
  );
  await provider.resolveCustomTextEditor(document, panel as unknown as vscode.WebviewPanel);
  return { document, webview: panel.webview, panel };
}

/** Every message of one type the editor posted, oldest first. */
export function postedOfType<T extends HostToWebview["type"]>(
  webview: FakeWebview,
  type: T,
): Array<Extract<HostToWebview, { type: T }>> {
  return webview.posted.filter((message): message is Extract<HostToWebview, { type: T }> => message.type === type);
}
