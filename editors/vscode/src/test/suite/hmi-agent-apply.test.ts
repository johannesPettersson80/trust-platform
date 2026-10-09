import * as assert from "assert";
import * as fs from "fs/promises";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";
import { WorkspaceDocuments } from "../../hmiBuilder/agent/documents";
import { applyDraftFiles } from "../../hmiBuilder/agent/apply";
import { DraftStore, Draft } from "../../hmiBuilder/agent/drafts";

suite("HMI agent editor transactions", function () {
  this.timeout(30000);
  let root: string;
  let documents: WorkspaceDocuments;
  setup(async () => {
    root = await fs.mkdtemp(path.join(os.tmpdir(), "trust-agent-apply-"));
    await fs.mkdir(path.join(root, "hmi/pages"), { recursive: true });
    await fs.writeFile(path.join(root, "hmi/pages/a.toml"), "original");
    documents = new WorkspaceDocuments(vscode.Uri.file(root));
  });
  teardown(async () => {
    for (const doc of vscode.workspace.textDocuments.filter((d) => d.uri.fsPath.startsWith(root + path.sep)))
      if (doc.isDirty) await doc.save();
    await fs.rm(root, { recursive: true, force: true });
  });
  async function draft(files: Record<string, string>): Promise<Draft> {
    return {
      id: "fixture",
      workspaceId: "fixture",
      revision: "1",
      state: "reviewed",
      findings: [],
      files,
      base: await Promise.all(Object.keys(files).map((file) => documents.read(file))),
    };
  }
  test("updates through the editor without implicit save and supports Undo", async () => {
    const file = "hmi/pages/a.toml";
    const doc = await vscode.workspace.openTextDocument(vscode.Uri.file(path.join(root, file)));
    await vscode.window.showTextDocument(doc);
    const proposal = await draft({ [file]: "agent change" });
    const outcome = await applyDraftFiles(documents, proposal, false, new AbortController().signal);
    assert.strictEqual(outcome.applied, true);
    assert.strictEqual(doc.getText(), "agent change");
    assert.ok(doc.isDirty);
    assert.strictEqual(await fs.readFile(doc.uri.fsPath, "utf8"), "original");
    await vscode.commands.executeCommand("undo");
    assert.strictEqual(doc.getText(), "original");
  });
  test("intervening manual edit prevents every draft file from changing", async () => {
    const proposal = await draft({ "hmi/pages/a.toml": "agent", "hmi/pages/new.toml": "new" });
    await fs.writeFile(path.join(root, "hmi/pages/a.toml"), "customer");
    await assert.rejects(applyDraftFiles(documents, proposal, false, new AbortController().signal), /CONFLICT/);
    assert.strictEqual(await fs.readFile(path.join(root, "hmi/pages/a.toml"), "utf8"), "customer");
    await assert.rejects(fs.stat(path.join(root, "hmi/pages/new.toml")));
  });
  test("creates a new page and explicit save persists its actual content", async () => {
    const proposal = await draft({ "hmi/pages/new.toml": "new page" });
    const outcome = await applyDraftFiles(documents, proposal, true, new AbortController().signal);
    assert.strictEqual(outcome.applied, true);
    assert.deepStrictEqual(outcome.saved, ["hmi/pages/new.toml"]);
    assert.strictEqual(await fs.readFile(path.join(root, "hmi/pages/new.toml"), "utf8"), "new page");
  });
  test("cancelled and unbased writes cannot change the project", async () => {
    const proposal = await draft({ "hmi/pages/a.toml": "agent" });
    const controller = new AbortController();
    controller.abort();
    await assert.rejects(applyDraftFiles(documents, proposal, false, controller.signal), /CANCELLED/);
    proposal.base = [];
    await assert.rejects(applyDraftFiles(documents, proposal, false, new AbortController().signal), /CONFLICT/);
    assert.strictEqual(await fs.readFile(path.join(root, "hmi/pages/a.toml"), "utf8"), "original");
  });
  // Spec 43 repairs of 7 October 2026, item 2: a conflict means the page text changed since the
  // draft was prepared; opening or re-versioning an unchanged page is no conflict.
  test("opening the prepared pages without changing them does not block application", async () => {
    const file = "hmi/pages/a.toml",
      other = "hmi/pages/b.toml";
    await fs.writeFile(path.join(root, other), "other page");
    const proposal = await draft({ [file]: "agent change" });
    proposal.base.push(await documents.read(other));
    const before = proposal.base.map((snapshot) => snapshot.revision);
    for (const page of [file, other])
      await vscode.window.showTextDocument(
        await vscode.workspace.openTextDocument(vscode.Uri.file(path.join(root, page))),
        { preview: false },
      );
    assert.notDeepStrictEqual(
      (await Promise.all([file, other].map((page) => documents.read(page)))).map((snapshot) => snapshot.revision),
      before,
      "fixture: opening changes the observed revision, not the text",
    );
    const outcome = await applyDraftFiles(documents, proposal, false, new AbortController().signal);
    assert.strictEqual(outcome.applied, true);
    const document = await vscode.workspace.openTextDocument(vscode.Uri.file(path.join(root, file)));
    assert.strictEqual(document.getText(), "agent change");
    assert.strictEqual(await fs.readFile(path.join(root, other), "utf8"), "other page");
  });
  test("an edit undone by a later edit leaves the text, so it is no conflict", async () => {
    const file = "hmi/pages/a.toml";
    const proposal = await draft({ [file]: "agent change" });
    const document = await vscode.workspace.openTextDocument(vscode.Uri.file(path.join(root, file)));
    for (const text of ["temporary", "original"]) {
      const edit = new vscode.WorkspaceEdit();
      edit.replace(document.uri, new vscode.Range(0, 0, document.lineCount, 0), text);
      assert.ok(await vscode.workspace.applyEdit(edit));
    }
    assert.ok(document.version > 1, "fixture: the page was re-versioned");
    const outcome = await applyDraftFiles(documents, proposal, false, new AbortController().signal);
    assert.strictEqual(outcome.applied, true);
    assert.strictEqual(document.getText(), "agent change");
  });
  test("changed text in an open page is a conflict and nothing is written", async () => {
    const file = "hmi/pages/a.toml";
    const proposal = await draft({ [file]: "agent change", "hmi/pages/new.toml": "new" });
    const document = await vscode.workspace.openTextDocument(vscode.Uri.file(path.join(root, file)));
    const edit = new vscode.WorkspaceEdit();
    edit.replace(document.uri, new vscode.Range(0, 0, document.lineCount, 0), "customer change");
    assert.ok(await vscode.workspace.applyEdit(edit));
    await assert.rejects(
      applyDraftFiles(documents, proposal, false, new AbortController().signal),
      /CONFLICT: hmi\/pages\/a\.toml changed/,
    );
    assert.strictEqual(document.getText(), "customer change", "the customer's unsaved text survives");
    await assert.rejects(fs.stat(path.join(root, "hmi/pages/new.toml")), "no other page of the draft was created");
  });
  test("a byte order mark on disk is not a change once the page is opened", async () => {
    const file = "hmi/pages/a.toml";
    await fs.writeFile(path.join(root, file), "﻿original");
    const proposal = await draft({ [file]: "agent change" });
    assert.strictEqual(proposal.base[0].text, "original", "disk text is read as the editor model holds it");
    const document = await vscode.workspace.openTextDocument(vscode.Uri.file(path.join(root, file)));
    const outcome = await applyDraftFiles(documents, proposal, false, new AbortController().signal);
    assert.strictEqual(outcome.applied, true);
    assert.strictEqual(document.getText(), "agent change");
  });
  test("a definite pre-commit cancellation leaves the same application key retryable", async () => {
    const store = new DraftStore(path.join(root, "private"));
    const proposal = await draft({ "hmi/pages/new.toml": "new page" });
    const stored = await store.create("fixture", proposal.base, proposal.files);
    const cancelled = new AbortController();
    cancelled.abort();
    await assert.rejects(
      store.apply(stored.id, stored.revision, "retry", "same", () =>
        applyDraftFiles(documents, stored, false, cancelled.signal),
      ),
      /CANCELLED/,
    );
    await assert.rejects(fs.stat(path.join(root, "hmi/pages/new.toml")));
    const applied = await new DraftStore(path.join(root, "private")).apply(
      stored.id,
      stored.revision,
      "retry",
      "same",
      () => applyDraftFiles(documents, stored, true, new AbortController().signal),
    );
    assert.strictEqual(applied.applied, true);
    assert.strictEqual(await fs.readFile(path.join(root, "hmi/pages/new.toml"), "utf8"), "new page");
  });
});
