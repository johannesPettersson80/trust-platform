import { disposeFixtureProject } from "./fixtureDocuments";
import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";

import { languageServerReady, openBuilder, postedOfType, until } from "./hmiBuilderHost";
import { runtimeDeployDeps } from "../../hmiBuilder/editor/deployRuntime";

// Specification 39, §7.6: the binding problems of page and popup files are errors in the Problems
// view, checked when a file opens or changes and when a source file of its project is saved.

const SOURCE = "PROGRAM Main\nVAR\n    Level : REAL;\nEND_VAR\nEND_PROGRAM\n";
const PAGE =
  'format = 1\ntitle = "Overview"\n\n[[object]]\nid = "level"\ntype = "value"\nat = [0, 0]\nsize = [240, 96]\n\n[object.bind]\nvalue = "Main.Gone"\n';
const SCREEN = 'format = 1\nsize = [1280, 720]\ntheme = "dark"\nsymbols = "flat"\n';

const fixtureProjects: string[] = [];

function project(page = PAGE): string {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "trust-hmi-problems-"));
  fixtureProjects.push(root);
  fs.mkdirSync(path.join(root, "src"), { recursive: true });
  fs.mkdirSync(path.join(root, "hmi", "pages"), { recursive: true });
  fs.writeFileSync(path.join(root, "src", "main.st"), SOURCE);
  fs.writeFileSync(path.join(root, "hmi", "screen.toml"), SCREEN);
  fs.writeFileSync(path.join(root, "hmi", "pages", "overview.toml"), page);
  return root;
}

/** The file's diagnostics of one severity (errors unless said) once `done` holds, within the time
 * limit. */
async function diagnosticsOf(
  uri: vscode.Uri,
  done: (found: readonly vscode.Diagnostic[]) => boolean,
  ms = 30000,
  severity = vscode.DiagnosticSeverity.Error,
): Promise<readonly vscode.Diagnostic[]> {
  const deadline = Date.now() + ms;
  for (;;) {
    const found = vscode.languages
      .getDiagnostics(uri)
      .filter((diagnostic) => diagnostic.source === "truST HMI" && diagnostic.severity === severity);
    if (done(found)) return found;
    if (Date.now() > deadline)
      assert.fail(`diagnostics of ${uri.fsPath}: ${JSON.stringify(found.map((diagnostic) => diagnostic.message))}`);
    await new Promise((resolve) => setTimeout(resolve, 200));
  }
}

suite("HMI Builder: binding problems in the Problems view", function () {
  this.timeout(90000);

  teardown(async () => {
    for (const root of fixtureProjects.splice(0)) await disposeFixtureProject(root);
    await vscode.commands.executeCommand("workbench.action.closeAllEditors");
  });

  test("a binding that does not resolve is an error at its line", async () => {
    const page = vscode.Uri.file(path.join(project(), "hmi", "pages", "overview.toml"));
    await vscode.workspace.openTextDocument(page);
    const found = await diagnosticsOf(page, (diagnostics) => diagnostics.length > 0);
    assert.strictEqual(found.length, 1, JSON.stringify(found));
    assert.strictEqual(found[0].severity, vscode.DiagnosticSeverity.Error);
    assert.strictEqual(found[0].range.start.line, 10, "the binding's line");
    assert.match(found[0].message, /Main\.Gone/);
  });

  test("saving a source file that declares the variable clears the problem", async () => {
    const root = project();
    const page = vscode.Uri.file(path.join(root, "hmi", "pages", "overview.toml"));
    await vscode.workspace.openTextDocument(page);
    await diagnosticsOf(page, (diagnostics) => diagnostics.length === 1);
    const source = await vscode.workspace.openTextDocument(vscode.Uri.file(path.join(root, "src", "main.st")));
    const edit = new vscode.WorkspaceEdit();
    edit.insert(source.uri, new vscode.Position(2, 0), "    Gone : REAL;\n");
    assert.ok(await vscode.workspace.applyEdit(edit));
    assert.ok(await source.save(), "saved");
    await diagnosticsOf(page, (diagnostics) => diagnostics.length === 0);
  });

  test("a page file that does not load shows its errors at their lines", async () => {
    const page = vscode.Uri.file(path.join(project("format = 1\ntitle = 3\n"), "hmi", "pages", "overview.toml"));
    await vscode.workspace.openTextDocument(page);
    const found = await diagnosticsOf(page, (diagnostics) => diagnostics.length > 0);
    assert.strictEqual(found[0].range.start.line, 1, JSON.stringify(found));
  });

  test("an edit that fixes the binding clears the problem without saving", async () => {
    const page = vscode.Uri.file(path.join(project(), "hmi", "pages", "overview.toml"));
    const document = await vscode.workspace.openTextDocument(page);
    await diagnosticsOf(page, (diagnostics) => diagnostics.length === 1);
    const edit = new vscode.WorkspaceEdit();
    const line = document.lineAt(10);
    edit.replace(page, line.range, 'value = "Main.Level"');
    assert.ok(await vscode.workspace.applyEdit(edit));
    await diagnosticsOf(page, (diagnostics) => diagnostics.length === 0);
  });
});

suite("HMI Builder: quality warnings in the Problems view", function () {
  this.timeout(90000);

  teardown(async () => {
    for (const root of fixtureProjects.splice(0)) await disposeFixtureProject(root);
    await vscode.commands.executeCommand("workbench.action.closeAllEditors");
  });

  test("a page's quality warnings are warnings at the object's line, beside its errors", async () => {
    const text =
      'format = 1\ntitle = "Overview"\n\n[[object]]\nid = "level"\ntype = "value"\nat = [0, 0]\nsize = [240, 96]\n\n[object.bind]\nvalue = "Main.Level"\n';
    const page = vscode.Uri.file(path.join(project(text), "hmi", "pages", "overview.toml"));
    await vscode.workspace.openTextDocument(page);
    const found = await diagnosticsOf(
      page,
      (diagnostics) => diagnostics.length > 0,
      30000,
      vscode.DiagnosticSeverity.Warning,
    );
    assert.strictEqual(found.length, 1, JSON.stringify(found.map((diagnostic) => diagnostic.message)));
    assert.strictEqual(found[0].range.start.line, 4, "the object's id line");
    assert.match(found[0].message, /without a unit/);
    assert.deepStrictEqual(
      vscode.languages
        .getDiagnostics(page)
        .filter((diagnostic) => diagnostic.severity === vscode.DiagnosticSeverity.Error),
      [],
      "no errors",
    );
  });

  test("a project colour that looks like an alarm colour is a warning at its line in screen.toml", async () => {
    const root = project();
    const screen = vscode.Uri.file(path.join(root, "hmi", "screen.toml"));
    fs.writeFileSync(screen.fsPath, `${SCREEN}\n[colors.dark]\naccent = "#f97316"\n`);
    await vscode.workspace.openTextDocument(screen);
    const found = await diagnosticsOf(
      screen,
      (diagnostics) => diagnostics.length > 0,
      30000,
      vscode.DiagnosticSeverity.Warning,
    );
    const alike = found.find((diagnostic) => /looks like/.test(diagnostic.message));
    assert.ok(alike, JSON.stringify(found.map((diagnostic) => [diagnostic.range.start.line, diagnostic.message])));
    assert.strictEqual(alike.range.start.line, 6, "the colour's line");
    assert.match(alike.message, /alarm/);
  });

  test("a program without an Hmi.Display call is a warning at the top of screen.toml, gone once it has one (§18 Phase 5)", async () => {
    const root = project();
    const screen = vscode.Uri.file(path.join(root, "hmi", "screen.toml"));
    await vscode.workspace.openTextDocument(screen);
    const mentions = (diagnostic: vscode.Diagnostic) => /Hmi\.Display/.test(diagnostic.message);
    const found = await diagnosticsOf(
      screen,
      (diagnostics) => diagnostics.some(mentions),
      30000,
      vscode.DiagnosticSeverity.Warning,
    );
    const missing = found.find(mentions);
    assert.strictEqual(missing?.range.start.line, 0, "the top of the screen layout");
    assert.match(missing.message, /live values/);
    // The library block and a program calling it, in the project's source.
    const source = await vscode.workspace.openTextDocument(vscode.Uri.file(path.join(root, "src", "main.st")));
    const edit = new vscode.WorkspaceEdit();
    edit.insert(
      source.uri,
      new vscode.Position(source.lineCount, 0),
      "\nNAMESPACE Hmi\nFUNCTION_BLOCK Display\nVAR_OUTPUT\n    Calls : UDINT;\nEND_VAR\n    Calls := Calls + 1;\nEND_FUNCTION_BLOCK\nEND_NAMESPACE\n\nPROGRAM Screens\nVAR\n    Display : Hmi.Display;\nEND_VAR\n    Display();\nEND_PROGRAM\n",
    );
    assert.ok(await vscode.workspace.applyEdit(edit));
    assert.ok(await source.save(), "saved");
    await diagnosticsOf(screen, (diagnostics) => !diagnostics.some(mentions), 30000, vscode.DiagnosticSeverity.Warning);
  });
});

suite("HMI Builder: text files in the Problems view", function () {
  this.timeout(90000);

  teardown(async () => {
    for (const root of fixtureProjects.splice(0)) await disposeFixtureProject(root);
    await vscode.commands.executeCommand("workbench.action.closeAllEditors");
  });

  test("a text file that does not load shows its error at its line (§14.5 item 8)", async () => {
    const file = vscode.Uri.file(path.join(project(), "hmi", "texts", "sv.toml"));
    fs.mkdirSync(path.dirname(file.fsPath), { recursive: true });
    fs.writeFileSync(file.fsPath, 'format = 1\nlanguage = "sv"\n\n[texts]\n"Level" = 3\n');
    await vscode.workspace.openTextDocument(file);
    const found = await diagnosticsOf(file, (diagnostics) => diagnostics.length > 0);
    assert.strictEqual(found.length, 1, JSON.stringify(found.map((diagnostic) => diagnostic.message)));
    assert.strictEqual(found[0].range.start.line, 4, "the value's line");
    assert.match(found[0].message, /must be text in quotes/);
  });

  test("a text cut in one of the project's languages is a warning at the object's line (§14.5 item 6)", async () => {
    const text =
      'format = 1\ntitle = "Overview"\n\n[[object]]\nid = "start"\ntype = "text"\nat = [0, 0]\nsize = [120, 40]\ntext = "Start"\n';
    const root = project(text);
    fs.writeFileSync(path.join(root, "hmi", "screen.toml"), `${SCREEN}languages = ["en", "de"]\n`);
    fs.mkdirSync(path.join(root, "hmi", "texts"), { recursive: true });
    fs.writeFileSync(
      path.join(root, "hmi", "texts", "de.toml"),
      'format = 1\nlanguage = "de"\n\n[texts]\n"Start" = "Pumpe jetzt sofort starten"\n',
    );
    const page = vscode.Uri.file(path.join(root, "hmi", "pages", "overview.toml"));
    await vscode.workspace.openTextDocument(page);
    const found = await diagnosticsOf(
      page,
      (diagnostics) => diagnostics.some((diagnostic) => /Deutsch/.test(diagnostic.message)),
      30000,
      vscode.DiagnosticSeverity.Warning,
    );
    const cut = found.find((diagnostic) => /Deutsch/.test(diagnostic.message));
    assert.strictEqual(cut?.range.start.line, 4, "the object's id line");
    assert.match(cut?.message ?? "", /Pumpe jetzt sofort starten/);
  });
});

suite("HMI Builder: faceplate and pair files in the Problems view", function () {
  this.timeout(90000);

  const PUMP = "FUNCTION_BLOCK Pump\nVAR_OUTPUT\n    Running : BOOL;\nEND_VAR\nEND_FUNCTION_BLOCK\n";
  const FACEPLATE =
    'format = 1\ntitle = "Pump"\nsize = [400, 300]\n\n[fields.Running]\naccepts = ["boolean"]\n\n[fields.Speed]\naccepts = ["number"]\n';
  const PAIR = 'format = 1\nblock = "Pump"\nfaceplate = "pump"\nsymbol = "pump"\n';

  /** A project with a pump block, its faceplate and its pair. */
  function paired(faceplate = FACEPLATE): string {
    const root = project();
    fs.writeFileSync(path.join(root, "src", "pump.st"), PUMP);
    fs.mkdirSync(path.join(root, "hmi", "panels"), { recursive: true });
    fs.mkdirSync(path.join(root, "hmi", "pairs"), { recursive: true });
    fs.writeFileSync(path.join(root, "hmi", "panels", "pump.toml"), faceplate);
    fs.writeFileSync(path.join(root, "hmi", "pairs", "pump.toml"), PAIR);
    return root;
  }

  teardown(async () => {
    for (const root of fixtureProjects.splice(0)) await disposeFixtureProject(root);
    await vscode.commands.executeCommand("workbench.action.closeAllEditors");
  });

  test("a pair whose block lacks a member its faceplate needs is an error at the faceplate line, gone once the block has it (§8.6)", async () => {
    const root = paired();
    const pair = vscode.Uri.file(path.join(root, "hmi", "pairs", "pump.toml"));
    await vscode.workspace.openTextDocument(pair);
    const found = await diagnosticsOf(pair, (diagnostics) => diagnostics.length > 0);
    assert.strictEqual(found.length, 1, JSON.stringify(found.map((diagnostic) => diagnostic.message)));
    assert.strictEqual(found[0].range.start.line, 2, "the faceplate's line, whose field names the member");
    assert.match(found[0].message, /Speed/);
    const source = await vscode.workspace.openTextDocument(vscode.Uri.file(path.join(root, "src", "pump.st")));
    const editor = await vscode.window.showTextDocument(source);
    await editor.edit((edit) =>
      edit.replace(
        new vscode.Range(0, 0, source.lineCount, 0),
        PUMP.replace("Running : BOOL;", "Running : BOOL;\n    Speed : REAL;"),
      ),
    );
    await source.save();
    await diagnosticsOf(pair, (diagnostics) => diagnostics.length === 0);
  });

  test("a faceplate object bound to a field of a kind its property does not take is an error at its line (§8.7)", async () => {
    // The switch writes its value, so its binding reads and writes; the field's kind is the problem.
    const faceplate = `${FACEPLATE}\n[[object]]\nid = "run"\ntype = "switch"\nat = [24, 24]\nsize = [160, 48]\n\n[object.bind]\nvalue = { path = "$Speed", direction = "read-write" }\n`;
    const file = vscode.Uri.file(path.join(paired(faceplate), "hmi", "panels", "pump.toml"));
    await vscode.workspace.openTextDocument(file);
    const found = await diagnosticsOf(file, (diagnostics) => diagnostics.length > 0);
    assert.strictEqual(found.length, 1, JSON.stringify(found.map((diagnostic) => diagnostic.message)));
    assert.strictEqual(found[0].range.start.line, 17, "the binding's line");
    assert.match(found[0].message, /the field `Speed` takes/);
  });
});

suite("HMI Builder editor: the shell's tokens", function () {
  this.timeout(60000);

  teardown(async () => {
    for (const root of fixtureProjects.splice(0)) await disposeFixtureProject(root);
  });

  test("the editor's page takes truST's shared webview tokens before its own styles (§3.5 item 5)", async () => {
    const page = path.join(project(), "hmi", "pages", "overview.toml");
    const { panel } = await openBuilder(page);
    const html = (panel.webview as unknown as { html: string }).html;
    const links = [...html.matchAll(/<link rel="stylesheet" href="([^"]+)">/g)].map((match) => match[1]);
    assert.ok(
      links.some((link) => link.endsWith("/src/webview/theme.css")),
      links.join(", "),
    );
    assert.ok(
      links.findIndex((link) => link.endsWith("/src/webview/theme.css")) <
        links.findIndex((link) => link.endsWith("/hmiBuilderWebview.css")),
      "the shared tokens come first",
    );
    panel.dispose();
  });

  test("the editor's own styles use the shared tokens, never VS Code's variables directly", () => {
    const own = fs.readFileSync(
      path.resolve(__dirname, "../../../src/hmiBuilder/editor/webview/hmiBuilder.css"),
      "utf8",
    );
    const shared = fs.readFileSync(path.resolve(__dirname, "../../../src/webview/theme.css"), "utf8");
    assert.deepStrictEqual(
      [...new Set(own.match(/var\(--vscode-[\w.-]+/g) ?? [])],
      [],
      "VS Code variables used directly",
    );
    const defined = new Set([...shared.matchAll(/(--trust-[\w-]+)\s*:/g)].map((match) => match[1]));
    const used = [...new Set([...own.matchAll(/var\((--trust-[\w-]+)/g)].map((match) => match[1]))];
    assert.deepStrictEqual(
      used.filter((token) => !defined.has(token)),
      [],
      "tokens the shared layer does not define",
    );
  });
});

suite("HMI Builder editor: project colours", function () {
  this.timeout(60000);

  teardown(async () => {
    for (const root of fixtureProjects.splice(0)) await disposeFixtureProject(root);
  });

  test("a colour chosen in the editor is written to screen.toml, Reset removes it, and the canvas gets the new screen", async () => {
    await languageServerReady();
    const root = project();
    const page = path.join(root, "hmi", "pages", "overview.toml");
    const screen = path.join(root, "hmi", "screen.toml");
    const { webview, panel } = await openBuilder(page);
    const settled = (): Promise<void> => new Promise((resolve) => setTimeout(resolve, 2000));
    webview.send({ type: "colour", theme: "dark", token: "accent", value: "#f97316" });
    await settled();
    assert.match(fs.readFileSync(screen, "utf8"), /\[colors\.dark\]\naccent = "#f97316"/, "the colour in screen.toml");
    assert.ok(
      postedOfType(webview, "screen").some((message) => message.screen?.colors?.dark?.accent === "#f97316"),
      "the new screen",
    );
    webview.send({ type: "colour", theme: "dark", token: "accent" });
    await settled();
    const after = fs.readFileSync(screen, "utf8");
    assert.doesNotMatch(after, /accent/, "the colour gone");
    assert.doesNotMatch(after, /\[colors/, "no empty table is left");
    panel.dispose();
  });

  test("the editor sends the project's colour warnings", async () => {
    await languageServerReady();
    const root = project();
    const page = path.join(root, "hmi", "pages", "overview.toml");
    const warning = {
      kind: "look-alike" as const,
      colour: { theme: "dark" as const, token: "accent" },
      message: "accent looks like an alarm colour.",
    };
    const { webview, panel } = await openBuilder(page, {
      objectProblems: () => ({}),
      colourWarnings: (at: string) => (path.normalize(at) === path.normalize(root) ? [warning] : []),
      onDidChange: new vscode.EventEmitter<vscode.Uri>().event,
    });
    webview.send({ type: "ready" });
    await until("the editor started", () => postedOfType(webview, "variables")[0]);
    assert.deepStrictEqual(postedOfType(webview, "colourWarnings").pop()?.warnings, [warning]);
    panel.dispose();
  });
});

suite("HMI Builder editor: problems on the canvas", function () {
  this.timeout(60000);

  teardown(async () => {
    for (const root of fixtureProjects.splice(0)) await disposeFixtureProject(root);
  });

  test("the editor sends its page's problems when it starts and whenever they change", async () => {
    await languageServerReady();
    const page = path.join(project(), "hmi", "pages", "overview.toml");
    const changed = new vscode.EventEmitter<vscode.Uri>();
    let current: Record<string, string[]> = { level: ['"Main.Gone" is not a variable of the program'] };
    const { webview, panel } = await openBuilder(page, {
      objectProblems: (uri) => (path.normalize(uri.fsPath) === path.normalize(page) ? current : {}),
      onDidChange: changed.event,
    });
    webview.send({ type: "ready" });
    // The variables come last when the editor starts.
    await until("the editor started", () => postedOfType(webview, "variables")[0]);
    assert.deepStrictEqual(postedOfType(webview, "problems")[0]?.problems, current, "the page's problems");
    current = {};
    changed.fire(vscode.Uri.file(path.join(path.dirname(page), "other.toml")));
    changed.fire(vscode.Uri.file(page));
    await new Promise((resolve) => setTimeout(resolve, 100));
    assert.deepStrictEqual(postedOfType(webview, "problems")[1]?.problems, {}, "the changed problems");
    assert.strictEqual(postedOfType(webview, "problems").length, 2, "another file's change sends nothing");
    panel.dispose();
  });

  test("the editor sends its page's quality warnings with its problems", async () => {
    await languageServerReady();
    const page = path.join(project(), "hmi", "pages", "overview.toml");
    const warning = { kind: "unit" as const, object: "level", message: "Level shows a REAL value without a unit." };
    const { webview, panel } = await openBuilder(page, {
      objectProblems: () => ({}),
      objectWarnings: (uri): Record<string, Array<typeof warning>> =>
        path.normalize(uri.fsPath) === path.normalize(page) ? { level: [warning] } : {},
      onDidChange: new vscode.EventEmitter<vscode.Uri>().event,
    });
    webview.send({ type: "ready" });
    await until("the editor started", () => postedOfType(webview, "variables")[0]);
    assert.deepStrictEqual(postedOfType(webview, "problems")[0]?.warnings, { level: [warning] });
    panel.dispose();
  });
});

suite("HMI Builder editor: Live mode on the host", function () {
  this.timeout(60000);

  teardown(async () => {
    for (const root of fixtureProjects.splice(0)) await disposeFixtureProject(root);
  });

  function fakeConnection(target: { endpoint: string; label: string } | undefined, answer: () => unknown) {
    const calls: Array<{ endpoint: string; operation: string; params: unknown }> = [];
    return {
      calls,
      connection: {
        target: async () => target,
        request: async (endpoint: string, _token: string | undefined, operation: string, params: unknown) => {
          calls.push({ endpoint, operation, params });
          return answer();
        },
      },
    };
  }

  test("Live mode reads the page's paths with hmi.read and passes the values on", async () => {
    await languageServerReady();
    const page = path.join(project(), "hmi", "pages", "overview.toml");
    const fake = fakeConnection({ endpoint: "tcp://127.0.0.1:9", label: "Simulator" }, () => ({
      cycle: 3,
      values: { "Main.Level": { value: 4.5 } },
      missing: [],
    }));
    const { webview, panel } = await openBuilder(page, undefined, fake.connection);
    webview.send({ type: "live", on: true, paths: ["Main.Level"] });
    await new Promise((resolve) => setTimeout(resolve, 700));
    assert.ok(postedOfType(webview, "liveValues")[0], "live values");
    assert.deepStrictEqual(postedOfType(webview, "liveValues")[0].update, {
      cycle: 3,
      values: { "Main.Level": { value: 4.5 } },
      missing: [],
    });
    assert.deepStrictEqual(fake.calls[0], {
      endpoint: "tcp://127.0.0.1:9",
      operation: "hmi.read",
      params: { paths: ["Main.Level"] },
    });
    assert.deepStrictEqual(lastOf(postedOfType(webview, "live")), {
      type: "live",
      state: "connected",
      target: "Simulator",
    });
    webview.send({ type: "live", on: false });
    const count = fake.calls.length;
    await new Promise((resolve) => setTimeout(resolve, 700));
    assert.ok(fake.calls.length <= count + 1, `polling stopped: ${count} then ${fake.calls.length}`);
    panel.dispose();
  });

  test("the variables the pickers show are read from the run target; without one there are no values", async () => {
    await languageServerReady();
    const page = path.join(project(), "hmi", "pages", "overview.toml");
    const fake = fakeConnection({ endpoint: "tcp://127.0.0.1:9", label: "Simulator" }, () => ({
      cycle: 3,
      values: { "Main.Level": { value: 4.5 } },
      missing: [],
    }));
    const { webview, panel } = await openBuilder(page, undefined, fake.connection);
    webview.send({ type: "watch", paths: ["Main.Level"] });
    await until("picker values", () => postedOfType(webview, "pickerValues")[0]);
    assert.deepStrictEqual(postedOfType(webview, "pickerValues")[0], {
      type: "pickerValues",
      update: { cycle: 3, values: { "Main.Level": { value: 4.5 } }, missing: [] },
    });
    assert.deepStrictEqual(fake.calls[0], {
      endpoint: "tcp://127.0.0.1:9",
      operation: "hmi.read",
      params: { paths: ["Main.Level"] },
    });
    assert.strictEqual(postedOfType(webview, "live").length, 0, "Live mode's state is not touched");
    webview.send({ type: "watch", paths: [] });
    const count = fake.calls.length;
    await new Promise((resolve) => setTimeout(resolve, 700));
    assert.ok(fake.calls.length <= count + 1, `reading stopped: ${count} then ${fake.calls.length}`);
    panel.dispose();
    const none = fakeConnection(undefined, () => ({}));
    const second = await openBuilder(page, undefined, none.connection);
    second.webview.send({ type: "watch", paths: ["Main.Level"] });
    await until("no values", () => postedOfType(second.webview, "pickerValues")[0]);
    assert.deepStrictEqual(postedOfType(second.webview, "pickerValues")[0], { type: "pickerValues" });
    second.panel.dispose();
  });

  test("Live mode says when no target runs and when the connection is lost", async () => {
    await languageServerReady();
    const page = path.join(project(), "hmi", "pages", "overview.toml");
    const none = fakeConnection(undefined, () => ({}));
    const first = await openBuilder(page, undefined, none.connection);
    first.webview.send({ type: "live", on: true, paths: ["Main.Level"] });
    await new Promise((resolve) => setTimeout(resolve, 500));
    assert.strictEqual(postedOfType(first.webview, "live")[0]?.state, "none", "no target");
    first.panel.dispose();
    const lost = fakeConnection({ endpoint: "tcp://127.0.0.1:9", label: "Line 1" }, () => {
      throw new Error("connection refused");
    });
    const second = await openBuilder(page, undefined, lost.connection);
    second.webview.send({ type: "live", on: true, paths: ["Main.Level"] });
    await new Promise((resolve) => setTimeout(resolve, 500));
    assert.deepStrictEqual(postedOfType(second.webview, "live")[0], {
      type: "live",
      state: "lost",
      target: "Line 1",
      reason: "connection refused",
    });
    second.panel.dispose();
  });
});

/** The last item of a list. */
function lastOf<T>(items: T[]): T | undefined {
  return items[items.length - 1];
}

suite("HMI Builder editor: Deploy on the host", function () {
  this.timeout(60000);

  teardown(async () => {
    for (const root of fixtureProjects.splice(0)) await disposeFixtureProject(root);
  });

  test("Deploy saves the page, checks and sends the bundle, and reports the outcome", async () => {
    await languageServerReady();
    const page = path.join(project(), "hmi", "pages", "overview.toml");
    const asked: string[] = [];
    const { document, webview, panel } = await openBuilder(page, undefined, undefined, {
      bundle: async (root) => {
        asked.push(root);
        return { ok: true, bundle: { format: 1, files: {} }, version: "abc123", problems: [] };
      },
      target: async () => ({ endpoint: "tcp://127.0.0.1:9000", token: "secret", label: "Line 1", simulator: false }),
      control: async () => ({
        hosts: [
          {
            runtimes: [
              { endpoints: [{ protocol: "web", source: "self", params: { listen: "127.0.0.1:8080", tls: false } }] },
            ],
          },
        ],
      }),
      post: async () => ({ status: 200, body: { ok: true, active: "abc123", previous: "old999" } }),
      // The address the language server gives for the topology above.
      address: runtimeDeployDeps.address,
    });
    const edit = new vscode.WorkspaceEdit();
    edit.insert(document.uri, new vscode.Position(0, 0), "# unsaved\n");
    assert.ok(await vscode.workspace.applyEdit(edit));
    webview.send({ type: "deploy" });
    await new Promise((resolve) => setTimeout(resolve, 800));
    assert.strictEqual(document.isDirty, false, "the page was saved first");
    assert.deepStrictEqual(asked, [path.dirname(path.dirname(path.dirname(page)))], "the project's bundle");
    assert.deepStrictEqual(postedOfType(webview, "deployed")[0]?.outcome, {
      ok: true,
      version: "abc123",
      target: "Line 1",
    });
    panel.dispose();
  });
});
