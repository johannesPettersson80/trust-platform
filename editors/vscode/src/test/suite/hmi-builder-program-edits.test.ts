import { disposeFixtureProject } from "./fixtureDocuments";
import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";

import { EquipmentEditorBridge } from "../../hmiBuilder/editor/equipmentDetails";
import type { HostToWebview } from "../../hmiBuilder/editor/messages";
import { renameEquipment } from "../../hmiBuilder/editor/renameEquipment";
import { languageServerReady, until } from "./hmiBuilderHost";

// Specification 39, §10: Add Missing Variables to Program declares a page's values in its program as
// normal edits, and leaves its equipment to Create PLC code; a second run adds nothing.

const SOURCE =
  "FUNCTION_BLOCK Pump\nVAR_INPUT\n    AutoRun : BOOL;\nEND_VAR\nVAR_OUTPUT\n    Running : BOOL;\nEND_VAR\n    Running := AutoRun;\nEND_FUNCTION_BLOCK\n\nPROGRAM Transfer\nVAR\n    Started : BOOL;\nEND_VAR\n    Started := TRUE;\nEND_PROGRAM\n\nCONFIGURATION Config\nRESOURCE Res ON PLC\nTASK Fast(INTERVAL := T#100ms, PRIORITY := 1);\nPROGRAM Main WITH Fast : Transfer;\nEND_RESOURCE\nEND_CONFIGURATION\n";

const PAGE =
  'format = 1\ntitle = "Overview"\n\n[[object]]\nid = "p-101"\ntag = "P-101"\ntype = "pump"\nat = [40, 40]\nsize = [128, 108]\n\n[object.bind]\nrunning = "Main.P101.Running"\n\n[object.click]\nfaceplate = "pump"\n\n[[object]]\nid = "level"\ntype = "value"\nat = [240, 120]\nsize = [160, 48]\n\n[object.bind]\nvalue = "Main.Level"\n';

const RUNTIME = '[bundle]\nname = "transfer"\n\n[resource]\nname = "Res"\n';

const fixtureProjects: string[] = [];

function project(): { root: string; page: string; source: string } {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "trust-hmi-add-"));
  fixtureProjects.push(root);
  const write = (file: string, text: string): string => {
    const full = path.join(root, file);
    fs.mkdirSync(path.dirname(full), { recursive: true });
    fs.writeFileSync(full, text);
    return full;
  };
  const source = write("src/main.st", SOURCE);
  write("hmi/screen.toml", "format = 1\n");
  write(
    "hmi/panels/pump.toml",
    'format = 1\ntitle = "Pump"\nsize = [400, 300]\n\n[fields.Running]\naccepts = ["boolean"]\n',
  );
  write(
    "hmi/pairs/pump.toml",
    'format = 1\nblock = "Pump"\nfaceplate = "pump"\nsymbol = "pump"\n\n[program]\nposition = "actuators"\n',
  );
  const page = write("hmi/pages/overview.toml", PAGE);
  return { root, page, source };
}

suite("HMI Builder editor: add missing variables", function () {
  this.timeout(60000);

  suiteSetup(languageServerReady);

  teardown(async () => {
    for (const root of fixtureProjects.splice(0)) await disposeFixtureProject(root);
    await vscode.commands.executeCommand("workbench.action.closeAllEditors");
  });

  test("the page's values are declared in its program, once, and its equipment is left to Create PLC code", async () => {
    const { page, source } = project();
    // The pump as the builder places it: equipment with its block and instance.
    fs.writeFileSync(
      page,
      PAGE.replace(
        'tag = "P-101"\ntype = "pump"',
        'type = "pump"\nblock = "Pump"\ninstance = "Main.P101"\nfaceplate = "pump"',
      ),
    );
    const first = (await vscode.commands.executeCommand("trust-lsp.hmiBuilder.addMissingVariables", {
      page: vscode.Uri.file(page).toString(),
      confirm: false,
    })) as {
      applied: boolean;
      added: Array<{ variable: string }>;
      skipped: Array<{ object: string; message: string }>;
    };
    assert.strictEqual(first.applied, true, JSON.stringify(first));
    assert.deepStrictEqual(
      first.added.map((entry) => entry.variable),
      ["Main.Level"],
    );
    assert.deepStrictEqual(first.skipped, [
      { object: "p-101", message: "equipment is declared and called by Create PLC code" },
    ]);
    const document = await vscode.workspace.openTextDocument(source);
    const text = document.getText();
    assert.ok(text.includes("VAR\n    Started : BOOL;\n    Level : LREAL;\nEND_VAR"), text);
    assert.ok(!text.includes("P101"), text);
    assert.ok(document.isDirty, "applied as normal edits the user saves or undoes");
    await document.save();
    const second = (await vscode.commands.executeCommand("trust-lsp.hmiBuilder.addMissingVariables", {
      page: vscode.Uri.file(page).toString(),
      confirm: false,
    })) as { added: unknown[] };
    assert.deepStrictEqual(second.added, [], "a second run adds nothing");
  });
});

suite("HMI Builder editor: create pages from program", function () {
  this.timeout(60000);

  suiteSetup(languageServerReady);

  teardown(async () => {
    for (const root of fixtureProjects.splice(0)) await disposeFixtureProject(root);
    await vscode.commands.executeCommand("workbench.action.closeAllEditors");
  });

  test("drawing stays separate from explicit PLC generation, including dirty buffers", async () => {
    const { root, page, source } = project();
    const intent =
      'format=1\ntitle="Overview"\n[[object]]\nid="pump"\ntype="pump"\nblock="Pump"\ninstance="Main.P101"\nat=[80,80]\nsize=[128,108]\n';
    const document = await vscode.workspace.openTextDocument(vscode.Uri.file(page));
    const edit = new vscode.WorkspaceEdit();
    edit.replace(document.uri, new vscode.Range(0, 0, document.lineCount, 0), intent);
    assert.ok(await vscode.workspace.applyEdit(edit));
    const declarations = await vscode.commands.executeCommand<{ ok: boolean; inspectionMode: string }>(
      "trust-lsp.hmiBindings",
      { root_uri: vscode.Uri.file(root).toString(), authoring: true },
    );
    assert.strictEqual(declarations?.ok, true);
    assert.strictEqual(declarations?.inspectionMode, "declarations");
    assert.strictEqual(fs.readFileSync(source, "utf8"), SOURCE, "drawing does not modify source");
    assert.strictEqual(
      await vscode.commands.executeCommand("trust-lsp.hmiBuilder.createPlcCode", {
        folder: vscode.Uri.file(root).toString(),
      }),
      true,
    );
    const generated = await vscode.workspace.openTextDocument(vscode.Uri.file(source));
    assert.strictEqual(generated.getText().match(/P101 : Pump;/g)?.length, 1);
    assert.ok(generated.getText().includes("Started := TRUE;"), "manual application logic is retained");
    assert.strictEqual(
      fs.readFileSync(source, "utf8"),
      generated.getText(),
      "the generated code is saved: the user had no unsaved changes of their own (§10.3 item 2)",
    );
    const before = generated.getText();
    assert.strictEqual(
      await vscode.commands.executeCommand("trust-lsp.hmiBuilder.createPlcCode", {
        folder: vscode.Uri.file(root).toString(),
      }),
      true,
    );
    assert.strictEqual(generated.getText(), before, "repeat generation is idempotent");
  });

  test("an overview page of the program's equipment is written, opened, and never overwritten", async () => {
    const { root, source } = project();
    fs.writeFileSync(source, SOURCE.replace("    Started : BOOL;\n", "    Started : BOOL;\n    P101 : Pump;\n"));
    fs.rmSync(path.join(root, "hmi", "pages"), { recursive: true, force: true });
    const first = (await vscode.commands.executeCommand("trust-lsp.hmiBuilder.createPagesFromProgram", {
      folder: vscode.Uri.file(root).toString(),
    })) as { page?: string; count: number };
    assert.strictEqual(first.count, 1, JSON.stringify(first));
    const file = path.join(root, "hmi", "pages", "overview.toml");
    assert.strictEqual(vscode.Uri.parse(first.page ?? "").fsPath, file);
    const parsed = (await vscode.commands.executeCommand("trust-lsp.hmiFileParse", {
      kind: "page",
      text: (await vscode.workspace.openTextDocument(vscode.Uri.file(file))).getText(),
      file,
    })) as { ok?: boolean; model?: { object: Array<{ type: string; tag?: string }> } };
    assert.deepStrictEqual(
      parsed.model?.object.map((object) => [object.type, object.tag]),
      [["pump", "P101"]],
    );
    const tab = vscode.window.tabGroups.activeTabGroup.activeTab?.input;
    assert.ok(
      tab instanceof vscode.TabInputCustom && tab.viewType === "trust-lsp.hmiBuilder",
      "the page opens in the HMI Builder",
    );
    assert.strictEqual(fs.readFileSync(source, "utf8").includes("P101 : Pump;"), true, "the program is unchanged");
    const second = (await vscode.commands.executeCommand("trust-lsp.hmiBuilder.createPagesFromProgram", {
      folder: vscode.Uri.file(root).toString(),
    })) as { page?: string; count: number };
    assert.strictEqual(second.count, 0, "existing equipment views are not duplicated");
    assert.strictEqual(second.page, undefined);
    assert.strictEqual(fs.existsSync(path.join(root, "hmi", "pages", "overview-2.toml")), false);
  });
});

suite("HMI Builder editor: create settings page", function () {
  this.timeout(60000);

  suiteSetup(languageServerReady);

  teardown(async () => {
    for (const root of fixtureProjects.splice(0)) await disposeFixtureProject(root);
    await vscode.commands.executeCommand("workbench.action.closeAllEditors");
  });

  test("a settings page with an entry for each retained variable is written and opened", async () => {
    const { root, source } = project();
    fs.writeFileSync(
      source,
      SOURCE.replace(
        "    Started : BOOL;\nEND_VAR\n",
        "    Started : BOOL;\nEND_VAR\nVAR RETAIN\n    HighLevel : REAL := 2.5;\n    PumpEnabled : BOOL;\nEND_VAR\n",
      ),
    );
    fs.writeFileSync(path.join(root, "runtime.toml"), RUNTIME);
    const result = (await vscode.commands.executeCommand("trust-lsp.hmiBuilder.createSettingsPage", {
      folder: vscode.Uri.file(root).toString(),
      program: "Main",
    })) as { page?: string; count: number };
    assert.strictEqual(result.count, 2, JSON.stringify(result));
    // The page works at once: the runtime's write policy lets Engineers write what it sets (§12.2.3).
    assert.strictEqual(
      fs.readFileSync(path.join(root, "runtime.toml"), "utf8"),
      `${RUNTIME}\n[[runtime.web.hmi.write]]\npath = "Main.HighLevel"\nrole = "engineer"\n\n[[runtime.web.hmi.write]]\npath = "Main.PumpEnabled"\nrole = "engineer"\n`,
      "saved with a rule for each setting",
    );
    const file = path.join(root, "hmi", "pages", "settings.toml");
    assert.strictEqual(vscode.Uri.parse(result.page ?? "").fsPath, file);
    const parsed = (await vscode.commands.executeCommand("trust-lsp.hmiFileParse", {
      kind: "page",
      text: (await vscode.workspace.openTextDocument(vscode.Uri.file(file))).getText(),
      file,
    })) as { model?: { object: Array<{ type: string; label?: string }> } };
    assert.deepStrictEqual(
      parsed.model?.object.map((object) => [object.type, object.label]),
      [
        ["numeric-input", "High level"],
        ["switch", "Pump enabled"],
      ],
    );
    const tab = vscode.window.tabGroups.activeTabGroup.activeTab?.input;
    assert.ok(
      tab instanceof vscode.TabInputCustom && tab.viewType === "trust-lsp.hmiBuilder",
      "the page opens in the HMI Builder",
    );
  });
});

// Specification 39, §12.2.3: a control the runtime's write policy does not cover offers the rule it
// needs; the rule takes the control's role and limits, is added once, and the problem clears.
suite("HMI Builder editor: allow writing", function () {
  this.timeout(90000);

  suiteSetup(languageServerReady);

  teardown(async () => {
    for (const root of fixtureProjects.splice(0)) await disposeFixtureProject(root);
    await vscode.commands.executeCommand("workbench.action.closeAllEditors");
  });

  test("the quick fix adds the control's rule to runtime.toml once, and the problem clears", async () => {
    const { root, source } = project();
    fs.writeFileSync(source, SOURCE.replace("    Started : BOOL;\n", "    Started : BOOL;\n    Level : REAL;\n"));
    fs.writeFileSync(path.join(root, "runtime.toml"), RUNTIME);
    const page = path.join(root, "hmi", "pages", "controls.toml");
    fs.writeFileSync(
      page,
      'format = 1\ntitle = "Controls"\n\n[[object]]\nid = "level"\ntype = "slider"\nat = [40, 40]\nsize = [264, 48]\nmin = 0\nmax = 80\nrole = "operator"\n\n[object.bind]\nvalue = { path = "Main.Level", direction = "read-write" }\n',
    );
    const uri = vscode.Uri.file(page);
    await vscode.window.showTextDocument(uri);
    const refused = (): vscode.Diagnostic | undefined =>
      vscode.languages
        .getDiagnostics(uri)
        .find((diagnostic) => /not writable by the runtime's write policy/.test(diagnostic.message));
    const problem = await until("the write-policy problem", refused, 30000);
    const actions =
      (await vscode.commands.executeCommand<vscode.CodeAction[]>(
        "vscode.executeCodeActionProvider",
        uri,
        problem.range,
      )) ?? [];
    const fix = actions.find(
      (action) => action.title === "Allow the runtime display to write Main.Level (runtime.toml)",
    );
    assert.ok(fix?.command, `offered: ${actions.map((action) => action.title).join("; ")}`);
    const added = await vscode.commands.executeCommand(fix.command.command, ...(fix.command.arguments ?? []));
    assert.deepStrictEqual(added, ["Main.Level"]);
    const rules = `${RUNTIME}\n[[runtime.web.hmi.write]]\npath = "Main.Level"\nrole = "operator"\nmin = 0.0\nmax = 80.0\n`;
    assert.strictEqual(fs.readFileSync(path.join(root, "runtime.toml"), "utf8"), rules, "saved, as the control asks");
    await until("the problem to clear", () => (refused() ? undefined : true), 30000);
    assert.deepStrictEqual(
      await vscode.commands.executeCommand(fix.command.command, ...(fix.command.arguments ?? [])),
      [],
      "a point runtime.toml names already is left",
    );
    assert.strictEqual(fs.readFileSync(path.join(root, "runtime.toml"), "utf8"), rules);
  });
});

// Specification 39, §10: Create PLC code's result reaches the disk at once: closing VS Code after it
// must not lose the code or leave an empty file behind.
suite("HMI Builder editor: create PLC code", function () {
  this.timeout(60000);

  suiteSetup(languageServerReady);

  teardown(async () => {
    for (const root of fixtureProjects.splice(0)) await disposeFixtureProject(root);
    await vscode.commands.executeCommand("workbench.action.closeAllEditors");
  });

  test("the program it edits is saved and the files it creates hold their text", async () => {
    const { root, page, source } = project();
    // With the Hmi library, Create PLC code also writes the screens program into a new file.
    const extension = path.resolve(__dirname, "..", "..", "..");
    fs.cpSync(path.join(extension, "media", "libraries", "hmi"), path.join(root, "libraries", "hmi"), {
      recursive: true,
    });
    fs.writeFileSync(
      path.join(root, "trust-lsp.toml"),
      'include_paths = ["src"]\n\n[dependencies]\nHmi = { path = "libraries/hmi", version = "0.1.0" }\n',
    );
    fs.writeFileSync(
      page,
      PAGE.replace(
        'tag = "P-101"\ntype = "pump"',
        'type = "pump"\nblock = "Pump"\ninstance = "Main.P101"\nfaceplate = "pump"',
      ),
    );
    const created = await vscode.commands.executeCommand<boolean>("trust-lsp.hmiBuilder.createPlcCode", {
      folder: vscode.Uri.file(root).toString(),
    });
    assert.strictEqual(created, true);
    assert.match(fs.readFileSync(source, "utf8"), /P101 : Pump;/, "the program on disk declares the pump");
    assert.match(
      fs.readFileSync(path.join(root, "src", "screens.st"), "utf8"),
      /Hmi\.Display/,
      "the new screens program is on disk",
    );
    const unsaved = vscode.workspace.textDocuments.filter(
      (document) => document.isDirty && document.uri.fsPath.startsWith(root),
    );
    assert.deepStrictEqual(
      unsaved.map((document) => path.relative(root, document.uri.fsPath)),
      [],
      "nothing is left unsaved",
    );
  });

  test("connecting an equipment's hardware saves the design, which has no editor", async () => {
    const { root, page, source } = project();
    fs.writeFileSync(source, OWN_PUMP_SOURCE);
    fs.writeFileSync(path.join(root, "hmi", "pairs", "pump.toml"), OWN_PUMP_PAIR);
    fs.writeFileSync(
      page,
      PAGE.replace(
        'tag = "P-101"\ntype = "pump"',
        'type = "pump"\nblock = "Pump"\ninstance = "Main.P101"\nfaceplate = "pump"',
      ),
    );
    const document = await vscode.workspace.openTextDocument(page);
    const posted: Array<{ type: string; error?: string; details?: { revision: number } }> = [];
    const bridge = new EquipmentEditorBridge(vscode.Uri.file(root), document, (message) => posted.push(message));
    await bridge.inspect("Main.P101");
    const inspected = await until("the pump's details", () => posted.find((message) => message.details));
    await bridge.configure("Main.P101", { signals: { RunCommand: "Main.PumpRun" } }, inspected.details!.revision);
    const configured = await until("the connection applied", () =>
      posted.find((message) => message.type === "equipmentConfigured"),
    );
    assert.strictEqual(configured.error, undefined);
    assert.match(
      fs.readFileSync(path.join(root, "plant.toml"), "utf8"),
      /RunCommand = "Main\.PumpRun"/,
      "the connection is on disk",
    );
    const unsaved = vscode.workspace.textDocuments.filter(
      (document) => document.isDirty && document.uri.fsPath.startsWith(root),
    );
    assert.deepStrictEqual(
      unsaved.map((document) => path.relative(root, document.uri.fsPath)),
      [],
      "nothing is left unsaved",
    );
    bridge.dispose();
  });

  test("a terminal that no PLC signal fits carries the language server's help", async () => {
    // Catches the help left out of the details the bridge posts: the canvas shows it under the terminal (HMI component
    // catalogue, section 8.1). The program declares its pump signal without an address, so no signal fits.
    const { root, page, source } = project();
    fs.writeFileSync(source, OWN_PUMP_SOURCE.replace("PumpRun AT %QX0.0 : BOOL;", "PumpRun : BOOL;"));
    fs.writeFileSync(path.join(root, "hmi", "pairs", "pump.toml"), OWN_PUMP_PAIR);
    fs.writeFileSync(
      page,
      PAGE.replace(
        'tag = "P-101"\ntype = "pump"',
        'type = "pump"\nblock = "Pump"\ninstance = "Main.P101"\nfaceplate = "pump"',
      ),
    );
    const document = await vscode.workspace.openTextDocument(page);
    const posted: HostToWebview[] = [];
    const bridge = new EquipmentEditorBridge(vscode.Uri.file(root), document, (message) => posted.push(message));
    await bridge.inspect("Main.P101");
    const details = await until("the pump's details", () => {
      for (const message of posted) if (message.type === "equipmentDetails" && message.details) return message.details;
      return undefined;
    });
    const run = details.terminals.find((terminal) => terminal.member === "RunCommand");
    assert.deepStrictEqual(run?.choices, [], "no signal fits");
    assert.strictEqual(
      run?.help,
      "No output of type BOOL is declared yet. Declare one with an address in your program, for example " +
        "P101_RunCommand AT %QX0.0 : BOOL;, then press Refresh.",
    );
    bridge.dispose();
  });

  test("settings kept from the running PLC reach the design only where they differ from it", async () => {
    const { root, page, source } = project();
    fs.writeFileSync(source, OWN_PUMP_SOURCE);
    fs.writeFileSync(path.join(root, "hmi", "pairs", "pump.toml"), OWN_PUMP_PAIR);
    fs.writeFileSync(
      page,
      PAGE.replace(
        'tag = "P-101"\ntype = "pump"',
        'type = "pump"\nblock = "Pump"\ninstance = "Main.P101"\nfaceplate = "pump"',
      ),
    );
    const document = await vscode.workspace.openTextDocument(page);
    const bridge = new EquipmentEditorBridge(vscode.Uri.file(root), document, () => undefined);
    const answer = await bridge.keep("Main.P101", { StartTimeout: 12, ServiceInterval: 4000 });
    assert.deepStrictEqual(
      answer,
      { ok: true, kept: ["StartTimeout"] },
      "the service interval is the project's already",
    );
    const design = fs.readFileSync(path.join(root, "plant.toml"), "utf8");
    assert.match(design, /StartTimeout = "T#12s"/);
    assert.doesNotMatch(design, /ServiceInterval/);
    assert.deepStrictEqual(await bridge.keep("Main.P101", { StartTimeout: 12 }), { ok: true, kept: [] });
    const unsaved = vscode.workspace.textDocuments.filter(
      (document) => document.isDirty && document.uri.fsPath.startsWith(root),
    );
    assert.deepStrictEqual(
      unsaved.map((document) => path.relative(root, document.uri.fsPath)),
      [],
      "nothing is left unsaved",
    );
    bridge.dispose();
  });
});

suite("HMI Builder editor: rename equipment", function () {
  this.timeout(60000);

  suiteSetup(languageServerReady);

  teardown(async () => {
    for (const root of fixtureProjects.splice(0)) await disposeFixtureProject(root);
    await vscode.commands.executeCommand("workbench.action.closeAllEditors");
  });

  test("renaming equipment renames its declaration, calls and page objects, saved", async () => {
    const { root, page, source } = project();
    fs.writeFileSync(
      source,
      OWN_PUMP_SOURCE.replace(
        "    PumpRun AT %QX0.0 : BOOL;\n",
        "    PumpRun AT %QX0.0 : BOOL;\n    P101 : Pump;\n",
      ).replace("    PumpRun := FALSE;\n", "    P101();\n"),
    );
    fs.writeFileSync(path.join(root, "hmi", "pairs", "pump.toml"), OWN_PUMP_PAIR);
    fs.writeFileSync(
      page,
      PAGE.replace(
        'tag = "P-101"\ntype = "pump"',
        'tag = "P101"\ntype = "pump"\nblock = "Pump"\ninstance = "Main.P101"\nfaceplate = "pump"',
      ),
    );
    assert.strictEqual(await renameEquipment(vscode.Uri.file(root), "Main.P101", "FeedPump"), undefined);
    const program = fs.readFileSync(source, "utf8");
    assert.match(program, /FeedPump : Pump;/);
    assert.match(program, /FeedPump\(\);/);
    const pageText = fs.readFileSync(page, "utf8");
    assert.match(pageText, /instance = "Main\.FeedPump"/);
    assert.match(pageText, /tag = "FeedPump"/);
    assert.match(pageText, /running = "Main\.FeedPump\.Running"/);
    const unsaved = vscode.workspace.textDocuments.filter(
      (document) => document.isDirty && document.uri.fsPath.startsWith(root),
    );
    assert.deepStrictEqual(
      unsaved.map((document) => path.relative(root, document.uri.fsPath)),
      [],
      "nothing is left unsaved",
    );
    assert.match((await renameEquipment(vscode.Uri.file(root), "Main.P999", "X")) ?? "", /P999/);
  });
});

/** A pump of the project's own with a hardware terminal and two settings, not yet declared, so the
 * builder owns it; the program has the output it connects to. */
const OWN_PUMP_SOURCE =
  "FUNCTION_BLOCK Pump\nVAR_INPUT\n    AutoRun : BOOL;\nEND_VAR\nVAR_OUTPUT\n    RunCommand : BOOL;\n    Running : BOOL;\nEND_VAR\nVAR PUBLIC RETAIN\n    StartTimeout : TIME := T#5s;\n    ServiceInterval : LREAL := 4000.0;\nEND_VAR\n    RunCommand := AutoRun;\n    Running := AutoRun;\nEND_FUNCTION_BLOCK\n\nPROGRAM Transfer\nVAR\n    PumpRun AT %QX0.0 : BOOL;\nEND_VAR\n    PumpRun := FALSE;\nEND_PROGRAM\n\nCONFIGURATION Config\nRESOURCE Res ON PLC\nTASK Fast(INTERVAL := T#100ms, PRIORITY := 1);\nPROGRAM Main WITH Fast : Transfer;\nEND_RESOURCE\nEND_CONFIGURATION\n";
const OWN_PUMP_PAIR =
  'format = 1\nblock = "Pump"\nfaceplate = "pump"\nsymbol = "pump"\n\n[[settings]]\nmember = "StartTimeout"\ntitle = "Start timeout"\nunit = "s"\nkey = true\n\n[[settings]]\nmember = "ServiceInterval"\ntitle = "Service interval"\nunit = "h"\n\n[[io]]\nmember = "RunCommand"\ntitle = "Run command"\ndirection = "output"\n\n[program]\nposition = "actuators"\n';
