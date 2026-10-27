import * as fs from "fs";
import * as path from "path";
import { spawn } from "child_process";
import * as vscode from "vscode";
import { getBinaryPath } from "./binary";
import { projectRequest } from "./projectRequest";

// Native Testing for Structured Text (specification 25 §11.1). The language server finds the tests and the project each
// runs in, and gives the keys that tie a `trust-dev test` result to its test (`trust-lsp.testDiscovery`, specification
// 22 "Test POU discovery"). This module keeps the Testing view, the Run Test lenses and the pass/fail decorations, runs
// `trust-dev test` and shows its results.

const TEST_DISCOVERY_COMMAND = "trust-lsp.testDiscovery";

type TestOutcome = "passed" | "failed" | "error";

/** One test as the language server describes it. */
type DiscoveredTest = {
  id: string;
  key: string;
  nameKey: string;
  path: string;
  fileKey: string;
  line: number; // 1-based
  name: string;
  kind: string;
  projectRoot: string;
  projectKey: string;
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

/** One run result with the keys of the test it belongs to, as the language server gives them. */
type KeyedCase = {
  key: string | null;
  nameKey: string;
  path: string;
  fileKey: string;
  line: number;
  kind: string;
  name: string;
  status: string;
  message?: string | null;
  outcome: TestOutcome | null;
};

type KeyedResults = {
  projectKey: string;
  cases: KeyedCase[];
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
  kind: string;
  status: TestOutcome;
  message?: string;
  key: string;
  projectKey: string;
};

type RuntimeCommandResult = {
  exitCode: number;
  stdout: string;
  stderr: string;
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

function workspaceFolderPaths(): string[] {
  return (vscode.workspace.workspaceFolders ?? []).map((folder) => folder.uri.fsPath);
}

function filePath(uri: vscode.Uri | undefined): string | undefined {
  return uri?.scheme === "file" ? uri.fsPath : undefined;
}

/** The files of the editors with unsaved changes, whose text the language server reads instead. */
function unsavedPaths(): string[] {
  return vscode.workspace.textDocuments
    .filter((document) => document.isDirty && document.uri.scheme === "file")
    .map((document) => document.uri.fsPath);
}

/** The tests of `documents`, or of every Structured Text file in the workspace without them; `editors` asks for the
 * documents' editor text (their code lenses). */
async function discoverTests(documents?: readonly vscode.Uri[], editors = false): Promise<DiscoveredTest[]> {
  const files = documents
    ?.filter((uri) => uri.scheme === "file")
    .map((uri) => (editors ? { path: uri.fsPath, uri: uri.toString() } : { path: uri.fsPath }));
  if (files && files.length === 0) {
    return [];
  }
  const answer = await projectRequest<{ tests: DiscoveredTest[] }>(TEST_DISCOVERY_COMMAND, {
    action: "discover",
    folders: workspaceFolderPaths(),
    unsaved: unsavedPaths(),
    ...(files ? { files } : {}),
  });
  return answer.ok ? answer.tests : [];
}

/** The project a run command uses, as the language server chooses it. */
async function resolveProjectRoot(
  args?: RunAllArgs | RunOneArgs
): Promise<{ root: string; projectKey: string } | undefined> {
  const answer = await projectRequest<{ root: string | null; projectKey: string | null }>(TEST_DISCOVERY_COMMAND, {
    action: "project",
    projectPath: filePath(toUri((args as RunAllArgs | undefined)?.projectUri)),
    path: filePath(toUri(args?.uri)),
    activePath: filePath(vscode.window.activeTextEditor?.document.uri),
    folders: workspaceFolderPaths(),
  });
  return answer.ok && answer.root && answer.projectKey
    ? { root: answer.root, projectKey: answer.projectKey }
    : undefined;
}

/** The results of a run with the keys of their tests. */
async function keyedResults(projectRoot: string, payload: RuntimePayload): Promise<KeyedResults> {
  const answer = await projectRequest<KeyedResults>(TEST_DISCOVERY_COMMAND, {
    action: "results",
    projectRoot,
    tests: payload.tests,
  });
  if (!answer.ok) {
    throw new Error(answer.error);
  }
  return answer;
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

function siblingTrustDevBinary(candidate: string | undefined): string | undefined {
  if (!candidate || (!path.isAbsolute(candidate) && !candidate.includes(path.sep))) {
    return undefined;
  }
  const suffix = process.platform === "win32" ? ".exe" : "";
  const sibling = path.join(path.dirname(candidate), `trust-dev${suffix}`);
  return fs.existsSync(sibling) ? sibling : undefined;
}

function resolveTestBinary(context: vscode.ExtensionContext): string {
  const envPath = process.env.ST_DEV_TEST_BIN?.trim();
  if (envPath && fs.existsSync(envPath)) {
    return envPath;
  }
  const envRuntimeSibling = siblingTrustDevBinary(process.env.ST_RUNTIME_TEST_BIN?.trim());
  if (envRuntimeSibling) {
    return envRuntimeSibling;
  }
  const configuredRuntime = getBinaryPath(context, "trust-runtime", "runtime.cli.path");
  const configuredRuntimeSibling = siblingTrustDevBinary(configuredRuntime);
  if (configuredRuntimeSibling) {
    return configuredRuntimeSibling;
  }
  return getBinaryPath(context, "trust-dev", "dev.cli.path");
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
  const binary = resolveTestBinary(context);
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

function toTestMessage(caseResult: KeyedCase): vscode.TestMessage {
  return new vscode.TestMessage(
    caseResult.message?.trim() || `Test ${caseResult.status}`
  );
}

/** A keyed result in the shape `trust-lsp.test.runOne` answers. */
function runtimeCase(result: KeyedCase): RuntimeCase {
  return {
    name: result.name,
    kind: result.kind,
    status: result.status,
    file: result.path,
    line: result.line,
    message: result.message,
  };
}

export function registerStTestIntegration(context: vscode.ExtensionContext): void {
  const controller = vscode.tests.createTestController(
    "trust-lsp.st-tests",
    "Structured Text Tests"
  );
  context.subscriptions.push(controller);

  const testById = new Map<string, DiscoveredTest>();
  const itemById = new Map<string, vscode.TestItem>();
  const resultByKey = new Map<string, KeyedCase & { projectKey: string }>();
  const stateByFileKey = new Map<string, Map<number, LastResultEntry>>();
  /** The language server's file key of each document URI it described, for the decorations. */
  const fileKeyByUri = new Map<string, string>();

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

  function rememberFileKeys(entries: ReadonlyArray<{ path: string; fileKey: string }>): void {
    for (const entry of entries) {
      fileKeyByUri.set(vscode.Uri.file(entry.path).toString(), entry.fileKey);
    }
  }

  function applyDecorations(): void {
    for (const editor of vscode.window.visibleTextEditors) {
      if (!isStructuredTextDocument(editor.document)) {
        continue;
      }
      const fileKey = fileKeyByUri.get(editor.document.uri.toString());
      const byLine = fileKey ? stateByFileKey.get(fileKey) : undefined;
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

  function clearProjectResults(projectKey: string): void {
    for (const [key, result] of Array.from(resultByKey.entries())) {
      if (result.projectKey === projectKey) {
        resultByKey.delete(key);
      }
    }
    for (const [fileKey, lineMap] of Array.from(stateByFileKey.entries())) {
      for (const [line, entry] of Array.from(lineMap.entries())) {
        if (entry.projectKey === projectKey) {
          lineMap.delete(line);
        }
      }
      if (lineMap.size === 0) {
        stateByFileKey.delete(fileKey);
      }
    }
  }

  function pruneResultsToDiscoveredTests(): void {
    const validKeys = new Set(Array.from(testById.values()).map((test) => test.key));
    for (const key of Array.from(resultByKey.keys())) {
      if (!validKeys.has(key)) {
        resultByKey.delete(key);
      }
    }
    for (const [fileKey, lineMap] of Array.from(stateByFileKey.entries())) {
      for (const [line, entry] of Array.from(lineMap.entries())) {
        if (!validKeys.has(entry.key)) {
          lineMap.delete(line);
        }
      }
      if (lineMap.size === 0) {
        stateByFileKey.delete(fileKey);
      }
    }
    applyDecorations();
  }

  function updateResultState(results: KeyedResults, replaceProject: boolean): void {
    if (replaceProject) {
      clearProjectResults(results.projectKey);
    }
    rememberFileKeys(results.cases);
    for (const testCase of results.cases) {
      if (!testCase.key) {
        continue;
      }
      resultByKey.set(testCase.key, { ...testCase, projectKey: results.projectKey });
      if (!testCase.outcome) {
        continue;
      }
      const lineMap = stateByFileKey.get(testCase.fileKey) ?? new Map<number, LastResultEntry>();
      lineMap.set(testCase.line, {
        uri: vscode.Uri.file(testCase.path).toString(),
        line: testCase.line,
        name: testCase.name,
        kind: testCase.kind,
        status: testCase.outcome,
        message: testCase.message ?? undefined,
        key: testCase.key,
        projectKey: results.projectKey,
      });
      stateByFileKey.set(testCase.fileKey, lineMap);
    }
    applyDecorations();
  }

  /** Sets each test item's error from its last result; with `projectKey`, only the items of that project. */
  function updateTestItemState(projectKey?: string): void {
    for (const test of testById.values()) {
      if (projectKey !== undefined && test.projectKey !== projectKey) {
        continue;
      }
      const item = itemById.get(test.id);
      if (!item) {
        continue;
      }
      const runtimeCase = resultByKey.get(test.key);
      if (!runtimeCase || !runtimeCase.outcome || runtimeCase.outcome === "passed") {
        item.error = undefined;
      } else {
        item.error = runtimeCase.message?.trim() || `Test ${runtimeCase.status}`;
      }
    }
  }

  function discoveredTestsForProject(projectKey: string): DiscoveredTest[] {
    return Array.from(testById.values()).filter((test) => test.projectKey === projectKey);
  }

  function runtimeResultForTarget(
    target: DiscoveredTest,
    results: KeyedResults
  ): KeyedCase | undefined {
    return (
      results.cases.find((testCase) => testCase.key === target.key) ??
      results.cases.find((testCase) => testCase.nameKey === target.nameKey)
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
    const discovered = await discoverTests();
    const fileItems = new Map<string, vscode.TestItem>();
    testById.clear();
    itemById.clear();
    rememberFileKeys(discovered);

    for (const test of discovered) {
      const uri = vscode.Uri.file(test.path);
      const fileId = `file:${uri.toString()}`;
      let fileItem = fileItems.get(fileId);
      if (!fileItem) {
        fileItem = controller.createTestItem(fileId, path.basename(test.path), uri);
        fileItems.set(fileId, fileItem);
      }
      const line = Math.max(0, test.line - 1);
      const item = controller.createTestItem(test.id, `${test.kind} ${test.name}`, uri);
      item.range = new vscode.Range(line, 0, line, 0);
      fileItem.children.add(item);
      testById.set(test.id, test);
      itemById.set(test.id, item);
    }

    controller.items.replace(Array.from(fileItems.values()));
    pruneResultsToDiscoveredTests();
    updateTestItemState();
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
    async provideCodeLenses(document: vscode.TextDocument): Promise<vscode.CodeLens[]> {
      if (!isStructuredTextDocument(document)) {
        return [];
      }
      const discovered = await discoverTests([document.uri], true);
      return discovered.map((test) => {
        const line = Math.max(0, test.line - 1);
        const range = new vscode.Range(line, 0, line, 0);
        return new vscode.CodeLens(range, {
          title: "Run Test",
          tooltip: `Run ${test.kind} ${test.name}`,
          command: ST_TEST_RUN_ONE_COMMAND,
          arguments: [
            {
              uri: document.uri.toString(),
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
    const project = await resolveProjectRoot(args);
    if (!project) {
      return undefined;
    }

    const uri = toUri(args?.uri);
    if (uri?.scheme === "file" && args?.name && args?.line && args?.kind) {
      const answer = await projectRequest<{ test: DiscoveredTest | null }>(TEST_DISCOVERY_COMMAND, {
        action: "identify",
        path: uri.fsPath,
        line: args.line,
        kind: args.kind,
        name: args.name,
        projectRoot: project.root,
      });
      if (answer.ok && answer.test) {
        const named = answer.test;
        return Array.from(testById.values()).find((test) => test.key === named.key) ?? named;
      }
    }

    const discovered = discoveredTestsForProject(project.projectKey);
    if (discovered.length === 0) {
      return undefined;
    }
    const pick = await vscode.window.showQuickPick(
      discovered.map((test) => ({
        label: test.name,
        description: `${test.kind} (${path.basename(test.path)}:${test.line})`,
        test,
      })),
      { title: "Select Structured Text test to run" }
    );
    return pick?.test;
  }

  async function runAll(projectRoot: string): Promise<{ payload: RuntimePayload; results: KeyedResults }> {
    const payload = await executeRuntimeTests(context, projectRoot);
    const results = await keyedResults(projectRoot, payload);
    updateResultState(results, true);
    updateTestItemState(results.projectKey);
    return { payload, results };
  }

  async function runOne(target: DiscoveredTest): Promise<KeyedCase | undefined> {
    const payload = await executeRuntimeTests(context, target.projectRoot, target.name);
    const results = await keyedResults(target.projectRoot, payload);
    updateResultState(results, false);
    updateTestItemState(results.projectKey);
    return runtimeResultForTarget(target, results);
  }

  context.subscriptions.push(
    vscode.commands.registerCommand(
      ST_TEST_RUN_ALL_COMMAND,
      async (args?: RunAllArgs): Promise<RuntimePayload | undefined> => {
        const project = await resolveProjectRoot(args);
        if (!project) {
          vscode.window.showErrorMessage("No project folder found for ST test run.");
          return undefined;
        }
        try {
          const { payload } = await runAll(project.root);
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
          return runtimeCase(result);
        } catch (error) {
          const detail = error instanceof Error ? error.message : String(error ?? "unknown error");
          vscode.window.showErrorMessage(`Failed to run ST test: ${detail}`);
          return undefined;
        }
      }
    )
  );

  context.subscriptions.push(
    vscode.commands.registerCommand(ST_TEST_STATE_COMMAND, () => {
      const entries: LastResultEntry[] = [];
      for (const byLine of stateByFileKey.values()) {
        for (const entry of byLine.values()) {
          entries.push(entry);
        }
      }
      entries.sort((a, b) =>
        a.uri.localeCompare(b.uri) || a.line - b.line || a.name.localeCompare(b.name)
      );
      return entries.map(({ uri, line, name, kind, status, message }) => ({
        uri,
        line,
        name,
        kind,
        status,
        message,
      }));
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand(
      ST_TEST_DISCOVER_COMMAND,
      async (args?: { uri?: vscode.Uri | string }): Promise<DiscoveredTest[]> => {
        const uri = toUri(args?.uri);
        return uri ? discoverTests([uri]) : [];
      }
    )
  );

  const runProfile = controller.createRunProfile(
    "Run",
    vscode.TestRunProfileKind.Run,
    async (request, token) => {
      const run = controller.createTestRun(request);
      try {
        const selected = collectDiscoveredTests(request.include);
        if (selected.length === 0) {
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
            let results: KeyedResults;
            try {
              results = (await runAll(projectRoot)).results;
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
              const result = runtimeResultForTarget(test, results);
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
      } finally {
        run.end();
      }
    },
    true
  );
  context.subscriptions.push(runProfile);

  controller.refreshHandler = async () => {
    await refreshDiscoveredTests();
  };

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
