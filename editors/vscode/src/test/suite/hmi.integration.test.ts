import * as assert from "assert";
import * as vscode from "vscode";
import type { LanguageClient } from "vscode-languageclient/node";

import {
  STHmiApplyPatchTool,
  STHmiExplainWidgetTool,
  STHmiGenerateCandidatesTool,
  STHmiGetBindingsTool,
  STHmiGetLayoutTool,
  STHmiPlanIntentTool,
  STHmiPreviewSnapshotTool,
  STHmiRunJourneyTool,
  STHmiTraceCaptureTool,
  STHmiValidateTool,
  __testSetRuntimeControlRequestHandler,
} from "../../lm-tools";
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

  function delay(ms: number): Promise<void> {
    return new Promise((resolve) => setTimeout(resolve, ms));
  }

  async function waitFor(predicate: () => boolean, timeoutMs = 5000): Promise<void> {
    const start = Date.now();
    while (Date.now() - start < timeoutMs) {
      if (predicate()) {
        return;
      }
      await delay(50);
    }
    assert.ok(predicate(), `Condition not met within ${timeoutMs}ms.`);
  }

  async function appendAndSave(uri: vscode.Uri, text: string): Promise<void> {
    const document = await vscode.workspace.openTextDocument(uri);
    const editor = await vscode.window.showTextDocument(document, {
      preview: true,
      preserveFocus: true,
    });
    const line = document.lineCount;
    await editor.edit((builder) => {
      builder.insert(new vscode.Position(line, 0), text);
    });
    await document.save();
  }

  function toolResultText(result: unknown): string {
    if (typeof result === "string") {
      return result;
    }
    if (!result || typeof result !== "object") {
      return String(result);
    }
    const objectResult = result as Record<string, unknown>;
    if (typeof objectResult.text === "string") {
      return objectResult.text;
    }
    const content = objectResult.content;
    if (Array.isArray(content)) {
      for (const entry of content) {
        if (
          entry &&
          typeof entry === "object" &&
          typeof (entry as { value?: unknown }).value === "string"
        ) {
          return (entry as { value: string }).value;
        }
      }
    }
    const parts = objectResult.parts;
    if (Array.isArray(parts)) {
      for (const part of parts) {
        if (
          part &&
          typeof part === "object" &&
          typeof (part as { value?: unknown }).value === "string"
        ) {
          return (part as { value: string }).value;
        }
        if (
          part &&
          typeof part === "object" &&
          typeof (part as { text?: unknown }).text === "string"
        ) {
          return (part as { text: string }).text;
        }
      }
    }
    try {
      return JSON.stringify(result);
    } catch {
      return String(result);
    }
  }

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
    __testSetRuntimeControlRequestHandler(undefined);
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

  test("descriptor watcher refreshes open panel on hmi toml and svg changes", async () => {
    const widgetId = "resource/RESOURCE/program/Main/field/run";
    let schemaVersion = 0;
    __testSetControlRequestHandler(async (_endpoint, _auth, requestType) => {
      if (requestType === "hmi.schema.get") {
        schemaVersion += 1;
        return {
          version: schemaVersion,
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
        return {
          connected: true,
          timestamp_ms: Date.now(),
          values: {
            [widgetId]: {
              v: false,
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
    const initialVersion = __testGetHmiPanelState().schema?.version ?? 0;

    const watchDir = vscode.Uri.joinPath(fixturesRoot, `watch-${Date.now()}`, "hmi");
    await vscode.workspace.fs.createDirectory(watchDir);
    await vscode.workspace.fs.writeFile(
      vscode.Uri.joinPath(watchDir, "overview.toml"),
      Buffer.from('title = "Overview"\n', "utf8"),
    );
    await appendAndSave(
      vscode.Uri.joinPath(watchDir, "overview.toml"),
      "\n# save-trigger\n",
    );
    await waitFor(
      () => (__testGetHmiPanelState().schema?.version ?? 0) > initialVersion,
      6000,
    );

    const afterTomlVersion = __testGetHmiPanelState().schema?.version ?? 0;
    await vscode.workspace.fs.writeFile(
      vscode.Uri.joinPath(watchDir, "plant.svg"),
      Buffer.from('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"></svg>', "utf8"),
    );
    await appendAndSave(
      vscode.Uri.joinPath(watchDir, "plant.svg"),
      "\n<!-- save-trigger -->\n",
    );
    await waitFor(
      () => (__testGetHmiPanelState().schema?.version ?? 0) > afterTomlVersion,
      6000,
    );
  });

  test("panel keeps section metadata for dashboard layouts", async () => {
    const runId = "resource/RESOURCE/program/Main/field/run";
    const speedId = "resource/RESOURCE/program/Main/field/speed";
    __testSetControlRequestHandler(async (_endpoint, _auth, requestType) => {
      if (requestType === "hmi.schema.get") {
        return {
          version: 1,
          mode: "read_only",
          read_only: true,
          resource: "RESOURCE",
          generated_at_ms: Date.now(),
          pages: [
            {
              id: "overview",
              title: "Overview",
              order: 0,
              kind: "dashboard",
              sections: [
                {
                  title: "Primary",
                  span: 8,
                  widget_ids: [runId],
                },
                {
                  title: "Secondary",
                  span: 4,
                  widget_ids: [speedId],
                },
              ],
            },
          ],
          widgets: [
            {
              id: runId,
              path: "Main.run",
              label: "Run",
              data_type: "BOOL",
              access: "read",
              writable: false,
              widget: "indicator",
              source: "program:Main",
              page: "overview",
              group: "Primary",
              order: 0,
              widget_span: 6,
            },
            {
              id: speedId,
              path: "Main.speed",
              label: "Speed",
              data_type: "REAL",
              access: "read",
              writable: false,
              widget: "gauge",
              source: "program:Main",
              page: "overview",
              group: "Secondary",
              order: 1,
              widget_span: 6,
            },
          ],
        };
      }
      if (requestType === "hmi.values.get") {
        return {
          connected: true,
          timestamp_ms: Date.now(),
          values: {
            [runId]: { v: true, q: "good", ts_ms: Date.now() },
            [speedId]: { v: 42.5, q: "good", ts_ms: Date.now() },
          },
        };
      }
      throw new Error(`Unexpected request type: ${requestType}`);
    });

    await vscode.commands.executeCommand("trust-lsp.hmi.openPreview");
    await __testForceRefreshSchema();
    await __testForcePollValues();

    const state = __testGetHmiPanelState();
    const page = state.schema?.pages.find((entry) => entry.id === "overview");
    assert.ok(page, "Expected overview page.");
    assert.strictEqual(page?.kind, "dashboard");
    assert.strictEqual(page?.sections?.length, 2);
    assert.strictEqual(state.values?.values[runId]?.v, true);
    assert.strictEqual(typeof state.values?.values[speedId]?.v, "number");
  });

  test("panel process page loads local svg asset and keeps bindings metadata", async () => {
    const workspaceFolder = vscode.workspace.workspaceFolders?.[0];
    assert.ok(workspaceFolder, "Expected workspace folder.");
    const hmiDir = vscode.Uri.joinPath(workspaceFolder.uri, "hmi");
    const svgName = `panel-process-${Date.now()}.svg`;
    const svgUri = vscode.Uri.joinPath(hmiDir, svgName);
    const levelId = "resource/RESOURCE/program/Main/field/level";
    try {
      await vscode.workspace.fs.createDirectory(hmiDir);
      await vscode.workspace.fs.writeFile(
        svgUri,
        Buffer.from(
          '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"><rect id="tankFill" x="10" y="10" width="80" height="20" /></svg>',
          "utf8",
        ),
      );

      __testSetControlRequestHandler(async (_endpoint, _auth, requestType) => {
        if (requestType === "hmi.schema.get") {
          return {
            version: 1,
            mode: "read_only",
            read_only: true,
            resource: "RESOURCE",
            generated_at_ms: Date.now(),
            pages: [
              {
                id: "process",
                title: "Process",
                order: 0,
                kind: "process",
                svg: svgName,
                bindings: [
                  {
                    selector: "#tankFill",
                    attribute: "height",
                    source: "Main.level",
                    scale: { min: 0, max: 100, output_min: 10, output_max: 90 },
                  },
                ],
              },
            ],
            widgets: [
              {
                id: levelId,
                path: "Main.level",
                label: "Level",
                data_type: "REAL",
                access: "read",
                writable: false,
                widget: "gauge",
                source: "program:Main",
                page: "process",
                group: "Process",
                order: 0,
              },
            ],
          };
        }
        if (requestType === "hmi.values.get") {
          return {
            connected: true,
            timestamp_ms: Date.now(),
            values: {
              [levelId]: { v: 55.0, q: "good", ts_ms: Date.now() },
            },
          };
        }
        throw new Error(`Unexpected request type: ${requestType}`);
      });

      await vscode.commands.executeCommand("trust-lsp.hmi.openPreview");
      await __testForceRefreshSchema();
      await __testForcePollValues();

      const state = __testGetHmiPanelState();
      const page = state.schema?.pages.find((entry) => entry.id === "process") as
        | { kind?: string; svg_content?: string | null; bindings?: unknown[] }
        | undefined;
      assert.ok(page, "Expected process page in schema.");
      assert.strictEqual(page?.kind, "process");
      assert.strictEqual(Array.isArray(page?.bindings), true);
      assert.ok(
        typeof page?.svg_content === "string" && page.svg_content.includes("<svg"),
        "Expected hydrated process SVG content.",
      );
      assert.strictEqual(typeof state.values?.values[levelId]?.v, "number");
    } finally {
      try {
        await vscode.workspace.fs.delete(svgUri, { useTrash: false });
      } catch {
        // Ignore cleanup failures for temporary SVG fixtures.
      }
    }
  });

  test("LM HMI tools provide layout snapshot and dry-run patch conflicts", async () => {
    const workspaceFolder = vscode.workspace.workspaceFolders?.[0];
    assert.ok(workspaceFolder, "Expected workspace folder.");
    const rootPath = workspaceFolder.uri.fsPath;
    const layoutTool = new STHmiGetLayoutTool();
    const patchTool = new STHmiApplyPatchTool();
    const tokenSource = new vscode.CancellationTokenSource();

    const layoutResult = await layoutTool.invoke(
      { input: { rootPath } },
      tokenSource.token,
    );
    const layoutPayload = JSON.parse(toolResultText(layoutResult));
    assert.strictEqual(typeof layoutPayload.exists, "boolean");
    if (layoutPayload.exists) {
      assert.ok(Array.isArray(layoutPayload.files), "Expected files array");
    }

    const invalidPatch = await patchTool.invoke(
      {
        input: {
          dry_run: true,
          rootPath,
          operations: [
            {
              op: "replace",
              path: "/invalid",
              value: 'title = "Invalid"\n',
            },
          ],
        },
      },
      tokenSource.token,
    );
    const invalidPayload = JSON.parse(toolResultText(invalidPatch));
    assert.strictEqual(invalidPayload.ok, false);
    assert.ok(
      Array.isArray(invalidPayload.conflicts) &&
        invalidPayload.conflicts.some(
          (entry: { code?: string }) => entry.code === "HMI_PATCH_INVALID_PATH",
        ),
      "Expected invalid path conflict code",
    );

    const testName = `hmi-tool-dry-run-${Date.now()}.toml`;
    const validPatch = await patchTool.invoke(
      {
        input: {
          dry_run: true,
          rootPath,
          operations: [
            {
              op: "add",
              path: `/files/${testName}`,
              value: 'title = "Dry Run"\n',
            },
          ],
        },
      },
      tokenSource.token,
    );
    const validPayload = JSON.parse(toolResultText(validPatch));
    assert.strictEqual(validPayload.ok, true);
    assert.ok(
      Array.isArray(validPayload.changes) &&
        validPayload.changes.some(
          (entry: { file?: string; action?: string }) =>
            entry.file === `hmi/${testName}` && entry.action === "add",
        ),
      "Expected dry-run add change",
    );
  });

  test("LM HMI get_bindings routes workspace executeCommand and validates inputs", async () => {
    const workspaceFolder = vscode.workspace.workspaceFolders?.[0];
    assert.ok(workspaceFolder, "Expected workspace folder.");
    const requests: Array<{ method: string; params: unknown }> = [];
    const mockClient = {
      sendRequest: async (method: string, params: unknown) => {
        requests.push({ method, params });
        return { ok: true, programs: [], globals: [] };
      },
    };
    const tool = new STHmiGetBindingsTool(
      () => mockClient as unknown as LanguageClient,
    );
    const tokenSource = new vscode.CancellationTokenSource();

    const result = await tool.invoke(
      { input: { rootPath: workspaceFolder.uri.fsPath } },
      tokenSource.token,
    );
    const payload = JSON.parse(toolResultText(result));
    assert.strictEqual(payload.command, "trust-lsp.hmiBindings");
    assert.strictEqual(payload.result?.ok, true);
    assert.strictEqual(requests.length, 1);
    assert.strictEqual(requests[0].method, "workspace/executeCommand");

    const requestParams = requests[0].params as {
      command?: string;
      arguments?: Array<Record<string, unknown>>;
    };
    assert.strictEqual(requestParams.command, "trust-lsp.hmiBindings");
    assert.ok(
      Array.isArray(requestParams.arguments) &&
        requestParams.arguments.length === 1 &&
        typeof requestParams.arguments[0].root_uri === "string",
    );

    const invalid = await tool.invoke(
      { input: { rootPath: "relative/path" } },
      tokenSource.token,
    );
    const invalidText = toolResultText(invalid);
    assert.ok(
      invalidText.includes("Error: rootPath must be an absolute path or URI."),
      `Expected rootPath validation error, got: ${invalidText}`,
    );
  });

  test("LM HMI Phase 6 plan_intent writes deterministic _intent.toml", async () => {
    const workspaceFolder = vscode.workspace.workspaceFolders?.[0];
    assert.ok(workspaceFolder, "Expected workspace folder.");
    const rootPath = workspaceFolder.uri.fsPath;
    const hmiRoot = vscode.Uri.joinPath(workspaceFolder.uri, "hmi");
    await vscode.workspace.fs.createDirectory(hmiRoot);
    const intentUri = vscode.Uri.joinPath(hmiRoot, "_intent.toml");
    try {
      await vscode.workspace.fs.delete(intentUri, { useTrash: false });
    } catch {
      // Ignore missing file in setup.
    }

    const tool = new STHmiPlanIntentTool();
    const tokenSource = new vscode.CancellationTokenSource();
    const input = {
      rootPath,
      summary: "Optimize alarm salience and operator response time.",
      goals: ["Reduce alarm acknowledgement latency", "Minimize navigation depth"],
      personas: ["Shift operator", "Maintenance technician"],
      kpis: ["alarm_ack_ms", "journey_clicks"],
      priorities: ["safety", "alarm_salience", "latency"],
      constraints: ["writes require authz", "respect write allowlist"],
    };

    const dryRunResult = await tool.invoke(
      { input: { ...input, dry_run: true } },
      tokenSource.token,
    );
    const dryRunPayload = JSON.parse(toolResultText(dryRunResult));
    assert.strictEqual(dryRunPayload.ok, true);
    assert.strictEqual(dryRunPayload.dry_run, true);
    assert.strictEqual(typeof dryRunPayload.content, "string");
    assert.ok(
      String(dryRunPayload.content).includes("[intent]"),
      "Expected intent TOML payload in dry run.",
    );

    const writeResult = await tool.invoke(
      { input: { ...input, dry_run: false } },
      tokenSource.token,
    );
    const writePayload = JSON.parse(toolResultText(writeResult));
    assert.strictEqual(writePayload.ok, true);
    assert.strictEqual(writePayload.dry_run, false);
    assert.strictEqual(writePayload.existed, false);
    assert.strictEqual(writePayload.changed, true);

    const written = Buffer.from(await vscode.workspace.fs.readFile(intentUri)).toString("utf8");
    assert.strictEqual(
      written,
      writePayload.content,
      "Written _intent.toml must match tool content output.",
    );

    const secondWrite = await tool.invoke(
      { input: { ...input, dry_run: false } },
      tokenSource.token,
    );
    const secondPayload = JSON.parse(toolResultText(secondWrite));
    assert.strictEqual(secondPayload.ok, true);
    assert.strictEqual(secondPayload.existed, true);
    assert.strictEqual(secondPayload.changed, false);
  });

  test("LM HMI Phase 6 validate emits _lock.json evidence and prunes retention", async () => {
    const workspaceFolder = vscode.workspace.workspaceFolders?.[0];
    assert.ok(workspaceFolder, "Expected workspace folder.");
    const rootPath = workspaceFolder.uri.fsPath;
    const hmiRoot = vscode.Uri.joinPath(workspaceFolder.uri, "hmi");
    await vscode.workspace.fs.createDirectory(hmiRoot);

    await vscode.workspace.fs.writeFile(
      vscode.Uri.joinPath(hmiRoot, "_config.toml"),
      Buffer.from(
        [
          "[write]",
          "enabled = false",
          "",
        ].join("\n"),
        "utf8",
      ),
    );
    await vscode.workspace.fs.writeFile(
      vscode.Uri.joinPath(hmiRoot, "overview.toml"),
      Buffer.from(
        [
          'title = "Overview"',
          "",
          "[[section]]",
          'title = "Main"',
          "span = 12",
          "",
          "[[section.widget]]",
          'type = "indicator"',
          'bind = "Main.run"',
          'label = "Run"',
          "",
        ].join("\n"),
        "utf8",
      ),
    );

    const evidenceRoot = vscode.Uri.joinPath(hmiRoot, "_evidence");
    await vscode.workspace.fs.createDirectory(evidenceRoot);
    for (const run of [
      "2026-02-13T11-59-00Z",
      "2026-02-13T11-59-01Z",
      "2026-02-13T11-59-02Z",
    ]) {
      const runUri = vscode.Uri.joinPath(evidenceRoot, run);
      await vscode.workspace.fs.createDirectory(runUri);
      await vscode.workspace.fs.writeFile(
        vscode.Uri.joinPath(runUri, "validation.json"),
        Buffer.from('{"ok":true}\n', "utf8"),
      );
    }

    const mockClient = {
      sendRequest: async (method: string, params: unknown) => {
        assert.strictEqual(method, "workspace/executeCommand");
        const commandParams = params as {
          command?: string;
          arguments?: Array<Record<string, unknown>>;
        };
        assert.strictEqual(commandParams.command, "trust-lsp.hmiBindings");
        assert.ok(Array.isArray(commandParams.arguments));
        return {
          ok: true,
          programs: [
            {
              name: "Main",
              variables: [
                {
                  name: "run",
                  path: "Main.run",
                  type: "BOOL",
                  qualifier: "VAR_OUTPUT",
                  writable: false,
                },
              ],
            },
          ],
          globals: [],
        };
      },
    };
    const tool = new STHmiValidateTool(
      () => mockClient as unknown as LanguageClient,
    );
    const tokenSource = new vscode.CancellationTokenSource();

    const dryRunResult = await tool.invoke(
      {
        input: {
          rootPath,
          dry_run: true,
        },
      },
      tokenSource.token,
    );
    const dryRunPayload = JSON.parse(toolResultText(dryRunResult));
    assert.strictEqual(dryRunPayload.dry_run, true);
    assert.strictEqual(typeof dryRunPayload.ok, "boolean");
    assert.strictEqual(dryRunPayload.evidencePath, null);

    const writeResult = await tool.invoke(
      {
        input: {
          rootPath,
          dry_run: false,
          prune: true,
          retain_runs: 2,
        },
      },
      tokenSource.token,
    );
    const writePayload = JSON.parse(toolResultText(writeResult));
    assert.strictEqual(writePayload.ok, true, `validate payload: ${JSON.stringify(writePayload)}`);
    assert.strictEqual(writePayload.dry_run, false);
    assert.strictEqual(writePayload.prune, true);
    assert.strictEqual(writePayload.lockPath, "hmi/_lock.json");
    assert.ok(
      typeof writePayload.evidencePath === "string" &&
        writePayload.evidencePath.startsWith("hmi/_evidence/"),
      "Expected evidencePath in non-dry-run validation.",
    );

    const lockUri = vscode.Uri.joinPath(hmiRoot, "_lock.json");
    const lockContent = Buffer.from(await vscode.workspace.fs.readFile(lockUri)).toString("utf8");
    const lockPayload = JSON.parse(lockContent);
    assert.strictEqual(lockPayload.version, 1);
    assert.ok(Array.isArray(lockPayload.widgets) && lockPayload.widgets.length >= 1);
    assert.strictEqual(lockPayload.widgets[0].path, "Main.run");
    assert.strictEqual(typeof lockPayload.widgets[0].binding_fingerprint, "string");

    const evidenceEntries = await vscode.workspace.fs.readDirectory(evidenceRoot);
    const evidenceRuns = evidenceEntries
      .filter(([, kind]) => kind === vscode.FileType.Directory)
      .map(([name]) => name)
      .sort();
    assert.ok(
      evidenceRuns.length <= 2,
      `Expected prune retention <=2 runs, got ${evidenceRuns.length}: ${evidenceRuns.join(", ")}`,
    );
    const latestRun = writePayload.evidencePath.slice("hmi/_evidence/".length);
    const latestUri = vscode.Uri.joinPath(evidenceRoot, latestRun);
    const validationUri = vscode.Uri.joinPath(latestUri, "validation.json");
    const journeysUri = vscode.Uri.joinPath(latestUri, "journeys.json");
    const validationContent = Buffer.from(await vscode.workspace.fs.readFile(validationUri)).toString("utf8");
    const journeysContent = Buffer.from(await vscode.workspace.fs.readFile(journeysUri)).toString("utf8");
    const validationPayload = JSON.parse(validationContent);
    const journeysPayload = JSON.parse(journeysContent);
    assert.strictEqual(typeof validationPayload.ok, "boolean");
    assert.ok(Array.isArray(validationPayload.checks));
    assert.ok(Array.isArray(journeysPayload.journeys));

    const secondWriteResult = await tool.invoke(
      {
        input: {
          rootPath,
          dry_run: false,
          prune: false,
        },
      },
      tokenSource.token,
    );
    const secondWritePayload = JSON.parse(toolResultText(secondWriteResult));
    assert.strictEqual(secondWritePayload.ok, true);
    const secondLockContent = Buffer.from(await vscode.workspace.fs.readFile(lockUri)).toString("utf8");
    assert.strictEqual(
      secondLockContent,
      lockContent,
      "Lock output must remain byte-stable across repeated validate runs with unchanged inputs.",
    );
  });

  test("LM HMI Phase 6 generate_candidates is deterministic and writes candidates evidence", async () => {
    const workspaceFolder = vscode.workspace.workspaceFolders?.[0];
    assert.ok(workspaceFolder, "Expected workspace folder.");
    const rootPath = workspaceFolder.uri.fsPath;
    const hmiRoot = vscode.Uri.joinPath(workspaceFolder.uri, "hmi");
    await vscode.workspace.fs.createDirectory(hmiRoot);
    await vscode.workspace.fs.writeFile(
      vscode.Uri.joinPath(hmiRoot, "_intent.toml"),
      Buffer.from(
        [
          "version = 1",
          "",
          "[intent]",
          'summary = "Candidate ranking determinism"',
          'priorities = ["safety", "alarm_salience", "latency"]',
          "",
        ].join("\n"),
        "utf8",
      ),
    );
    await vscode.workspace.fs.writeFile(
      vscode.Uri.joinPath(hmiRoot, "overview.toml"),
      Buffer.from(
        [
          'title = "Overview"',
          "",
          "[[section]]",
          'title = "Main"',
          "span = 12",
          "",
          "[[section.widget]]",
          'type = "indicator"',
          'bind = "Main.run"',
          "",
          "[[section.widget]]",
          'type = "gauge"',
          'bind = "Main.speed"',
          "",
        ].join("\n"),
        "utf8",
      ),
    );

    const mockClient = {
      sendRequest: async () => ({
        ok: true,
        programs: [
          {
            name: "Main",
            variables: [
              { path: "Main.run", type: "BOOL", qualifier: "VAR_OUTPUT", writable: false },
              { path: "Main.speed", type: "REAL", qualifier: "VAR_OUTPUT", writable: false },
            ],
          },
        ],
        globals: [],
      }),
    };
    const tool = new STHmiGenerateCandidatesTool(
      () => mockClient as unknown as LanguageClient,
    );
    const tokenSource = new vscode.CancellationTokenSource();

    const firstDryRun = await tool.invoke(
      {
        input: {
          rootPath,
          dry_run: true,
          candidate_count: 3,
        },
      },
      tokenSource.token,
    );
    const firstPayload = JSON.parse(toolResultText(firstDryRun));
    const secondDryRun = await tool.invoke(
      {
        input: {
          rootPath,
          dry_run: true,
          candidate_count: 3,
        },
      },
      tokenSource.token,
    );
    const secondPayload = JSON.parse(toolResultText(secondDryRun));
    assert.strictEqual(firstPayload.ok, true);
    assert.strictEqual(secondPayload.ok, true);
    assert.deepStrictEqual(
      secondPayload.candidates,
      firstPayload.candidates,
      "Candidate ranking/output must be deterministic for identical inputs.",
    );

    const runId = "2026-02-13T12-34-56Z";
    const writeResult = await tool.invoke(
      {
        input: {
          rootPath,
          dry_run: false,
          run_id: runId,
          candidate_count: 3,
        },
      },
      tokenSource.token,
    );
    const writePayload = JSON.parse(toolResultText(writeResult));
    assert.strictEqual(writePayload.ok, true);
    assert.strictEqual(writePayload.dry_run, false);
    assert.strictEqual(writePayload.run_id, runId);
    const candidatesUri = vscode.Uri.joinPath(
      hmiRoot,
      "_evidence",
      runId,
      "candidates.json",
    );
    const candidatesContent = Buffer.from(
      await vscode.workspace.fs.readFile(candidatesUri),
    ).toString("utf8");
    const candidatesPayload = JSON.parse(candidatesContent);
    assert.ok(Array.isArray(candidatesPayload.candidates));
    assert.ok(candidatesPayload.candidates.length >= 1);
  });

  test("LM HMI Phase 6 trace_capture writes scenario traces and preview_snapshot writes viewport artifacts", async () => {
    const workspaceFolder = vscode.workspace.workspaceFolders?.[0];
    assert.ok(workspaceFolder, "Expected workspace folder.");
    const rootPath = workspaceFolder.uri.fsPath;
    const hmiRoot = vscode.Uri.joinPath(workspaceFolder.uri, "hmi");
    await vscode.workspace.fs.createDirectory(hmiRoot);
    await vscode.workspace.fs.writeFile(
      vscode.Uri.joinPath(hmiRoot, "overview.toml"),
      Buffer.from(
        [
          'title = "Overview"',
          "",
          "[[section]]",
          'title = "Main"',
          "span = 12",
          "",
          "[[section.widget]]",
          'type = "indicator"',
          'bind = "Main.run"',
          "",
        ].join("\n"),
        "utf8",
      ),
    );
    const mockClient = {
      sendRequest: async () => ({
        ok: true,
        programs: [
          {
            name: "Main",
            variables: [{ path: "Main.run", type: "BOOL", qualifier: "VAR_OUTPUT", writable: false }],
          },
        ],
        globals: [],
      }),
    };

    let valueTick = 0;
    __testSetRuntimeControlRequestHandler(
      async (_endpoint, _auth, requestType, params) => {
        if (requestType === "hmi.schema.get") {
          return {
            version: 1,
            mode: "read_only",
            read_only: true,
            pages: [{ id: "overview", title: "Overview", order: 0 }],
            widgets: [
              {
                id: "resource/resource/program/main/field/run",
                path: "Main.run",
                label: "Run",
                data_type: "BOOL",
                writable: false,
                page: "overview",
                group: "Main",
              },
            ],
          };
        }
        if (requestType === "hmi.values.get") {
          valueTick += 1;
          const ids =
            typeof params === "object" &&
            params !== null &&
            Array.isArray((params as { ids?: unknown }).ids)
              ? ((params as { ids: string[] }).ids)
              : [];
          const values: Record<string, { v: unknown; q: string; ts_ms: number }> = {};
          for (const id of ids) {
            values[id] = {
              v: valueTick % 2 === 0,
              q: "good",
              ts_ms: Date.now(),
            };
          }
          return {
            connected: true,
            timestamp_ms: Date.now(),
            values,
          };
        }
        throw new Error(`Unexpected request type: ${requestType}`);
      },
    );

    const traceTool = new STHmiTraceCaptureTool();
    const tokenSource = new vscode.CancellationTokenSource();
    const traceRun = "2026-02-13T12-40-00Z";
    const traceResult = await traceTool.invoke(
      {
        input: {
          rootPath,
          dry_run: false,
          run_id: traceRun,
          scenario: "fault",
          samples: 3,
          sample_interval_ms: 10,
        },
      },
      tokenSource.token,
    );
    const tracePayload = JSON.parse(toolResultText(traceResult));
    assert.strictEqual(tracePayload.ok, true);
    assert.strictEqual(tracePayload.run_id, traceRun);
    assert.ok(Array.isArray(tracePayload.samples));
    assert.strictEqual(tracePayload.samples.length, 3);
    const traceUri = vscode.Uri.joinPath(
      hmiRoot,
      "_evidence",
      traceRun,
      "trace-fault.json",
    );
    const traceContent = Buffer.from(await vscode.workspace.fs.readFile(traceUri)).toString("utf8");
    const traceFilePayload = JSON.parse(traceContent);
    assert.strictEqual(traceFilePayload.scenario, "fault");

    const candidatesTool = new STHmiGenerateCandidatesTool(
      () => mockClient as unknown as LanguageClient,
    );
    const candidateRun = "2026-02-13T12-41-00Z";
    const candidatesResult = await candidatesTool.invoke(
      {
        input: {
          rootPath,
          dry_run: false,
          run_id: candidateRun,
          candidate_count: 2,
        },
      },
      tokenSource.token,
    );
    const candidatesPayload = JSON.parse(toolResultText(candidatesResult));
    assert.strictEqual(candidatesPayload.ok, true);
    assert.ok(Array.isArray(candidatesPayload.candidates));

    const previewTool = new STHmiPreviewSnapshotTool(
      () => mockClient as unknown as LanguageClient,
    );
    const previewResult = await previewTool.invoke(
      {
        input: {
          rootPath,
          dry_run: false,
          run_id: candidateRun,
          candidate_id: candidatesPayload.candidates[0].id,
          viewports: ["desktop", "mobile"],
        },
      },
      tokenSource.token,
    );
    const previewPayload = JSON.parse(toolResultText(previewResult));
    assert.strictEqual(previewPayload.ok, true);
    assert.strictEqual(previewPayload.files.length, 2);
    for (const file of previewPayload.files as Array<{ path: string }>) {
      const fileUri = vscode.Uri.joinPath(workspaceFolder.uri, ...file.path.split("/"));
      const fileContent = Buffer.from(await vscode.workspace.fs.readFile(fileUri)).toString("utf8");
      assert.ok(fileContent.includes("<svg"), `Expected SVG snapshot in ${file.path}`);
    }
  });

  test("LM HMI Phase 6 run_journey executes API/event flow and explain_widget reports provenance", async () => {
    const workspaceFolder = vscode.workspace.workspaceFolders?.[0];
    assert.ok(workspaceFolder, "Expected workspace folder.");
    const rootPath = workspaceFolder.uri.fsPath;
    const hmiRoot = vscode.Uri.joinPath(workspaceFolder.uri, "hmi");
    await vscode.workspace.fs.createDirectory(hmiRoot);
    await vscode.workspace.fs.writeFile(
      vscode.Uri.joinPath(hmiRoot, "_config.toml"),
      Buffer.from(
        [
          "[write]",
          "enabled = true",
          'allow = ["resource/resource/program/main/field/other"]',
          "",
        ].join("\n"),
        "utf8",
      ),
    );
    await vscode.workspace.fs.writeFile(
      vscode.Uri.joinPath(hmiRoot, "overview.toml"),
      Buffer.from(
        [
          'title = "Overview"',
          "",
          "[[section]]",
          'title = "Main"',
          "span = 12",
          "",
          "[[section.widget]]",
          'type = "indicator"',
          'bind = "Main.run"',
          "",
        ].join("\n"),
        "utf8",
      ),
    );

    const runtimeRequests: string[] = [];
    __testSetRuntimeControlRequestHandler(
      async (_endpoint, _auth, requestType) => {
        runtimeRequests.push(requestType);
        if (requestType === "hmi.schema.get") {
          return {
            version: 1,
            mode: "read_write",
            read_only: false,
            pages: [{ id: "overview", title: "Overview", order: 0 }],
            widgets: [
              {
                id: "resource/resource/program/main/field/run",
                path: "Main.run",
                label: "Run",
                data_type: "BOOL",
                writable: true,
                page: "overview",
                group: "Main",
              },
            ],
          };
        }
        if (requestType === "hmi.values.get") {
          return {
            connected: true,
            timestamp_ms: Date.now(),
            values: {
              "resource/resource/program/main/field/run": {
                v: false,
                q: "good",
                ts_ms: Date.now(),
              },
            },
          };
        }
        if (requestType === "hmi.write") {
          throw new Error("Unexpected runtime write call");
        }
        throw new Error(`Unexpected request type: ${requestType}`);
      },
    );

    const journeyTool = new STHmiRunJourneyTool();
    const tokenSource = new vscode.CancellationTokenSource();
    const runId = "2026-02-13T12-42-00Z";
    const journeyResult = await journeyTool.invoke(
      {
        input: {
          rootPath,
          dry_run: false,
          run_id: runId,
          scenario: "fault-recovery",
          journeys: [
            {
              id: "fault_recovery",
              steps: [
                { action: "read_values", ids: ["resource/resource/program/main/field/run"] },
                {
                  action: "write",
                  widget_id: "resource/resource/program/main/field/run",
                  value: true,
                  expect_error_code: "HMI_JOURNEY_WRITE_NOT_ALLOWLISTED",
                },
                { action: "wait", duration_ms: 10 },
              ],
            },
          ],
        },
      },
      tokenSource.token,
    );
    const journeyPayload = JSON.parse(toolResultText(journeyResult));
    assert.strictEqual(journeyPayload.ok, true, JSON.stringify(journeyPayload));
    assert.ok(
      Array.isArray(journeyPayload.journeys) &&
        journeyPayload.journeys.length === 1,
      "Expected one executed journey result.",
    );
    const firstStep = journeyPayload.journeys[0]?.steps?.[1];
    assert.strictEqual(firstStep?.status, "passed");
    assert.strictEqual(firstStep?.code, "HMI_JOURNEY_WRITE_NOT_ALLOWLISTED");
    assert.ok(runtimeRequests.includes("hmi.values.get"));
    assert.ok(!runtimeRequests.includes("hmi.write"));
    const journeysUri = vscode.Uri.joinPath(hmiRoot, "_evidence", runId, "journeys.json");
    const journeysPayload = JSON.parse(
      Buffer.from(await vscode.workspace.fs.readFile(journeysUri)).toString("utf8"),
    );
    assert.strictEqual(journeysPayload.scenario, "fault-recovery");

    const mockClient = {
      sendRequest: async () => ({
        ok: true,
        programs: [
          {
            name: "Main",
            variables: [
              {
                path: "Main.run",
                type: "BOOL",
                qualifier: "VAR_OUTPUT",
                writable: true,
              },
            ],
          },
        ],
        globals: [],
      }),
    };
    const explainTool = new STHmiExplainWidgetTool(
      () => mockClient as unknown as LanguageClient,
    );
    const explainResult = await explainTool.invoke(
      {
        input: {
          rootPath,
          path: "Main.run",
        },
      },
      tokenSource.token,
    );
    const explainPayload = JSON.parse(toolResultText(explainResult));
    assert.strictEqual(explainPayload.ok, true);
    assert.strictEqual(explainPayload.widget.path, "Main.run");
    assert.strictEqual(explainPayload.provenance.write_policy.enabled, true);
    assert.strictEqual(explainPayload.provenance.write_policy.allowlisted, false);
  });

  test("LM HMI tools honor cancellation tokens", async () => {
    const workspaceFolder = vscode.workspace.workspaceFolders?.[0];
    assert.ok(workspaceFolder, "Expected workspace folder.");
    const layoutTool = new STHmiGetLayoutTool();
    const patchTool = new STHmiApplyPatchTool();
    const intentTool = new STHmiPlanIntentTool();
    const traceTool = new STHmiTraceCaptureTool();
    const candidateTool = new STHmiGenerateCandidatesTool(() => undefined);
    const previewTool = new STHmiPreviewSnapshotTool(() => undefined);
    const journeyTool = new STHmiRunJourneyTool();
    const explainTool = new STHmiExplainWidgetTool(() => undefined);
    const validateTool = new STHmiValidateTool(() => undefined);
    const bindingsTool = new STHmiGetBindingsTool(() => undefined);
    const tokenSource = new vscode.CancellationTokenSource();
    tokenSource.cancel();

    const layoutResult = await layoutTool.invoke(
      { input: { rootPath: workspaceFolder.uri.fsPath } },
      tokenSource.token,
    );
    assert.strictEqual(toolResultText(layoutResult), "Cancelled.");

    const patchResult = await patchTool.invoke(
      {
        input: {
          dry_run: true,
          rootPath: workspaceFolder.uri.fsPath,
          operations: [{ op: "add", path: "/files/cancel.toml", value: "title = \"x\"\n" }],
        },
      },
      tokenSource.token,
    );
    assert.strictEqual(toolResultText(patchResult), "Cancelled.");

    const intentResult = await intentTool.invoke(
      { input: { rootPath: workspaceFolder.uri.fsPath } },
      tokenSource.token,
    );
    assert.strictEqual(toolResultText(intentResult), "Cancelled.");

    const traceResult = await traceTool.invoke(
      { input: { rootPath: workspaceFolder.uri.fsPath } },
      tokenSource.token,
    );
    assert.strictEqual(toolResultText(traceResult), "Cancelled.");

    const candidateResult = await candidateTool.invoke(
      { input: { rootPath: workspaceFolder.uri.fsPath } },
      tokenSource.token,
    );
    assert.strictEqual(toolResultText(candidateResult), "Cancelled.");

    const previewResult = await previewTool.invoke(
      { input: { rootPath: workspaceFolder.uri.fsPath } },
      tokenSource.token,
    );
    assert.strictEqual(toolResultText(previewResult), "Cancelled.");

    const journeyResult = await journeyTool.invoke(
      { input: { rootPath: workspaceFolder.uri.fsPath } },
      tokenSource.token,
    );
    assert.strictEqual(toolResultText(journeyResult), "Cancelled.");

    const explainResult = await explainTool.invoke(
      { input: { rootPath: workspaceFolder.uri.fsPath } },
      tokenSource.token,
    );
    assert.strictEqual(toolResultText(explainResult), "Cancelled.");

    const validateResult = await validateTool.invoke(
      { input: { rootPath: workspaceFolder.uri.fsPath } },
      tokenSource.token,
    );
    assert.strictEqual(toolResultText(validateResult), "Cancelled.");

    const bindingsResult = await bindingsTool.invoke(
      { input: { rootPath: workspaceFolder.uri.fsPath } },
      tokenSource.token,
    );
    assert.strictEqual(toolResultText(bindingsResult), "Cancelled.");
  });

});
