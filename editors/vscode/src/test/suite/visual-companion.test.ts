import * as assert from "assert";
import * as vscode from "vscode";

// Specification 17 §3 and specification 25 §11.4: every visual program executes through its
// Structured Text companion `<base>.st` and runtime entry `<base>.visual.runtime.st`, which the
// language server writes (`trust-lsp.visualCompanion`) and the extension puts beside the program.
// The generators themselves are tested in Rust (crates/trust-visual); these tests prove the wiring.

const LADDER = `{
  "schemaVersion": 2,
  "networks": [
    {
      "id": "rung_1",
      "order": 0,
      "nodes": [
        { "id": "c1", "type": "contact", "contactType": "NO", "variable": "%IX0.0", "position": { "x": 100, "y": 100 } },
        { "id": "c2", "type": "contact", "contactType": "NC", "variable": "%IX0.1", "position": { "x": 200, "y": 100 } },
        { "id": "q1", "type": "coil", "coilType": "NORMAL", "variable": "%QX0.0", "position": { "x": 300, "y": 100 } }
      ],
      "edges": [],
      "layout": { "y": 100 }
    }
  ],
  "variables": [],
  "metadata": { "name": "conveyor", "description": "test" }
}
`;

interface CompanionAnswer {
  ok: boolean;
  kind?: string;
  notVisual?: boolean;
  companion?: { file: string; text: string };
  runtimeEntry?: { file: string; text: string };
  errors?: Array<{ message: string }>;
}

async function companionOf(file: string, text: string): Promise<CompanionAnswer> {
  const answer = await vscode.commands.executeCommand<CompanionAnswer>("trust-lsp.visualCompanion", {
    file,
    text,
  });
  assert.ok(answer, "the language server answers trust-lsp.visualCompanion");
  return answer;
}

async function readText(uri: vscode.Uri): Promise<string> {
  return Buffer.from(await vscode.workspace.fs.readFile(uri)).toString("utf8");
}

/** Waits until the language server offers its commands (it starts with the extension). */
async function serverReady(): Promise<void> {
  for (let attempt = 0; attempt < 150; attempt += 1) {
    if ((await vscode.commands.getCommands(true)).includes("trust-lsp.visualCompanion")) {
      return;
    }
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  assert.fail("the language server never offered trust-lsp.visualCompanion");
}

suite("Visual ST companion generation", function () {
  this.timeout(60000);
  let folder: vscode.Uri;

  suiteSetup(async () => {
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    await serverReady();
    const root = vscode.workspace.workspaceFolders?.[0]?.uri;
    assert.ok(root, "the tests need a workspace folder");
    folder = vscode.Uri.joinPath(root, "tmp", "visual-companion");
    await vscode.workspace.fs.createDirectory(folder);
  });

  suiteTeardown(async () => {
    await vscode.commands.executeCommand("workbench.action.closeAllEditors");
    await vscode.workspace.fs.delete(folder, { recursive: true, useTrash: false });
  });

  // Catches a request the extension shapes differently from the command (wrong argument names) or
  // an answer it cannot read: the companion and runtime entry come back with their file names.
  test("the language server answers both files of a Ladder program", async () => {
    const answer = await companionOf("/work/conveyor.ladder.json", LADDER);
    assert.strictEqual(answer.ok, true, JSON.stringify(answer.errors));
    assert.strictEqual(answer.kind, "ladder");
    assert.strictEqual(answer.companion?.file, "conveyor.st");
    assert.strictEqual(answer.runtimeEntry?.file, "conveyor.visual.runtime.st");
    assert.match(answer.companion?.text ?? "", /ld_io_QX0_0 := ld_io_IX0_0 AND NOT \(ld_io_IX0_1\);/);
    assert.match(answer.runtimeEntry?.text ?? "", /ld_io_QX0_0 AT %QX0\.0 : BOOL;/);
  });

  // Catches the extension deciding by itself which files are visual programs.
  test("a file that is no visual program is named so", async () => {
    const answer = await companionOf("/work/package.json", "{}");
    assert.strictEqual(answer.ok, false);
    assert.strictEqual(answer.notVisual, true);
  });

  // Catches a sync command that no longer writes the files beside the program, or writes text the
  // language server did not answer.
  test("Sync Structured Text writes the companion and runtime entry beside the program", async () => {
    const program = vscode.Uri.joinPath(folder, "conveyor.ladder.json");
    await vscode.workspace.fs.writeFile(program, Buffer.from(LADDER, "utf8"));
    await vscode.commands.executeCommand("trust-lsp.visual.syncCompanionSt", program);
    const expected = await companionOf(program.fsPath, LADDER);
    assert.strictEqual(await readText(vscode.Uri.joinPath(folder, "conveyor.st")), expected.companion?.text);
    assert.strictEqual(
      await readText(vscode.Uri.joinPath(folder, "conveyor.visual.runtime.st")),
      expected.runtimeEntry?.text
    );
  });
});
