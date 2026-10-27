import { disposeFixtureProject } from "./fixtureDocuments";
import * as fs from "fs/promises";
import * as os from "os";
import * as path from "path";
import { HmiAgentTool, workspaceIdentity } from "../../hmiBuilder/agent/tools";
import { cancellableNativeRead } from "../../hmiBuilder/agent/lifetime";
import * as assert from "assert";
import { spawn } from "child_process";
import * as readline from "readline";
import * as vscode from "vscode";
import { languageServerReady } from "./hmiBuilderHost";

function toolInput(name: string, input: object): object {
  return name === "hmi_compose_page"
    ? { key: `fixture-compose:${(input as { file?: string }).file}`, ...input }
    : input;
}

/** The trust-lsp binary under test, started with `args` as the agent's client starts it (specification 43, §3.1). */
function startAgentServer(args: string[]): {
  call(method: string, params?: object): Promise<Record<string, unknown>>;
  output(): string;
  close(): void;
} {
  const server = process.env.ST_LSP_TEST_SERVER?.trim();
  assert.ok(server, "the test runner names the trust-lsp binary under test");
  const child = spawn(server, args, { stdio: ["pipe", "pipe", "pipe"] });
  let output = "";
  child.stdout.on("data", (chunk) => (output += chunk.toString()));
  child.stderr.on("data", (chunk) => (output += chunk.toString()));
  const pending = new Map<unknown, (message: Record<string, unknown>) => void>();
  const lines = readline.createInterface({ input: child.stdout });
  lines.on("line", (line) => {
    const message = JSON.parse(line) as Record<string, unknown>;
    pending.get(message.id)?.(message);
  });
  let next = 0;
  return {
    call: (method, params) =>
      new Promise((resolve, reject) => {
        const id = ++next;
        const timer = setTimeout(() => reject(new Error(`${method} was not answered`)), 50000);
        pending.set(id, (message) => {
          clearTimeout(timer);
          resolve(message);
        });
        child.stdin.write(JSON.stringify({ jsonrpc: "2.0", id, method, params }) + "\n");
      }),
    output: () => output,
    close: () => {
      lines.close();
      child.kill();
    },
  };
}

suite("HMI agent tools in VS Code", function () {
  this.timeout(60000);
  let fixtureRoot: vscode.Uri;
  let fixtureId: string;
  async function changeFolder(add: boolean): Promise<void> {
    const folders = vscode.workspace.workspaceFolders ?? [];
    const index = add
      ? folders.length
      : folders.findIndex((folder) => folder.uri.toString() === fixtureRoot.toString());
    if (index < 0) return;
    await new Promise<void>((resolve, reject) => {
      const timer = setTimeout(() => {
        changed.dispose();
        reject(new Error("workspace change timed out"));
      }, 10000);
      const changed = vscode.workspace.onDidChangeWorkspaceFolders(() => {
        clearTimeout(timer);
        changed.dispose();
        resolve();
      });
      const accepted = add
        ? vscode.workspace.updateWorkspaceFolders(index, 0, { uri: fixtureRoot, name: "HMI agent fixture" })
        : vscode.workspace.updateWorkspaceFolders(index, 1);
      if (!accepted) {
        clearTimeout(timer);
        changed.dispose();
        reject(new Error("workspace change refused"));
      }
    });
  }
  suiteSetup(async () => {
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    await languageServerReady();
    fixtureRoot = vscode.Uri.file(await fs.mkdtemp(path.join(os.tmpdir(), "trust-agent-tools-")));
    fixtureId = workspaceIdentity(fixtureRoot);
    const root = fixtureRoot;
    await vscode.workspace.fs.createDirectory(vscode.Uri.joinPath(root, "src"));
    await vscode.workspace.fs.createDirectory(vscode.Uri.joinPath(root, "hmi/pages"));
    await vscode.workspace.fs.writeFile(
      vscode.Uri.joinPath(root, "src/main.st"),
      Buffer.from("PROGRAM Main\nVAR Level : REAL; END_VAR\nEND_PROGRAM\n"),
    );
    await vscode.workspace.fs.writeFile(
      vscode.Uri.joinPath(root, "hmi/screen.toml"),
      Buffer.from('format = 1\nsize = [1280, 800]\ntheme = "dark"\nsymbols = "flat"\n'),
    );
    await vscode.workspace.fs.writeFile(
      vscode.Uri.joinPath(root, "hmi/pages/process.toml"),
      Buffer.from(
        'format = 1\ntitle = "Process"\n[[object]]\nid = "level"\ntype = "value"\nat = [40, 40]\nsize = [200, 64]\n[object.bind]\nvalue = "Main.Level"\n',
      ),
    );
    await changeFolder(true);
  });
  suiteTeardown(async () => {
    if (!fixtureRoot) return;
    for (const document of vscode.workspace.textDocuments)
      if (document.uri.fsPath.startsWith(fixtureRoot.fsPath + path.sep) && document.isDirty) await document.save();
    await disposeFixtureProject(fixtureRoot.fsPath);
    await changeFolder(false);
  });
  test("explicit connection command exposes the same unsaved page through MCP", async () => {
    const commands = await vscode.commands.getCommands(true);
    assert.ok(commands.includes("trust-lsp.hmiBuilder.connectAgent"), "Missing scoped connection command");
    const connection = await vscode.commands.executeCommand<{
      connectionFile: string;
      configuration: { args: string[] };
    }>("trust-lsp.hmiBuilder.connectAgent", { workspaceId: fixtureId, reveal: false });
    assert.ok(connection?.connectionFile);
    const { connectionFile, configuration } = connection!;
    assert.deepStrictEqual(configuration.args, ["hmi-agent", connectionFile]);
    const descriptor = JSON.parse(await fs.readFile(connectionFile, "utf8"));
    const root = fixtureRoot;
    const document = await vscode.workspace.openTextDocument(vscode.Uri.joinPath(root, "hmi/pages/process.toml"));
    const edit = new vscode.WorkspaceEdit();
    const text = document.getText();
    edit.replace(
      document.uri,
      new vscode.Range(0, 0, document.lineCount, 0),
      text.replace('title = "Process"', 'title = "Manual draft"'),
    );
    assert.ok(await vscode.workspace.applyEdit(edit));
    const agent = startAgentServer(configuration.args);
    try {
      const response = await agent.call("tools/call", {
        name: "hmi_inspect_page",
        arguments: { contractVersion: 1, input: { file: "hmi/pages/process.toml" } },
      });
      const payload = JSON.parse((response.result as { content: Array<{ text: string }> }).content[0].text);
      assert.strictEqual(payload.ok, true, JSON.stringify(payload));
      assert.strictEqual(payload.data.title, "Manual draft");
      assert.strictEqual(payload.data.dirty, true);
      const successResult = response.result as { structuredContent?: unknown; isError?: boolean };
      assert.deepStrictEqual(successResult.structuredContent, payload, "MCP carries the shared structured envelope");
      assert.strictEqual(successResult.isError, false);
      const rejected = await agent.call("tools/call", {
        name: "hmi_inspect_page",
        arguments: { contractVersion: 1, input: { file: "../outside" } },
      });
      const failure = rejected.result as { isError?: boolean; structuredContent?: { ok: boolean; code: string } };
      assert.strictEqual(failure.isError, true, "MCP marks domain rejection as a tool failure");
      assert.strictEqual(failure.structuredContent?.ok, false);
      assert.strictEqual(failure.structuredContent?.code, "INVALID_ARGUMENT");

      const listed = await agent.call("tools/list");
      const bridgeTools = (listed.result as { tools: Array<{ name: string; annotations: { readOnlyHint: boolean } }> })
        .tools;
      assert.strictEqual(bridgeTools.length, 9, "MCP exposes the same nine authoring tools");
      assert.strictEqual(bridgeTools.find((tool) => tool.name === "hmi_apply_draft")?.annotations.readOnlyHint, false);
      assert.ok(!agent.output().includes(descriptor.capability), "the capability never reaches the agent");
    } finally {
      agent.close();
    }
  });

  test("authoring uses customer agent tools rather than the retired model-calling command", async () => {
    const commands = await vscode.commands.getCommands(true);
    assert.ok(
      !commands.includes("trust-lsp.hmiBuilder.draftPage"),
      "retired direct-model command must not remain active",
    );
    assert.ok(commands.includes("trust-lsp.hmiBuilder.connectAgent"));
  });

  test("cancellation after the editor commits reports the applied outcome", async () => {
    const storageRoot = await fs.mkdtemp(path.join(os.tmpdir(), "trust-agent-cancel-"));
    const fontFile = path.resolve(__dirname, "../../../media/fonts/InterVariable.woff2");
    const client = {
      sendRequest: (_method: string, request: { command: string; arguments: unknown[] }) =>
        vscode.commands.executeCommand(request.command, ...request.arguments),
    };
    const getClient = (() => client) as unknown as import("../../lm-tools/shared").LspClientProvider;
    const cancellation = new vscode.CancellationTokenSource();
    async function call(name: string, input: object) {
      const tool = new HmiAgentTool(name, getClient, { storageRoot, fontFile });
      const result = await tool.invoke(
        { input: { contractVersion: 1, workspaceId: fixtureId, input: toolInput(name, input) } },
        cancellation.token,
      );
      return JSON.parse((result.content[0] as vscode.LanguageModelTextPart).value);
    }
    let changed: vscode.Disposable | undefined;
    try {
      const file = "hmi/pages/cancel-after-commit.toml";
      const created = await call("hmi_compose_page", {
        file,
        title: "Committed proposal",
        pattern: "process-overview",
        items: [{ id: "level", type: "value", bind: { value: { path: "Main.Level", direction: "read" } } }],
      });
      assert.strictEqual(created.ok, true, JSON.stringify(created));
      const target = { draftId: created.data.draftId, revision: created.data.revision };
      assert.strictEqual((await call("hmi_apply_draft", { ...target, action: "review" })).ok, true);
      const uri = vscode.Uri.joinPath(fixtureRoot, file);
      changed = vscode.workspace.onDidChangeTextDocument((event) => {
        if (event.document.uri.toString() === uri.toString() && event.document.getText().includes("Committed proposal"))
          cancellation.cancel();
      });
      const applied = await call("hmi_apply_draft", { ...target, action: "apply", save: false });
      assert.ok(cancellation.token.isCancellationRequested, "cancellation must reach the adapter after the real edit");
      const document = await vscode.workspace.openTextDocument(uri);
      assert.ok(document.getText().includes("Committed proposal"), "source was actually committed");
      assert.strictEqual(applied.ok, true, JSON.stringify(applied));
      assert.strictEqual(applied.data.applied, true);
    } finally {
      changed?.dispose();
      cancellation.dispose();
      await fs.rm(storageRoot, { recursive: true, force: true });
    }
  });

  test("cancelled native requests retain a typed cancellation result", async () => {
    const cancellation = new vscode.CancellationTokenSource();
    cancellation.cancel();
    const storageRoot = await fs.mkdtemp(path.join(os.tmpdir(), "trust-agent-cancelled-"));
    try {
      const result = await new HmiAgentTool("hmi_get_context", undefined, { storageRoot, fontFile: "" }).invoke(
        { input: { contractVersion: 1, workspaceId: fixtureId, input: {} } },
        cancellation.token,
      );
      const payload = JSON.parse((result.content[0] as vscode.LanguageModelTextPart).value);
      assert.strictEqual(payload.ok, false);
      assert.strictEqual(payload.code, "CANCELLED");
    } finally {
      cancellation.dispose();
      await fs.rm(storageRoot, { recursive: true, force: true });
    }
  });

  test("tool discovery describes binding maps and semantic edit shapes precisely", () => {
    const compose = JSON.parse(
      JSON.stringify(vscode.lm.tools.find((tool) => tool.name === "hmi_compose_page")!.inputSchema),
    );
    const singlePage = compose.properties.input.oneOf.find(
      (variant: { properties?: { items?: unknown } }) => variant.properties?.items,
    );
    assert.ok(singlePage, "single-page composition remains discoverable alongside multi-page proposals");
    const item = singlePage.properties.items.items;
    assert.ok(
      item.properties?.bind?.additionalProperties?.properties?.path,
      "composition binding map must document path and direction",
    );
    assert.deepStrictEqual(item.properties.bind.additionalProperties.required, ["path", "direction"]);
    const edit = JSON.parse(
      JSON.stringify(vscode.lm.tools.find((tool) => tool.name === "hmi_edit_draft")!.inputSchema),
    );
    const alternatives = edit.properties.input.properties.operations.items.oneOf;
    assert.ok(Array.isArray(alternatives), "semantic operation variants must be discoverable");
    const update = alternatives.find(
      (variant: { properties: { op: { const: string } } }) => variant.properties.op.const === "update",
    );
    assert.ok(update.properties.properties.properties.label);
    assert.ok(!update.properties.changes, "do not advertise an unsupported changes alias");
  });

  test("operation deadlines end the native requests and leave caller cancellation independent", async () => {
    const parent = new vscode.CancellationTokenSource();
    const storageRoot = await fs.mkdtemp(path.join(os.tmpdir(), "trust-agent-deadline-"));
    try {
      const tool = Reflect.construct(HmiAgentTool, [
        "hmi_get_context",
        undefined,
        { storageRoot, fontFile: "" },
        1,
      ]) as HmiAgentTool;
      const result = await tool.invoke(
        { input: { contractVersion: 1, workspaceId: fixtureId, input: {} } },
        parent.token,
      );
      assert.strictEqual(parent.token.isCancellationRequested, false, "the request owns its deadline, not its caller");
      const payload = JSON.parse((result.content[0] as vscode.LanguageModelTextPart).value);
      assert.strictEqual(payload.code, "CANCELLED");
    } finally {
      parent.dispose();
      await fs.rm(storageRoot, { recursive: true, force: true });
    }
  });

  test("cancellation ends read-only native calls even if the provider ignores its token", async () => {
    const cancellation = new vscode.CancellationTokenSource();
    let settled = false;
    const pending = cancellableNativeRead(cancellation.token, () => new Promise<never>(() => undefined)).then(
      () => undefined,
      (error: unknown) => {
        settled = true;
        throw error;
      },
    );
    try {
      cancellation.cancel();
      await new Promise((resolve) => setTimeout(resolve, 80));
      assert.ok(settled, "a cancelled read must not wait for a non-cooperating provider");
      await assert.rejects(pending, (error: unknown) => error instanceof vscode.CancellationError);
    } finally {
      cancellation.dispose();
    }
  });
});
