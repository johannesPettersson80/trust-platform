import * as assert from "assert";
import * as vscode from "vscode";

import { validateLadderProgramValue } from "../../visual/ladderToSt";

// Specification 25, §11.4: creating a Ladder, Blockly, Statechart, or SFC program opens the new
// file in its visual editor, in the active editor group and with focus. With
// trust.visual.openStructuredTextCompanionOnCreate on, the generated Structured Text companion
// opens beside it without taking focus; with it off, no companion opens. While
// trust.visual.openVisualEditorsAutomatically is on, a visual file opened as text also opens in
// its visual editor.

interface VisualProgramKind {
  command: string;
  suffix: string;
  /** The custom editor that package.json contributes for the suffix. */
  viewType: string;
  /** Arguments that make the command create `<name><suffix>` in `folder` without a name prompt. */
  args(folder: vscode.Uri, name: string): object;
}

const LADDER: VisualProgramKind = {
  command: "trust-lsp.ladder.new",
  suffix: ".ladder.json",
  viewType: "trust-lsp.ladder.editor",
  args: (folder, name) => ({ targetUri: folder, ladderName: name }),
};
const BLOCKLY: VisualProgramKind = {
  command: "trust-lsp.blockly.new",
  suffix: ".blockly.json",
  viewType: "trust-lsp.blockly.editor",
  args: (folder, name) => ({ targetUri: folder, blocklyName: name }),
};
const STATECHART: VisualProgramKind = {
  command: "trust-lsp.statechart.new",
  suffix: ".statechart.json",
  viewType: "trust-lsp.statechartEditor",
  // This command takes the new file itself as its target.
  args: (folder, name) => ({ targetUri: vscode.Uri.joinPath(folder, `${name}.statechart.json`) }),
};
const SFC: VisualProgramKind = {
  command: "trust-lsp.sfc.new",
  suffix: ".sfc.json",
  viewType: "trust-lsp.sfc.editor",
  args: (folder, name) => ({ targetUri: folder, sfcName: name }),
};
const KINDS = [LADDER, BLOCKLY, STATECHART, SFC];

/** A tab as "<view type> <file>" for a custom editor and as "text <file>" for a text editor. */
function describeTab(tab: vscode.Tab): string {
  const input = tab.input;
  if (input instanceof vscode.TabInputCustom) {
    return `${input.viewType} ${input.uri.fsPath}`;
  }
  if (input instanceof vscode.TabInputText) {
    return `text ${input.uri.fsPath}`;
  }
  return `other ${tab.label}`;
}

function tabsIn(groups: readonly vscode.TabGroup[]): string[] {
  return groups.flatMap((group) => group.tabs.map(describeTab));
}

function allTabs(): string[] {
  return tabsIn(vscode.window.tabGroups.all);
}

function tabsBesideActiveGroup(): string[] {
  return tabsIn(vscode.window.tabGroups.all.filter((group) => !group.isActive));
}

function activeTab(): string | undefined {
  const tab = vscode.window.tabGroups.activeTabGroup.activeTab;
  return tab ? describeTab(tab) : undefined;
}

/** Gives the editor tabs up to ten seconds to reach the state that `reached` checks. */
async function waitForTabs(reached: () => boolean): Promise<void> {
  for (let attempt = 0; attempt < 100 && !reached(); attempt += 1) {
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
}

suite("New visual programs open in their visual editor", function () {
  this.timeout(60000);
  let folder: vscode.Uri;
  let folders = 0;

  suiteSetup(async () => {
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    await vscode.commands.executeCommand("workbench.action.closeAllEditors");
  });

  setup(async () => {
    const root = vscode.workspace.workspaceFolders?.[0]?.uri;
    assert.ok(root, "the tests need a workspace folder");
    folders += 1;
    folder = vscode.Uri.joinPath(root, "tmp", `visual-new-program-${folders}`);
    await vscode.workspace.fs.createDirectory(folder);
  });

  teardown(async () => {
    await vscode.commands.executeCommand("workbench.action.closeAllEditors");
    await vscode.workspace.fs.delete(folder, { recursive: true, useTrash: false });
  });

  /** Creates `name` with the kind's command; returns the tabs of the program and of its companion. */
  async function createProgram(
    kind: VisualProgramKind,
    name: string
  ): Promise<{ visual: string; companion: string }> {
    await vscode.commands.executeCommand(kind.command, kind.args(folder, name));
    return {
      visual: `${kind.viewType} ${vscode.Uri.joinPath(folder, `${name}${kind.suffix}`).fsPath}`,
      companion: `text ${vscode.Uri.joinPath(folder, `${name}.st`).fsPath}`,
    };
  }

  // Catches the raw JSON text, or the Structured Text companion, in front of the new program.
  async function opensInItsVisualEditor(kind: VisualProgramKind): Promise<void> {
    const { visual } = await createProgram(kind, "conveyor");
    await waitForTabs(() => activeTab() === visual);
    assert.strictEqual(activeTab(), visual, `open tabs: ${allTabs().join(" | ")}`);
  }

  // Catches the companion opening in the visual editor's group, or beside it with the focus.
  async function opensCompanionBeside(kind: VisualProgramKind): Promise<void> {
    const { visual, companion } = await createProgram(kind, "conveyor");
    await waitForTabs(
      () => activeTab() === visual && tabsBesideActiveGroup().includes(companion)
    );
    assert.strictEqual(
      activeTab(),
      visual,
      `the visual editor keeps the focus; open tabs: ${allTabs().join(" | ")}`
    );
    assert.ok(
      tabsBesideActiveGroup().includes(companion),
      `the companion opens in another group; open tabs: ${allTabs().join(" | ")}`
    );
  }

  // Catches a new Ladder program its own editor cannot read: the template once wrote `connections` and no `layout`, which the
  // schema-v2 check refuses, so the program opened as "Invalid Ladder Program" and got no companion.
  test("a new Ladder program passes the schema-v2 check", async () => {
    await createProgram(LADDER, "conveyor");
    const file = vscode.Uri.joinPath(folder, "conveyor.ladder.json");
    const text = Buffer.from(await vscode.workspace.fs.readFile(file)).toString("utf8");
    assert.doesNotThrow(() => validateLadderProgramValue(JSON.parse(text)));
  });
  test("Ladder opens the new program in its visual editor", () => opensInItsVisualEditor(LADDER));
  test("Ladder opens the companion beside without focus", () => opensCompanionBeside(LADDER));
  test("Blockly opens the new program in its visual editor", () => opensInItsVisualEditor(BLOCKLY));
  test("Blockly opens the companion beside without focus", () => opensCompanionBeside(BLOCKLY));
  test("Statechart opens the new program in its visual editor", () => opensInItsVisualEditor(STATECHART));
  test("Statechart opens the companion beside without focus", () => opensCompanionBeside(STATECHART));
  test("SFC opens the new program in its visual editor", () => opensInItsVisualEditor(SFC));
  test("SFC opens the companion beside without focus", () => opensCompanionBeside(SFC));

  // Catches a companion, or the raw JSON text, opening although the companion setting is off.
  test("with the companion setting off, only the visual editor opens", async () => {
    const settings = vscode.workspace.getConfiguration("trust");
    await settings.update(
      "visual.openStructuredTextCompanionOnCreate",
      false,
      vscode.ConfigurationTarget.Workspace
    );
    try {
      for (const [index, kind] of KINDS.entries()) {
        const { visual } = await createProgram(kind, `mixer_${index}`);
        await waitForTabs(() => activeTab() === visual);
        assert.deepStrictEqual(allTabs(), [visual], kind.command);
        await vscode.commands.executeCommand("workbench.action.closeAllEditors");
      }
    } finally {
      await settings.update(
        "visual.openStructuredTextCompanionOnCreate",
        undefined,
        vscode.ConfigurationTarget.Workspace
      );
    }
  });

  // Catches a visual file left in the text editor because its suffix is missing from the
  // automatic reopening, as `.sfc.json` was.
  test("a visual file opened as text also opens in its visual editor", async () => {
    for (const kind of KINDS) {
      const file = vscode.Uri.joinPath(folder, `as_text${kind.suffix}`);
      // The content does not decide which editor opens.
      await vscode.workspace.fs.writeFile(file, Buffer.from("{}\n", "utf8"));
      await vscode.window.showTextDocument(await vscode.workspace.openTextDocument(file));
      const visual = `${kind.viewType} ${file.fsPath}`;
      await waitForTabs(() => allTabs().includes(visual));
      assert.ok(allTabs().includes(visual), `${kind.suffix} opened as text; open tabs: ${allTabs().join(" | ")}`);
      await vscode.commands.executeCommand("workbench.action.closeAllEditors");
    }
  });
});
