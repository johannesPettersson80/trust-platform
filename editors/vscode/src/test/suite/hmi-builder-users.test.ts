import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";

import { ADD_USER_COMMAND, REMOVE_USER_COMMAND, projectUsers } from "../../hmiBuilder/editor/hmiUsers";
import { disposeFixtureProject } from "./fixtureDocuments";

// Specification 39, §12.8: Screen settings add and remove the runtime display's users. A user is
// written to runtime.toml with the hash `trust-runtime hmi-password` makes, never the password. The
// rules (the name, the password's length, the file's text) are the language server's and are tested
// in Rust (`hmi_builder::users`); this suite proves the commands' wiring.

const RUNTIME = '[bundle]\nversion = 1\n\n[resource]\nname = "Res"\n\n[runtime.web]\nenabled = true\n';

suite("HMI Builder editor: users of the runtime display", function () {
  this.timeout(60000);
  const roots: string[] = [];
  let previous: string | undefined;

  suiteSetup(async () => {
    const config = vscode.workspace.getConfiguration("trust");
    previous = config.get<string>("runtime.executablePath");
    const runtime = process.env.ST_RUNTIME_TEST_BIN;
    if (runtime) await config.update("runtime.executablePath", runtime, vscode.ConfigurationTarget.Global);
  });

  suiteTeardown(async () => {
    await vscode.workspace
      .getConfiguration("trust")
      .update("runtime.executablePath", previous, vscode.ConfigurationTarget.Global);
  });

  teardown(async () => {
    for (const root of roots.splice(0)) await disposeFixtureProject(root);
    await vscode.commands.executeCommand("workbench.action.closeAllEditors");
  });

  test("a user is added with the password's hash, refused twice, and removed", async () => {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), "trust-hmi-users-"));
    roots.push(root);
    fs.mkdirSync(path.join(root, "hmi"));
    fs.writeFileSync(path.join(root, "hmi", "screen.toml"), "format = 1\n");
    const file = path.join(root, "runtime.toml");
    fs.writeFileSync(file, RUNTIME);
    const project = vscode.Uri.file(root).toString();
    const added = await vscode.commands.executeCommand(ADD_USER_COMMAND, {
      project,
      name: "anna",
      role: "operator",
      password: "correct horse",
    });
    assert.strictEqual(added, "anna");
    const text = fs.readFileSync(file, "utf8");
    assert.ok(text.startsWith(RUNTIME), "the file is kept, the user appended");
    assert.deepStrictEqual(await projectUsers(vscode.Uri.file(root)), [{ name: "anna", role: "operator" }]);
    assert.match(text, /\npassword = "pbkdf2-sha256:600000:[A-Za-z0-9_-]+:[A-Za-z0-9_-]+"\n/);
    assert.ok(!text.includes("correct horse"), "the password itself is never written");
    assert.strictEqual(
      await vscode.commands.executeCommand(ADD_USER_COMMAND, {
        project,
        name: "Anna",
        role: "engineer",
        password: "something else",
      }),
      undefined,
      "a name is taken whatever its case",
    );
    assert.strictEqual(await vscode.commands.executeCommand(REMOVE_USER_COMMAND, { project, name: "anna" }), true);
    assert.strictEqual(fs.readFileSync(file, "utf8"), RUNTIME, "saved as it was");
  });
});
