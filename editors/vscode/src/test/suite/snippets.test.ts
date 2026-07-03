import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";

type SnippetDef = {
  prefix: string | string[];
  body: string | string[];
  description?: string;
};

type SnippetMap = Record<string, SnippetDef>;

const EXPECTED_PREFIXES = [
  "ton-usage",
  "state-machine",
  "fb-template",
  "for-loop",
  "edge-detect",
] as const;

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function snippetFilePath(): string {
  return path.resolve(__dirname, "../../../snippets/st.code-snippets");
}

function readSnippetMap(): SnippetMap {
  const raw = fs.readFileSync(snippetFilePath(), "utf8");
  return JSON.parse(raw) as SnippetMap;
}

function toLines(body: string | string[]): string[] {
  return Array.isArray(body) ? body : body.split(/\r?\n/);
}

function expandSnippetBody(body: string | string[]): string {
  const text = toLines(body).join("\n");
  return text
    .replace(/\$\{\d+:([^}]+)\}/g, "$1")
    .replace(/\$\{\d+\|([^}]+)\|\}/g, (_, options: string) =>
      options.split(",")[0]
    )
    .replace(/\$\d+/g, "");
}

async function waitForNoErrors(
  uri: vscode.Uri,
  timeoutMs = 10000
): Promise<void> {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    const diagnostics = vscode.languages
      .getDiagnostics(uri)
      .filter((diag) => diag.severity === vscode.DiagnosticSeverity.Error);
    if (diagnostics.length === 0) {
      return;
    }
    await delay(200);
  }
  const diagnostics = vscode.languages
    .getDiagnostics(uri)
    .filter((diag) => diag.severity === vscode.DiagnosticSeverity.Error);
  assert.strictEqual(
    diagnostics.length,
    0,
    `Expected no errors, got: ${diagnostics
      .map((diag) => `${diag.code ?? ""} ${diag.message}`.trim())
      .join("; ")}`
  );
}

async function createDocument(
  fixturesRoot: vscode.Uri,
  name: string,
  contents: string
): Promise<vscode.TextDocument> {
  const uri = vscode.Uri.joinPath(fixturesRoot, name);
  await vscode.workspace.fs.writeFile(uri, Buffer.from(contents));
  let doc = await vscode.workspace.openTextDocument(uri);
  if (doc.languageId !== "structured-text") {
    doc = await vscode.languages.setTextDocumentLanguage(doc, "structured-text");
  }
  await vscode.window.showTextDocument(doc);
  return doc;
}

suite("Snippet contributions (VS Code)", function () {
  this.timeout(30000);
  let fixturesRoot: vscode.Uri;
  let previousSnippetSuggestions:
    | "top"
    | "bottom"
    | "inline"
    | "none"
    | undefined;
  let previousShowSnippets: boolean | undefined;

  suiteSetup(async () => {
    const workspaceFolder = vscode.workspace.workspaceFolders?.[0];
    assert.ok(workspaceFolder, "Expected a workspace folder for tests.");
    fixturesRoot = vscode.Uri.joinPath(
      workspaceFolder.uri,
      "tmp",
      "vscode-snippets"
    );
    await vscode.workspace.fs.createDirectory(fixturesRoot);

    const editorConfig = vscode.workspace.getConfiguration("editor");
    previousSnippetSuggestions = editorConfig.get<
      "top" | "bottom" | "inline" | "none" | undefined
    >("snippetSuggestions");
    previousShowSnippets = editorConfig.get<boolean | undefined>(
      "suggest.showSnippets"
    );
    await editorConfig.update(
      "snippetSuggestions",
      "top",
      vscode.ConfigurationTarget.Workspace
    );
    await editorConfig.update(
      "suggest.showSnippets",
      true,
      vscode.ConfigurationTarget.Workspace
    );
  });

  suiteTeardown(async () => {
    try {
      await vscode.workspace.fs.delete(fixturesRoot, {
        recursive: true,
        useTrash: false,
      });
    } catch {
      // Ignore cleanup failures in test teardown.
    }

    const editorConfig = vscode.workspace.getConfiguration("editor");
    await editorConfig.update(
      "snippetSuggestions",
      previousSnippetSuggestions,
      vscode.ConfigurationTarget.Workspace
    );
    await editorConfig.update(
      "suggest.showSnippets",
      previousShowSnippets,
      vscode.ConfigurationTarget.Workspace
    );
  });

  test("snippet JSON file is valid and includes required patterns", () => {
    const snippets = readSnippetMap();

    for (const prefix of EXPECTED_PREFIXES) {
      const entry = snippets[prefix];
      assert.ok(entry, `Missing snippet '${prefix}'.`);
      assert.ok(entry.description && entry.description.length > 0, `${prefix} should include description.`);

      const normalizedPrefixes = Array.isArray(entry.prefix)
        ? entry.prefix
        : [entry.prefix];
      assert.ok(
        normalizedPrefixes.includes(prefix),
        `${prefix} should be discoverable by its canonical prefix.`
      );

      const bodyText = toLines(entry.body).join("\n");
      const placeholderMatches = bodyText.match(/\$\{\d+:[A-Za-z][A-Za-z0-9_]*\}/g) ?? [];
      assert.ok(
        placeholderMatches.length > 0,
        `${prefix} should include meaningful named placeholders.`
      );
    }
  });

  test("snippet contribution is registered with identifier-friendly aliases", () => {
    const extension = vscode.extensions.getExtension("trust-platform.trust-lsp");
    assert.ok(extension, "Expected the truST extension to be loaded.");
    const contributedSnippets =
      (
        extension.packageJSON as {
          contributes?: {
            snippets?: Array<{ language?: string; path?: string }>;
          };
        }
      ).contributes?.snippets ?? [];
    assert.ok(
      contributedSnippets.some(
        (entry) =>
          entry.language === "structured-text" &&
          entry.path === "./snippets/st.code-snippets"
      ),
      "package.json must contribute the ST snippets to the structured-text language"
    );

    const snippets = readSnippetMap();
    for (const prefix of EXPECTED_PREFIXES) {
      const entry = snippets[prefix];
      assert.ok(entry, `Missing snippet '${prefix}'.`);
      const aliases = Array.isArray(entry.prefix) ? entry.prefix : [entry.prefix];
      assert.ok(
        aliases.includes(prefix),
        `${prefix} must keep the readable canonical alias`
      );
      assert.ok(
        aliases.some((candidate) => /^[A-Za-z_][A-Za-z0-9_]*$/.test(candidate)),
        `${prefix} must provide an identifier-safe alias for typed completion`
      );
      assert.ok(
        entry.description && entry.description.trim().length > 0,
        `${prefix} must include snippet documentation`
      );
    }
  });

  test("expanded snippet bodies are syntactically valid ST", async () => {
    const snippets = readSnippetMap();

    for (const prefix of EXPECTED_PREFIXES) {
      const entry = snippets[prefix];
      assert.ok(entry, `Missing snippet '${prefix}'.`);
      const expanded = expandSnippetBody(entry.body);
      const doc = await createDocument(fixturesRoot, `expanded-${prefix}.st`, expanded);
      await waitForNoErrors(doc.uri);
    }
  });
});
