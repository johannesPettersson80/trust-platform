import * as assert from "assert";
import * as vscode from "vscode";

import {
  __testForcePollValues,
  __testForceRefreshSchema,
  __testGetHmiPanelState,
  __testLoadLayoutOverrides,
  __testResetHmiPanelState,
  __testResolveWidgetLocation,
  __testSaveLayoutPayload,
  __testSetControlRequestHandler,
  HmiWidgetSchema,
} from "../../hmiPanel";

suite("HMI preview integration (VS Code)", function () {
  this.timeout(30000);

  let fixturesRoot: vscode.Uri;

  suiteSetup(async () => {
    const workspaceFolder = vscode.workspace.workspaceFolders?.[0];
    assert.ok(workspaceFolder, "Expected workspace folder for extension tests.");
    fixturesRoot = vscode.Uri.joinPath(workspaceFolder.uri, "tmp", "vscode-hmi-preview");
    await vscode.workspace.fs.createDirectory(fixturesRoot);
  });

  suiteTeardown(async () => {
    __testResetHmiPanelState();
    try {
      await vscode.workspace.fs.delete(fixturesRoot, {
        recursive: true,
        useTrash: false,
      });
    } catch {
      // Ignore cleanup failures in test teardown.
    }
  });

  teardown(() => {
    __testSetControlRequestHandler(undefined);
    __testResetHmiPanelState();
  });

  test("panel open + schema/value refresh pipeline", async () => {
    const widgetId = "resource/RESOURCE/program/Main/field/run";
    let pollCount = 0;
    __testSetControlRequestHandler(async (_endpoint, _auth, requestType) => {
      if (requestType === "hmi.schema.get") {
        return {
          version: 1,
          mode: "read_only",
          read_only: true,
          resource: "RESOURCE",
          generated_at_ms: Date.now(),
          pages: [{ id: "overview", title: "Overview", order: 0 }],
          widgets: [
            {
              id: widgetId,
              path: "Main.run",
              label: "Run",
              data_type: "BOOL",
              access: "read",
              writable: false,
              widget: "indicator",
              source: "program:Main",
              page: "overview",
              group: "General",
              order: 0,
            },
          ],
        };
      }
      if (requestType === "hmi.values.get") {
        pollCount += 1;
        return {
          connected: true,
          timestamp_ms: Date.now(),
          freshness_ms: 0,
          values: {
            [widgetId]: {
              v: pollCount % 2 === 0,
              q: "good",
              ts_ms: Date.now(),
            },
          },
        };
      }
      throw new Error(`Unexpected request type: ${requestType}`);
    });

    await vscode.commands.executeCommand("trust-lsp.hmi.openPreview");
    await __testForceRefreshSchema();

    let state = __testGetHmiPanelState();
    assert.ok(state.hasPanel, "Expected HMI preview panel to be open.");
    assert.strictEqual(state.schema?.widgets.length, 1, "Expected one widget in schema.");

    await __testForcePollValues();
    const first = __testGetHmiPanelState().values?.values[widgetId]?.v as boolean | undefined;
    await __testForcePollValues();
    const second = __testGetHmiPanelState().values?.values[widgetId]?.v as boolean | undefined;
    assert.notStrictEqual(first, undefined, "Expected first polled value.");
    assert.notStrictEqual(second, undefined, "Expected second polled value.");
    assert.notStrictEqual(first, second, "Expected value updates on subsequent poll.");
  });

  test("widget navigation resolves declaration location", async () => {
    const sources = vscode.Uri.joinPath(fixturesRoot, "sources");
    await vscode.workspace.fs.createDirectory(sources);
    const sourceFile = vscode.Uri.joinPath(sources, "NavigationMain.st");
    const text = [
      "PROGRAM Main",
      "VAR",
      "    run : BOOL := FALSE;",
      "END_VAR",
      "END_PROGRAM",
      "",
    ].join("\n");
    await vscode.workspace.fs.writeFile(sourceFile, Buffer.from(text, "utf8"));

    const widget: HmiWidgetSchema = {
      id: "resource/RESOURCE/program/Main/field/run",
      path: "Main.run",
      label: "Run",
      data_type: "BOOL",
      access: "read",
      writable: false,
      widget: "indicator",
      source: "program:Main",
      page: "overview",
      group: "General",
      order: 0,
    };

    const location = await __testResolveWidgetLocation(widget);
    assert.ok(location, "Expected navigation location for Main.run.");
    assert.strictEqual(location?.uri.fsPath, sourceFile.fsPath);
    assert.strictEqual(location?.range.start.line, 2);
  });

  test("layout persistence accepts valid payload and rejects invalid page IDs", async () => {
    const valid = {
      widgets: {
        "Main.run": {
          label: "Run Command",
          page: "overview",
          group: "Controls",
          order: 10,
        },
      },
    };
    await __testSaveLayoutPayload(fixturesRoot, valid);

    const loaded = await __testLoadLayoutOverrides(fixturesRoot);
    assert.deepStrictEqual(loaded["Main.run"], {
      label: "Run Command",
      page: "overview",
      group: "Controls",
      order: 10,
    });

    await assert.rejects(
      __testSaveLayoutPayload(fixturesRoot, {
        widgets: {
          "Main.run": {
            page: "bad page",
          },
        },
      })
    );

    const unchanged = await __testLoadLayoutOverrides(fixturesRoot);
    assert.deepStrictEqual(unchanged["Main.run"], loaded["Main.run"]);
  });
});

