import * as assert from "assert";
import * as fs from "fs/promises";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";
import { workspaceIdentity } from "../../hmiBuilder/agent/tools";
import { disposeFixtureProject } from "./fixtureDocuments";
import { languageServerReady } from "./hmiBuilderHost";

const svg = (fill: string) =>
  `<svg xmlns="http://www.w3.org/2000/svg" width="160" height="80" viewBox="0 0 160 80"><rect width="160" height="80" rx="12" fill="${fill}"/><circle cx="40" cy="40" r="20" fill="white"/></svg>`;
suite("HMI agent project images", function () {
  this.timeout(60000);
  let root: vscode.Uri;
  async function folder(add: boolean) {
    const folders = vscode.workspace.workspaceFolders ?? [];
    const index = add ? folders.length : folders.findIndex((item) => item.uri.toString() === root.toString());
    await new Promise<void>((resolve, reject) => {
      const timer = setTimeout(() => {
        listener.dispose();
        reject(new Error("fixture workspace change timed out"));
      }, 10000);
      const listener = vscode.workspace.onDidChangeWorkspaceFolders(() => {
        clearTimeout(timer);
        listener.dispose();
        resolve();
      });
      if (!vscode.workspace.updateWorkspaceFolders(index, add ? 0 : 1, ...(add ? [{ uri: root }] : []))) {
        clearTimeout(timer);
        listener.dispose();
        reject(new Error("fixture workspace change refused"));
      }
    });
  }
  async function call(name: string, input: object) {
    const cancellation = new vscode.CancellationTokenSource();
    try {
      const result = await vscode.lm.invokeTool(
        name,
        { input: { contractVersion: 1, workspaceId: workspaceIdentity(root), input }, toolInvocationToken: undefined },
        cancellation.token,
      );
      return { result, payload: JSON.parse((result.content[0] as vscode.LanguageModelTextPart).value) };
    } finally {
      cancellation.dispose();
    }
  }
  setup(async () => {
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    await languageServerReady();
    root = vscode.Uri.file(await fs.mkdtemp(path.join(os.tmpdir(), "trust-agent-assets-")));
    for (const directory of ["src", "hmi/pages", "hmi/assets"])
      await fs.mkdir(path.join(root.fsPath, directory), { recursive: true });
    await fs.writeFile(path.join(root.fsPath, "src/main.st"), "PROGRAM AssetFixture\nEND_PROGRAM\n");
    await fs.writeFile(
      path.join(root.fsPath, "hmi/screen.toml"),
      'format = 1\nsize = [640, 360]\ntheme = "dark"\nsymbols = "flat"\n',
    );
    await fs.writeFile(path.join(root.fsPath, "hmi/assets/logo.svg"), svg("#0088cc"));
    await folder(true);
  });
  teardown(async () => {
    for (const document of vscode.workspace.textDocuments)
      if (document.uri.fsPath.startsWith(root.fsPath + path.sep) && document.isDirty) await document.save();
    await disposeFixtureProject(root.fsPath);
    await folder(false);
  });
  test("actual previews use project images and dirty SVG text, and changed images block stale apply", async () => {
    const file = "hmi/pages/image-page.toml",
      asset = "hmi/assets/logo.svg";
    const created = await call("hmi_compose_page", {
      key: "asset-preview",
      file,
      title: "Project logo",
      operatorTask: "Identify the project logo",
      rationale: "Keep branding separate from process controls.",
      assumptions: ["The logo is supplied by the project owner."],
      pattern: "process-overview",
      items: [{ id: "logo", type: "image", image: asset, size: [320, 160] }],
    });
    assert.strictEqual(created.payload.ok, true, JSON.stringify(created.payload));
    assert.strictEqual(created.payload.data.intent?.operatorTask, "Identify the project logo");
    assert.ok(created.payload.data.intent?.assumptions.includes("The logo is supplied by the project owner."));
    const target = { draftId: created.payload.data.draftId, revision: created.payload.data.revision };
    const first = await call("hmi_render_draft", target);
    assert.strictEqual(first.payload.ok, true, JSON.stringify(first.payload));
    assert.ok(first.payload.data.assetRevisions[asset]);
    assert.ok(first.payload.data.manifest, "the public tool needs a bound image manifest");
    assert.strictEqual(first.payload.data.manifest.draftId, target.draftId);
    assert.strictEqual(first.payload.data.manifest.draftRevision, target.revision);
    assert.match(first.payload.data.manifest.fileSha256, /^[a-f0-9]{64}$/);
    assert.match(first.payload.data.manifest.renderer.sha256, /^[a-f0-9]{64}$/);
    assert.strictEqual(first.payload.data.manifest.origin, "synthetic");
    const document = await vscode.workspace.openTextDocument(vscode.Uri.joinPath(root, asset));
    const change = new vscode.WorkspaceEdit();
    change.replace(document.uri, new vscode.Range(0, 0, document.lineCount, 0), svg("#bb5500"));
    assert.ok(await vscode.workspace.applyEdit(change));
    const second = await call("hmi_render_draft", target);
    assert.strictEqual(second.payload.ok, true, JSON.stringify(second.payload));
    assert.notStrictEqual(first.payload.data.assetRevisions[asset], second.payload.data.assetRevisions[asset]);
    const png = (value: typeof first) =>
      value.result.content.find((part) => part instanceof vscode.LanguageModelDataPart) as vscode.LanguageModelDataPart;
    assert.notDeepStrictEqual(png(first).data, png(second).data, "the actual raster changes with the unsaved asset");
    assert.strictEqual(
      await fs.readFile(document.uri.fsPath, "utf8"),
      svg("#0088cc"),
      "preview does not save the asset",
    );
    assert.strictEqual((await call("hmi_apply_draft", { ...target, action: "review" })).payload.ok, true);
    const changedAgain = new vscode.WorkspaceEdit();
    changedAgain.replace(document.uri, new vscode.Range(0, 0, document.lineCount, 0), svg("#007733"));
    assert.ok(await vscode.workspace.applyEdit(changedAgain));
    const rejected = await call("hmi_apply_draft", { ...target, action: "apply" });
    assert.strictEqual(rejected.payload.ok, false);
    assert.strictEqual(rejected.payload.code, "CONFLICT");
    await assert.rejects(fs.stat(path.join(root.fsPath, file)), "stale image review cannot create source");
    const refreshed = await call("hmi_apply_draft", { ...target, action: "review" });
    assert.strictEqual(refreshed.payload.ok, true, JSON.stringify(refreshed.payload));
    assert.deepStrictEqual(
      refreshed.payload.data.intent,
      created.payload.data.intent,
      "intent survives draft storage and review",
    );
    const retried = await call("hmi_apply_draft", { ...target, action: "apply" });
    assert.strictEqual(
      retried.payload.ok,
      true,
      "a definite asset preflight failure must allow apply after a fresh review: " + JSON.stringify(retried.payload),
    );
    await call("hmi_apply_draft", { ...target, action: "discard" });
    if (process.env.HMI_AGENT_RENDER_OUT) {
      await fs.mkdir(process.env.HMI_AGENT_RENDER_OUT, { recursive: true });
      await fs.writeFile(path.join(process.env.HMI_AGENT_RENDER_OUT, "project-image.png"), png(second).data);
    }
  });
  test("rejects external SVG style resources instead of silently rendering an incomplete image", async () => {
    const asset = path.join(root.fsPath, "hmi/assets/logo.svg");
    const created = await call("hmi_compose_page", {
      key: "unsupported-assets",
      file: "hmi/pages/external.toml",
      title: "External resource",
      pattern: "process-overview",
      items: [{ id: "logo", type: "image", image: "hmi/assets/logo.svg", size: [320, 160] }],
    });
    assert.strictEqual(created.payload.ok, true, JSON.stringify(created.payload));
    const target = { draftId: created.payload.data.draftId, revision: created.payload.data.revision };
    const cases = [
      '<style>@import url("https://example.invalid/theme.css");</style>',
      '<style>rect { fill: url("https://example.invalid/paint.svg#paint"); }</style>',
      '<rect width="100" height="80" style="fill:url(https://example.invalid/paint.svg#paint)"/>',
      '<rect width="100" height="80" fill="url(https://example.invalid/paint.svg#paint)"/>',
      '<animate attributeName="opacity" values="0;1" dur="1s" repeatCount="indefinite"/>',
    ];
    for (const content of cases) {
      await fs.writeFile(asset, svg("#0088cc").replace("</svg>", content + "</svg>"));
      const result = await call("hmi_render_draft", target);
      assert.strictEqual(result.payload.ok, false, `unsupported resource accepted: ${content}`);
      assert.strictEqual(result.payload.code, "RENDER_FAILED", JSON.stringify(result.payload));
    }
    await call("hmi_apply_draft", { ...target, action: "discard" });
  });
});
