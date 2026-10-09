import { disposeFixtureProject } from "./fixtureDocuments";
import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";

import type { Catalogue } from "../../hmiBuilder/editor/catalogue";
import type { VariableNode } from "../../hmiBuilder/editor/messages";
import {
  bindingDirection,
  componentForVariable,
  matchesSearch,
  PickerRow,
  pickerTree,
  refusal,
  variableLabel,
  whyNoComponent,
} from "../../hmiBuilder/editor/variables";
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
const LIMIT = value("Main.Limit", "REAL", ["number", "value"], { writable: false, writeBlock: "is a constant" });
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
    children: [P101, SPEED, LIMIT],
  },
];

const CATALOGUE: Catalogue = {
  format: 1,
  components: [
    {
      type: "text",
      category: "display",
      phase: 1,
      size: [160, 32],
      properties: { text: { accepts: ["value"], directions: ["read"] } },
    },
    {
      type: "value",
      category: "display",
      phase: 1,
      size: [240, 96],
      properties: { value: { accepts: ["value"], directions: ["read"] } },
    },
    {
      type: "status",
      category: "display",
      phase: 1,
      size: [160, 48],
      properties: { state: { accepts: ["boolean", "enumeration", "number"], directions: ["read"] } },
    },
    {
      type: "table",
      category: "display",
      phase: 1,
      size: [480, 240],
      properties: { value: { accepts: ["array", "structure"], directions: ["read"] } },
    },
  ],
};

const paths = (rows: PickerRow[]): string[] => rows.flatMap((row) => [row.node.path, ...paths(row.children)]);

suite("HMI Builder picker", () => {
  test("a binding is offered only variables of a kind it accepts", () => {
    const state = { accepts: ["boolean"], directions: ["read"] };
    assert.strictEqual(refusal("status", "state", state, RUNNING, "read"), undefined);
    assert.ok(refusal("status", "state", state, SPEED, "read"));
  });

  test("a binding that writes is offered only variables that can be written", () => {
    const entry = { accepts: ["number"], directions: ["read-write", "write"] };
    assert.strictEqual(bindingDirection(entry), "read-write");
    assert.strictEqual(bindingDirection(entry, "write"), "write");
    assert.strictEqual(refusal("numeric-input", "value", entry, SPEED, "read-write"), undefined);
    assert.ok(refusal("numeric-input", "value", entry, LIMIT, "read-write"));
    assert.strictEqual(
      refusal("setpoint", "low-limit", { accepts: ["number"], directions: ["read"] }, LIMIT, "read"),
      undefined,
    );
  });

  test("a refusal says why, in the binding checks' words", () => {
    const toggle = { accepts: ["boolean"], directions: ["read-write", "write"] };
    assert.strictEqual(
      refusal("switch", "value", toggle, SPEED, "read-write"),
      `"Main.Speed" is REAL; a switch's value needs a boolean`,
    );
    const entry = { accepts: ["number"], directions: ["read-write", "write"] };
    assert.strictEqual(
      refusal("numeric-input", "value", entry, LIMIT, "read-write"),
      `"Main.Limit" is a constant and cannot be written`,
    );
  });

  test("the picker keeps the variables it offers and the programs and blocks that hold them", () => {
    const rows = pickerTree(TREE, (node) => node.kinds.includes("boolean"));
    assert.deepStrictEqual(paths(rows), ["Main", "Main.P101", "Main.P101.Running"]);
    assert.deepStrictEqual(paths(pickerTree(TREE, () => true)), [
      "Main",
      "Main.P101",
      "Main.P101.Running",
      "Main.Speed",
      "Main.Limit",
    ]);
  });

  test("search matches the path without regard to case", () => {
    assert.ok(matchesSearch(SPEED, "speed"));
    assert.ok(matchesSearch(RUNNING, "P101.run"));
    assert.ok(!matchesSearch(SPEED, "level"));
    assert.deepStrictEqual(paths(pickerTree(TREE, (node) => matchesSearch(node, "SPEED"))), ["Main", "Main.Speed"]);
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
    assert.deepStrictEqual(paths(pickerTree([levels], () => true)), ["Main.Levels", "Main.Levels[1]"]);
  });

  test("a variable dropped on the canvas creates the matching component", () => {
    const made = (node: VariableNode): string | undefined => {
      const found = componentForVariable(CATALOGUE, node);
      return found && `${found.component.type}.${found.property}`;
    };
    assert.strictEqual(made(RUNNING), "status.state");
    assert.strictEqual(made(SPEED), "value.value");
    assert.strictEqual(made(value("Main.Name", "STRING", ["text", "value"])), "value.value");
    assert.strictEqual(made(value("Main.Mode", "Modes", ["enumeration", "value"])), "value.value");
    assert.strictEqual(
      made({ ...SPEED, path: "Main.Level", kind: "structure", type: "Reading", kinds: ["structure"] }),
      "table.value",
    );
    assert.strictEqual(
      made({ ...SPEED, path: "Main.Grid", kind: "array", type: "ARRAY[1..2] OF INT", kinds: ["array"] }),
      "table.value",
    );
    assert.strictEqual(made(P101), undefined);
    assert.strictEqual(made(TREE[0]), undefined);
  });

  test("a program or function block instance dropped on the canvas says to drag one of its members", () => {
    assert.strictEqual(
      whyNoComponent(P101),
      `"Main.P101" is a function block instance (Pump): drag one of its members onto the page`,
    );
    assert.strictEqual(whyNoComponent(TREE[0]), `"Main" is a program: drag one of its variables onto the page`);
  });

  test("an object made from a variable is labelled with the last part of its path", () => {
    assert.strictEqual(variableLabel("Main.P101.Running"), "Running");
    assert.strictEqual(variableLabel("Main.Pumps[2]"), "Pumps[2]");
    assert.strictEqual(variableLabel("Main.Pumps[1].Running"), "Running");
    assert.strictEqual(variableLabel("Main.Grid[1,0]"), "Grid[1,0]");
  });
});
