import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";

// Specification 16 §2: the Ladder editor's Validate and Save refuse a file that breaks the
// schema-v2 contract. The check is the language server's (`trust-lsp.visualValidate`, tested in
// crates/trust-visual); these tests prove the extension reaches it with the fixture files.

function fixturePath(name: string): string {
  const sourceFixture = path.resolve(__dirname, "..", "..", "..", "src", "test", "fixtures", "ladder", name);
  if (fs.existsSync(sourceFixture)) {
    return sourceFixture;
  }
  return path.resolve(__dirname, "..", "fixtures", "ladder", name);
}

interface ValidateAnswer {
  ok: boolean;
  errors?: Array<{ message: string }>;
}

async function validate(name: string): Promise<ValidateAnswer> {
  const text = fs.readFileSync(fixturePath(name), "utf8");
  for (let attempt = 0; attempt < 150; attempt += 1) {
    if ((await vscode.commands.getCommands(true)).includes("trust-lsp.visualValidate")) {
      break;
    }
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  const answer = await vscode.commands.executeCommand<ValidateAnswer>("trust-lsp.visualValidate", {
    kind: "ladder",
    text,
  });
  assert.ok(answer, "the language server answers trust-lsp.visualValidate");
  return answer;
}

suite("Ladder schema v2 fixtures", function () {
  this.timeout(30000);

  suiteSetup(async () => {
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
  });

  // Catches a Validate that accepts everything or never reaches the language server.
  test("accepts schema v2 fixtures", async () => {
    const answer = await validate("schema-v2-valid.ladder.json");
    assert.strictEqual(answer.ok, true, JSON.stringify(answer.errors));
  });

  test("rejects legacy schema fixtures with actionable error", async () => {
    const answer = await validate("schema-v1-legacy.ladder.json");
    assert.strictEqual(answer.ok, false);
    assert.match(answer.errors?.[0]?.message ?? "", /Unsupported ladder schema/);
  });

  test("rejects invalid enum symbols with actionable error", async () => {
    const answer = await validate("schema-v2-invalid-symbols.ladder.json");
    assert.strictEqual(answer.ok, false);
    assert.match(answer.errors?.[0]?.message ?? "", /coilType must be NORMAL, SET, RESET, or NEGATED/);
  });
});
