import { disposeFixtureProject } from "./fixtureDocuments";
import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";

import type { VariableNode } from "../../hmiBuilder/editor/messages";
import { PickerRow, searched, treeRows } from "../../hmiBuilder/editor/variables";
import { languageServerReady, openBuilder, postedOfType, until } from "./hmiBuilderHost";

// Specification 39, §7.1: the editor gives the webview the program's variables for the picker,
// reads them again when a source file is saved, and says why while the program does not compile.

const MAIN = `PROGRAM Main
VAR
    Speed : REAL;
    Running : BOOL;
END_VAR
END_PROGRAM
`;

const fixtureProjects: string[] = [];

function project(source: string): { root: string; page: string } {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "trust-hmi-variables-"));
  fixtureProjects.push(root);
  fs.mkdirSync(path.join(root, "hmi", "pages"), { recursive: true });
  fs.mkdirSync(path.join(root, "src"));
  fs.writeFileSync(path.join(root, "src", "main.st"), source);
  const page = path.join(root, "hmi", "pages", "overview.toml");
  fs.writeFileSync(page, 'format = 1\ntitle = "Overview"\n');
  return { root, page };
}

function find(nodes: VariableNode[], wanted: string): VariableNode | undefined {
  for (const node of nodes) {
    if (node.path === wanted) return node;
    const inside = find([...(node.children ?? []), ...(node.array ? [node.array.element] : [])], wanted);
    if (inside) return inside;
  }
  return undefined;
}

suite("HMI Builder editor: variables", function () {
  this.timeout(60000);

  suiteSetup(languageServerReady);

  teardown(async () => {
    for (const root of fixtureProjects.splice(0)) await disposeFixtureProject(root);
  });

  test("on ready the editor sends the program's variables, with their kinds and whether they can be written", async () => {
    const { page } = project(MAIN);
    const { webview, panel } = await openBuilder(page);
    webview.send({ type: "ready" });
    const message = await until("the variables", () => postedOfType(webview, "variables")[0]);
    assert.strictEqual(message.problem, undefined, message.problem);
    const speed = find(message.variables, "Main.Speed");
    assert.ok(speed, JSON.stringify(message.variables));
    assert.strictEqual(speed.type, "REAL");
    assert.ok(speed.kinds.includes("number"), JSON.stringify(speed));
    assert.strictEqual(speed.writable, true);
    panel.dispose();
  });

  test("saving a source file of the project sends the variables again", async () => {
    const { root, page } = project(MAIN);
    const { webview, panel } = await openBuilder(page);
    webview.send({ type: "ready" });
    await until("the first variables", () => postedOfType(webview, "variables")[0]);
    const source = await vscode.workspace.openTextDocument(vscode.Uri.file(path.join(root, "src", "main.st")));
    const edit = new vscode.WorkspaceEdit();
    edit.insert(source.uri, new vscode.Position(3, 0), "    Level : REAL;\n");
    assert.ok(await vscode.workspace.applyEdit(edit));
    assert.ok(await source.save());
    const again = await until("the variables after the save", () =>
      postedOfType(webview, "variables").find((message) => find(message.variables, "Main.Level") !== undefined),
    );
    assert.ok(find(again.variables, "Main.Speed"));
    panel.dispose();
  });

  test("an incomplete declaration does not hide the other declared variables", async () => {
    const { page } = project("PROGRAM Main\nVAR\n    Speed : REAL;\n    Broken : ;\nEND_VAR\nEND_PROGRAM\n");
    const { webview, panel } = await openBuilder(page);
    webview.send({ type: "ready" });
    const message = await until("the answer", () => postedOfType(webview, "variables")[0]);
    assert.strictEqual(find(message.variables, "Main.Speed")?.type, "REAL", JSON.stringify(message));
    const broken = find(message.variables, "Main.Broken");
    if (broken) assert.strictEqual(broken.writable, false, "an unresolved type is not a valid write target");
    panel.dispose();
  });
});

const value = (path: string, type: string, kinds: string[], extra: Partial<VariableNode> = {}): VariableNode => ({
  name: path.slice(path.lastIndexOf(".") + 1),
  path,
  kind: "value",
  type,
  kinds,
  writable: true,
  ...extra,
});

const SPEED = value("Main.Speed", "REAL", ["number", "value"]);
const RUNNING = value("Main.P101.Running", "BOOL", ["boolean", "value"], {
  writable: false,
  writeBlock: "is an output of Pump",
});
const P101: VariableNode = {
  name: "P101",
  path: "Main.P101",
  kind: "function-block",
  type: "Pump",
  kinds: [],
  writable: true,
  children: [RUNNING],
};
const TREE: VariableNode[] = [
  {
    name: "Main",
    path: "Main",
    kind: "program",
    type: "Main",
    kinds: [],
    writable: true,
    children: [P101, SPEED],
  },
];

const paths = (rows: PickerRow[]): string[] => rows.flatMap((row) => [row.node.path, ...paths(row.children)]);

// What a binding is offered, the binding a variable gives and what a variable dropped on the canvas
// creates are the language server's rules (`trust-lsp.hmiVariablePicker`), proven in
// crates/trust-lsp/src/handlers/commands/hmi_builder/variables_tests.rs. Listing the tree and
// searching it are the picker's own.
suite("HMI Builder picker", () => {
  test("search matches the path without regard to case, keeping the variables that hold the match", () => {
    assert.deepStrictEqual(paths(searched(treeRows(TREE), "speed")), ["Main", "Main.Speed"]);
    assert.deepStrictEqual(paths(searched(treeRows(TREE), "P101.run")), ["Main", "Main.P101", "Main.P101.Running"]);
    assert.deepStrictEqual(paths(searched(treeRows(TREE), "level")), []);
    assert.deepStrictEqual(paths(searched(treeRows(TREE), "  ")), [
      "Main",
      "Main.P101",
      "Main.P101.Running",
      "Main.Speed",
    ]);
  });

  test("search keeps a row that only holds the offered variables from matching itself", () => {
    // The language server's rows for a boolean binding: `Main` and `Main.P101` only hold `Running`.
    const offered: PickerRow[] = [
      {
        node: TREE[0],
        offered: false,
        children: [{ node: P101, offered: false, children: [{ node: RUNNING, offered: true, children: [] }] }],
      },
    ];
    assert.deepStrictEqual(paths(searched(offered, "P101")), ["Main", "Main.P101", "Main.P101.Running"]);
    assert.deepStrictEqual(paths(searched(offered, "Main")), ["Main", "Main.P101", "Main.P101.Running"]);
    assert.deepStrictEqual(paths(searched(offered, "Pump")), []);
  });

  test("an array lists its element under it", () => {
    const element = value("Main.Levels[1]", "REAL", ["number", "value"], { name: "[1]" });
    const levels: VariableNode = {
      name: "Levels",
      path: "Main.Levels",
      kind: "array",
      type: "ARRAY[1..4] OF REAL",
      kinds: ["array"],
      writable: true,
      array: { dimensions: [[1, 4]], element },
    };
    assert.deepStrictEqual(paths(treeRows([levels])), ["Main.Levels", "Main.Levels[1]"]);
  });
});
