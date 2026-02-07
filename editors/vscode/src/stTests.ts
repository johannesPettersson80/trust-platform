import * as fs from "fs";
import * as path from "path";
import { spawn } from "child_process";
import * as vscode from "vscode";
import { getBinaryPath } from "./binary";

const ST_SOURCE_GLOB = "**/*.{st,ST,pou,POU}";
const ST_SOURCE_EXCLUDE = "**/{node_modules,target,.git}/**";

type TestKind = "TEST_PROGRAM" | "TEST_FUNCTION_BLOCK";
type TestOutcome = "passed" | "failed" | "error";

type DiscoveredTest = {
  id: string;
  key: string;
  uri: vscode.Uri;
  line: number; // 1-based
  name: string;
  kind: TestKind;
  projectRoot: string;
};

type RuntimeSummary = {
  total: number;
  passed: number;
  failed: number;
  errors: number;
};

type RuntimeCase = {
  name: string;
  kind: string;
  status: string;
  file: string;
  line: number;
  message?: string | null;
};

type RuntimePayload = {
  version: number;
  project: string;
  summary: RuntimeSummary;
  tests: RuntimeCase[];
};

type RunAllArgs = {
  projectUri?: vscode.Uri | string;
  uri?: vscode.Uri | string;
};

type RunOneArgs = {
  uri?: vscode.Uri | string;
  name?: string;
  kind?: string;
  line?: number;
};

type LastResultEntry = {
  uri: string;
  line: number;
  name: string;
  kind: TestKind;
  status: TestOutcome;
  message?: string;
};

type RuntimeCommandResult = {
  exitCode: number;
  stdout: string;
  stderr: string;
};

const TEST_KIND_SET: Record<TestKind, true> = {
  TEST_PROGRAM: true,
  TEST_FUNCTION_BLOCK: true,
};

export const ST_TEST_RUN_ALL_COMMAND = "trust-lsp.test.runAll";
export const ST_TEST_RUN_ONE_COMMAND = "trust-lsp.test.runOne";
const ST_TEST_STATE_COMMAND = "trust-lsp.test._state";
const ST_TEST_DISCOVER_COMMAND = "trust-lsp.test._discover";

function isStructuredTextDocument(document: vscode.TextDocument): boolean {
  return document.languageId === "structured-text";
}

function toUri(input?: vscode.Uri | string): vscode.Uri | undefined {
  if (!input) {
    return undefined;
  }
  if (input instanceof vscode.Uri) {
    return input;
  }
  try {
    if (input.includes("://")) {
      return vscode.Uri.parse(input);
    }
    return vscode.Uri.file(input);
  } catch {
    return undefined;
  }
}

function normalizeFsPath(fsPath: string): string {
  const normalized = path.normalize(path.resolve(fsPath));
  return process.platform === "win32"
    ? normalized.toLocaleLowerCase()
    : normalized;
}

function discoveredTestKey(
  fsPath: string,
  line: number,
  kind: TestKind,
  name: string
): string {
  return `${normalizeFsPath(fsPath)}::${line}::${kind}::${name.toLocaleLowerCase()}`;
}

function runtimeCaseKey(projectRoot: string, test: RuntimeCase): string | undefined {
  const kindText = test.kind.toUpperCase();
  if (kindText !== "TEST_PROGRAM" && kindText !== "TEST_FUNCTION_BLOCK") {
    return undefined;
  }
  const kind = kindText as TestKind;
  const file = path.isAbsolute(test.file)
    ? test.file
    : path.join(projectRoot, test.file);
  const line = Number.isFinite(test.line) ? test.line : 0;
  if (line <= 0) {
    return undefined;
  }
  return discoveredTestKey(file, line, kind, test.name);
}

function isRuntimePayload(value: unknown): value is RuntimePayload {
  if (!value || typeof value !== "object") {
    return false;
  }
  const payload = value as Partial<RuntimePayload>;
  return (
    typeof payload.version === "number" &&
    typeof payload.project === "string" &&
    !!payload.summary &&
    typeof payload.summary.total === "number" &&
    typeof payload.summary.passed === "number" &&
    typeof payload.summary.failed === "number" &&
    typeof payload.summary.errors === "number" &&
    Array.isArray(payload.tests)
  );
}

function extractJsonPayload(stdout: string): string {
  const start = stdout.indexOf("{");
  const end = stdout.lastIndexOf("}");
  if (start < 0 || end <= start) {
    return stdout.trim();
  }
  return stdout.slice(start, end + 1);
}

function discoverTestsFromText(
  uri: vscode.Uri,
  text: string,
  projectRoot: string
): DiscoveredTest[] {
  const tests: DiscoveredTest[] = [];
  const lines = text.split(/\r?\n/);
  for (let idx = 0; idx < lines.length; idx += 1) {
    const lineText = lines[idx];
    const match = lineText.match(
      /^\s*(TEST_PROGRAM|TEST_FUNCTION_BLOCK)\s+([A-Za-z_][A-Za-z0-9_]*)\b/i
    );
    if (!match) {
      continue;
    }
    const kind = match[1].toUpperCase() as TestKind;
    const name = match[2];
    const line = idx + 1;
    const key = discoveredTestKey(uri.fsPath, line, kind, name);
    tests.push({
      id: `${uri.toString()}::${line}::${kind}::${name}`,
      key,
      uri,
      line,
      name,
      kind,
      projectRoot,
    });
  }
  return tests;
}

async function readText(uri: vscode.Uri): Promise<string | undefined> {
  const open = vscode.workspace.textDocuments.find(
    (doc) => doc.uri.toString() === uri.toString()
  );
  if (open) {
    return open.getText();
  }
  try {
    const bytes = await vscode.workspace.fs.readFile(uri);
    return new TextDecoder("utf-8").decode(bytes);
  } catch {
    return undefined;
  }
}

function hasSourcesFolder(root: string): boolean {
  return fs.existsSync(path.join(root, "sources"));
}

function nearestProjectRoot(fsPath: string): string | undefined {
  let current = fs.existsSync(fsPath) && fs.statSync(fsPath).isDirectory()
    ? fsPath
    : path.dirname(fsPath);
  while (true) {
    if (hasSourcesFolder(current)) {
      return current;
    }
    const parent = path.dirname(current);
    if (parent === current) {
      return undefined;
    }
    current = parent;
  }
}

function resolveProjectRootFromUri(uri?: vscode.Uri): string | undefined {
  if (!uri) {
    return undefined;
  }
  const nearest = nearestProjectRoot(uri.fsPath);
  if (nearest) {
    return nearest;
  }
  const folder = vscode.workspace.getWorkspaceFolder(uri);
  return folder?.uri.fsPath;
}

function resolveProjectRoot(args?: RunAllArgs | RunOneArgs): string | undefined {
  const directUri = toUri((args as RunAllArgs | undefined)?.projectUri);
  if (directUri && hasSourcesFolder(directUri.fsPath)) {
    return directUri.fsPath;
  }

  const uriArg = toUri(args?.uri);
  if (uriArg) {
    const nearest = nearestProjectRoot(uriArg.fsPath);
    if (nearest) {
      return nearest;
    }
    if (hasSourcesFolder(uriArg.fsPath)) {
      return uriArg.fsPath;
    }
    const fromUri = resolveProjectRootFromUri(uriArg);
    if (fromUri) {
      return fromUri;
    }
  }

  const active = vscode.window.activeTextEditor?.document.uri;
  if (active) {
    const fromActive = resolveProjectRootFromUri(active);
    if (fromActive) {
      return fromActive;
    }
  }

  const first = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
  return first;
}

function resolveRuntimeBinary(context: vscode.ExtensionContext): string {
  const envPath = process.env.ST_RUNTIME_TEST_BIN?.trim();
  if (envPath && fs.existsSync(envPath)) {
    return envPath;
  }
  return getBinaryPath(context, "trust-runtime", "runtime.cli.path");
}

function runRuntimeCommand(
  binary: string,
  args: string[],
  cwd: string
): Promise<RuntimeCommandResult> {
  return new Promise((resolve, reject) => {
    const child = spawn(binary, args, {
      cwd,
      env: process.env,
      windowsHide: true,
    });

    let stdout = "";
    let stderr = "";
    child.stdout?.on("data", (chunk: Buffer | string) => {
      stdout += chunk.toString();
    });
    child.stderr?.on("data", (chunk: Buffer | string) => {
      stderr += chunk.toString();
    });
    child.on("error", reject);
    child.on("close", (code) => {
      resolve({
        exitCode: code ?? -1,
        stdout,
        stderr,
      });
    });
  });
}

async function executeRuntimeTests(
  context: vscode.ExtensionContext,
  projectRoot: string,
  filter?: string
): Promise<RuntimePayload> {
  const binary = resolveRuntimeBinary(context);
  const args = ["test", "--project", projectRoot, "--output", "json"];
  if (filter && filter.trim()) {
    args.push("--filter", filter.trim());
  }
  const result = await runRuntimeCommand(binary, args, projectRoot);
  const payloadText = extractJsonPayload(result.stdout);
  let parsed: unknown;
  try {
    parsed = JSON.parse(payloadText);
  } catch (error) {
    const detail =
      error instanceof Error ? error.message : String(error ?? "unknown parse error");
    throw new Error(
      `Failed to parse test output JSON from trust-runtime: ${detail}\nstdout:\n${result.stdout}\nstderr:\n${result.stderr}`
    );
  }
  if (!isRuntimePayload(parsed)) {
    throw new Error(
      `Invalid test output payload from trust-runtime.\nstdout:\n${result.stdout}\nstderr:\n${result.stderr}`
    );
  }
  return parsed;
}

function asOutcome(status: string): TestOutcome | undefined {
  if (status === "passed" || status === "failed" || status === "error") {
    return status;
  }
  return undefined;
}

function toTestMessage(caseResult: RuntimeCase): vscode.TestMessage {
  return new vscode.TestMessage(
    caseResult.message?.trim() || `Test ${caseResult.status}`
  );
}

export function registerStTestIntegration(context: vscode.ExtensionContext): void {
  const controller = vscode.tests.createTestController(
    "trust-lsp.st-tests",
    "Structured Text Tests"
  );
  context.subscriptions.push(controller);

  const testById = new Map<string, DiscoveredTest>();
  const itemById = new Map<string, vscode.TestItem>();
  const resultByKey = new Map<string, RuntimeCase>();
  const stateByUriLine = new Map<string, Map<number, LastResultEntry>>();

  const passDecoration = vscode.window.createTextEditorDecorationType({
    isWholeLine: true,
    after: {
      contentText: " PASS",
      margin: "0 0 0 1rem",
      color: new vscode.ThemeColor("testing.iconPassed"),
    },
  });
  const failDecoration = vscode.window.createTextEditorDecorationType({
    isWholeLine: true,
    after: {
      contentText: " FAIL",
      margin: "0 0 0 1rem",
      color: new vscode.ThemeColor("testing.iconFailed"),
    },
  });
  const errorDecoration = vscode.window.createTextEditorDecorationType({
    isWholeLine: true,
    after: {
      contentText: " ERROR",
      margin: "0 0 0 1rem",
      color: new vscode.ThemeColor("testing.iconErrored"),
    },
  });
  context.subscriptions.push(passDecoration, failDecoration, errorDecoration);

  function applyDecorations(): void {
    for (const editor of vscode.window.visibleTextEditors) {
      if (!isStructuredTextDocument(editor.document)) {
        continue;
      }
      const uriKey = normalizeFsPath(editor.document.uri.fsPath);
      const byLine = stateByUriLine.get(uriKey);
      const pass: vscode.DecorationOptions[] = [];
      const fail: vscode.DecorationOptions[] = [];
      const error: vscode.DecorationOptions[] = [];
      if (byLine) {
        for (const entry of byLine.values()) {
          const lineIndex = Math.max(0, entry.line - 1);
          const range = new vscode.Range(lineIndex, 0, lineIndex, 0);
          const option: vscode.DecorationOptions = {
            range,
            hoverMessage: entry.message,
          };
          if (entry.status === "passed") {
            pass.push(option);
          } else if (entry.status === "failed") {
            fail.push(option);
          } else {
            error.push(option);
          }
        }
      }
      editor.setDecorations(passDecoration, pass);
      editor.setDecorations(failDecoration, fail);
      editor.setDecorations(errorDecoration, error);
    }
  }

  function clearProjectResults(projectRoot: string): void {
    const normalizedRoot = normalizeFsPath(projectRoot);
    for (const key of Array.from(resultByKey.keys())) {
      if (key.startsWith(normalizedRoot)) {
        resultByKey.delete(key);
      }
    }
    for (const uriKey of Array.from(stateByUriLine.keys())) {
      if (uriKey.startsWith(normalizedRoot)) {
        stateByUriLine.delete(uriKey);
      }
    }
  }

  function updateResultState(
    payload: RuntimePayload,
    projectRoot: string,
    replaceProject: boolean
  ): void {
    if (replaceProject) {
      clearProjectResults(projectRoot);
    }
    for (const testCase of payload.tests) {
      const key = runtimeCaseKey(projectRoot, testCase);
      if (!key) {
        continue;
      }
      resultByKey.set(key, testCase);
      const outcome = asOutcome(testCase.status);
      if (!outcome) {
        continue;
      }
      const absoluteFile = path.isAbsolute(testCase.file)
        ? testCase.file
        : path.join(projectRoot, testCase.file);
      const uriKey = normalizeFsPath(absoluteFile);
      const lineMap = stateByUriLine.get(uriKey) ?? new Map<number, LastResultEntry>();
      lineMap.set(testCase.line, {
        uri: vscode.Uri.file(absoluteFile).toString(),
        line: testCase.line,
        name: testCase.name,
        kind: testCase.kind.toUpperCase() as TestKind,
        status: outcome,
        message: testCase.message ?? undefined,
      });
      stateByUriLine.set(uriKey, lineMap);
    }
    applyDecorations();
  }

  function updateTestItemState(projectRoot: string): void {
    const normalizedRoot = normalizeFsPath(projectRoot);
    for (const test of testById.values()) {
      if (!normalizeFsPath(test.projectRoot).startsWith(normalizedRoot)) {
        continue;
      }
      const item = itemById.get(test.id);
      if (!item) {
        continue;
      }
      const runtimeCase = resultByKey.get(test.key);
      if (!runtimeCase) {
        item.error = undefined;
        continue;
      }
      const outcome = asOutcome(runtimeCase.status);
      if (!outcome || outcome === "passed") {
        item.error = undefined;
      } else {
        item.error = runtimeCase.message?.trim() || `Test ${runtimeCase.status}`;
      }
    }
  }

  function discoveredTestsForProject(projectRoot: string): DiscoveredTest[] {
    const normalizedRoot = normalizeFsPath(projectRoot);
    return Array.from(testById.values()).filter((test) =>
      normalizeFsPath(test.projectRoot).startsWith(normalizedRoot)
    );
  }

  function runtimeResultForTarget(
    target: DiscoveredTest,
    payload: RuntimePayload
  ): RuntimeCase | undefined {
    const exact = payload.tests.find((testCase) => {
      const key = runtimeCaseKey(target.projectRoot, testCase);
      return !!key && key === target.key;
    });
    if (exact) {
      return exact;
    }
    return payload.tests.find(
      (testCase) =>
        testCase.kind.toUpperCase() === target.kind &&
        testCase.name.toLocaleLowerCase() === target.name.toLocaleLowerCase()
    );
  }

  function collectDiscoveredTests(
    items: readonly vscode.TestItem[] | undefined
  ): DiscoveredTest[] {
    if (!items || items.length === 0) {
      return Array.from(testById.values());
    }
    const selected = new Map<string, DiscoveredTest>();
    const stack = [...items];
    while (stack.length > 0) {
      const current = stack.pop();
      if (!current) {
        continue;
      }
      const discovered = testById.get(current.id);
      if (discovered) {
        selected.set(discovered.id, discovered);
        continue;
      }
      current.children.forEach((child) => {
        stack.push(child);
      });
    }
    return Array.from(selected.values());
  }

  async function refreshDiscoveredTests(): Promise<void> {
    const fileUris = await vscode.workspace.findFiles(ST_SOURCE_GLOB, ST_SOURCE_EXCLUDE);
    const fileItems = new Map<string, vscode.TestItem>();
    testById.clear();
    itemById.clear();

    for (const uri of fileUris) {
      const projectRoot = resolveProjectRootFromUri(uri);
      if (!projectRoot) {
        continue;
      }
      const text = await readText(uri);
      if (!text) {
        continue;
      }
      const discovered = discoverTestsFromText(uri, text, projectRoot);
      if (discovered.length === 0) {
        continue;
      }

      const fileId = `file:${uri.toString()}`;
      let fileItem = fileItems.get(fileId);
      if (!fileItem) {
        fileItem = controller.createTestItem(fileId, path.basename(uri.fsPath), uri);
        fileItems.set(fileId, fileItem);
      }

      for (const test of discovered) {
        const line = Math.max(0, test.line - 1);
        const range = new vscode.Range(line, 0, line, 0);
        const label = `${test.kind} ${test.name}`;
        const item = controller.createTestItem(test.id, label, test.uri);
        item.range = range;
        fileItem.children.add(item);
        testById.set(test.id, test);
        itemById.set(test.id, item);
      }
    }

    controller.items.replace(Array.from(fileItems.values()));
    updateTestItemState(vscode.workspace.workspaceFolders?.[0]?.uri.fsPath ?? "");
  }

  let refreshTimer: NodeJS.Timeout | undefined;
  function scheduleRefresh(delayMs = 100): void {
    if (refreshTimer) {
      clearTimeout(refreshTimer);
    }
    refreshTimer = setTimeout(() => {
      void refreshDiscoveredTests();
    }, delayMs);
  }

  const codeLensProvider: vscode.CodeLensProvider = {
    provideCodeLenses(document: vscode.TextDocument): vscode.CodeLens[] {
      if (!isStructuredTextDocument(document)) {
        return [];
      }
      const projectRoot = resolveProjectRootFromUri(document.uri);
      if (!projectRoot) {
        return [];
      }
      const discovered = discoverTestsFromText(document.uri, document.getText(), projectRoot);
      return discovered.map((test) => {
        const line = Math.max(0, test.line - 1);
        const range = new vscode.Range(line, 0, line, 0);
        return new vscode.CodeLens(range, {
          title: "Run Test",
          tooltip: `Run ${test.kind} ${test.name}`,
          command: ST_TEST_RUN_ONE_COMMAND,
          arguments: [
            {
              uri: test.uri.toString(),
              line: test.line,
              kind: test.kind,
              name: test.name,
            } satisfies RunOneArgs,
          ],
        });
      });
    },
  };
  context.subscriptions.push(
    vscode.languages.registerCodeLensProvider(
      { scheme: "file", language: "structured-text" },
      codeLensProvider
    )
  );

  async function resolveSingleTarget(args?: RunOneArgs): Promise<DiscoveredTest | undefined> {
    const root = resolveProjectRoot(args);
    if (!root) {
      return undefined;
    }

    const uri = toUri(args?.uri);
    if (uri && args?.name && args?.line && args?.kind) {
      const kind = args.kind.toUpperCase();
      if (kind in TEST_KIND_SET) {
        const key = discoveredTestKey(uri.fsPath, args.line, kind as TestKind, args.name);
        const existing = Array.from(testById.values()).find((test) => test.key === key);
        if (existing) {
          return existing;
        }
        return {
          id: `${uri.toString()}::${args.line}::${kind}::${args.name}`,
          key,
          uri,
          line: args.line,
          name: args.name,
          kind: kind as TestKind,
          projectRoot: root,
        };
      }
    }

    const discovered = discoveredTestsForProject(root);
    if (discovered.length === 0) {
      return undefined;
    }
    const pick = await vscode.window.showQuickPick(
      discovered.map((test) => ({
        label: test.name,
        description: `${test.kind} (${path.basename(test.uri.fsPath)}:${test.line})`,
        test,
      })),
      { title: "Select Structured Text test to run" }
    );
    return pick?.test;
  }

  async function runAll(projectRoot: string): Promise<RuntimePayload> {
    const payload = await executeRuntimeTests(context, projectRoot);
    updateResultState(payload, projectRoot, true);
    updateTestItemState(projectRoot);
    return payload;
  }

  async function runOne(target: DiscoveredTest): Promise<RuntimeCase | undefined> {
    const payload = await executeRuntimeTests(context, target.projectRoot, target.name);
    updateResultState(payload, target.projectRoot, false);
    updateTestItemState(target.projectRoot);
    return runtimeResultForTarget(target, payload);
  }

  context.subscriptions.push(
    vscode.commands.registerCommand(
      ST_TEST_RUN_ALL_COMMAND,
      async (args?: RunAllArgs): Promise<RuntimePayload | undefined> => {
        const projectRoot = resolveProjectRoot(args);
        if (!projectRoot) {
          vscode.window.showErrorMessage("No project folder found for ST test run.");
          return undefined;
        }
        try {
          const payload = await runAll(projectRoot);
          const summary = payload.summary;
          const level = summary.failed + summary.errors > 0 ? "error" : "info";
          const message = `ST tests: ${summary.passed} passed, ${summary.failed} failed, ${summary.errors} errors`;
          if (level === "error") {
            vscode.window.showErrorMessage(message);
          } else {
            vscode.window.showInformationMessage(message);
          }
          return payload;
        } catch (error) {
          const detail = error instanceof Error ? error.message : String(error ?? "unknown error");
          vscode.window.showErrorMessage(`Failed to run ST tests: ${detail}`);
          return undefined;
        }
      }
    )
  );

  context.subscriptions.push(
    vscode.commands.registerCommand(
      ST_TEST_RUN_ONE_COMMAND,
      async (args?: RunOneArgs): Promise<RuntimeCase | undefined> => {
        const target = await resolveSingleTarget(args);
        if (!target) {
          vscode.window.showWarningMessage("No ST test selected.");
          return undefined;
        }
        try {
          const result = await runOne(target);
          if (!result) {
            vscode.window.showErrorMessage(
              `No runtime result found for ${target.kind} ${target.name}.`
            );
            return undefined;
          }
          const summary = `${result.kind} ${result.name}: ${result.status}`;
          if (result.status === "passed") {
            vscode.window.showInformationMessage(summary);
          } else {
            vscode.window.showErrorMessage(summary);
          }
          return result;
        } catch (error) {
          const detail = error instanceof Error ? error.message : String(error ?? "unknown error");
          vscode.window.showErrorMessage(`Failed to run ST test: ${detail}`);
          return undefined;
        }
      }
    )
  );

  context.subscriptions.push(
    vscode.commands.registerCommand(ST_TEST_STATE_COMMAND, (): LastResultEntry[] => {
      const entries: LastResultEntry[] = [];
      for (const byLine of stateByUriLine.values()) {
        for (const entry of byLine.values()) {
          entries.push(entry);
        }
      }
      entries.sort((a, b) =>
        a.uri.localeCompare(b.uri) || a.line - b.line || a.name.localeCompare(b.name)
      );
      return entries;
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand(
      ST_TEST_DISCOVER_COMMAND,
      async (args?: { uri?: vscode.Uri | string }): Promise<DiscoveredTest[]> => {
        const uri = toUri(args?.uri);
        if (!uri) {
          return [];
        }
        const projectRoot = resolveProjectRootFromUri(uri);
        if (!projectRoot) {
          return [];
        }
        const text = await readText(uri);
        if (!text) {
          return [];
        }
        return discoverTestsFromText(uri, text, projectRoot);
      }
    )
  );

  const runProfile = controller.createRunProfile(
    "Run",
    vscode.TestRunProfileKind.Run,
    (request, token) => {
      void (async () => {
        const run = controller.createTestRun(request);
        const selected = collectDiscoveredTests(request.include);
        if (selected.length === 0) {
          run.end();
          return;
        }

        for (const test of selected) {
          const item = itemById.get(test.id);
          if (item) {
            run.enqueued(item);
          }
        }

        const runAllMode = !request.include || request.include.length === 0;
        if (runAllMode) {
          const byProject = new Map<string, DiscoveredTest[]>();
          for (const test of selected) {
            const group = byProject.get(test.projectRoot) ?? [];
            group.push(test);
            byProject.set(test.projectRoot, group);
          }
          for (const [projectRoot, tests] of byProject) {
            if (token.isCancellationRequested) {
              break;
            }
            let payload: RuntimePayload | undefined;
            try {
              payload = await runAll(projectRoot);
            } catch (error) {
              const message = error instanceof Error ? error.message : String(error);
              for (const test of tests) {
                const item = itemById.get(test.id);
                if (item) {
                  run.started(item);
                  run.errored(item, new vscode.TestMessage(message));
                }
              }
              continue;
            }
            for (const test of tests) {
              const item = itemById.get(test.id);
              if (!item) {
                continue;
              }
              run.started(item);
              const result = runtimeResultForTarget(test, payload);
              if (!result) {
                run.skipped(item);
                continue;
              }
              if (result.status === "passed") {
                run.passed(item);
              } else if (result.status === "failed") {
                run.failed(item, toTestMessage(result));
              } else {
                run.errored(item, toTestMessage(result));
              }
            }
          }
          run.end();
          return;
        }

        for (const test of selected) {
          if (token.isCancellationRequested) {
            break;
          }
          const item = itemById.get(test.id);
          if (!item) {
            continue;
          }
          run.started(item);
          try {
            const result = await runOne(test);
            if (!result) {
              run.skipped(item);
              continue;
            }
            if (result.status === "passed") {
              run.passed(item);
            } else if (result.status === "failed") {
              run.failed(item, toTestMessage(result));
            } else {
              run.errored(item, toTestMessage(result));
            }
          } catch (error) {
            const message = error instanceof Error ? error.message : String(error);
            run.errored(item, new vscode.TestMessage(message));
          }
        }
        run.end();
      })();
    },
    true
  );
  context.subscriptions.push(runProfile);

  context.subscriptions.push(
    vscode.workspace.onDidOpenTextDocument((document) => {
      if (isStructuredTextDocument(document)) {
        scheduleRefresh();
      }
    }),
    vscode.workspace.onDidSaveTextDocument((document) => {
      if (isStructuredTextDocument(document)) {
        scheduleRefresh();
      }
    }),
    vscode.workspace.onDidCreateFiles(() => scheduleRefresh()),
    vscode.workspace.onDidDeleteFiles(() => scheduleRefresh()),
    vscode.workspace.onDidRenameFiles(() => scheduleRefresh()),
    vscode.workspace.onDidChangeWorkspaceFolders(() => scheduleRefresh()),
    vscode.window.onDidChangeVisibleTextEditors(() => applyDecorations())
  );

  scheduleRefresh(0);
}
