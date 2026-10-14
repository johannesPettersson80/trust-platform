import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";
import * as engine from "../../hmiBuilder/agent/engineCommands";
import { applyDraft, reviewDraft } from "../../hmiBuilder/agent/proposalFlow";
import { disposeFixtureProject } from "./fixtureDocuments";
import { languageServerReady } from "./hmiBuilderHost";

// Specification 43, §3 and §4: each authoring command runs in trust-lsp, and the extension forwards the request and
// returns the answer unchanged. These tests call every forwarded function against the real language server on a
// temporary HMI project. The rules are proved by the Rust tests; these prove the wiring, the answer envelope, the
// unsaved buffers, the cancellation of reads and mutations, and one reviewed application through the editor.

const WORKSPACE_ID = "hmi-agent-engine-commands";

const PROGRAM = [
  "PROGRAM Transfer",
  "VAR",
  "    Level : REAL;",
  "END_VAR",
  "    Level := Level + 1.0;",
  "END_PROGRAM",
  "",
  "CONFIGURATION Config",
  "RESOURCE Res ON PLC",
  "TASK Fast(INTERVAL := T#100ms, PRIORITY := 1);",
  "PROGRAM Main WITH Fast : Transfer;",
  "END_RESOURCE",
  "END_CONFIGURATION",
  "",
].join("\n");

const OVERVIEW = [
  "format = 1",
  'title = "Overview"',
  "",
  "[[object]]",
  'id = "level"',
  'type = "value"',
  "at = [240, 120]",
  "size = [160, 48]",
  "",
  "[object.bind]",
  'value = "Main.Level"',
  "",
].join("\n");

const PICTURE = [
  "format = 1",
  'title = "Picture"',
  "",
  "[[object]]",
  'id = "logo"',
  'type = "image"',
  "at = [240, 120]",
  "size = [160, 96]",
  'image = "hmi/assets/missing.png"',
  "",
].join("\n");

const PROJECT_FILES: Record<string, string> = {
  "src/main.st": PROGRAM,
  "hmi/screen.toml": "format = 1\nsize = [1280, 720]\ntheme = \"dark\"\nsymbols = \"flat\"\n\n[navigation]\nstyle = \"none\"\n",
  "hmi/pages/overview.toml": OVERVIEW,
};

const roots: string[] = [];
const storages: string[] = [];

/** A temporary project and its draft folder, both outside the workspace, with the files written. */
function fixture(files: Record<string, string> = PROJECT_FILES): { root: string; project: engine.EngineProject } {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "trust-hmi-engine-"));
  const storage = fs.mkdtempSync(path.join(os.tmpdir(), "trust-hmi-engine-storage-"));
  roots.push(root);
  storages.push(storage);
  for (const [file, text] of Object.entries(files)) {
    const full = path.join(root, file);
    fs.mkdirSync(path.dirname(full), { recursive: true });
    fs.writeFileSync(full, text);
  }
  const source = new vscode.CancellationTokenSource();
  return {
    root,
    project: {
      root: vscode.Uri.file(root),
      workspaceId: WORKSPACE_ID,
      storage,
      token: source.token,
    },
  };
}

/** A request envelope for the connected workspace. */
function envelope(input: Record<string, unknown>): unknown {
  return engine.envelopeFor(WORKSPACE_ID, input);
}

/** The answer of a command that must succeed; a refusal fails the test with its code. */
function answered(answer: engine.AuthoringResult): Extract<engine.AuthoringResult, { ok: true }> {
  if (!answer.ok) assert.fail(`refused: ${JSON.stringify(answer)}`);
  return answer;
}

/** The refusal of a command that must refuse. */
function refused(answer: engine.AuthoringResult): Extract<engine.AuthoringResult, { ok: false }> {
  if (answer.ok) assert.fail(`answered, not refused: ${JSON.stringify(answer)}`);
  return answer;
}

/** A new draft copied from the project's overview page. */
async function copiedDraft(
  project: engine.EngineProject,
  key: string,
  file: string,
  source = "hmi/pages/overview.toml",
): Promise<{ draftId: string; revision: string }> {
  const composed = answered(
    await engine.compose(project, envelope({ key, file, sourceFile: source })),
  );
  const draftId = String(composed.data.draftId);
  return { draftId, revision: String(composed.data.revision) };
}

suite("HMI agent: the forwarded authoring commands", function () {
  this.timeout(120000);

  suiteSetup(languageServerReady);

  teardown(async () => {
    for (const root of roots.splice(0)) await disposeFixtureProject(root);
    for (const storage of storages.splice(0)) fs.rmSync(storage, { recursive: true, force: true });
    await vscode.commands.executeCommand("workbench.action.closeAllEditors");
  });

  test("each forwarded command answers the shared envelope on a real project", async () => {
    const { project } = fixture();

    const context = answered(await engine.context(project, envelope({ sections: ["pages"] })));
    assert.deepStrictEqual(
      (context.data.pages as Array<{ file: string }>).map((page) => page.file),
      ["hmi/pages/overview.toml"],
    );

    const library = answered(await engine.library(project, envelope({ query: "value" })));
    assert.ok(Array.isArray(library.data.components), JSON.stringify(library.data));
    assert.ok(Array.isArray(library.data.compositionPatterns), JSON.stringify(library.data));

    const inspected = answered(await engine.inspect(project, envelope({ file: "hmi/pages/overview.toml" })));
    assert.ok(typeof inspected.data === "object" && inspected.data !== null, JSON.stringify(inspected));

    const { draftId, revision } = await copiedDraft(project, "copy-1", "hmi/pages/copy.toml");
    const listed = answered(await engine.drafts(project, "list"));
    assert.ok(JSON.stringify(listed.data.drafts).includes(draftId), JSON.stringify(listed.data));

    const read = answered(await engine.drafts(project, "read", draftId));
    assert.ok(Object.keys(read.data.files as object).includes("hmi/pages/copy.toml"), JSON.stringify(read.data));

    const edited = answered(
      await engine.edit(
        project,
        envelope({
          draftId,
          revision,
          key: "move-1",
          file: "hmi/pages/copy.toml",
          operations: [{ op: "move", ids: ["level"], delta: [8, 0] }],
        }),
      ),
    );
    const moved = String(edited.data.revision);
    assert.notStrictEqual(moved, revision);

    const validated = answered(await engine.validate(project, envelope({ draftId, revision: moved })));
    assert.strictEqual(typeof validated.data.engineeringValid, "boolean", JSON.stringify(validated.data));

    const prepared = answered(await engine.review(project, "prepare", envelope({ draftId, revision: moved, action: "review" })));
    assert.strictEqual((prepared.data.pages as unknown[]).length, 1, JSON.stringify(prepared.data));

    const checked = answered(await engine.checkInput(project, "hmi_render_draft", envelope({ draftId, revision: moved })));
    assert.strictEqual(checked.data.valid, true, JSON.stringify(checked.data));

    answered(await engine.drafts(project, "cleanup"));

    const discarded = answered(await engine.discard(project, envelope({ draftId, revision: moved, action: "discard" })));
    assert.strictEqual(discarded.data.discarded, true, JSON.stringify(discarded.data));
  });

  test("a malformed input and an unsupported contract version come back as typed refusals", async () => {
    const { project } = fixture();

    const unknownSection = refused(await engine.context(project, envelope({ sections: ["not-a-section"] })));
    assert.strictEqual(unknownSection.code, "INVALID_ARGUMENT");
    assert.strictEqual(unknownSection.retryable, false);

    const version = refused(
      await engine.context(project, { contractVersion: 2, workspaceId: WORKSPACE_ID, input: {} }),
    );
    assert.strictEqual(version.code, "UNSUPPORTED_VERSION");
  });

  test("the answer reflects an unsaved edit of an open page", async () => {
    const { project, root } = fixture();
    const pagePath = path.join(root, "hmi/pages/overview.toml");
    const document = await vscode.workspace.openTextDocument(vscode.Uri.file(pagePath));
    const editor = await vscode.window.showTextDocument(document, { preview: false });
    const titleLine = document.getText().split("\n").indexOf('title = "Overview"');
    assert.ok(titleLine >= 0, "the fixture page has its title line");
    const replaced = await editor.edit((builder) =>
      builder.replace(document.lineAt(titleLine).range, 'title = "Unsaved overview"'),
    );
    assert.strictEqual(replaced, true);
    assert.strictEqual(document.isDirty, true);

    const context = answered(await engine.context(project, envelope({ sections: ["pages"] })));
    const page = (context.data.pages as Array<{ file: string; title: string; dirty: boolean }>).find(
      (entry) => entry.file === "hmi/pages/overview.toml",
    );
    assert.deepStrictEqual({ title: page?.title, dirty: page?.dirty }, { title: "Unsaved overview", dirty: true });
  });

  test("a cancelled discard keeps the draft, and a cancelled review never records a review", async () => {
    const { project } = fixture();
    const { draftId, revision } = await copiedDraft(project, "copy-2", "hmi/pages/copy-2.toml");
    const cancelled = new vscode.CancellationTokenSource();
    cancelled.cancel();
    const cancelledProject: engine.EngineProject = { ...project, token: cancelled.token };

    await assert.rejects(
      engine.discard(cancelledProject, envelope({ draftId, revision, action: "discard" })),
      (error: unknown) => error instanceof vscode.CancellationError,
    );
    const kept = answered(await engine.drafts(project, "read", draftId));
    assert.strictEqual(kept.data.state, "draft", JSON.stringify(kept.data));

    await assert.rejects(
      reviewDraft(
        cancelledProject,
        "",
        envelope({ draftId, revision, action: "review" }),
        undefined,
        new AbortController().signal,
      ),
      (error: unknown) => error instanceof vscode.CancellationError,
    );
    const unreviewed = answered(await engine.drafts(project, "read", draftId));
    assert.strictEqual(unreviewed.data.state, "draft", JSON.stringify(unreviewed.data));
  });

  test("a review whose project image is missing is refused and leaves the draft unreviewed", async () => {
    const { project } = fixture({ ...PROJECT_FILES, "hmi/pages/picture.toml": PICTURE });
    const { draftId, revision } = await copiedDraft(project, "picture-copy", "hmi/pages/picture-copy.toml", "hmi/pages/picture.toml");

    await assert.rejects(
      reviewDraft(
        project,
        "",
        envelope({ draftId, revision, action: "review" }),
        undefined,
        new AbortController().signal,
      ),
      (error: unknown) => error instanceof Error && error.message.startsWith("RENDER_FAILED:"),
    );
    const unreviewed = answered(await engine.drafts(project, "read", draftId));
    assert.strictEqual(unreviewed.data.state, "draft", JSON.stringify(unreviewed.data));
  });

  test("a reviewed draft is applied as one editor transaction, and its replay writes nothing", async () => {
    const { project, root } = fixture();
    const { draftId, revision } = await copiedDraft(project, "copy-3", "hmi/pages/copy-3.toml");
    const read = answered(await engine.drafts(project, "read", draftId));
    const text = (read.data.files as Record<string, string>)["hmi/pages/copy-3.toml"];
    answered(await engine.review(project, "record", envelope({ draftId, revision, action: "review" })));

    const request = envelope({ draftId, revision, action: "apply", save: true });
    const applied = await applyDraft(project, request, new AbortController().signal);
    assert.strictEqual(applied.data.draftId, draftId, JSON.stringify(applied.data));
    assert.strictEqual(fs.readFileSync(path.join(root, "hmi/pages/copy-3.toml"), "utf8"), text);

    const replay = await applyDraft(project, request, new AbortController().signal);
    assert.strictEqual(replay.data.committed, true, JSON.stringify(replay.data));

    const retired = refused(await engine.drafts(project, "read", draftId));
    assert.strictEqual(retired.code, "EXPIRED_DRAFT");
  });

  test("a project whose program does not compile still lists its pages and names the failure", async () => {
    const { project } = fixture({
      "src/main.st": "PROGRAM Broken\nVAR\n    x : ;\nEND_PROGRAM\n",
      "hmi/screen.toml": "format = 1\nsize = [1280, 720]\ntheme = \"dark\"\nsymbols = \"flat\"\n\n[navigation]\nstyle = \"none\"\n",
      "hmi/pages/overview.toml": OVERVIEW,
    });
    const context = answered(await engine.context(project, envelope({ sections: ["pages", "variables"] })));
    const compilation = context.data.compilation as { status: string; errors: string[] };
    assert.strictEqual(compilation.status, "failed", JSON.stringify(context.data));
    assert.ok(compilation.errors.length > 0, JSON.stringify(compilation));
    assert.strictEqual(context.data.partial, true);
    assert.strictEqual(context.data.variables, null);
    assert.deepStrictEqual(
      (context.data.pages as Array<{ file: string }>).map((page) => page.file),
      ["hmi/pages/overview.toml"],
    );
  });
});
