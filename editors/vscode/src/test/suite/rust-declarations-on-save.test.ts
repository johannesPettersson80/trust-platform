import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/** The text of `file` once it holds `expected`, within `timeoutMs`. */
async function waitForText(file: string, expected: string, timeoutMs: number): Promise<string> {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    if (fs.existsSync(file)) {
      const text = fs.readFileSync(file, "utf8");
      if (text.includes(expected)) {
        return text;
      }
    }
    await delay(250);
  }
  throw new Error(`${file} does not hold ${expected} after ${timeoutMs} ms`);
}

// Specification 48 RU-24: the extension runs `trust-dev rust declarations` when a `.rs` file of the
// project's crate is saved, so the generated declarations follow the crate without a build.
suite("Rust POU declarations on save (specification 48 RU-24)", function () {
  this.timeout(120000);
  let projectRoot: string;

  suiteSetup(() => {
    projectRoot = fs.mkdtempSync(path.join(os.tmpdir(), "trust-rust-declarations-"));
    fs.mkdirSync(path.join(projectRoot, "src"));
    fs.mkdirSync(path.join(projectRoot, "rust", "src"), { recursive: true });
    fs.writeFileSync(path.join(projectRoot, "trust-lsp.toml"), 'include_paths = ["src"]\n');
    fs.writeFileSync(path.join(projectRoot, "src", "main.st"), "PROGRAM Main\nEND_PROGRAM\n");
    fs.writeFileSync(
      path.join(projectRoot, "rust", "Cargo.toml"),
      '[package]\nname = "line"\nversion = "0.1.0"\nedition = "2024"\n'
    );
    fs.writeFileSync(
      path.join(projectRoot, "rust", "src", "lib.rs"),
      '#[trust_function(name = "MAX_LIMITED")]\n#[budget("2us")]\n' +
        "pub fn max_limited(_cx: &mut Cx, a: i32, b: i32, limit: i32) -> Result<i32, Fault> {\n" +
        "    Ok(a.max(b).min(limit))\n}\n"
    );
  });

  suiteTeardown(async () => {
    await vscode.commands.executeCommand("workbench.action.closeAllEditors");
    fs.rmSync(projectRoot, { recursive: true, force: true });
  });

  test("saving a Rust file of the crate writes the generated declarations", async () => {
    // Catches an extension that leaves the declarations to the build, which then refuses the
    // project as out of date: RU-23's file appears with RU-22's declaration once lib.rs is saved.
    const lib = vscode.Uri.file(path.join(projectRoot, "rust", "src", "lib.rs"));
    const document = await vscode.workspace.openTextDocument(lib);
    await vscode.window.showTextDocument(document);
    const edit = new vscode.WorkspaceEdit();
    edit.insert(lib, new vscode.Position(document.lineCount, 0), "\n");
    assert.ok(await vscode.workspace.applyEdit(edit), "the edit applies");
    assert.ok(await document.save(), "lib.rs saves");
    const written = await waitForText(
      path.join(projectRoot, "src", "generated", "rust_pous.st"),
      "FUNCTION MAX_LIMITED : DINT",
      90000
    );
    assert.ok(written.includes("FUNCTION MAX_LIMITED : DINT"), written);
  });
});
