import * as assert from "assert";
import * as fs from "fs/promises";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";
import { BundledExamplePreviews } from "../../examples/bundledFilePreview";

suite("Bundled example file preview", () => {
  let root: string;
  const entry = { id: "control-preview", path: "control/preview" };
  setup(async () => {
    root = await fs.mkdtemp(path.join(os.tmpdir(), "trust-example-preview-"));
    await fs.mkdir(path.join(root, entry.path, "src"), { recursive: true });
    await fs.writeFile(path.join(root, entry.path, "src/Main.st"), "PROGRAM Main\nEND_PROGRAM\n");
  });
  teardown(async () => { await fs.rm(root, { recursive: true, force: true }); });

  test("preserves the exact opened source when the bundled file later changes", async () => {
    const previews = new BundledExamplePreviews(vscode.Uri.file(root));
    const uri = await previews.prepare(entry, "src/Main.st", ["src/Main.st"]);
    const before = "PROGRAM Main\nEND_PROGRAM\n";
    assert.strictEqual(previews.provideTextDocumentContent(uri), before);
    await fs.writeFile(path.join(root, entry.path, "src/Main.st"), "PROGRAM Changed\nEND_PROGRAM\n");
    const replacement = await previews.prepare(entry, "src/Main.st", ["src/Main.st"]);
    assert.notStrictEqual(replacement.toString(), uri.toString());
    assert.strictEqual(previews.provideTextDocumentContent(uri), before);
    assert.match(previews.provideTextDocumentContent(replacement), /PROGRAM Changed/);
  });

  test("rejects unlisted targets, outside paths, schemes and fabricated preview URIs", async () => {
    const previews = new BundledExamplePreviews(vscode.Uri.file(root));
    for (const target of ["lesson.json", "../private.st", "/etc/passwd", "file:///etc/passwd", "src\\Main.st", "%2e%2e/private.st"]) {
      await assert.rejects(() => previews.prepare(entry, target, ["src/Main.st"]));
    }
    for (const target of ["../private.st", "/etc/passwd", "file:///etc/passwd", "src\\Main.st", "%2e%2e/private.st"]) {
      await assert.rejects(() => previews.prepare(entry, target, [target]), "even an authored target cannot escape the project");
    }
    const uri = await previews.prepare(entry, "src/Main.st", ["src/Main.st"]);
    assert.throws(() => previews.provideTextDocumentContent(uri.with({ path: "/private.st" })));
    await assert.rejects(() => previews.prepare({ ...entry, path: "../outside" }, "Main.st", ["Main.st"]));
  });

  test("rejects an authored link through a symlink outside the example", async function () {
    if (process.platform === "win32") this.skip(); // Creating symlinks can require Windows developer privileges.
    const outside = path.join(root, "outside.st");
    await fs.writeFile(outside, "private fixture");
    await fs.symlink(outside, path.join(root, entry.path, "src/linked.st"));
    const previews = new BundledExamplePreviews(vscode.Uri.file(root));
    await assert.rejects(() => previews.prepare(entry, "src/linked.st", ["src/linked.st"]));
  });

  test("rejects missing, binary and oversized files without creating a preview", async () => {
    const previews = new BundledExamplePreviews(vscode.Uri.file(root));
    await assert.rejects(() => previews.prepare(entry, "missing.st", ["missing.st"]));
    await fs.writeFile(path.join(root, entry.path, "binary.st"), Buffer.from([0, 255, 0]));
    await assert.rejects(() => previews.prepare(entry, "binary.st", ["binary.st"]));
    await fs.writeFile(path.join(root, entry.path, "large.st"), Buffer.alloc(2 * 1024 * 1024 + 1, 65));
    await assert.rejects(() => previews.prepare(entry, "large.st", ["large.st"]));
  });
});
