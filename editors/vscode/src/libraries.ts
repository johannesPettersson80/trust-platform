import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import type { LanguageClient } from "vscode-languageclient/node";

import {
  collectSymbolSummaries,
  classifyGitPin,
  parseDependencyEntries,
  parsePackageVersion,
  posixPath,
  removeDependency,
  upsertDependency,
  type DependencySpec,
  type LibraryDependencyEntry,
  type SymbolSummary,
} from "./librariesModel";

export const OPEN_LIBRARIES_COMMAND = "trust-lsp.libraries.open";
export const ADD_LIBRARY_COMMAND = "trust-lsp.libraries.add";
export const REMOVE_LIBRARY_COMMAND = "trust-lsp.libraries.remove";
export const UPDATE_LIBRARY_COMMAND = "trust-lsp.libraries.update";

type CuratedLibraryId = "oscat" | "plcopen_motion";

interface CuratedLibrary {
  readonly id: CuratedLibraryId;
  readonly label: string;
  readonly dependencyName: string;
  readonly folderName: string;
  readonly packagedPath: readonly string[];
  readonly purpose: string;
}

interface AddLibraryArgs {
  readonly source?:
    | { readonly kind: "curated"; readonly id: CuratedLibraryId }
    | { readonly kind: "local"; readonly path: string; readonly createManifest?: boolean }
    | {
        readonly kind: "git";
        readonly name: string;
        readonly url: string;
        readonly pin?: { readonly rev?: string; readonly tag?: string; readonly branch?: string };
        readonly version?: string;
      };
  readonly simulateCancel?: boolean;
}

interface LibraryViewEntry {
  readonly name: string;
  readonly label: string;
  readonly version?: string;
  readonly source: "bundled" | "local" | "git";
  readonly path?: string;
  readonly status: "resolved" | "resolving" | "failed";
  readonly detail: string;
  readonly symbols: SymbolSummary[];
  readonly updateAvailable?: {
    readonly current?: string;
    readonly next: string;
    readonly curatedId: CuratedLibraryId;
  };
  readonly canFixPath?: boolean;
}

interface ProjectInfoLibrary {
  readonly name?: string;
  readonly version?: string;
  readonly path?: string;
}

const CURATED: readonly CuratedLibrary[] = [
  {
    id: "oscat",
    label: "OSCAT",
    dependencyName: "OSCAT",
    folderName: "oscat",
    packagedPath: ["media", "libraries", "oscat"],
    purpose: "General-purpose utility blocks and functions",
  },
  {
    id: "plcopen_motion",
    label: "PLCopen Motion",
    dependencyName: "PLCopenMotionSingleAxis",
    folderName: "plcopen_motion",
    packagedPath: ["media", "libraries", "plcopen_motion"],
    purpose: "Single-axis motion control blocks",
  },
];

const panelByRoot = new Map<string, LibrariesPanel>();

export function registerLibraries(
  context: vscode.ExtensionContext,
  options: { readonly getClient: () => LanguageClient | undefined }
): void {
  const refresh = () => {
    for (const panel of panelByRoot.values()) {
      void panel.refresh();
    }
  };
  const watcher = vscode.workspace.createFileSystemWatcher("**/trust-lsp.toml");
  context.subscriptions.push(watcher);
  context.subscriptions.push(watcher.onDidChange(refresh));
  context.subscriptions.push(watcher.onDidCreate(refresh));
  context.subscriptions.push(watcher.onDidDelete(refresh));

  context.subscriptions.push(
    vscode.commands.registerCommand(OPEN_LIBRARIES_COMMAND, async () => {
      const root = await requireProjectRoot();
      if (!root) {
        return;
      }
      const key = root.fsPath;
      let panel = panelByRoot.get(key);
      if (!panel) {
        panel = new LibrariesPanel(context, root, options.getClient);
        panelByRoot.set(key, panel);
      }
      panel.reveal();
      await panel.refresh();
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand(ADD_LIBRARY_COMMAND, async (args?: AddLibraryArgs) => {
      const root = await requireProjectRoot();
      if (!root) {
        return false;
      }
      const result = await addLibrary(context, root, args);
      if (result.ok || !result.message) {
        await refreshPanel(root);
      }
      return result.ok;
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand(REMOVE_LIBRARY_COMMAND, async (name?: string) => {
      const root = await requireProjectRoot();
      if (!root || !name) {
        return false;
      }
      const ok = await removeLibrary(root, name, true);
      await refreshPanel(root);
      return ok;
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand(UPDATE_LIBRARY_COMMAND, async (id?: CuratedLibraryId) => {
      const root = await requireProjectRoot();
      if (!root || !id) {
        return false;
      }
      const ok = await updateCuratedLibrary(context, root, id, true);
      await refreshPanel(root);
      return ok;
    })
  );
}

export async function projectLibrariesSnapshot(
  context: vscode.ExtensionContext,
  root: vscode.Uri,
  getClient?: () => LanguageClient | undefined
): Promise<LibraryViewEntry[]> {
  const manifest = await readProjectManifest(root);
  const dependencies = parseDependencyEntries(manifest);
  const projectInfo = await readProjectInfo(root, getClient);
  const byName = new Map(projectInfo.map((lib) => [String(lib.name ?? ""), lib]));
  const entries: LibraryViewEntry[] = [];
  for (const dep of dependencies) {
    const info = byName.get(dep.name);
    entries.push(await viewEntryForDependency(context, root, dep, info));
  }
  entries.sort((a, b) => a.label.localeCompare(b.label));
  return entries;
}

class LibrariesPanel {
  private readonly panel: vscode.WebviewPanel;
  private lastError = "";

  constructor(
    private readonly context: vscode.ExtensionContext,
    private readonly root: vscode.Uri,
    private readonly getClient: () => LanguageClient | undefined
  ) {
    this.panel = vscode.window.createWebviewPanel(
      "trust.libraries",
      "Libraries",
      vscode.ViewColumn.Beside,
      {
        enableScripts: true,
        retainContextWhenHidden: true,
        localResourceRoots: [context.extensionUri],
      }
    );
    this.panel.webview.html = this.html();
    this.panel.onDidDispose(() => {
      panelByRoot.delete(root.fsPath);
    });
    this.panel.webview.onDidReceiveMessage((message) => {
      void this.onMessage(message);
    });
  }

  reveal(): void {
    this.panel.reveal(vscode.ViewColumn.Beside);
  }

  async refresh(error = ""): Promise<void> {
    this.lastError = error;
    const libraries = await projectLibrariesSnapshot(this.context, this.root, this.getClient);
    await this.panel.webview.postMessage({
      type: "state",
      libraries,
      error: this.lastError,
      curated: CURATED,
    });
  }

  private async onMessage(message: unknown): Promise<void> {
    if (!message || typeof message !== "object") {
      return;
    }
    const msg = message as {
      type?: string;
      id?: CuratedLibraryId;
      name?: string;
      path?: string;
      createManifest?: boolean;
      gitName?: string;
      gitUrl?: string;
      gitPin?: string;
      gitVersion?: string;
      text?: string;
    };
    switch (msg.type) {
      case "ready":
        await this.refresh("");
        return;
      case "add":
        await vscode.commands.executeCommand(ADD_LIBRARY_COMMAND);
        return;
      case "addCurated":
        if (msg.id) {
          await vscode.commands.executeCommand(ADD_LIBRARY_COMMAND, {
            source: { kind: "curated", id: msg.id },
          });
        }
        return;
      case "addLocal":
        if (msg.path) {
          await vscode.commands.executeCommand(ADD_LIBRARY_COMMAND, {
            source: { kind: "local", path: msg.path, createManifest: Boolean(msg.createManifest) },
          });
        }
        return;
      case "addGit":
        if (msg.gitName && msg.gitUrl) {
          await vscode.commands.executeCommand(ADD_LIBRARY_COMMAND, {
            source: {
              kind: "git",
              name: msg.gitName,
              url: msg.gitUrl,
              pin: msg.gitPin ? classifyGitPin(msg.gitPin) : undefined,
              version: msg.gitVersion || undefined,
            },
          });
        }
        return;
      case "remove":
        if (msg.name) {
          await vscode.commands.executeCommand(REMOVE_LIBRARY_COMMAND, msg.name);
        }
        return;
      case "update":
        if (msg.id) {
          await vscode.commands.executeCommand(UPDATE_LIBRARY_COMMAND, msg.id);
        }
        return;
      case "openSource":
        if (msg.path) {
          await openFirstSourceFile(msg.path);
        }
        return;
      case "fixPath":
        if (msg.name) {
          await fixLibraryPath(this.root, msg.name);
          await this.refresh("");
        }
        return;
      case "copySymbol":
        if (msg.text) {
          await vscode.env.clipboard.writeText(msg.text);
          void vscode.window.showInformationMessage("Library snippet copied.");
        }
        return;
      case "insertDeclaration":
        if (msg.text) {
          await insertDeclarationText(msg.text);
        }
        return;
    }
  }

  private html(): string {
    const nonce = nonceValue();
    const themeUri = this.panel.webview.asWebviewUri(
      vscode.Uri.joinPath(this.context.extensionUri, "src", "webview", "theme.css")
    );
    const csp = `default-src 'none'; style-src ${this.panel.webview.cspSource} 'unsafe-inline'; script-src 'nonce-${nonce}';`;
    return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <meta http-equiv="Content-Security-Policy" content="${csp}" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>Libraries</title>
  <link rel="stylesheet" href="${themeUri}" />
  <style>
    * { box-sizing: border-box; }
    body {
      margin: 0;
      font-family: var(--vscode-font-family);
      font-size: var(--vscode-font-size);
    }
    .shell {
      max-width: 760px;
      margin: 0 auto;
      padding: 16px;
    }
    header {
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 12px;
      border-bottom: 1px solid var(--trust-border);
      padding-bottom: 12px;
      margin-bottom: 12px;
    }
    .crumb {
      color: var(--trust-text-muted);
      font-size: 11px;
      font-weight: 650;
      margin-bottom: 4px;
    }
    h1 {
      color: var(--trust-text);
      font-size: 17px;
      line-height: 1.2;
      margin: 0;
    }
    .section-title {
      color: var(--trust-text-muted);
      font-size: 10px;
      font-weight: 750;
      letter-spacing: 0.6px;
      text-transform: uppercase;
      margin: 18px 0 8px;
    }
    .empty,
    .error {
      border: 1px solid var(--trust-border);
      border-radius: var(--trust-radius);
      background: var(--trust-surface);
      color: var(--trust-text-muted);
      padding: 16px;
    }
    .error {
      border-color: color-mix(in srgb, var(--trust-danger) 55%, var(--trust-border));
      color: var(--trust-text);
    }
    .choices {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(190px, 1fr));
      gap: 8px;
      margin-top: 10px;
    }
    .choice {
      align-items: flex-start;
      display: flex;
      flex-direction: column;
      gap: 3px;
      text-align: left;
      min-height: 60px;
    }
    .choice strong,
    .row-title strong { color: var(--trust-text); }
    .choice span,
    .row-detail,
    .symbol-count { color: var(--trust-text-muted); font-size: 12px; }
    .list { display: flex; flex-direction: column; gap: 8px; }
    details {
      border: 1px solid var(--trust-border);
      border-radius: var(--trust-radius);
      background: var(--trust-surface);
      overflow: hidden;
    }
    details[open] { background: var(--trust-surface-raised); }
    summary {
      cursor: pointer;
      display: grid;
      grid-template-columns: 1fr auto;
      gap: 10px;
      list-style: none;
      padding: 10px 12px;
    }
    summary::-webkit-details-marker { display: none; }
    .row-title {
      align-items: center;
      display: flex;
      flex-wrap: wrap;
      gap: 7px;
      min-width: 0;
    }
    .badge {
      border: 1px solid var(--trust-border);
      border-radius: 999px;
      color: var(--trust-text-muted);
      font-size: 10px;
      font-weight: 700;
      padding: 2px 7px;
      text-transform: uppercase;
    }
    .badge.ok { border-color: color-mix(in srgb, var(--trust-ok) 55%, var(--trust-border)); color: var(--trust-ok); }
    .badge.warn { border-color: color-mix(in srgb, var(--trust-warn) 55%, var(--trust-border)); color: var(--trust-warn); }
    .badge.error { border-color: color-mix(in srgb, var(--trust-danger) 55%, var(--trust-border)); color: var(--trust-danger); }
    .row-actions { display: flex; flex-wrap: wrap; gap: 6px; justify-content: flex-end; }
    .contents {
      border-top: 1px solid var(--trust-border);
      padding: 8px 12px 12px;
    }
    .symbol-tools {
      align-items: center;
      display: grid;
      gap: 8px;
      grid-template-columns: minmax(0, 1fr) auto;
      margin: 8px 0;
    }
    .symbol-search {
      width: 100%;
    }
    .symbol-browser {
      display: grid;
      gap: 8px;
    }
    .symbol-list {
      border: 1px solid var(--trust-border);
      border-radius: var(--trust-radius);
      overflow: hidden;
    }
    .symbol-row {
      align-items: center;
      background: transparent;
      border: 0;
      border-bottom: 1px solid var(--trust-border);
      color: var(--trust-text);
      cursor: pointer;
      display: grid;
      gap: 8px;
      grid-template-columns: minmax(0, 1fr) auto;
      padding: 7px 8px;
      text-align: left;
      width: 100%;
    }
    .symbol-row:last-child {
      border-bottom: 0;
    }
    .symbol-row:hover,
    .symbol-row.is-selected {
      background: var(--trust-surface);
    }
    .symbol-main {
      min-width: 0;
    }
    .symbol-name,
    .symbol-declaration {
      border: 1px solid var(--trust-border);
      border-radius: var(--trust-radius-sm);
      color: var(--trust-text);
      font-family: var(--vscode-editor-font-family);
      font-size: 11px;
      padding: 3px 6px;
    }
    .symbol-declaration {
      display: block;
      overflow-wrap: anywhere;
    }
    .symbol-kind {
      color: var(--trust-text-muted);
      font-size: 10px;
      font-weight: 750;
      letter-spacing: 0.5px;
      text-transform: uppercase;
    }
    .symbol-detail {
      background: var(--trust-surface);
      border: 1px solid var(--trust-border);
      border-radius: var(--trust-radius);
      display: grid;
      gap: 8px;
      padding: 10px;
    }
    .symbol-detail-actions,
    .pager {
      align-items: center;
      display: flex;
      flex-wrap: wrap;
      gap: 6px;
    }
    .pager {
      color: var(--trust-text-muted);
      font-size: 12px;
      justify-content: space-between;
    }
    .symbol-empty {
      border: 1px dashed var(--trust-border);
      border-radius: var(--trust-radius);
      color: var(--trust-text-muted);
      padding: 10px;
    }
    .add-panel {
      border: 1px solid var(--trust-border);
      border-radius: var(--trust-radius);
      background: var(--trust-surface);
      margin-bottom: 12px;
      padding: 12px;
    }
    .add-grid {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(240px, 1fr));
      gap: 12px;
    }
    label {
      color: var(--trust-text-muted);
      display: flex;
      flex-direction: column;
      font-size: 11px;
      font-weight: 650;
      gap: 4px;
      margin-top: 8px;
    }
    label.inline {
      align-items: center;
      flex-direction: row;
      font-weight: 500;
    }
    input {
      background: var(--vscode-input-background);
      border: 1px solid var(--vscode-input-border, var(--trust-border));
      color: var(--vscode-input-foreground);
      font-family: var(--vscode-font-family);
      font-size: var(--vscode-font-size);
      min-width: 0;
      padding: 6px 7px;
    }
  </style>
</head>
<body>
  <main class="shell">
    <header>
      <div>
        <div class="crumb">Project / Libraries</div>
        <h1>Libraries</h1>
      </div>
      <button class="trust-button trust-button--primary" id="add">Add library...</button>
    </header>
    <div id="error"></div>
    <div class="add-panel" id="addPanel" hidden>
      <div class="add-grid">
        <section>
          <div class="section-title">Local folder</div>
          <label>Folder path<input id="localPath" data-library-field="localPath" placeholder="/path/to/library" /></label>
          <label class="inline"><input id="createManifest" type="checkbox" /> Create a truST library manifest if missing</label>
          <button class="trust-button" id="addLocal">Add local folder</button>
        </section>
        <section>
          <div class="section-title">Git</div>
          <label>Dependency name<input id="gitName" data-library-field="gitName" placeholder="VendorLib" /></label>
          <label>Repository URL<input id="gitUrl" data-library-field="gitUrl" placeholder="file:///path/to/library" /></label>
          <label>Tag, branch, or commit<input id="gitPin" data-library-field="gitPin" placeholder="v1.0.0" /></label>
          <label>Version (optional)<input id="gitVersion" data-library-field="gitVersion" placeholder="1.0.0" /></label>
          <button class="trust-button" id="addGit">Add from Git</button>
        </section>
      </div>
    </div>
    <div class="section-title">Added</div>
    <div id="libraries"></div>
    <div class="section-title">Curated</div>
    <div class="choices" id="curated"></div>
  </main>
  <script nonce="${nonce}">
    const vscode = acquireVsCodeApi();
    const librariesEl = document.getElementById('libraries');
    const curatedEl = document.getElementById('curated');
    const errorEl = document.getElementById('error');
    const addPanel = document.getElementById('addPanel');
    document.getElementById('add').addEventListener('click', () => { addPanel.hidden = !addPanel.hidden; });
    document.getElementById('addLocal').addEventListener('click', () => vscode.postMessage({
      type: 'addLocal',
      path: document.getElementById('localPath').value,
      createManifest: document.getElementById('createManifest').checked
    }));
    document.getElementById('addGit').addEventListener('click', () => vscode.postMessage({
      type: 'addGit',
      gitName: document.getElementById('gitName').value,
      gitUrl: document.getElementById('gitUrl').value,
      gitPin: document.getElementById('gitPin').value,
      gitVersion: document.getElementById('gitVersion').value
    }));
    function esc(value) {
      return String(value ?? '').replace(/[&<>"']/g, (ch) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[ch]));
    }
    const PAGE_SIZE = 24;
    const symbolBrowserState = Object.create(null);
    const openLibraries = Object.create(null);
    let lastState = { libraries: [], curated: [] };
    function symbolKey(symbol) {
      return symbol.kind + ':' + symbol.name;
    }
    function kindLabel(kind) {
      if (kind === 'function_block') return 'Function block';
      if (kind === 'function') return 'Function';
      return 'Type';
    }
    function shortKind(kind) {
      if (kind === 'function_block') return 'FB';
      if (kind === 'function') return 'FUN';
      return 'TYPE';
    }
    function browserState(lib) {
      symbolBrowserState[lib.name] = symbolBrowserState[lib.name] || { query: '', page: 0, selected: '' };
      return symbolBrowserState[lib.name];
    }
    function symbolText(symbol) {
      return [symbol.name, symbol.kind, symbol.declaration, symbol.file].join(' ').toLowerCase();
    }
    function instanceName(name) {
      const base = String(name || 'libraryBlock').replace(/[^A-Za-z0-9_]/g, '');
      return (base.charAt(0).toLowerCase() + base.slice(1)) || 'libraryBlock';
    }
    function declarationText(symbol) {
      if (symbol.kind === 'function_block') return instanceName(symbol.name) + ' : ' + symbol.name + ';';
      if (symbol.kind === 'type') return 'value : ' + symbol.name + ';';
      return symbol.name + '(...)';
    }
    function findSymbol(libraryName, key) {
      const lib = (lastState.libraries || []).find((candidate) => candidate.name === libraryName);
      return (lib?.symbols || []).find((symbol) => symbolKey(symbol) === key);
    }
    function renderSymbolRow(lib, symbol, selectedKey) {
      const key = symbolKey(symbol);
      const selected = key === selectedKey ? ' is-selected' : '';
      return '<button class="symbol-row' + selected + '" data-symbol-select="' + esc(lib.name) + '" data-symbol-key="' + esc(key) + '"><div class="symbol-main"><span class="symbol-name">' + esc(symbol.name) + '</span><div class="row-detail">' + esc(symbol.declaration || kindLabel(symbol.kind)) + '</div></div><span class="symbol-kind">' + esc(shortKind(symbol.kind)) + '</span></button>';
    }
    function renderSymbolDetail(lib, symbol) {
      if (!symbol) {
        return '<div class="symbol-detail"><div class="row-detail">Search or select a symbol to see its declaration and insert it into Structured Text.</div></div>';
      }
      const key = symbolKey(symbol);
      return '<div class="symbol-detail"><div><div class="symbol-kind">' + esc(kindLabel(symbol.kind)) + '</div><strong>' + esc(symbol.name) + '</strong></div><code class="symbol-declaration">' + esc(symbol.declaration || declarationText(symbol)) + '</code><div class="row-detail">Insert snippet: <code>' + esc(declarationText(symbol)) + '</code></div><div class="symbol-detail-actions"><button class="trust-button trust-button--primary" data-symbol-insert="' + esc(lib.name) + '" data-symbol-key="' + esc(key) + '">Insert declaration</button><button class="trust-button" data-symbol-copy="' + esc(lib.name) + '" data-symbol-key="' + esc(key) + '">Copy snippet</button></div></div>';
    }
    function renderSymbolBrowser(lib) {
      const symbols = lib.symbols || [];
      if (!symbols.length) {
        return '<div class="symbol-empty">No symbols found in this library.</div>';
      }
      const state = browserState(lib);
      const query = state.query.trim().toLowerCase();
      const filtered = query ? symbols.filter((symbol) => symbolText(symbol).includes(query)) : symbols;
      const pageCount = Math.max(1, Math.ceil(filtered.length / PAGE_SIZE));
      state.page = Math.min(Math.max(0, state.page), pageCount - 1);
      const start = state.page * PAGE_SIZE;
      const pageSymbols = filtered.slice(start, start + PAGE_SIZE);
      if (!filtered.some((symbol) => symbolKey(symbol) === state.selected)) {
        state.selected = pageSymbols[0] ? symbolKey(pageSymbols[0]) : '';
      }
      const selected = filtered.find((symbol) => symbolKey(symbol) === state.selected);
      const rows = pageSymbols.length
        ? pageSymbols.map((symbol) => renderSymbolRow(lib, symbol, state.selected)).join('')
        : '<div class="symbol-empty">No matching symbols. Clear the search or try another name.</div>';
      return '<div class="symbol-browser"><div class="symbol-tools"><input class="symbol-search" data-symbol-search="' + esc(lib.name) + '" placeholder="Search all ' + esc(countLabel(symbols.length, 'symbol')) + '" value="' + esc(state.query) + '" /><span class="symbol-count">' + esc(countLabel(filtered.length, 'match', 'matches')) + '</span></div><div class="symbol-list">' + rows + '</div><div class="pager"><button class="trust-button" data-symbol-page="' + esc(lib.name) + '" data-page="-1"' + (state.page <= 0 ? ' disabled' : '') + '>Previous</button><span>Page ' + esc(state.page + 1) + ' of ' + esc(pageCount) + '</span><button class="trust-button" data-symbol-page="' + esc(lib.name) + '" data-page="1"' + (state.page >= pageCount - 1 ? ' disabled' : '') + '>Next</button></div>' + renderSymbolDetail(lib, selected) + '</div>';
    }
    function countLabel(count, singular, plural) {
      return String(count) + ' ' + (count === 1 ? singular : (plural || singular + 's'));
    }
    function libraryRow(lib) {
      const statusClass = lib.status === 'resolved' ? 'ok' : lib.status === 'failed' ? 'error' : 'warn';
      const update = lib.updateAvailable ? '<button class="trust-button" data-update="' + esc(lib.updateAvailable.curatedId) + '">Update to ' + esc(lib.updateAvailable.next) + '</button>' : '';
      const open = lib.path ? '<button class="trust-button" data-open="' + esc(lib.path) + '">View source</button>' : '';
      const fix = lib.canFixPath ? '<button class="trust-button" data-fix="' + esc(lib.name) + '">Fix path</button>' : '';
      return '<details data-library-row="' + esc(lib.name) + '"' + (openLibraries[lib.name] ? ' open' : '') + '><summary><div><div class="row-title"><strong>' + esc(lib.label) + '</strong><span class="badge">' + esc(lib.source) + '</span><span class="badge ' + statusClass + '">' + esc(lib.status) + '</span>' + (lib.version ? '<span class="badge">' + esc(lib.version) + '</span>' : '') + '</div><div class="row-detail">' + esc(lib.detail) + '</div></div><div class="row-actions">' + update + fix + open + '<button class="trust-button trust-button--danger" data-remove="' + esc(lib.name) + '">Remove</button></div></summary><div class="contents"><div class="symbol-count">' + esc(countLabel((lib.symbols || []).length, 'symbol')) + '</div>' + renderSymbolBrowser(lib) + '</div></details>';
    }
    function render(state) {
      lastState = state;
      errorEl.innerHTML = state.error ? '<div class="error" role="alert">' + esc(state.error) + ' <button class="trust-button" id="retryAdd">Fix and retry</button></div>' : '';
      const libraries = state.libraries || [];
      librariesEl.innerHTML = libraries.length ? '<div class="list">' + libraries.map(libraryRow).join('') + '</div>' : '<div class="empty">No libraries added. Add OSCAT, PLCopen Motion, or your own.</div>';
      curatedEl.innerHTML = (state.curated || []).map((item) => '<button class="trust-button choice" data-curated="' + esc(item.id) + '"><strong>' + esc(item.label) + '</strong><span>' + esc(item.purpose) + '</span></button>').join('');
      document.querySelectorAll('[data-curated]').forEach((el) => el.addEventListener('click', () => vscode.postMessage({ type: 'addCurated', id: el.getAttribute('data-curated') })));
      document.querySelectorAll('[data-remove]').forEach((el) => el.addEventListener('click', () => vscode.postMessage({ type: 'remove', name: el.getAttribute('data-remove') })));
      document.querySelectorAll('[data-open]').forEach((el) => el.addEventListener('click', () => vscode.postMessage({ type: 'openSource', path: el.getAttribute('data-open') })));
      document.querySelectorAll('[data-update]').forEach((el) => el.addEventListener('click', () => vscode.postMessage({ type: 'update', id: el.getAttribute('data-update') })));
      document.querySelectorAll('[data-fix]').forEach((el) => el.addEventListener('click', () => vscode.postMessage({ type: 'fixPath', name: el.getAttribute('data-fix') })));
      document.querySelectorAll('[data-library-row]').forEach((el) => el.addEventListener('toggle', () => {
        openLibraries[el.getAttribute('data-library-row')] = el.open;
      }));
      document.querySelectorAll('[data-symbol-search]').forEach((el) => el.addEventListener('input', () => {
        const name = el.getAttribute('data-symbol-search');
        symbolBrowserState[name] = symbolBrowserState[name] || { query: '', page: 0, selected: '' };
        symbolBrowserState[name].query = el.value;
        symbolBrowserState[name].page = 0;
        symbolBrowserState[name].selected = '';
        render(lastState);
      }));
      document.querySelectorAll('[data-symbol-page]').forEach((el) => el.addEventListener('click', () => {
        const name = el.getAttribute('data-symbol-page');
        symbolBrowserState[name] = symbolBrowserState[name] || { query: '', page: 0, selected: '' };
        symbolBrowserState[name].page += Number(el.getAttribute('data-page') || 0);
        render(lastState);
      }));
      document.querySelectorAll('[data-symbol-select]').forEach((el) => el.addEventListener('click', () => {
        const name = el.getAttribute('data-symbol-select');
        symbolBrowserState[name] = symbolBrowserState[name] || { query: '', page: 0, selected: '' };
        symbolBrowserState[name].selected = el.getAttribute('data-symbol-key') || '';
        render(lastState);
      }));
      document.querySelectorAll('[data-symbol-insert]').forEach((el) => el.addEventListener('click', () => {
        const symbol = findSymbol(el.getAttribute('data-symbol-insert'), el.getAttribute('data-symbol-key'));
        if (symbol) vscode.postMessage({ type: 'insertDeclaration', text: declarationText(symbol) });
      }));
      document.querySelectorAll('[data-symbol-copy]').forEach((el) => el.addEventListener('click', () => {
        const symbol = findSymbol(el.getAttribute('data-symbol-copy'), el.getAttribute('data-symbol-key'));
        if (symbol) vscode.postMessage({ type: 'copySymbol', text: declarationText(symbol) });
      }));
      document.getElementById('retryAdd')?.addEventListener('click', () => { addPanel.hidden = false; });
    }
    window.addEventListener('message', (event) => {
      if (event.data && event.data.type === 'state') render(event.data);
    });
    vscode.postMessage({ type: 'ready' });
  </script>
</body>
</html>`;
  }
}

async function addLibrary(
  context: vscode.ExtensionContext,
  root: vscode.Uri,
  args?: AddLibraryArgs
): Promise<{ ok: boolean; message?: string }> {
  if (args?.simulateCancel) {
    return { ok: false, message: "No library added." };
  }
  const source = args?.source ?? (await promptLibrarySource());
  if (!source) {
    return { ok: false, message: "No library added." };
  }
  try {
    switch (source.kind) {
      case "curated":
        await addCuratedLibrary(context, root, source.id);
        return { ok: true };
      case "local":
        await addLocalLibrary(root, source.path, source.createManifest);
        return { ok: true };
      case "git":
        await addGitLibrary(root, source);
        return { ok: true };
    }
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    const panel = panelByRoot.get(root.fsPath);
    await panel?.refresh(message);
    void vscode.window.showWarningMessage(message);
    return { ok: false, message };
  }
}

async function promptLibrarySource(): Promise<AddLibraryArgs["source"] | undefined> {
  const pick = await vscode.window.showQuickPick(
    [
      ...CURATED.map((library) => ({
        label: library.label,
        description: library.purpose,
        source: { kind: "curated" as const, id: library.id },
      })),
      {
        label: "From local folder...",
        description: "Add a library already on this computer",
        source: { kind: "local" as const },
      },
      {
        label: "From Git...",
        description: "Add a pinned library from a Git repository",
        source: { kind: "git" as const },
      },
    ],
    { title: "Add library", placeHolder: "Choose a library source" }
  );
  if (!pick) {
    return undefined;
  }
  if (pick.source.kind === "curated") {
    return pick.source;
  }
  if (pick.source.kind === "local") {
    const selected = await vscode.window.showOpenDialog({
      canSelectFiles: false,
      canSelectFolders: true,
      canSelectMany: false,
      openLabel: "Select Library Folder",
    });
    const folder = selected?.[0];
    return folder ? { kind: "local", path: folder.fsPath } : undefined;
  }

  const url = await vscode.window.showInputBox({
    title: "Add library from Git",
    prompt: "Repository URL",
    placeHolder: "file:///path/to/library",
  });
  if (!url) {
    return undefined;
  }
  const name = await vscode.window.showInputBox({
    title: "Add library from Git",
    prompt: "Dependency name",
    placeHolder: "VendorLib",
  });
  if (!name) {
    return undefined;
  }
  const pin = await vscode.window.showInputBox({
    title: "Add library from Git",
    prompt: "Tag, branch, or commit",
    placeHolder: "v1.0.0",
  });
  return {
    kind: "git",
    name,
    url,
    pin: pin ? classifyGitPin(pin) : undefined,
  };
}

async function addCuratedLibrary(
  context: vscode.ExtensionContext,
  root: vscode.Uri,
  id: CuratedLibraryId
): Promise<void> {
  const library = curatedById(id);
  const source = packagedLibraryPath(context, library);
  if (!fs.existsSync(source)) {
    throw new Error(`Bundled ${library.label} files are missing from the extension.`);
  }
  const destination = path.join(root.fsPath, "libraries", library.folderName);
  await copyDirectory(source, destination);
  const version = readPackageVersion(destination);
  await writeDependency(root, library.dependencyName, {
    path: posixPath(path.relative(root.fsPath, destination)),
    version,
  });
  void vscode.window.showInformationMessage(`${library.label} added to this project.`);
}

async function addLocalLibrary(
  root: vscode.Uri,
  folderPath: string,
  createManifest?: boolean
): Promise<void> {
  if (!fs.existsSync(folderPath) || !fs.statSync(folderPath).isDirectory()) {
    throw new Error("No library added — the selected folder does not exist.");
  }
  const manifest = path.join(folderPath, "trust-lsp.toml");
  if (!fs.existsSync(manifest)) {
    const create =
      createManifest ??
      ((await vscode.window.showWarningMessage(
        "This folder is not a truST library. Create a library manifest?",
        "Create",
        "Cancel"
      )) === "Create");
    if (!create) {
      throw new Error("No library added — the folder needs a truST library manifest.");
    }
    fs.writeFileSync(manifest, `[package]\nversion = "0.1.0"\n\n[project]\ninclude_paths = ["src"]\n`);
    fs.mkdirSync(path.join(folderPath, "src"), { recursive: true });
  }
  const name = path.basename(folderPath).replace(/[^A-Za-z0-9_]/g, "_") || "LocalLibrary";
  const version = readPackageVersion(folderPath);
  await writeDependency(root, name, {
    path: posixPath(path.relative(root.fsPath, folderPath)),
    version,
  });
  void vscode.window.showInformationMessage(`${name} added to this project.`);
}

async function addGitLibrary(
  root: vscode.Uri,
  source: Extract<NonNullable<AddLibraryArgs["source"]>, { kind: "git" }>
): Promise<void> {
  const spec: DependencySpec = { git: source.url, version: source.version, ...source.pin };
  await writeDependency(root, source.name, spec);
  void vscode.window.showInformationMessage(`${source.name} added from Git.`);
}

async function removeLibrary(
  root: vscode.Uri,
  name: string,
  confirm: boolean
): Promise<boolean> {
  if (confirm) {
    const selected = await vscode.window.showWarningMessage(
      `Remove ${name} from this project?`,
      "Remove",
      "Cancel"
    );
    if (selected !== "Remove") {
      return false;
    }
  }
  const manifest = await readProjectManifest(root);
  await writeProjectManifest(root, removeDependency(manifest, name));
  void vscode.window.showInformationMessage(`${name} removed from this project.`);
  return true;
}

async function updateCuratedLibrary(
  context: vscode.ExtensionContext,
  root: vscode.Uri,
  id: CuratedLibraryId,
  confirm: boolean
): Promise<boolean> {
  const library = curatedById(id);
  const destination = path.join(root.fsPath, "libraries", library.folderName);
  const next = readPackageVersion(packagedLibraryPath(context, library)) ?? "unknown";
  const current = readPackageVersion(destination) ?? "unknown";
  if (confirm) {
    const selected = await vscode.window.showInformationMessage(
      `Update ${library.label} from ${current} to ${next}?`,
      "Update",
      "Cancel"
    );
    if (selected !== "Update") {
      void vscode.window.showInformationMessage(`${library.label} update skipped.`);
      return false;
    }
  }
  await addCuratedLibrary(context, root, id);
  return true;
}

async function writeDependency(
  root: vscode.Uri,
  name: string,
  spec: DependencySpec
): Promise<void> {
  const manifest = await readProjectManifest(root);
  await writeProjectManifest(root, upsertDependency(manifest, name, spec));
}

async function readProjectManifest(root: vscode.Uri): Promise<string> {
  const uri = vscode.Uri.joinPath(root, "trust-lsp.toml");
  const bytes = await vscode.workspace.fs.readFile(uri);
  return Buffer.from(bytes).toString("utf8");
}

async function writeProjectManifest(root: vscode.Uri, content: string): Promise<void> {
  const uri = vscode.Uri.joinPath(root, "trust-lsp.toml");
  await vscode.workspace.fs.writeFile(uri, Buffer.from(content));
}

async function requireProjectRoot(): Promise<vscode.Uri | undefined> {
  const root = vscode.workspace.workspaceFolders?.[0]?.uri;
  if (!root) {
    void vscode.window.showWarningMessage("Open a truST project before managing libraries.");
    return undefined;
  }
  try {
    await vscode.workspace.fs.stat(vscode.Uri.joinPath(root, "trust-lsp.toml"));
    return root;
  } catch {
    void vscode.window.showWarningMessage("Open a truST project before managing libraries.");
    return undefined;
  }
}

function countLabel(count: number, singular: string, plural = `${singular}s`): string {
  return `${count} ${count === 1 ? singular : plural}`;
}

async function viewEntryForDependency(
  context: vscode.ExtensionContext,
  root: vscode.Uri,
  dependency: LibraryDependencyEntry,
  projectInfo?: ProjectInfoLibrary
): Promise<LibraryViewEntry> {
  const curated = CURATED.find((candidate) => candidate.dependencyName === dependency.name);
  const label = curated?.label ?? dependency.name;
  const source = curated ? "bundled" : dependency.source;
  const manifestPath = resolveDependencyPath(root.fsPath, dependency);
  const resolvedPath = curated ? manifestPath : projectInfo?.path ?? manifestPath;
  const exists = resolvedPath ? fs.existsSync(resolvedPath) : false;
  const status = exists ? "resolved" : dependency.source === "git" ? "resolving" : "failed";
  const packageVersion = resolvedPath ? readPackageVersion(resolvedPath) : undefined;
  const version = curated
    ? packageVersion ?? dependency.version ?? projectInfo?.version
    : projectInfo?.version ?? packageVersion ?? dependency.version;
  const symbols = exists && resolvedPath ? await readSymbols(resolvedPath) : [];
  const nextVersion = curated ? readPackageVersion(packagedLibraryPath(context, curated)) : undefined;
  const updateAvailable =
    curated && nextVersion && version && version !== nextVersion
      ? { current: version, next: nextVersion, curatedId: curated.id }
      : undefined;
  return {
    name: dependency.name,
    label,
    version,
    source,
    path: resolvedPath,
    status,
    detail: status === "failed"
      ? "Path is missing. Fix or remove this library."
      : status === "resolving"
        ? "Waiting for the dependency resolver to pin this Git library."
        : `${countLabel(symbols.length, "symbol")} available`,
    symbols,
    updateAvailable,
    canFixPath: status === "failed" && Boolean(dependency.path),
  };
}

async function fixLibraryPath(root: vscode.Uri, name: string): Promise<boolean> {
  const selected = await vscode.window.showOpenDialog({
    canSelectFiles: false,
    canSelectFolders: true,
    canSelectMany: false,
    openLabel: "Select Library Folder",
    title: `Fix ${name} library path`,
  });
  const folder = selected?.[0];
  if (!folder) {
    return false;
  }
  const manifestPath = path.join(folder.fsPath, "trust-lsp.toml");
  if (!fs.existsSync(manifestPath)) {
    const message = "No library path changed — the selected folder needs a truST library manifest.";
    const panel = panelByRoot.get(root.fsPath);
    await panel?.refresh(message);
    void vscode.window.showWarningMessage(message);
    return false;
  }
  const version = readPackageVersion(folder.fsPath);
  await writeDependency(root, name, {
    path: posixPath(path.relative(root.fsPath, folder.fsPath)),
    version,
  });
  void vscode.window.showInformationMessage(`${name} path updated.`);
  return true;
}

async function readProjectInfo(
  root: vscode.Uri,
  getClient?: () => LanguageClient | undefined
): Promise<ProjectInfoLibrary[]> {
  const client = getClient?.();
  if (!client) {
    return [];
  }
  try {
    const response = await client.sendRequest<{ projects?: Array<{ libraries?: ProjectInfoLibrary[] }> }>(
      "workspace/executeCommand",
      {
        command: "trust-lsp.projectInfo",
        arguments: [{ root_uri: root.toString() }],
      }
    );
    return response.projects?.[0]?.libraries ?? [];
  } catch {
    return [];
  }
}

async function readSymbols(rootPath: string): Promise<SymbolSummary[]> {
  const files = await findStFiles(rootPath);
  return collectSymbolSummaries(
    files.map((file) => ({
      file,
      text: fs.readFileSync(file, "utf8"),
    }))
  );
}

async function findStFiles(rootPath: string): Promise<string[]> {
  const out: string[] = [];
  const stack = [rootPath];
  while (stack.length > 0) {
    const current = stack.pop()!;
    for (const entry of fs.readdirSync(current, { withFileTypes: true })) {
      const full = path.join(current, entry.name);
      if (entry.isDirectory()) {
        stack.push(full);
      } else if (/\.(st|pou)$/i.test(entry.name)) {
        out.push(full);
      }
    }
  }
  return out.sort();
}

function resolveDependencyPath(root: string, dependency: LibraryDependencyEntry): string | undefined {
  if (dependency.path) {
    return path.isAbsolute(dependency.path) ? dependency.path : path.join(root, dependency.path);
  }
  return undefined;
}

function readPackageVersion(rootPath: string): string | undefined {
  const manifest = path.join(rootPath, "trust-lsp.toml");
  if (!fs.existsSync(manifest)) {
    return undefined;
  }
  return parsePackageVersion(fs.readFileSync(manifest, "utf8"));
}

function packagedLibraryPath(context: vscode.ExtensionContext, library: CuratedLibrary): string {
  return path.join(context.extensionPath, ...library.packagedPath);
}

function curatedById(id: CuratedLibraryId): CuratedLibrary {
  const library = CURATED.find((candidate) => candidate.id === id);
  if (!library) {
    throw new Error(`Unknown curated library: ${id}`);
  }
  return library;
}

async function copyDirectory(source: string, destination: string): Promise<void> {
  fs.rmSync(destination, { recursive: true, force: true });
  fs.mkdirSync(destination, { recursive: true });
  fs.cpSync(source, destination, { recursive: true });
}

async function openFirstSourceFile(rootPath: string): Promise<void> {
  if (!fs.existsSync(rootPath)) {
    return;
  }
  const file = (await findStFiles(rootPath))[0];
  if (!file) {
    return;
  }
  const doc = await vscode.workspace.openTextDocument(vscode.Uri.file(file));
  await vscode.window.showTextDocument(doc, { preview: false });
  void vscode.window.showInformationMessage("Library source opened from the vendored project copy.");
}

async function insertDeclarationText(text: string): Promise<void> {
  const editor = structuredTextEditorForInsertion();
  if (!editor) {
    await vscode.env.clipboard.writeText(text);
    void vscode.window.showWarningMessage(
      "Open a Structured Text file to insert the declaration. The snippet was copied instead."
    );
    return;
  }
  const declarationEdit = declarationInsertion(editor.document, text);
  if (declarationEdit) {
    await editor.edit((edit) => edit.insert(declarationEdit.position, declarationEdit.text));
    const line = declarationEdit.position.line;
    editor.revealRange(new vscode.Range(line, 0, line, 0), vscode.TextEditorRevealType.InCenterIfOutsideViewport);
    void vscode.window.showInformationMessage("Library declaration inserted.");
    return;
  }
  await editor.insertSnippet(new vscode.SnippetString(text));
  void vscode.window.showInformationMessage("Library declaration inserted.");
}

function structuredTextEditorForInsertion(): vscode.TextEditor | undefined {
  const active = vscode.window.activeTextEditor;
  if (active && isStructuredTextDocument(active.document)) {
    return active;
  }
  return vscode.window.visibleTextEditors.find((editor) => isStructuredTextDocument(editor.document)) ?? active;
}

function isStructuredTextDocument(document: vscode.TextDocument): boolean {
  return /\.(st|pou)$/i.test(document.uri.fsPath);
}

function declarationInsertion(
  document: vscode.TextDocument,
  text: string
): { position: vscode.Position; text: string } | undefined {
  const trimmed = text.trim();
  if (!/^[A-Za-z_][A-Za-z0-9_]*\s*:/.test(trimmed)) {
    return undefined;
  }
  for (let line = 0; line < document.lineCount; line += 1) {
    if (!/^\s*VAR\b/i.test(document.lineAt(line).text)) {
      continue;
    }
    const nextLine = line + 1 < document.lineCount ? document.lineAt(line + 1).text : "";
    const indent = nextLine.match(/^\s+(?=\S)/)?.[0] ?? "    ";
    return {
      position: new vscode.Position(line + 1, 0),
      text: `${indent}${trimmed}\n`,
    };
  }
  return undefined;
}

async function refreshPanel(root: vscode.Uri): Promise<void> {
  await panelByRoot.get(root.fsPath)?.refresh("");
}

function nonceValue(): string {
  const chars = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
  let out = "";
  for (let i = 0; i < 32; i += 1) {
    out += chars[Math.floor(Math.random() * chars.length)];
  }
  return out;
}
