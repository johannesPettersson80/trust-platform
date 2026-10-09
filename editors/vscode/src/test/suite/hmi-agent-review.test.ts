import * as assert from "assert";
import * as fs from "fs/promises";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";
import { AuthoringService } from "../../hmiBuilder/agent/service";
import { WorkspaceDocuments } from "../../hmiBuilder/agent/documents";

// Spec 43 §6: unsuccessful preview production cannot certify a proposal for application.
suite("HMI agent review completion", () => {
  let root: string;
  setup(async () => {
    root = await fs.mkdtemp(path.join(os.tmpdir(), "trust-agent-review-"));
  });
  teardown(async () => {
    await fs.rm(root, { recursive: true, force: true });
  });
  test("a renderer resource failure leaves the exact proposal unreviewed", async () => {
    const service = new AuthoringService(
      "fixture",
      new WorkspaceDocuments(vscode.Uri.file(root)),
      async (name, args) => {
        if (name === "trust-lsp.hmiFileParse")
          return { ok: true, model: { format: 1, title: "Review", object: [], pipe: [] } };
        if (name === "trust-lsp.hmiFileCheck") {
          assert.strictEqual(args.authoring, true, "review uses declaration-only checks");
          return { ok: true, problems: [] };
        }
        throw new Error(`Unexpected native request ${name}`);
      },
      path.join(root, "private"),
      path.join(root, "missing-font.woff2"),
    );
    service.queries.context = async () => ({
      screen: { size: [640, 360], theme: "dark" },
      catalogue: { format: 1, components: [] },
      symbols: [],
      variables: [],
      formats: {},
      texts: { languages: [{ tag: "en", name: "English" }], builtin: {}, project: {} },
    });
    const draft = await service.store.create("fixture", [], { "hmi/pages/review.toml": "fixture" });
    await assert.rejects(
      service.execute(
        "hmi_apply_draft",
        { draftId: draft.id, revision: draft.revision, action: "review" },
        new AbortController().signal,
      ),
      /ENOENT|font/i,
    );
    assert.strictEqual(
      (await service.store.get(draft.id)).state,
      "draft",
      "failed preview must not mark the proposal reviewed",
    );
  });
});
