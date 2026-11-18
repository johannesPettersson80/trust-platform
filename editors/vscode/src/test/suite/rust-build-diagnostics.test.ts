import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";

import { __testShowBuildDiagnostics } from "../../rustBuildDiagnostics";

// Specification 25 §6.10 and specification 48 RU-44: Problems shows the errors and warnings that
// the debug adapter reports after a Simulator build of a project with Rust POUs (the event
// `trustBuildDiagnostics`, whose body the adapter's tests pin), each at its place, with the source
// `truST (rustc)`, in place of the last build's. This proves the extension's wiring: the places it
// shows are the adapter's 1-based places on VS Code's 0-based positions.
suite("Rust crate build diagnostics in Problems (specification 25 §6.10)", function () {
  let projectRoot: string;
  let lib: vscode.Uri;
  let pou: vscode.Uri;

  suiteSetup(async () => {
    const extension = vscode.extensions.getExtension("trust-platform.trust-lsp");
    assert.ok(extension, "Expected trust-lsp extension to be installed for tests.");
    await extension!.activate();
    projectRoot = fs.mkdtempSync(path.join(os.tmpdir(), "trust-rust-build-diagnostics-"));
    fs.mkdirSync(path.join(projectRoot, "rust", "src"), { recursive: true });
    fs.writeFileSync(path.join(projectRoot, "rust", "src", "lib.rs"), "");
    fs.writeFileSync(path.join(projectRoot, "rust", "src", "pou.rs"), "");
    lib = vscode.Uri.file(path.join(projectRoot, "rust", "src", "lib.rs"));
    pou = vscode.Uri.file(path.join(projectRoot, "rust", "src", "pou.rs"));
  });

  suiteTeardown(() => {
    __testShowBuildDiagnostics({ source: "truST (rustc)", diagnostics: [] });
    fs.rmSync(projectRoot, { recursive: true, force: true });
  });

  test("a build's error and warning are shown at their places with the build's source", () => {
    // The adapter's E0308 at line 10, columns 21 to 27, of lib.rs is VS Code's line 9,
    // characters 20 to 26; a warning in another file is shown there. Catches 1-based places
    // shown one line and one column late, a lost code or source, and a warning shown as an error.
    __testShowBuildDiagnostics({
      source: "truST (rustc)",
      diagnostics: [
        {
          path: lib.fsPath,
          severity: "error",
          code: "E0308",
          message: "mismatched types",
          line: 10,
          column: 21,
          end_line: 10,
          end_column: 27,
        },
        {
          path: pou.fsPath,
          severity: "warning",
          code: "unused_variables",
          message: "unused variable: `spare`",
          line: 3,
          column: 9,
          end_line: 3,
          end_column: 14,
        },
      ],
    });
    const [error] = vscode.languages.getDiagnostics(lib).filter((found) => found.source === "truST (rustc)");
    assert.ok(error, "the error is in Problems");
    assert.strictEqual(error.message, "mismatched types");
    assert.strictEqual(error.code, "E0308");
    assert.strictEqual(error.severity, vscode.DiagnosticSeverity.Error);
    assert.deepStrictEqual(
      [error.range.start.line, error.range.start.character, error.range.end.line, error.range.end.character],
      [9, 20, 9, 26]
    );
    const [warning] = vscode.languages.getDiagnostics(pou).filter((found) => found.source === "truST (rustc)");
    assert.ok(warning, "the warning is in Problems");
    assert.strictEqual(warning.severity, vscode.DiagnosticSeverity.Warning);
    assert.strictEqual(warning.code, "unused_variables");
    assert.deepStrictEqual(
      [warning.range.start.line, warning.range.start.character, warning.range.end.line, warning.range.end.character],
      [2, 8, 2, 13]
    );
  });

  test("the next build's diagnostics replace the last build's", () => {
    // Specification 25 §6.10: a build without diagnostics clears the last build's, and a build
    // with other diagnostics shows only those. Catches diagnostics that pile up over builds and
    // an error that stays after the crate was fixed.
    __testShowBuildDiagnostics({
      source: "truST (rustc)",
      diagnostics: [
        {
          path: pou.fsPath,
          severity: "error",
          code: null,
          message: "an error without a code",
          line: 1,
          column: 1,
          end_line: 1,
          end_column: 2,
        },
      ],
    });
    assert.strictEqual(
      vscode.languages.getDiagnostics(lib).filter((found) => found.source === "truST (rustc)").length,
      0,
      "lib.rs's error of the last build is gone"
    );
    const [only] = vscode.languages.getDiagnostics(pou).filter((found) => found.source === "truST (rustc)");
    assert.strictEqual(only?.message, "an error without a code");
    assert.strictEqual(only?.code, undefined);
    __testShowBuildDiagnostics({ source: "truST (rustc)", diagnostics: [] });
    assert.strictEqual(
      vscode.languages.getDiagnostics(pou).filter((found) => found.source === "truST (rustc)").length,
      0,
      "a build without diagnostics clears them"
    );
  });
});
