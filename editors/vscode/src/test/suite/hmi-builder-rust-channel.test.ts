import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";

import { disposeFixtureProject } from "./fixtureDocuments";
import { languageServerReady, openBuilder, postedOfType, until } from "./hmiBuilderHost";

// Specification 39, §17.3 (AGENTS.md: all logic is in Rust): the webview asks the language server's
// HMI Builder rules through the editor with a `rust` message; the editor answers each with the
// language server's answer, `rustAnswer`, and forwards no other command.

const fixtureProjects: string[] = [];

function project(): string {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "trust-hmi-rust-"));
  fixtureProjects.push(root);
  fs.mkdirSync(path.join(root, "hmi", "pages"), { recursive: true });
  fs.mkdirSync(path.join(root, "src"));
  fs.writeFileSync(path.join(root, "src", "main.st"), "PROGRAM Main\nEND_PROGRAM\n");
  fs.writeFileSync(path.join(root, "hmi", "pages", "overview.toml"), 'format = 1\ntitle = "Overview"\n');
  return root;
}

suite("HMI Builder editor: the webview's questions to the language server", function () {
  this.timeout(60000);
  suiteSetup(languageServerReady);
  teardown(async () => {
    for (const root of fixtureProjects.splice(0)) await disposeFixtureProject(root);
  });

  test("a question to one of the rules is answered with the language server's answer", async () => {
    const root = project();
    const { webview, panel } = await openBuilder(path.join(root, "hmi", "pages", "overview.toml"));
    const page = {
      title: "Overview",
      object: [{ id: "value-1", type: "value", at: [0, 0], size: [160, 48], rotation: 0, bind: {} }],
      pipe: [],
    };
    webview.send({
      type: "rust",
      id: 7,
      command: "trust-lsp.hmiClipboard",
      argument: { action: "copy", page, ids: ["value-1"] },
    });
    const answered = await until("the answer", () =>
      postedOfType(webview, "rustAnswer").find((message) => message.id === 7),
    );
    const answer = answered.answer as { ok?: boolean; text?: string };
    assert.strictEqual(answer.ok, true, JSON.stringify(answer));
    const copied = JSON.parse(answer.text ?? "{}");
    assert.deepStrictEqual(
      [copied["trust-hmi-objects"], copied.objects?.[0]?.id],
      [1, "value-1"],
      "the copy as the language server makes it",
    );
    panel.dispose();
  });

  test("a command that is not one of the HMI Builder's rules is not forwarded", async () => {
    const root = project();
    const { webview, panel } = await openBuilder(path.join(root, "hmi", "pages", "overview.toml"));
    webview.send({ type: "rust", id: 8, command: "workbench.action.closeAllEditors", argument: {} });
    const answered = await until("the answer", () =>
      postedOfType(webview, "rustAnswer").find((message) => message.id === 8),
    );
    assert.deepStrictEqual(answered.answer, {
      ok: false,
      error: "workbench.action.closeAllEditors is not one of the HMI Builder's rules.",
    });
    panel.dispose();
  });
});
