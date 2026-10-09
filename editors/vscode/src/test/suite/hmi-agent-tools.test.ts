import { proposalFor } from "../../hmiBuilder/editor/proposals";
import { AuthoringService } from "../../hmiBuilder/agent/service";
import { disposeFixtureProject } from "./fixtureDocuments";
import * as fs from "fs/promises";
import * as os from "os";
import * as path from "path";
import { HmiAgentTool, workspaceIdentity } from "../../hmiBuilder/agent/tools";
import * as assert from "assert";
import * as vscode from "vscode";
import { languageServerReady } from "./hmiBuilderHost";

function toolInput(name: string, input: object): object {
  return name === "hmi_compose_page"
    ? { key: `fixture-compose:${(input as { file?: string }).file}`, ...input }
    : input;
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
  test("agent discovers and invokes context, library and selected-page inspection", async () => {
    const names = vscode.lm.tools.map((tool) => tool.name);
    for (const name of ["hmi_get_context", "hmi_browse_library", "hmi_inspect_page"])
      assert.ok(names.includes(name), `Missing agent tool ${name}`);
    const root = fixtureRoot;
    async function call(name: string, input: object, workspaceId?: string) {
      const result = await vscode.lm.invokeTool(
        name,
        {
          input: { contractVersion: 1, workspaceId: workspaceId ?? fixtureId, input: toolInput(name, input) },
          toolInvocationToken: undefined,
        },
        new vscode.CancellationTokenSource().token,
      );
      return JSON.parse((result.content[0] as vscode.LanguageModelTextPart).value);
    }
    const context = await call("hmi_get_context", {});
    assert.strictEqual(context.ok, true, JSON.stringify(context));
    assert.ok(context.data.workspaceId);
    assert.ok(context.data.variables.length > 0);
    const libraryImage = await vscode.lm.invokeTool(
      "hmi_browse_library",
      {
        input: { contractVersion: 1, workspaceId: context.data.workspaceId, input: { query: "pump" } },
        toolInvocationToken: undefined,
      },
      new vscode.CancellationTokenSource().token,
    );
    assert.ok(
      libraryImage.content.some(
        (part) => part instanceof vscode.LanguageModelDataPart && part.mimeType === "image/png",
      ),
      "library must show the actual components visually",
    );
    const library = JSON.parse((libraryImage.content[0] as vscode.LanguageModelTextPart).value);
    assert.strictEqual(library.ok, true, JSON.stringify(library));
    assert.ok(library.data.components.some((component: { type: string }) => component.type === "pump"));
    const page = await call(
      "hmi_inspect_page",
      { file: "hmi/pages/process.toml", ids: ["level"] },
      context.data.workspaceId,
    );
    assert.strictEqual(page.ok, true, JSON.stringify(page));
    assert.deepStrictEqual(
      page.data.objects.map((object: { id: string }) => object.id),
      ["level"],
    );
    const region = await call(
      "hmi_inspect_page",
      { file: "hmi/pages/process.toml", region: { x: 800, y: 600, width: 100, height: 100 } },
      context.data.workspaceId,
    );
    assert.strictEqual(region.ok, true, JSON.stringify(region));
    assert.deepStrictEqual(region.data.objects, [], "an empty region does not select the whole page");

    const denied = await call("hmi_inspect_page", { file: "../outside" }, context.data.workspaceId);
    assert.strictEqual(denied.ok, false);
  });
  test("explicit connection command exposes the same unsaved page through MCP", async () => {
    const commands = await vscode.commands.getCommands(true);
    assert.ok(commands.includes("trust-lsp.hmiBuilder.connectAgent"), "Missing scoped connection command");
    const connection = await vscode.commands.executeCommand<{ connectionFile: string }>(
      "trust-lsp.hmiBuilder.connectAgent",
      { workspaceId: fixtureId, reveal: false },
    );
    assert.ok(connection?.connectionFile);
    const fs = await import("fs/promises");
    const http = await import("http");
    const descriptor = JSON.parse(await fs.readFile(connection!.connectionFile, "utf8"));
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
    const response = await new Promise<Record<string, unknown>>((resolve, reject) => {
      const request = http.request(
        descriptor.url,
        {
          method: "POST",
          headers: { Authorization: `Bearer ${descriptor.capability}`, "Content-Type": "application/json" },
        },
        (result) => {
          let body = "";
          result.on("data", (chunk) => (body += chunk));
          result.on("end", () => resolve(JSON.parse(body)));
        },
      );
      request.on("error", reject);
      request.end(
        JSON.stringify({
          jsonrpc: "2.0",
          id: 11,
          method: "tools/call",
          params: {
            name: "hmi_inspect_page",
            arguments: { contractVersion: 1, input: { file: "hmi/pages/process.toml" } },
          },
        }),
      );
    });
    const payload = JSON.parse((response.result as { content: Array<{ text: string }> }).content[0].text);
    assert.strictEqual(payload.ok, true, JSON.stringify(payload));
    assert.strictEqual(payload.data.title, "Manual draft");
    assert.strictEqual(payload.data.dirty, true);
    assert.ok(!JSON.stringify(response).includes(descriptor.capability));
    const successResult = response.result as { structuredContent?: unknown; isError?: boolean };
    assert.deepStrictEqual(successResult.structuredContent, payload, "MCP carries the shared structured envelope");
    assert.strictEqual(successResult.isError, false);
    const rejected = await new Promise<Record<string, unknown>>((resolve, reject) => {
      const request = http.request(
        descriptor.url,
        {
          method: "POST",
          headers: { Authorization: `Bearer ${descriptor.capability}`, "Content-Type": "application/json" },
        },
        (result) => {
          let body = "";
          result.on("data", (chunk) => (body += chunk));
          result.on("end", () => resolve(JSON.parse(body)));
        },
      );
      request.on("error", reject);
      request.end(
        JSON.stringify({
          jsonrpc: "2.0",
          id: 13,
          method: "tools/call",
          params: { name: "hmi_inspect_page", arguments: { contractVersion: 1, input: { file: "../outside" } } },
        }),
      );
    });
    const failure = rejected.result as { isError?: boolean; structuredContent?: { ok: boolean; code: string } };
    assert.strictEqual(failure.isError, true, "MCP marks domain rejection as a tool failure");
    assert.strictEqual(failure.structuredContent?.ok, false);
    assert.strictEqual(failure.structuredContent?.code, "INVALID_ARGUMENT");

    const listed = await new Promise<Record<string, unknown>>((resolve, reject) => {
      const request = http.request(
        descriptor.url,
        {
          method: "POST",
          headers: { Authorization: `Bearer ${descriptor.capability}`, "Content-Type": "application/json" },
        },
        (result) => {
          let body = "";
          result.on("data", (chunk) => (body += chunk));
          result.on("end", () => resolve(JSON.parse(body)));
        },
      );
      request.on("error", reject);
      request.end(JSON.stringify({ jsonrpc: "2.0", id: 12, method: "tools/list" }));
    });
    const bridgeTools = (listed.result as { tools: Array<{ name: string; annotations: { readOnlyHint: boolean } }> })
      .tools;
    assert.strictEqual(bridgeTools.length, 9, "MCP exposes the same nine authoring tools");
    assert.strictEqual(bridgeTools.find((tool) => tool.name === "hmi_apply_draft")?.annotations.readOnlyHint, false);
  });

  test("agent composes, edits, validates and renders an isolated draft", async () => {
    for (const name of [
      "hmi_compose_page",
      "hmi_edit_draft",
      "hmi_validate_draft",
      "hmi_render_draft",
      "hmi_preview_states",
      "hmi_apply_draft",
    ])
      assert.ok(
        vscode.lm.tools.some((tool) => tool.name === name),
        `Missing agent tool ${name}`,
      );
    async function call(name: string, input: object) {
      const result = await vscode.lm.invokeTool(
        name,
        {
          input: { contractVersion: 1, workspaceId: fixtureId, input: toolInput(name, input) },
          toolInvocationToken: undefined,
        },
        new vscode.CancellationTokenSource().token,
      );
      return { result, payload: JSON.parse((result.content[0] as vscode.LanguageModelTextPart).value) };
    }
    const composed = await call("hmi_compose_page", {
      file: "hmi/pages/agent-created.toml",
      title: "Agent tank",
      pattern: "process-overview",
      items: [
        {
          id: "tank",
          type: "tank",
          region: "process",
          min: 0,
          max: 3,
          bind: { level: { path: "Main.Level", direction: "read" } },
        },
        {
          id: "level",
          type: "value",
          region: "measurements",
          unit: "m",
          label: "Tank level",
          bind: { value: { path: "Main.Level", direction: "read" } },
        },
      ],
    });
    assert.strictEqual(composed.payload.ok, true, JSON.stringify(composed.payload));
    const draftId = composed.payload.data.draftId;
    const source = vscode.Uri.joinPath(fixtureRoot, "hmi/pages/agent-created.toml");
    await assert.rejects(Promise.resolve(vscode.workspace.fs.stat(source)), "source is untouched");
    const edited = await call("hmi_edit_draft", {
      draftId,
      revision: composed.payload.data.revision,
      key: "move-tank",
      operations: [{ op: "move", ids: ["tank"], delta: [8, 0] }],
    });
    assert.strictEqual(edited.payload.ok, true, JSON.stringify(edited.payload));
    assert.ok(
      edited.payload.data.objectChanges?.[0]?.modified?.some(
        (object: { id: string; properties: string[] }) => object.id === "tank" && object.properties.includes("at"),
      ),
      "edit returns its exact native-model change receipt",
    );
    const replay = await call("hmi_edit_draft", {
      draftId,
      revision: composed.payload.data.revision,
      key: "move-tank",
      operations: [{ delta: [8, 0], ids: ["tank"], op: "move" }],
    });
    assert.deepStrictEqual(replay.payload, edited.payload, "replayed edit returns its original outcome");
    const revision = edited.payload.data.revision;
    const checked = await call("hmi_validate_draft", { draftId, revision });
    assert.strictEqual(checked.payload.ok, true, JSON.stringify(checked.payload));
    assert.strictEqual(checked.payload.data.engineeringValid, true, JSON.stringify(checked.payload));
    const rendered = await call("hmi_render_draft", { draftId, revision, theme: "dark", crop: { ids: ["tank"] } });
    assert.strictEqual(rendered.payload.ok, true, JSON.stringify(rendered.payload));
    assert.ok(
      rendered.result.content.some(
        (part) => part instanceof vscode.LanguageModelDataPart && part.mimeType === "image/png",
      ),
    );
    assert.strictEqual(
      rendered.result.content.filter((part) => part instanceof vscode.LanguageModelDataPart).length,
      2,
      "whole page and requested native-scale crop must both reach the agent",
    );
    assert.strictEqual(rendered.payload.data.artifacts[1].kind, "crop");
    const states = await call("hmi_preview_states", { draftId, revision, states: ["stale", "unknown"] });
    assert.strictEqual(states.payload.ok, true, JSON.stringify(states.payload));
    assert.strictEqual(states.payload.data.frames.length, 2);
    assert.deepStrictEqual(
      states.payload.data.frames.map(
        (frame: { artifacts: Array<{ imageIndex: number }> }) => frame.artifacts[0].imageIndex,
      ),
      [0, 1],
      "each scenario manifest must refer to its own returned image",
    );
    const invalid = await call("hmi_edit_draft", {
      draftId,
      revision,
      key: "missing-binding",
      operations: [{ op: "bind", id: "level", property: "value", binding: { path: "Main.Gone", direction: "read" } }],
    });
    assert.strictEqual(invalid.payload.ok, false, JSON.stringify(invalid.payload));
    assert.strictEqual(
      invalid.payload.code,
      "VALIDATION_FAILED",
      "invalid binding edits must be rejected before draft mutation",
    );
    const unresolved = await call("hmi_edit_draft", {
      draftId,
      revision,
      key: "explicit-unresolved-binding",
      allowUnresolved: true,
      operations: [{ op: "bind", id: "level", property: "value", binding: { path: "Main.Gone", direction: "read" } }],
    });
    assert.strictEqual(unresolved.payload.ok, true, JSON.stringify(unresolved.payload));
    const findings = await call("hmi_validate_draft", { draftId, revision: unresolved.payload.data.revision });
    assert.strictEqual(findings.payload.data.engineeringValid, false);
    const applied = await call("hmi_apply_draft", {
      draftId,
      revision: unresolved.payload.data.revision,
      action: "apply",
    });
    assert.strictEqual(applied.payload.ok, false, "invalid binding must prevent application");
    const discarded = await call("hmi_apply_draft", {
      draftId,
      revision: unresolved.payload.data.revision,
      action: "discard",
    });
    assert.strictEqual(discarded.payload.ok, true);
    const discardReplay = await call("hmi_apply_draft", {
      draftId,
      revision: unresolved.payload.data.revision,
      action: "discard",
    });
    assert.deepStrictEqual(discardReplay.payload, discarded.payload, "discard retry returns the original outcome");
    await assert.rejects(Promise.resolve(vscode.workspace.fs.stat(source)));
  });

  test("reviewed valid draft applies through the editor and can be undone", async () => {
    async function call(name: string, input: object) {
      const result = await vscode.lm.invokeTool(
        name,
        {
          input: { contractVersion: 1, workspaceId: fixtureId, input: toolInput(name, input) },
          toolInvocationToken: undefined,
        },
        new vscode.CancellationTokenSource().token,
      );
      return JSON.parse((result.content[0] as vscode.LanguageModelTextPart).value);
    }
    const created = await call("hmi_compose_page", {
      file: "hmi/pages/agent-applied.toml",
      title: "Reviewed tank",
      pattern: "process-overview",
      items: [{ id: "level", type: "value", unit: "m", bind: { value: { path: "Main.Level", direction: "read" } } }],
    });
    assert.strictEqual(created.ok, true, JSON.stringify(created));
    const target = { draftId: created.data.draftId, revision: created.data.revision };
    const firstReview = await call("hmi_apply_draft", { ...target, action: "review" });
    assert.strictEqual(firstReview.ok, true);
    const refined = await call("hmi_edit_draft", {
      ...target,
      key: "after-review",
      operations: [{ op: "update", id: "level", properties: { label: "Reviewed level" } }],
    });
    assert.strictEqual(refined.ok, true, JSON.stringify(refined));
    assert.strictEqual(
      proposalFor(vscode.Uri.parse(firstReview.data.reviewUri))?.status,
      "stale",
      "editing invalidates the open review",
    );
    target.revision = refined.data.revision;
    const latestReview = await call("hmi_apply_draft", { ...target, action: "review" });
    assert.strictEqual(latestReview.ok, true);
    assert.strictEqual(
      (
        await call("hmi_edit_draft", {
          draftId: created.data.draftId,
          revision: created.data.revision,
          key: "after-review",
          operations: [{ op: "update", id: "level", properties: { label: "Reviewed level" } }],
        })
      ).ok,
      true,
    );
    assert.strictEqual(
      proposalFor(vscode.Uri.parse(latestReview.data.reviewUri))?.status,
      "ready",
      "an idempotent old edit retry does not invalidate the current review",
    );
    const applied = await call("hmi_apply_draft", { ...target, action: "apply", save: false });
    assert.strictEqual(applied.ok, true, JSON.stringify(applied));
    assert.strictEqual(applied.data.applied, true);
    const replay = await call("hmi_apply_draft", { ...target, action: "apply", save: false });
    assert.deepStrictEqual(replay, applied, "retry returns the durable original receipt without applying again");
    const uri = vscode.Uri.joinPath(fixtureRoot, "hmi/pages/agent-applied.toml");
    const doc = await vscode.workspace.openTextDocument(uri);
    assert.ok(doc.getText().includes("Reviewed tank"));
    await vscode.window.showTextDocument(doc);
    await vscode.commands.executeCommand("undo");
    const stillOpen = vscode.workspace.textDocuments.find((d) => d.uri.toString() === uri.toString() && !d.isClosed);
    assert.ok(!stillOpen || !stillOpen.getText().includes("Reviewed tank"), "Undo removes the applied text");
    await new Promise((resolve) => setTimeout(resolve, 500));
    assert.deepStrictEqual(
      vscode.languages.getDiagnostics(uri).filter((item) => item.source === "truST HMI"),
      [],
      `Undo must clear diagnostics of the deleted page (dirty=${stillOpen?.isDirty}, closed=${stillOpen?.isClosed})`,
    );
  });

  test("authoring uses customer agent tools rather than the retired model-calling command", async () => {
    const commands = await vscode.commands.getCommands(true);
    assert.ok(
      !commands.includes("trust-lsp.hmiBuilder.draftPage"),
      "retired direct-model command must not remain active",
    );
    assert.ok(commands.includes("trust-lsp.hmiBuilder.connectAgent"));
  });

  test("agent refines an existing dirty page without replacing the customer's other work", async () => {
    async function call(name: string, input: object) {
      const result = await vscode.lm.invokeTool(
        name,
        {
          input: { contractVersion: 1, workspaceId: fixtureId, input: toolInput(name, input) },
          toolInvocationToken: undefined,
        },
        new vscode.CancellationTokenSource().token,
      );
      return JSON.parse((result.content[0] as vscode.LanguageModelTextPart).value);
    }
    const file = "hmi/pages/process.toml";
    const created = await call("hmi_compose_page", { file, sourceFile: file });
    assert.strictEqual(created.ok, true, JSON.stringify(created));
    const edited = await call("hmi_edit_draft", {
      draftId: created.data.draftId,
      revision: created.data.revision,
      key: "label-existing",
      operations: [{ op: "update", id: "level", properties: { label: "Operating level" } }],
    });
    assert.strictEqual(edited.ok, true, JSON.stringify(edited));
    const target = { draftId: edited.data.draftId, revision: edited.data.revision };
    const review = await vscode.lm.invokeTool(
      "hmi_apply_draft",
      {
        input: { contractVersion: 1, workspaceId: fixtureId, input: { ...target, action: "review" } },
        toolInvocationToken: undefined,
      },
      new vscode.CancellationTokenSource().token,
    );
    const reviewed = JSON.parse((review.content[0] as vscode.LanguageModelTextPart).value);
    assert.strictEqual(reviewed.ok, true, JSON.stringify(reviewed));
    assert.deepStrictEqual(
      reviewed.data.objectChanges?.[0]?.modified,
      [{ id: "level", properties: ["label"] }],
      "review identifies the exact object properties changed",
    );
    const reviewImages = review.content.filter(
      (part) => part instanceof vscode.LanguageModelDataPart,
    ) as vscode.LanguageModelDataPart[];
    assert.strictEqual(reviewImages.length, 2, "review returns both the captured base and proposal images");
    assert.notDeepStrictEqual(
      reviewImages[0].data,
      reviewImages[1].data,
      "the two previews show the actual label change",
    );
    if (process.env.HMI_AGENT_RENDER_OUT) {
      await fs.mkdir(process.env.HMI_AGENT_RENDER_OUT, { recursive: true });
      await fs.writeFile(path.join(process.env.HMI_AGENT_RENDER_OUT, "proposal-before.png"), reviewImages[0].data);
      await fs.writeFile(path.join(process.env.HMI_AGENT_RENDER_OUT, "proposal-after.png"), reviewImages[1].data);
    }

    assert.deepStrictEqual(
      reviewed.data.previews.map((preview: { kind: string; imageIndex: number }) => [preview.kind, preview.imageIndex]),
      [
        ["before", 0],
        ["after", 1],
      ],
    );
    assert.ok(
      vscode.window.tabGroups.all
        .flatMap((group) => group.tabs)
        .some(
          (tab) =>
            tab.input instanceof vscode.TabInputCustom &&
            tab.input.viewType === "trust-lsp.hmiBuilder" &&
            tab.input.uri.scheme === "trust-hmi-proposal",
        ),
      "review opens an isolated proposal in the existing HMI Builder",
    );

    const applied = await call("hmi_apply_draft", { ...target, action: "apply", save: false });
    assert.strictEqual(applied.ok, true, JSON.stringify(applied));
    assert.strictEqual(
      proposalFor(vscode.Uri.parse(reviewed.data.reviewUri))?.status,
      "applied",
      "agent application updates the open review state",
    );
    const document = await vscode.workspace.openTextDocument(vscode.Uri.joinPath(fixtureRoot, file));
    assert.ok(document.getText().includes("Manual draft"), "manual title survives");
    assert.ok(document.getText().includes("Operating level"));
    assert.ok(document.isDirty);
    assert.ok(document.getText().includes("Main.Level"), "binding survives");
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

  test("custom project symbols appear in visual browsing and draft previews", async () => {
    const directory = vscode.Uri.joinPath(fixtureRoot, "hmi/symbols");
    await vscode.workspace.fs.createDirectory(directory);
    await vscode.workspace.fs.writeFile(
      vscode.Uri.joinPath(directory, "vessel.toml"),
      Buffer.from('format = 1\ntitle = "Project vessel"\nsize = [160, 160]\n'),
    );
    await vscode.workspace.fs.writeFile(
      vscode.Uri.joinPath(directory, "vessel.svg"),
      Buffer.from(
        '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"><circle cx="50" cy="50" r="40" fill="#22d3ee"/></svg>',
      ),
    );
    async function call(name: string, input: object) {
      const result = await vscode.lm.invokeTool(
        name,
        {
          input: { contractVersion: 1, workspaceId: fixtureId, input: toolInput(name, input) },
          toolInvocationToken: undefined,
        },
        new vscode.CancellationTokenSource().token,
      );
      return { result, payload: JSON.parse((result.content[0] as vscode.LanguageModelTextPart).value) };
    }
    const library = await call("hmi_browse_library", { query: "my:vessel" });
    assert.strictEqual(library.payload.ok, true, JSON.stringify(library.payload));
    assert.ok(
      library.result.content.some((part) => part instanceof vscode.LanguageModelDataPart),
      "custom component needs a real contact-sheet image",
    );
    assert.deepStrictEqual(library.payload.data.unpictured, []);
    const created = await call("hmi_compose_page", {
      file: "hmi/pages/custom-agent.toml",
      title: "Project vessel",
      pattern: "equipment-detail",
      items: [{ id: "vessel", type: "my:vessel" }],
    });
    assert.strictEqual(created.payload.ok, true, JSON.stringify(created.payload));
    const target = { draftId: created.payload.data.draftId, revision: created.payload.data.revision };
    try {
      const rendered = await call("hmi_render_draft", target);
      assert.strictEqual(rendered.payload.ok, true, JSON.stringify(rendered.payload));
      assert.ok(rendered.result.content.some((part) => part instanceof vscode.LanguageModelDataPart));
    } finally {
      await call("hmi_apply_draft", { ...target, action: "discard" });
    }
  });

  test("draft preview resolves placed faceplates using unsaved template text", async () => {
    await vscode.workspace.fs.writeFile(
      vscode.Uri.joinPath(fixtureRoot, "src/agent_faceplate.st"),
      Buffer.from(
        "FUNCTION_BLOCK AgentPump\nVAR_OUTPUT Level : REAL := 1.2; END_VAR\nEND_FUNCTION_BLOCK\nPROGRAM AgentMachine\nVAR P1 : AgentPump; END_VAR\nP1();\nEND_PROGRAM\n",
      ),
    );
    const directory = vscode.Uri.joinPath(fixtureRoot, "hmi/panels");
    await vscode.workspace.fs.createDirectory(directory);
    const uri = vscode.Uri.joinPath(directory, "agent-card.toml");
    const text =
      'format = 1\ntitle = "Agent card"\nsize = [280, 160]\n[fields.Level]\naccepts = ["number"]\n[[object]]\nid = "level"\ntype = "value"\nat = [16, 16]\nsize = [240, 64]\nlabel = "Saved level"\nunit = "m"\n[object.bind]\nvalue = "$Level"\n';
    await vscode.workspace.fs.writeFile(uri, Buffer.from(text));
    const template = await vscode.workspace.openTextDocument(uri);
    const edit = new vscode.WorkspaceEdit();
    edit.replace(uri, new vscode.Range(0, 0, template.lineCount, 0), text.replace("Saved level", "Unsaved level"));
    assert.ok(await vscode.workspace.applyEdit(edit));
    async function call(name: string, input: object) {
      const result = await vscode.lm.invokeTool(
        name,
        {
          input: { contractVersion: 1, workspaceId: fixtureId, input: toolInput(name, input) },
          toolInvocationToken: undefined,
        },
        new vscode.CancellationTokenSource().token,
      );
      return { result, payload: JSON.parse((result.content[0] as vscode.LanguageModelTextPart).value) };
    }
    const created = await call("hmi_compose_page", {
      file: "hmi/pages/agent-faceplate.toml",
      title: "Equipment",
      pattern: "equipment-detail",
      items: [
        { id: "card", type: "faceplate", faceplate: "agent-card", instance: "AgentMachine.P1", size: [320, 240] },
      ],
    });
    assert.strictEqual(created.payload.ok, true, JSON.stringify(created.payload));
    const target = { draftId: created.payload.data.draftId, revision: created.payload.data.revision };
    try {
      const rendered = await call("hmi_render_draft", { ...target, crop: { ids: ["card"] } });
      assert.strictEqual(rendered.payload.ok, true, JSON.stringify(rendered.payload));
      assert.ok(
        rendered.payload.data.placed.card.model.object.some(
          (object: { label?: string }) => object.label === "Unsaved level",
        ),
      );
      assert.strictEqual(
        rendered.result.content.filter((part) => part instanceof vscode.LanguageModelDataPart).length,
        2,
      );
      assert.ok(template.isDirty, "preview must not save the template");
      if (process.env.HMI_AGENT_RENDER_OUT) {
        const png = rendered.result.content.find(
          (part) => part instanceof vscode.LanguageModelDataPart,
        ) as vscode.LanguageModelDataPart;
        await fs.writeFile(path.join(process.env.HMI_AGENT_RENDER_OUT, "agent-faceplate.png"), png.data);
      }
    } finally {
      await call("hmi_apply_draft", { ...target, action: "discard" });
    }
  });

  test("agent renders the selected project language from unsaved translation text", async () => {
    const screenUri = vscode.Uri.joinPath(fixtureRoot, "hmi/screen.toml");
    const screenBefore = await vscode.workspace.fs.readFile(screenUri);
    await vscode.workspace.fs.writeFile(
      screenUri,
      Buffer.from('format = 1\nsize = [1280, 800]\ntheme = "dark"\nsymbols = "flat"\nlanguages = ["en", "sv"]\n'),
    );
    await vscode.workspace.fs.createDirectory(vscode.Uri.joinPath(fixtureRoot, "hmi/texts"));
    const uri = vscode.Uri.joinPath(fixtureRoot, "hmi/texts/sv.toml");
    await vscode.workspace.fs.writeFile(
      uri,
      Buffer.from('format = 1\nlanguage = "sv"\n[texts]\n"Tank overview" = "Sparad vy"\n'),
    );
    const document = await vscode.workspace.openTextDocument(uri);
    const edit = new vscode.WorkspaceEdit();
    edit.replace(
      uri,
      new vscode.Range(0, 0, document.lineCount, 0),
      document.getText().replace("Sparad vy", "Tanköversikt"),
    );
    await vscode.workspace.applyEdit(edit);
    async function call(name: string, input: object) {
      const result = await vscode.lm.invokeTool(
        name,
        {
          input: { contractVersion: 1, workspaceId: fixtureId, input: toolInput(name, input) },
          toolInvocationToken: undefined,
        },
        new vscode.CancellationTokenSource().token,
      );
      if (
        process.env.HMI_AGENT_RENDER_OUT &&
        name === "hmi_render_draft" &&
        (input as { language?: string }).language === "sv"
      ) {
        const png = result.content.find((part) => part instanceof vscode.LanguageModelDataPart) as
          vscode.LanguageModelDataPart | undefined;
        if (png) await fs.writeFile(path.join(process.env.HMI_AGENT_RENDER_OUT, "agent-swedish.png"), png.data);
      }
      return JSON.parse((result.content[0] as vscode.LanguageModelTextPart).value);
    }
    let target: { draftId: string; revision: string } | undefined;
    try {
      const created = await call("hmi_compose_page", {
        file: "hmi/pages/agent-language.toml",
        title: "Tank overview",
        pattern: "process-overview",
        items: [{ id: "level", type: "value", unit: "m", bind: { value: { path: "Main.Level", direction: "read" } } }],
      });
      assert.strictEqual(created.ok, true, JSON.stringify(created));
      target = { draftId: created.data.draftId, revision: created.data.revision };
      const rendered = await call("hmi_render_draft", { ...target, language: "sv" });
      assert.strictEqual(rendered.ok, true, JSON.stringify(rendered));
      assert.strictEqual(rendered.data.language, "sv");
      assert.ok(
        rendered.data.texts.includes("Tanköversikt"),
        "actual browser text uses the unsaved project translation",
      );
      assert.ok(document.isDirty);
      const longTranslation = new vscode.WorkspaceEdit();
      longTranslation.replace(
        uri,
        new vscode.Range(0, 0, document.lineCount, 0),
        document.getText().replace("Tanköversikt", "Väldigt lång processtitel ".repeat(16)),
      );
      assert.ok(await vscode.workspace.applyEdit(longTranslation));
      const checked = await call("hmi_validate_draft", target);
      assert.strictEqual(checked.ok, true, JSON.stringify(checked));
      assert.ok(
        checked.data.findings.some(
          (finding: { rule: string; object: string; message: string }) =>
            finding.rule === "cut-text" && finding.object === "page-heading" && finding.message.includes("Svenska"),
        ),
        "validation must report clipping in the project's unsaved translation",
      );
      const unsupported = await call("hmi_render_draft", { ...target, language: "invented" });
      assert.strictEqual(unsupported.ok, false);
      assert.strictEqual(unsupported.code, "INVALID_ARGUMENT");
    } finally {
      if (target) await call("hmi_apply_draft", { ...target, action: "discard" });
      await vscode.workspace.fs.writeFile(screenUri, screenBefore);
    }
  });

  test("published mutation schemas reject unknown fields before creating or changing a draft", async () => {
    async function call(name: string, input: object) {
      const result = await vscode.lm.invokeTool(
        name,
        {
          input: { contractVersion: 1, workspaceId: fixtureId, input: toolInput(name, input) },
          toolInvocationToken: undefined,
        },
        new vscode.CancellationTokenSource().token,
      );
      return JSON.parse((result.content[0] as vscode.LanguageModelTextPart).value);
    }
    const malformed = await call("hmi_compose_page", {
      file: "hmi/pages/invalid-schema.toml",
      title: "Invalid request",
      pattern: "process-overview",
      items: [],
      unsupportedOption: true,
    });
    try {
      assert.strictEqual(malformed.ok, false, "unknown fields must not silently create a draft");
      assert.strictEqual(malformed.code, "INVALID_ARGUMENT");
    } finally {
      if (malformed.ok)
        await call("hmi_apply_draft", {
          draftId: malformed.data.draftId,
          revision: malformed.data.revision,
          action: "discard",
        });
    }
    const created = await call("hmi_compose_page", {
      file: "hmi/pages/schema-edits.toml",
      title: "Schema edits",
      pattern: "process-overview",
      items: [{ id: "tank", type: "tank" }],
    });
    assert.strictEqual(created.ok, true, JSON.stringify(created));
    const target = { draftId: created.data.draftId, revision: created.data.revision };
    try {
      const edited = await call("hmi_edit_draft", {
        ...target,
        key: "malformed-move",
        operations: [{ op: "move", ids: ["tank"], delta: [8, 0], typo: true }],
      });
      assert.strictEqual(edited.ok, false);
      assert.strictEqual(edited.code, "INVALID_ARGUMENT");
      const unchanged = await call("hmi_validate_draft", target);
      assert.strictEqual(unchanged.ok, true, "rejected operation must not advance the draft revision");
    } finally {
      await call("hmi_apply_draft", { ...target, action: "discard" });
    }
  });

  test("creation retries return their original proposal and discarded proposals stay expired", async () => {
    async function call(name: string, input: object) {
      const result = await vscode.lm.invokeTool(
        name,
        {
          input: { contractVersion: 1, workspaceId: fixtureId, input: toolInput(name, input) },
          toolInvocationToken: undefined,
        },
        new vscode.CancellationTokenSource().token,
      );
      return JSON.parse((result.content[0] as vscode.LanguageModelTextPart).value);
    }
    const input = {
      key: "create-retry",
      file: "hmi/pages/retried-creation.toml",
      title: "Retry proposal",
      pattern: "process-overview",
      items: [{ id: "tank", type: "tank" }],
    };
    const first = await call("hmi_compose_page", input);
    assert.strictEqual(first.ok, true, JSON.stringify(first));
    try {
      const context = await call("hmi_get_context", {});
      assert.ok(
        context.data.drafts?.some((draft: { draftId: string }) => draft.draftId === first.data.draftId),
        "context must expose recoverable draft identifiers",
      );
      const replay = await call("hmi_compose_page", { ...input });
      assert.deepStrictEqual(replay, first);
      const changed = await call("hmi_compose_page", { ...input, title: "Different proposal" });
      assert.strictEqual(changed.ok, false);
      assert.strictEqual(changed.code, "CONFLICT");
    } finally {
      await call("hmi_apply_draft", { draftId: first.data.draftId, revision: first.data.revision, action: "discard" });
    }
    const second = await call("hmi_compose_page", {
      ...input,
      key: "discard-reviewed",
      file: "hmi/pages/discard-reviewed.toml",
    });
    const review = await call("hmi_apply_draft", {
      draftId: second.data.draftId,
      revision: second.data.revision,
      action: "review",
    });
    assert.strictEqual(review.ok, true, JSON.stringify(review));
    assert.strictEqual(
      (
        await call("hmi_apply_draft", {
          draftId: second.data.draftId,
          revision: second.data.revision,
          action: "discard",
        })
      ).ok,
      true,
    );
    assert.strictEqual(
      proposalFor(vscode.Uri.parse(review.data.reviewUri))?.status,
      "discarded",
      "discard updates the open review",
    );
    const expired = await call("hmi_compose_page", input);
    assert.strictEqual(expired.ok, false);
    assert.strictEqual(expired.code, "EXPIRED_DRAFT");
  });

  test("cancelled native requests retain a typed cancellation result", async () => {
    const cancellation = new vscode.CancellationTokenSource();
    const client = {
      sendRequest: async () => {
        cancellation.cancel();
        throw new vscode.CancellationError();
      },
    };
    const provider = (() => client) as unknown as import("../../lm-tools/shared").LspClientProvider;
    try {
      const result = await new HmiAgentTool("hmi_get_context", provider).invoke(
        { input: { contractVersion: 1, workspaceId: fixtureId, input: {} } },
        cancellation.token,
      );
      const payload = JSON.parse((result.content[0] as vscode.LanguageModelTextPart).value);
      assert.strictEqual(payload.ok, false);
      assert.strictEqual(payload.code, "CANCELLED");
    } finally {
      cancellation.dispose();
    }
  });

  test("late cancellation preserves a durably created proposal", async () => {
    const storageRoot = await fs.mkdtemp(path.join(os.tmpdir(), "trust-agent-created-cancel-"));
    const cancellation = new vscode.CancellationTokenSource();
    const client = {
      sendRequest: (_method: string, request: { command: string; arguments: unknown[] }) =>
        vscode.commands.executeCommand(request.command, ...request.arguments),
    };
    const provider = (() => client) as unknown as import("../../lm-tools/shared").LspClientProvider;
    const original = AuthoringService.prototype.execute;
    AuthoringService.prototype.execute = async function (...args: Parameters<typeof original>) {
      const outcome = await original.apply(this, args);
      if (args[0] === "hmi_compose_page") cancellation.cancel();
      return outcome;
    };
    try {
      const tool = new HmiAgentTool("hmi_compose_page", provider, {
        storageRoot,
        fontFile: path.resolve(__dirname, "../../../media/fonts/InterVariable.woff2"),
      });
      const result = await tool.invoke(
        {
          input: {
            contractVersion: 1,
            workspaceId: fixtureId,
            input: {
              key: "created-cancel",
              file: "hmi/pages/created-cancel.toml",
              title: "Saved proposal",
              pattern: "process-overview",
              items: [],
            },
          },
        },
        cancellation.token,
      );
      const records = await fs.readdir(path.join(storageRoot, fixtureId));
      assert.strictEqual(
        records.filter((file) => file.endsWith(".json")).length,
        1,
        "creation actually persisted before cancellation",
      );
      const payload = JSON.parse((result.content[0] as vscode.LanguageModelTextPart).value);
      assert.strictEqual(payload.ok, true, JSON.stringify(payload));
      assert.ok(payload.data.draftId);
    } finally {
      AuthoringService.prototype.execute = original;
      cancellation.dispose();
      await fs.rm(storageRoot, { recursive: true, force: true });
    }
  });

  test("a changed PLC source blocks applying an older design proposal", async () => {
    async function call(name: string, input: object) {
      const result = await vscode.lm.invokeTool(
        name,
        {
          input: { contractVersion: 1, workspaceId: fixtureId, input: toolInput(name, input) },
          toolInvocationToken: undefined,
        },
        new vscode.CancellationTokenSource().token,
      );
      return JSON.parse((result.content[0] as vscode.LanguageModelTextPart).value);
    }
    const created = await call("hmi_compose_page", {
      file: "hmi/pages/stale-program.toml",
      title: "Stale program",
      pattern: "process-overview",
      items: [{ id: "level", type: "value", unit: "m", bind: { value: { path: "Main.Level", direction: "read" } } }],
    });
    assert.strictEqual(created.ok, true, JSON.stringify(created));
    const target = { draftId: created.data.draftId, revision: created.data.revision };
    assert.strictEqual((await call("hmi_apply_draft", { ...target, action: "review" })).ok, true);
    const doc = await vscode.workspace.openTextDocument(vscode.Uri.joinPath(fixtureRoot, "src/main.st"));
    const before = doc.getText();
    const change = new vscode.WorkspaceEdit();
    change.replace(
      doc.uri,
      new vscode.Range(0, 0, doc.lineCount, 0),
      before.replace("Level : REAL;", "Level : REAL := 2.0;"),
    );
    await vscode.workspace.applyEdit(change);
    try {
      const applied = await call("hmi_apply_draft", { ...target, action: "apply" });
      assert.strictEqual(applied.ok, false, "source revision changed despite matching variable names");
      assert.strictEqual(applied.code, "CONFLICT");
    } finally {
      const restore = new vscode.WorkspaceEdit();
      restore.replace(doc.uri, new vscode.Range(0, 0, doc.lineCount, 0), before);
      await vscode.workspace.applyEdit(restore);
      await call("hmi_apply_draft", { ...target, action: "discard" });
    }
  });

  test("context follows unsaved ST instead of advertising stale bindings", async () => {
    const root = fixtureRoot;
    const document = await vscode.workspace.openTextDocument(vscode.Uri.joinPath(root, "src/main.st"));
    const before = document.getText();
    const edit = new vscode.WorkspaceEdit();
    edit.replace(
      document.uri,
      new vscode.Range(0, 0, document.lineCount, 0),
      before.replace("Level : REAL", "Unsaved : BOOL"),
    );
    assert.ok(await vscode.workspace.applyEdit(edit));
    const result = await vscode.lm.invokeTool(
      "hmi_get_context",
      { input: { contractVersion: 1, workspaceId: fixtureId, input: {} }, toolInvocationToken: undefined },
      new vscode.CancellationTokenSource().token,
    );
    const payload = JSON.parse((result.content[0] as vscode.LanguageModelTextPart).value);
    assert.strictEqual(payload.ok, true, JSON.stringify(payload));
    assert.ok(JSON.stringify(payload.data.variables).includes("Main.Unsaved"));
    assert.ok(!JSON.stringify(payload.data.variables).includes("Main.Level"));
    assert.strictEqual(payload.data.sourceState, "editor");
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

  test("operation deadlines cancel the actual native requests and leave caller cancellation independent", async () => {
    const tokens: vscode.CancellationToken[] = [];
    const releases: Array<() => void> = [];
    let started!: () => void;
    const ready = new Promise<void>((resolve) => {
      started = resolve;
    });
    const client = {
      sendRequest: (_method: string, _request: unknown, token: vscode.CancellationToken) =>
        new Promise((_resolve, reject) => {
          tokens.push(token);
          started();
          const abort = () => reject(new vscode.CancellationError());
          const listener = token.onCancellationRequested(abort);
          releases.push(() => {
            listener.dispose();
            abort();
          });
          if (token.isCancellationRequested) abort();
        }),
    };
    const getClient = (() => client) as unknown as import("../../lm-tools/shared").LspClientProvider;
    const parent = new vscode.CancellationTokenSource();
    const tool = Reflect.construct(HmiAgentTool, ["hmi_get_context", getClient, undefined, 1000]) as HmiAgentTool;
    const pending = tool.invoke({ input: { contractVersion: 1, workspaceId: fixtureId, input: {} } }, parent.token);
    try {
      await ready;
      await new Promise((resolve) => setTimeout(resolve, 1100));
      assert.ok(
        tokens.length > 0 && tokens.every((token) => token.isCancellationRequested),
        "deadline must reach the LSP cancellation tokens",
      );
      assert.strictEqual(parent.token.isCancellationRequested, false, "the request owns its deadline, not its caller");
      const result = await pending;
      const payload = JSON.parse((result.content[0] as vscode.LanguageModelTextPart).value);
      assert.strictEqual(payload.code, "CANCELLED");
    } finally {
      releases.forEach((release) => release());
      await pending;
      parent.dispose();
    }
  });

  test("cancellation ends read-only native calls even if the provider ignores its token", async () => {
    const releases: Array<() => void> = [];
    let started!: () => void;
    const ready = new Promise<void>((resolve) => {
      started = resolve;
    });
    const client = {
      sendRequest: () =>
        new Promise((resolve) => {
          releases.push(() => resolve({ ok: false, error: "released fixture" }));
          started();
        }),
    };
    const getClient = (() => client) as unknown as import("../../lm-tools/shared").LspClientProvider;
    const cancellation = new vscode.CancellationTokenSource();
    let settled = false;
    const pending = new HmiAgentTool("hmi_get_context", getClient)
      .invoke({ input: { contractVersion: 1, workspaceId: fixtureId, input: {} } }, cancellation.token)
      .then((result) => {
        settled = true;
        return result;
      });
    try {
      await ready;
      cancellation.cancel();
      await new Promise((resolve) => setTimeout(resolve, 80));
      assert.ok(settled, "a cancelled metadata request must not wait for a non-cooperating provider");
      const payload = JSON.parse(((await pending).content[0] as vscode.LanguageModelTextPart).value);
      assert.strictEqual(payload.code, "CANCELLED");
    } finally {
      releases.forEach((release) => release());
      await pending;
      cancellation.dispose();
    }
  });
});
