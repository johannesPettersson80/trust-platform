import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";

import type { HmiObjectModel, HmiPageModel } from "../../hmiBuilder/drawing/model";
import { linkedUpdate } from "../../hmiBuilder/editor/pidLink";
import { languageServerReady, openBuilder, postedOfType, until } from "./hmiBuilderHost";

// Specification 39, §18 phase 4 item 2: a page and its P&ID share their process objects: a
// change of one's type, tag or bindings, its coming or going, and the pipes between them reach the
// other view; places, displays and texts stay each view's own.

const PID_SIZE: [number, number] = [2000, 1000];
const SCREEN: [number, number] = [1000, 500];

function object(id: string, type: string, at: [number, number], extra: Partial<HmiObjectModel> = {}): HmiObjectModel {
  return { id, type, at, size: [128, 108], rotation: 0, bind: {}, ...extra };
}

const PID: HmiPageModel = {
  title: "Tank farm",
  object: [
    object("tk101", "tank", [200, 200], {
      tag: "TK101",
      bind: { level: { path: "Main.TK101.Level", direction: "read" } },
    }),
    object("p101", "pump", [600, 400], {
      tag: "P101",
      bind: { running: { path: "Main.P101.Running", direction: "read" } },
    }),
    object("xv101", "valve", [1000, 400], {
      tag: "XV101",
      bind: { open: { path: "Main.XV101.Open", direction: "read" } },
    }),
    object("pt101", "pressure-sensor", [800, 200], { tag: "PT101", size: [176, 56], attach: "pipe:0" }),
    object("lt101", "level-sensor", [100, 100], { tag: "LT101", size: [176, 56], attach: "tk101" }),
    object("note", "text", [100, 900], { text: "Tank farm, line 1" }),
  ],
  pipe: [
    { from: "tk101.outlet", to: "p101.inlet", waypoints: [[400, 450]] },
    { from: "p101.outlet", to: "xv101.inlet" },
  ],
};

const PAGE: HmiPageModel = {
  title: "Tank farm",
  pid: "tank-farm",
  object: [
    object("tk101", "tank", [100, 100], {
      tag: "TK101",
      bind: { level: { path: "Main.TK101.Level", direction: "read" } },
    }),
    object("p101", "pump", [300, 200], {
      tag: "P101",
      bind: { running: { path: "Main.P101.Running", direction: "read" } },
    }),
    object("xv101", "valve", [500, 200], {
      tag: "XV101",
      bind: { open: { path: "Main.XV101.Open", direction: "read" } },
    }),
    object("pt101", "pressure-sensor", [400, 100], { tag: "PT101", size: [176, 56], attach: "pipe:0" }),
    object("lt101", "level-sensor", [50, 50], { tag: "LT101", size: [176, 56], attach: "tk101" }),
    object("level", "value", [700, 50], {
      label: "Level",
      bind: { value: { path: "Main.TK101.Level", direction: "read" } },
    }),
  ],
  pipe: [
    { from: "tk101.outlet", to: "p101.inlet" },
    { from: "p101.outlet", to: "xv101.inlet", waypoints: [[420, 260]] },
  ],
};

const find = (page: HmiPageModel | undefined, id: string): HmiObjectModel | undefined =>
  page?.object.find((item) => item.id === id);

suite("HMI Builder: a page and its P&ID", () => {
  test("a shared object's tag, type and bindings changed in the P&ID change on the page, its place there kept", () => {
    const edited: HmiPageModel = {
      ...PID,
      object: PID.object.map((item) =>
        item.id === "p101"
          ? {
              ...item,
              type: "dosing-pump",
              tag: "P102",
              bind: { running: { path: "Main.P102.Running", direction: "read" } },
              at: [900, 900],
            }
          : item,
      ),
    };
    const page = linkedUpdate(edited, PID_SIZE, PAGE, SCREEN);
    assert.ok(page, "the page changes");
    const pump = find(page, "p101");
    assert.deepStrictEqual(
      [pump?.type, pump?.tag, pump?.bind.running?.path],
      ["dosing-pump", "P102", "Main.P102.Running"],
    );
    assert.deepStrictEqual(pump?.at, [300, 200], "its place on the page is the page's own");
  });

  test("an object added to one view comes into the other scaled to its size, one deleted leaves it; displays and texts stay in their view", () => {
    const added: HmiPageModel = {
      ...PID,
      object: [
        ...PID.object.filter((item) => item.id !== "xv101"),
        object("fv102", "control-valve", [1400, 600], { tag: "FV102" }),
      ],
    };
    const page = linkedUpdate(added, PID_SIZE, PAGE, SCREEN);
    assert.deepStrictEqual(find(page, "fv102")?.at, [700, 300], "scaled from the P&ID to the screen");
    assert.strictEqual(find(page, "xv101"), undefined, "deleted on the P&ID, gone from the page");
    assert.ok(find(page, "level"), "the page's own value stays");
    assert.strictEqual(find(page, "note"), undefined, "the P&ID's text stays on the P&ID");
    const back = linkedUpdate(PAGE, SCREEN, PID, PID_SIZE);
    assert.strictEqual(back, undefined, "the views agree: nothing to change");
    const pid = linkedUpdate(
      { ...PAGE, object: [...PAGE.object, object("e101", "heat-exchanger", [600, 400], { tag: "E101" })] },
      SCREEN,
      PID,
      PID_SIZE,
    );
    assert.deepStrictEqual(find(pid, "e101")?.at, [1200, 800], "added on the page, it comes into the P&ID");
    assert.strictEqual(find(pid, "level"), undefined, "a display does not");
  });

  test("the pipes between shared objects are one in both views, each keeping its own bends", () => {
    const rerouted: HmiPageModel = {
      ...PID,
      pipe: [
        { from: "tk101.outlet", to: "p101.inlet", waypoints: [[400, 450]] },
        { from: "p101.outlet", to: "xv101.inlet", style: "signal" },
      ],
    };
    const page = linkedUpdate(rerouted, PID_SIZE, PAGE, SCREEN);
    assert.deepStrictEqual(page?.pipe, [
      { from: "tk101.outlet", to: "p101.inlet" },
      { from: "p101.outlet", to: "xv101.inlet", style: "signal", waypoints: [[420, 260]] },
    ]);
  });
});

const SCREEN_TOML = 'format = 1\nsize = [1280, 720]\ntheme = "dark"\nsymbols = "flat"\n';
const PID_TOML =
  'format = 1\ntitle = "Tank farm"\n\n[[object]]\nid = "tk101"\ntag = "TK101"\ntype = "tank"\nat = [200, 200]\nsize = [128, 172]\n\n[[object]]\nid = "p101"\ntag = "P101"\ntype = "pump"\nat = [600, 400]\nsize = [128, 108]\n\n[object.bind]\nrunning = "Main.P101.Running"\n\n[[pipe]]\nfrom = "tk101.outlet"\nto = "p101.inlet"\n';
const PAGE_TOML =
  'format = 1\ntitle = "Tank farm"\npid = "tank-farm"\n\n[[object]]\nid = "tk101"\ntag = "TK101"\ntype = "tank"\nat = [100, 100]\nsize = [128, 172]\n\n[[object]]\nid = "p101"\ntag = "P101"\ntype = "pump"\nat = [300, 200]\nsize = [128, 108]\n\n[object.bind]\nrunning = "Main.P101.Running"\n\n[[object]]\nid = "speed"\ntype = "value"\nat = [600, 100]\nsize = [160, 48]\n\n[[pipe]]\nfrom = "tk101.outlet"\nto = "p101.inlet"\n';

function linkedProject(): string {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "trust-hmi-pid-"));
  for (const folder of ["src", "hmi/pages", "hmi/pid"]) fs.mkdirSync(path.join(root, folder), { recursive: true });
  fs.writeFileSync(path.join(root, "src", "main.st"), "PROGRAM Main\nEND_PROGRAM\n");
  fs.writeFileSync(path.join(root, "hmi", "screen.toml"), SCREEN_TOML);
  fs.writeFileSync(path.join(root, "hmi", "pid", "tank-farm.toml"), PID_TOML);
  fs.writeFileSync(path.join(root, "hmi", "pages", "tank-farm.toml"), PAGE_TOML);
  return root;
}

suite("HMI Builder: the P&ID view in the editor", function () {
  this.timeout(60000);
  suiteSetup(async () => languageServerReady());
  teardown(async () => vscode.commands.executeCommand("workbench.action.closeAllEditors"));

  test("a P&ID opens in the P&ID view, and an edit there reaches the page that shares its objects", async () => {
    const root = linkedProject();
    const { webview, panel } = await openBuilder(path.join(root, "hmi", "pid", "tank-farm.toml"));
    webview.send({ type: "ready" });
    await until("the P&ID view", () => postedOfType(webview, "view").find((message) => message.view === "pid"));
    const loaded = await until("the P&ID", () => postedOfType(webview, "page")[0]);
    webview.send({
      type: "edit",
      model: {
        ...loaded.model,
        object: loaded.model.object.map((item) => (item.id === "p101" ? { ...item, tag: "P102" } : item)),
      },
    });
    const page = path.join(root, "hmi", "pages", "tank-farm.toml");
    await until("the page with the new tag", () => /tag = "P102"/.test(fs.readFileSync(page, "utf8")), 20000);
    assert.match(fs.readFileSync(page, "utf8"), /id = "speed"/, "the page's own value stays");
    panel.dispose();
  });

  test("equipment added on the page comes into its P&ID", async () => {
    const root = linkedProject();
    const { webview, panel } = await openBuilder(path.join(root, "hmi", "pages", "tank-farm.toml"));
    webview.send({ type: "ready" });
    const loaded = await until("the page", () => postedOfType(webview, "page")[0]);
    const exchanger: HmiObjectModel = {
      id: "e101",
      type: "heat-exchanger",
      tag: "E101",
      at: [500, 300],
      size: [128, 108],
      rotation: 0,
      bind: {},
    };
    webview.send({ type: "edit", model: { ...loaded.model, object: [...loaded.model.object, exchanger] } });
    const pid = path.join(root, "hmi", "pid", "tank-farm.toml");
    await until("the P&ID with the exchanger", () => /id = "e101"/.test(fs.readFileSync(pid, "utf8")), 20000);
    assert.doesNotMatch(fs.readFileSync(pid, "utf8"), /id = "speed"/, "the page's value stays on the page");
    panel.dispose();
  });

  test("a P&ID's binding problems are errors in the Problems view", async () => {
    const root = linkedProject();
    const pid = vscode.Uri.file(path.join(root, "hmi", "pid", "tank-farm.toml"));
    await vscode.window.showTextDocument(await vscode.workspace.openTextDocument(pid));
    const deadline = Date.now() + 30000;
    for (;;) {
      const found = vscode.languages.getDiagnostics(pid).filter((diagnostic) => diagnostic.source === "truST HMI");
      if (found.some((diagnostic) => /Main\.P101\.Running/.test(diagnostic.message))) break;
      if (Date.now() > deadline)
        assert.fail(`diagnostics: ${JSON.stringify(found.map((diagnostic) => diagnostic.message))}`);
      await new Promise((resolve) => setTimeout(resolve, 200));
    }
  });

  test("a P&ID is a drawing: its objects are not warned of as unbound, while the page's are", async () => {
    const root = linkedProject();
    const pid = vscode.Uri.file(path.join(root, "hmi", "pid", "tank-farm.toml"));
    const page = vscode.Uri.file(path.join(root, "hmi", "pages", "tank-farm.toml"));
    const checked = (uri: vscode.Uri) =>
      vscode.languages.getDiagnostics(uri).filter((diagnostic) => diagnostic.source === "truST HMI");
    const unbound = (uri: vscode.Uri) =>
      checked(uri).filter((diagnostic) => /is not bound to a variable/.test(diagnostic.message));
    await vscode.window.showTextDocument(await vscode.workspace.openTextDocument(page));
    await until("the page's warnings", () => unbound(page).length > 0 || undefined, 30000);
    await vscode.window.showTextDocument(await vscode.workspace.openTextDocument(pid));
    await until(
      "the P&ID's checks",
      () => checked(pid).some((diagnostic) => /Main\.P101\.Running/.test(diagnostic.message)) || undefined,
      30000,
    );
    await new Promise((resolve) => setTimeout(resolve, 1500));
    assert.deepStrictEqual(
      unbound(pid).map((diagnostic) => diagnostic.message),
      [],
      "a P&ID's objects need no variable",
    );
  });

  test("Generate Process Screen from P&ID writes a working page linked to it, with its objects bound as on the P&ID", async () => {
    const root = linkedProject();
    fs.rmSync(path.join(root, "hmi", "pages", "tank-farm.toml"));
    fs.writeFileSync(
      path.join(root, "src", "main.st"),
      "PROGRAM Main\nVAR\n    P101 : Pump;\nEND_VAR\nEND_PROGRAM\n\nFUNCTION_BLOCK Pump\nVAR_OUTPUT\n    Running : BOOL;\nEND_VAR\nEND_FUNCTION_BLOCK\n",
    );
    await vscode.commands.executeCommand("trust-lsp.hmiBuilder.generateProcessScreen", {
      pid: path.join(root, "hmi", "pid", "tank-farm.toml"),
    });
    const page = path.join(root, "hmi", "pages", "tank-farm.toml");
    await until("the generated page", () => fs.existsSync(page), 20000);
    const text = fs.readFileSync(page, "utf8");
    assert.match(text, /^format = 1\ntitle = "Tank farm"\npid = "tank-farm"\n/);
    assert.match(text, /id = "tk101"/);
    assert.match(text, /running = "Main\.P101\.Running"/, "bound as on the P&ID");
    const checked = await vscode.commands.executeCommand<{ ok: boolean; problems: unknown[] }>(
      "trust-lsp.hmiFileCheck",
      { text, file: "hmi/pages/tank-farm.toml", root_uri: vscode.Uri.file(root).toString() },
    );
    assert.deepStrictEqual([checked?.ok, checked?.problems], [true, []], "a working page: every binding resolves");
    await until(
      "the page in the builder",
      () => {
        const input = vscode.window.tabGroups.activeTabGroup.activeTab?.input;
        return input instanceof vscode.TabInputCustom && input.uri.fsPath === page;
      },
      20000,
    );
  });
});
