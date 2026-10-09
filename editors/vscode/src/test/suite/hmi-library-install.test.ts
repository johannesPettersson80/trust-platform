import * as assert from "assert";
import * as fs from "fs/promises";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";
import { disposeFixtureProject } from "./fixtureDocuments";
import { prepareBundledLibraries } from "../../hmiBuilder/editor/libraryEdit";

suite("HMI bundled library installation", () => {
  test("packaged Plant sources and visuals match their repository owner", async () => {
    const extension = path.resolve(__dirname, "../../..");
    const source = path.resolve(extension, "../../libraries/plant");
    const packaged = path.join(extension, "media/libraries/plant");
    const files = async (root: string, prefix = ""): Promise<string[]> => {
      const result: string[] = [];
      for (const entry of await fs.readdir(path.join(root, prefix), { withFileTypes: true })) {
        if (["target", "node_modules", ".git", "__pycache__", ".trust-lsp"].includes(entry.name)) continue;
        const relative = path.join(prefix, entry.name);
        if (entry.isDirectory()) result.push(...(await files(root, relative)));
        else if (entry.isFile()) result.push(relative);
      }
      return result.sort();
    };
    const expected = await files(source);
    assert.deepStrictEqual(await files(packaged), expected, "one packaged Plant inventory");
    for (const file of expected)
      assert.deepStrictEqual(
        await fs.readFile(path.join(packaged, file)),
        await fs.readFile(path.join(source, file)),
        file,
      );
    await assert.rejects(
      fs.access(path.join(extension, "media/libraries/equipment")),
      "no competing Equipment package",
    );
  });
  test("Plant brings nested OSCAT dependencies in one edit without installing another equipment implementation", async () => {
    const root = await fs.mkdtemp(path.join(os.tmpdir(), "trust-plant-library-"));
    try {
      const base = path.join(root, "extension", "media", "libraries");
      const write = async (file: string, text: string) => {
        await fs.mkdir(path.dirname(file), { recursive: true });
        await fs.writeFile(file, text);
      };
      await write(
        path.join(base, "plant", "trust-lsp.toml"),
        '[package]\nname="Plant"\nversion="0.1.0"\n[dependencies]\nOscatOop={path="../oscat/oop",version="0.1.0"}\n',
      );
      await write(
        path.join(base, "oscat", "oop", "trust-lsp.toml"),
        '[package]\nname="OscatOop"\nversion="0.1.0"\n[dependencies]\nOSCAT={path="..",version="0.1.0"}\n',
      );
      await write(path.join(base, "oscat", "trust-lsp.toml"), '[package]\nname="OSCAT"\nversion="0.1.0"\n');
      await write(path.join(base, "plant", "src", "pump.st"), "FUNCTION_BLOCK Pump END_FUNCTION_BLOCK\n");
      await write(path.join(base, "plant", "examples", "demo", "src", "Main.st"), "PROGRAM Main END_PROGRAM\n");
      // A new project's manifest, not open in an editor.
      const project = path.join(root, "project");
      await write(path.join(project, "trust-lsp.toml"), '[project]\ninclude_paths = ["src"]\n');
      const edit = new vscode.WorkspaceEdit();
      const check = await prepareBundledLibraries(
        vscode.Uri.file(project),
        path.join(root, "extension"),
        [{ name: "Plant", folder: "plant" }],
        edit,
      );
      await check();
      assert.deepStrictEqual(
        await fs.readdir(project),
        ["trust-lsp.toml"],
        "preparing the edit does not mutate the project",
      );
      await assert.rejects(
        prepareBundledLibraries(
          vscode.Uri.file(project),
          path.join(root, "extension"),
          [{ name: "Bad", folder: "../outside" }],
          new vscode.WorkspaceEdit(),
        ),
        /package collection/,
      );
      assert.strictEqual(await vscode.workspace.applyEdit(edit), true);
      for (const relative of ["oscat/oop/trust-lsp.toml", "oscat/trust-lsp.toml", "plant/src/pump.st"]) {
        assert.deepStrictEqual(
          await fs.readFile(path.join(project, "libraries", relative)),
          await fs.readFile(path.join(base, relative)),
          relative,
        );
      }
      assert.ok(
        !vscode.workspace.textDocuments.some((document) =>
          document.uri.fsPath.startsWith(path.join(project, "libraries") + path.sep),
        ),
        "installing a package must not open all its sources as editor documents",
      );
      await assert.rejects(fs.access(path.join(project, "libraries", "equipment")));
      await assert.rejects(
        fs.access(path.join(project, "libraries", "plant", "examples")),
        "a library's example projects stay in the library",
      );
      assert.match(
        await fs.readFile(path.join(project, "trust-lsp.toml"), "utf8"),
        /Plant\s*=\s*\{[^}]*path\s*=\s*"libraries\/plant"/,
        "the closed manifest on disk declares the library",
      );
    } finally {
      await disposeFixtureProject(root);
    }
  });
});
