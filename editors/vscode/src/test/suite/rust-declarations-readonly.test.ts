import * as assert from "assert";
import * as vscode from "vscode";

// Specification 48 RU-50: a project's generated declarations of its Rust POUs are read-only, so an edit of them is
// refused; the file's own header names the Rust source to change. The extension marks every
// `src/generated/rust_pous.st` read-only through VS Code's `files.readonlyInclude`.
suite("Rust POU declarations (specification 48 RU-50)", () => {
  test("the generated declarations of a project's Rust POUs are read-only", () => {
    const readonly = vscode.workspace
      .getConfiguration("files")
      .get<Record<string, boolean>>("readonlyInclude");
    assert.strictEqual(readonly?.["**/src/generated/rust_pous.st"], true);
  });
});
