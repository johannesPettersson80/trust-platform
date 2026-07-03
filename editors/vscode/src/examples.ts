import * as vscode from "vscode";

import { FOCUS_MAIN_KEY } from "./newProject";
import {
  hardwareBadge,
  parseManifest,
  type ExampleEntry,
} from "./examples/model";

// S-25 — "Start from example": a browsable gallery of bundled starters with hardware badges. Choosing a
// card copies an editable working project and opens it (focus Main.st). The user never hand-edits TOML
// to start; a "No hardware" starter is immediately runnable in the Simulator.

export const START_FROM_EXAMPLE_COMMAND = "trust.examples.start";

let examplesPanel: ExamplesGalleryPanel | undefined;

export function registerExamples(context: vscode.ExtensionContext): void {
  context.subscriptions.push(
    vscode.commands.registerCommand(START_FROM_EXAMPLE_COMMAND, () =>
      openExamplesGallery(context)
    )
  );
}

async function loadManifest(
  context: vscode.ExtensionContext
): Promise<ExampleEntry[]> {
  const uri = vscode.Uri.joinPath(
    context.extensionUri,
    "media",
    "examples",
    "manifest.json"
  );
  const data = await vscode.workspace.fs.readFile(uri);
  return parseManifest(JSON.parse(Buffer.from(data).toString("utf8")));
}

async function openExamplesGallery(context: vscode.ExtensionContext): Promise<void> {
  let entries: ExampleEntry[];
  try {
    entries = await loadManifest(context);
  } catch (error) {
    void vscode.window.showErrorMessage(
      `Could not load the examples list: ${String(error)}`
    );
    return;
  }

  if (!examplesPanel) {
    examplesPanel = new ExamplesGalleryPanel(context);
  }
  examplesPanel.reveal(entries);
}

class ExamplesGalleryPanel {
  private readonly panel: vscode.WebviewPanel;
  private entries: ExampleEntry[] = [];

  constructor(private readonly context: vscode.ExtensionContext) {
    this.panel = vscode.window.createWebviewPanel(
      "trust.examples",
      "Start from example",
      vscode.ViewColumn.Beside,
      {
        enableScripts: true,
        retainContextWhenHidden: true,
        localResourceRoots: [context.extensionUri],
      }
    );
    this.panel.webview.html = this.html();
    this.panel.onDidDispose(() => {
      examplesPanel = undefined;
    });
    this.panel.webview.onDidReceiveMessage((message) => {
      void this.onMessage(message);
    });
  }

  reveal(entries: ExampleEntry[]): void {
    this.entries = entries;
    this.panel.reveal(vscode.ViewColumn.Beside);
    void this.postState();
  }

  private async onMessage(message: unknown): Promise<void> {
    if (!message || typeof message !== "object") {
      return;
    }
    const msg = message as { type?: string; id?: string };
    switch (msg.type) {
      case "ready":
        await this.postState();
        return;
      case "useExample": {
        const entry = this.entries.find((candidate) => candidate.id === msg.id);
        if (entry) {
          await copyExample(this.context, entry);
        }
        return;
      }
    }
  }

  private async postState(): Promise<void> {
    await this.panel.webview.postMessage({
      type: "state",
      examples: this.entries.map((entry) => ({
        id: entry.id,
        title: entry.title,
        description: entry.description,
        hardware: entry.hardware,
        hardwareLabel: hardwareBadge(entry.hardware),
        tags: entry.tags ?? [],
      })),
    });
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
  <title>Start from example</title>
  <link rel="stylesheet" href="${themeUri}" />
  <style>
    * { box-sizing: border-box; }
    body {
      margin: 0;
      font-family: var(--vscode-font-family);
      font-size: var(--vscode-font-size);
    }
    .shell {
      max-width: 980px;
      margin: 0 auto;
      padding: 18px;
    }
    header {
      border-bottom: 1px solid var(--trust-border);
      margin-bottom: 14px;
      padding-bottom: 12px;
    }
    .crumb {
      color: var(--trust-text-muted);
      font-size: 11px;
      font-weight: 650;
      margin-bottom: 5px;
    }
    h1 {
      color: var(--trust-text);
      font-size: 18px;
      line-height: 1.2;
      margin: 0;
    }
    .lead {
      color: var(--trust-text-muted);
      font-size: 12px;
      line-height: 1.45;
      margin: 6px 0 0;
      max-width: 660px;
    }
    .toolbar {
      align-items: center;
      display: flex;
      flex-wrap: wrap;
      gap: 8px;
      margin: 0 0 14px;
    }
    .search {
      min-width: 220px;
      flex: 1 1 260px;
    }
    input {
      background: var(--trust-input-bg);
      border: 1px solid var(--trust-input-border);
      border-radius: var(--trust-radius-sm);
      color: var(--trust-text);
      font: inherit;
      padding: 7px 9px;
      width: 100%;
    }
    .chips {
      display: flex;
      flex-wrap: wrap;
      gap: 6px;
    }
    .filter-group {
      align-items: center;
      display: flex;
      flex-wrap: wrap;
      gap: 6px;
    }
    .filter-label {
      color: var(--trust-text-muted);
      font-size: 11px;
      font-weight: 650;
      margin-right: 2px;
      text-transform: uppercase;
    }
    .chip {
      border: 1px solid var(--trust-border);
      border-radius: var(--trust-pill);
      background: transparent;
      color: var(--trust-text);
      cursor: pointer;
      font: inherit;
      font-size: 12px;
      padding: 5px 9px;
    }
    .chip[aria-pressed="true"] {
      background: var(--trust-selected-bg);
      border-color: var(--trust-accent);
    }
    .grid {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(230px, min(100%, 340px)));
      justify-content: start;
      gap: 10px;
    }
    .card {
      background: var(--trust-surface);
      border: 1px solid var(--trust-border);
      border-radius: var(--trust-radius);
      display: flex;
      flex-direction: column;
      gap: 9px;
      min-height: 178px;
      padding: 12px;
    }
    .card h2 {
      color: var(--trust-text);
      font-size: 14px;
      line-height: 1.25;
      margin: 0;
    }
    .card p {
      color: var(--trust-text-muted);
      font-size: 12px;
      line-height: 1.45;
      margin: 0;
    }
    .badges {
      display: flex;
      flex-wrap: wrap;
      gap: 5px;
    }
    .badge {
      border: 1px solid var(--trust-border);
      border-radius: var(--trust-pill);
      color: var(--trust-text-muted);
      font-size: 10px;
      font-weight: 700;
      padding: 2px 7px;
    }
    .badge.hardware {
      text-transform: uppercase;
    }
    .badge.ok {
      border-color: color-mix(in srgb, var(--trust-ok) 55%, var(--trust-border));
      color: var(--trust-ok);
    }
    .badge.requires {
      border-color: color-mix(in srgb, var(--trust-warn) 62%, var(--trust-border));
      color: var(--trust-warn);
    }
    .actions {
      margin-top: auto;
    }
    .empty {
      border: 1px solid var(--trust-border);
      border-radius: var(--trust-radius);
      color: var(--trust-text-muted);
      padding: 18px;
      text-align: center;
    }
    .empty p {
      margin: 0 0 10px;
    }
  </style>
</head>
<body>
  <main class="shell">
    <header>
      <div class="crumb">truST</div>
      <h1>Start from example</h1>
      <p class="lead">Choose a runnable starter. Examples are copied into an editable project; hardware requirements are shown before you start.</p>
    </header>
    <section class="toolbar" id="toolbar" aria-label="Example filters">
      <label class="search" id="searchWrap" aria-label="Search examples">
        <input id="search" placeholder="Search examples" />
      </label>
      <div class="chips" id="chips"></div>
    </section>
    <section class="grid" id="grid" aria-label="Examples"></section>
  </main>
  <script nonce="${nonce}">
    const vscode = acquireVsCodeApi();
    let examples = [];
    let hardwareFilter = "all";
    let tagFilter = "all";
    let search = "";

    const grid = document.getElementById("grid");
    const chips = document.getElementById("chips");
    const searchInput = document.getElementById("search");
    const searchWrap = document.getElementById("searchWrap");

    function tagsFor(example) {
      return [example.hardware === "none" ? "no-hardware" : "hardware"].concat(example.tags || []);
    }

    function allTags() {
      const filters = new Set(["all"]);
      for (const example of examples) {
        for (const tag of example.tags || []) {
          filters.add(tag);
        }
      }
      return Array.from(filters);
    }

    function hardwareFilterLabel(value) {
      if (value === "all") { return "All"; }
      if (value === "no-hardware") { return "No hardware"; }
      if (value === "hardware") { return "Needs hardware"; }
      return value;
    }

    const TAG_LABELS = {
      ads: "ADS",
      ethercat: "EtherCAT",
      gpio: "GPIO",
      hmi: "HMI",
      plcopen: "PLCopen",
      raspberrypi: "Raspberry Pi",
      twincat: "TwinCAT",
    };

    function titleCaseTag(value) {
      return value
        .split("-")
        .filter(Boolean)
        .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
        .join(" ");
    }

    function tagFilterLabel(value) {
      if (value === "all") { return "All categories"; }
      return TAG_LABELS[value] || titleCaseTag(value);
    }

    function renderFilters() {
      searchWrap.hidden = false;
      chips.innerHTML = "";
      const hardwareGroup = document.createElement("div");
      hardwareGroup.className = "filter-group";
      hardwareGroup.setAttribute("aria-label", "Hardware filter");
      const hardwareLabel = document.createElement("span");
      hardwareLabel.className = "filter-label";
      hardwareLabel.textContent = "Hardware";
      hardwareGroup.appendChild(hardwareLabel);
      for (const value of ["all", "no-hardware", "hardware"]) {
        const button = document.createElement("button");
        button.className = "chip";
        button.type = "button";
        button.textContent = hardwareFilterLabel(value);
        button.dataset.filterKind = "hardware";
        button.dataset.filterValue = value;
        button.setAttribute("aria-pressed", value === hardwareFilter ? "true" : "false");
        button.addEventListener("click", () => {
          hardwareFilter = value;
          render();
        });
        hardwareGroup.appendChild(button);
      }
      chips.appendChild(hardwareGroup);

      const categoryGroup = document.createElement("div");
      categoryGroup.className = "filter-group";
      categoryGroup.setAttribute("aria-label", "Category filter");
      const categoryLabel = document.createElement("span");
      categoryLabel.className = "filter-label";
      categoryLabel.textContent = "Category";
      categoryGroup.appendChild(categoryLabel);
      for (const value of allTags()) {
        const button = document.createElement("button");
        button.className = "chip";
        button.type = "button";
        button.textContent = tagFilterLabel(value);
        button.dataset.filterKind = "category";
        button.dataset.filterValue = value;
        button.setAttribute("aria-pressed", value === tagFilter ? "true" : "false");
        button.addEventListener("click", () => {
          tagFilter = value;
          render();
        });
        categoryGroup.appendChild(button);
      }
      chips.appendChild(categoryGroup);
    }

    function matches(example) {
      const text = (example.title + " " + example.description + " " + (example.tags || []).join(" ")).toLowerCase();
      const query = search.trim().toLowerCase();
      const tags = tagsFor(example);
      const hardwareOk = hardwareFilter === "all" || tags.includes(hardwareFilter);
      const tagOk = tagFilter === "all" || tags.includes(tagFilter);
      return hardwareOk && tagOk && (!query || text.includes(query));
    }

    function render() {
      renderFilters();
      grid.innerHTML = "";
      const visible = examples.filter(matches);
      if (!visible.length) {
        const empty = document.createElement("div");
        empty.className = "empty";
        const text = document.createElement("p");
        text.textContent = "No examples match this search and filter.";
        empty.appendChild(text);
        const reset = document.createElement("button");
        reset.className = "trust-button trust-button--secondary";
        reset.type = "button";
        reset.textContent = "Clear search and filters";
        reset.addEventListener("click", () => {
          search = "";
          hardwareFilter = "all";
          tagFilter = "all";
          searchInput.value = "";
          render();
        });
        empty.appendChild(reset);
        grid.appendChild(empty);
        return;
      }
      for (const example of visible) {
        const card = document.createElement("article");
        card.className = "card";
        card.dataset.exampleId = example.id;

        const title = document.createElement("h2");
        title.textContent = example.title;
        card.appendChild(title);

        const badges = document.createElement("div");
        badges.className = "badges";
        const hw = document.createElement("span");
        hw.className = "badge hardware " + (example.hardware === "none" ? "ok" : "requires");
        hw.textContent = example.hardwareLabel;
        badges.appendChild(hw);
        for (const tag of example.tags || []) {
          const badge = document.createElement("span");
          badge.className = "badge";
          badge.textContent = tagFilterLabel(tag);
          badges.appendChild(badge);
        }
        card.appendChild(badges);

        const desc = document.createElement("p");
        desc.textContent = example.description;
        card.appendChild(desc);

        const actions = document.createElement("div");
        actions.className = "actions";
        const use = document.createElement("button");
        use.className = "trust-button trust-button--primary";
        use.type = "button";
        use.textContent = "Use this example";
        use.addEventListener("click", () => vscode.postMessage({ type: "useExample", id: example.id }));
        actions.appendChild(use);
        card.appendChild(actions);
        grid.appendChild(card);
      }
    }

    searchInput.addEventListener("input", () => {
      search = searchInput.value || "";
      render();
    });

    window.addEventListener("message", (event) => {
      const msg = event.data;
      if (!msg || msg.type !== "state") { return; }
      examples = Array.isArray(msg.examples) ? msg.examples : [];
      render();
    });

    vscode.postMessage({ type: "ready" });
  </script>
</body>
</html>`;
  }
}

async function copyExample(
  context: vscode.ExtensionContext,
  entry: ExampleEntry
): Promise<void> {
  const automation = readAcceptanceCopyOverride(entry);
  const base = automation?.base ?? (await promptExampleDestination());
  if (!base) {
    return;
  }

  const name = automation?.name ?? (await promptExampleName(entry));
  if (!name) {
    return;
  }

  const source = vscode.Uri.joinPath(
    context.extensionUri,
    "media",
    "examples",
    entry.path
  );
  const dest = vscode.Uri.joinPath(base, name.trim());

  if (await pathExists(dest)) {
    const choice = await vscode.window.showWarningMessage(
      `${dest.fsPath} already exists. Overwrite its contents with the example?`,
      { modal: true },
      "Overwrite"
    );
    if (choice !== "Overwrite") {
      return;
    }
  }

  try {
    await vscode.workspace.fs.copy(source, dest, { overwrite: true });
  } catch (error) {
    void vscode.window.showErrorMessage(
      `Could not copy the example: ${String(error)}`
    );
    return;
  }

  // Focus Main.st after the window reloads (same mechanism as Create project).
  await context.globalState.update(
    FOCUS_MAIN_KEY,
    vscode.Uri.joinPath(dest, "src", "Main.st").fsPath
  );
  if (automation?.openFolder === false) {
    return;
  }
  await vscode.commands.executeCommand("vscode.openFolder", dest, false);
}

async function promptExampleDestination(): Promise<vscode.Uri | undefined> {
  const baseSelection = await vscode.window.showOpenDialog({
    canSelectFiles: false,
    canSelectFolders: true,
    canSelectMany: false,
    openLabel: "Select destination folder",
  });
  return baseSelection?.[0];
}

async function promptExampleName(entry: ExampleEntry): Promise<string | undefined> {
  return vscode.window.showInputBox({
    prompt: "New project folder name",
    value: entry.id,
    validateInput: (value) => validateExampleFolderName(value),
  });
}

function validateExampleFolderName(value: string): string | undefined {
  const trimmed = value.trim();
  if (!trimmed) {
    return "A folder name is required.";
  }
  if (trimmed.includes("/") || trimmed.includes("\\")) {
    return "The name must not contain path separators.";
  }
  return undefined;
}

function readAcceptanceCopyOverride(
  entry: ExampleEntry
): { base: vscode.Uri; name: string; openFolder: boolean } | undefined {
  const base = process.env.TRUST_UX_EXAMPLE_DESTINATION;
  if (!base) {
    return undefined;
  }
  const name = (process.env.TRUST_UX_EXAMPLE_NAME || entry.id).trim();
  const validation = validateExampleFolderName(name);
  if (validation) {
    throw new Error(`Invalid TRUST_UX_EXAMPLE_NAME: ${validation}`);
  }
  return {
    base: vscode.Uri.file(base),
    name,
    openFolder: process.env.TRUST_UX_EXAMPLE_OPEN_FOLDER !== "0",
  };
}

async function pathExists(uri: vscode.Uri): Promise<boolean> {
  try {
    await vscode.workspace.fs.stat(uri);
    return true;
  } catch {
    return false;
  }
}

function nonceValue(): string {
  const chars =
    "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
  let nonce = "";
  for (let i = 0; i < 32; i += 1) {
    nonce += chars.charAt(Math.floor(Math.random() * chars.length));
  }
  return nonce;
}
