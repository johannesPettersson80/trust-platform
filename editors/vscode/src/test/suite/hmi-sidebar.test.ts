// Specification 39, §21.3 item 8: the truST sidebar's HMI destination opens the project's start
// page in the HMI Builder, or starts New HMI Screen for a project without screens.
import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";

import { openHmi, startPage } from "../../hmiBuilder/editor/openHmi";

/** A temporary project folder with the given files, removed afterwards. */
function project(files: Record<string, string>): { folder: vscode.Uri; dispose: () => void } {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "trust-open-hmi-"));
  for (const [name, text] of Object.entries(files)) {
    const file = path.join(root, name);
    fs.mkdirSync(path.dirname(file), { recursive: true });
    fs.writeFileSync(file, text);
  }
  return { folder: vscode.Uri.file(root), dispose: () => fs.rmSync(root, { recursive: true, force: true }) };
}

const SCREEN = 'format = 1\nsize = [1280, 800]\ntheme = "dark"\nsymbols = "flat"\n';
const PAGE = 'format = 1\ntitle = "Page"\n';

suite("HMI in the truST sidebar", () => {
  test("the sidebar's HMI destination runs Open HMI, the one command for it", async () => {
    const { registerTrustHome } = require("../../trustHomeView");
    const registerProvider = vscode.window.registerWebviewViewProvider;
    const registerCommand = vscode.commands.registerCommand;
    const executeCommand = vscode.commands.executeCommand;
    const subscriptions: vscode.Disposable[] = [];
    let provider: { onMessage: (message: unknown) => Promise<void> } | undefined;
    const calls: unknown[][] = [];
    (vscode.window as any).registerWebviewViewProvider = (_id: string, value: typeof provider) => {
      provider = value;
      return new vscode.Disposable(() => {});
    };
    (vscode.commands as any).registerCommand = () => new vscode.Disposable(() => {});
    (vscode.commands as any).executeCommand = async (...args: unknown[]) => {
      calls.push(args);
    };
    try {
      registerTrustHome({ subscriptions } as vscode.ExtensionContext);
      assert.ok(provider, "the registered home provider");
      await provider.onMessage({ type: "navHmiBuilder" });
      assert.deepStrictEqual(calls, [["trust-lsp.hmiBuilder.open"]]);
    } finally {
      subscriptions.forEach((subscription) => subscription.dispose());
      (vscode.window as any).registerWebviewViewProvider = registerProvider;
      (vscode.commands as any).registerCommand = registerCommand;
      (vscode.commands as any).executeCommand = executeCommand;
    }
  });

  test("the start page is the screen's start page, else the first page, else the screen", async () => {
    const started = project({
      "hmi/screen.toml": `${SCREEN}start_page = "tanks"\n`,
      "hmi/pages/alarms.toml": PAGE,
      "hmi/pages/tanks.toml": PAGE,
    });
    const first = project({ "hmi/screen.toml": SCREEN, "hmi/pages/b.toml": PAGE, "hmi/pages/a.toml": PAGE });
    const bare = project({ "hmi/screen.toml": SCREEN });
    const none = project({ "trust-lsp.toml": "[project]\n" });
    try {
      assert.strictEqual(path.basename((await startPage(started.folder))!.fsPath), "tanks.toml");
      assert.strictEqual(path.basename((await startPage(first.folder))!.fsPath), "a.toml");
      assert.strictEqual(path.basename((await startPage(bare.folder))!.fsPath), "screen.toml");
      assert.strictEqual(await startPage(none.folder), undefined, "a project without screens has none");
    } finally {
      [started, first, bare, none].forEach((folder) => folder.dispose());
    }
  });

  test("Open HMI opens the start page in the HMI Builder, or starts New HMI Screen", async () => {
    const executeCommand = vscode.commands.executeCommand;
    const calls: unknown[][] = [];
    (vscode.commands as any).executeCommand = async (...args: unknown[]) => {
      calls.push(args);
    };
    const withScreens = project({ "hmi/screen.toml": SCREEN, "hmi/pages/overview.toml": PAGE });
    const without = project({ "trust-lsp.toml": "[project]\n" });
    try {
      await openHmi(withScreens.folder);
      assert.strictEqual(calls[0][0], "vscode.openWith");
      assert.strictEqual(path.basename((calls[0][1] as vscode.Uri).fsPath), "overview.toml");
      assert.strictEqual(calls[0][2], "trust-lsp.hmiBuilder");
      await openHmi(without.folder);
      assert.deepStrictEqual(calls[1], ["trust-lsp.hmiBuilder.newScreen", { folder: without.folder.toString() }]);
    } finally {
      (vscode.commands as any).executeCommand = executeCommand;
      withScreens.dispose();
      without.dispose();
    }
  });
});
