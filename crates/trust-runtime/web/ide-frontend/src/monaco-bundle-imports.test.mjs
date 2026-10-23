import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const sourcePath = path.join(__dirname, "monaco-bundle.ts");
const source = fs.readFileSync(sourcePath, "utf8");
const { default: config } = await import("../vite.monaco.config.ts");

const requiredImports = [
  "monaco-editor/editor/contrib/gotoSymbol/browser/goToCommands",
  "monaco-editor/editor/contrib/gotoSymbol/browser/link/goToDefinitionAtPosition",
  "monaco-editor/editor/contrib/rename/browser/rename",
  "monaco-editor/editor/standalone/browser/referenceSearch/standaloneReferenceSearch",
];

for (const modulePath of requiredImports) {
  test(`monaco bundle keeps required contribution import: ${modulePath}`, () => {
    assert.match(
      source,
      new RegExp(`import\\s+"${modulePath.replaceAll("/", "\\/")}";`),
    );
  });
}

test("Monaco CSS resolves its exact inline asset import", () => {
  const specifier = "monaco-editor/editor/editor.main.css?inline";
  assert.ok(source.includes(`import editorCss from "${specifier}";`));
  const replacement = config.resolve.alias[specifier];
  assert.equal(
    replacement,
    path.resolve(__dirname, "../node_modules/monaco-editor/min/vs/editor/editor.main.css") + "?inline",
  );
  assert.ok(fs.statSync(replacement.slice(0, -"?inline".length)).isFile());
});

test("Monaco's vendored sanitizer import resolves the locked upstream patch", () => {
  const specifier = "./dompurify/dompurify.js";
  const owner = fs.readFileSync(
    path.resolve(__dirname, "../node_modules/monaco-editor/esm/vs/base/browser/domSanitize.js"),
    "utf8",
  );
  assert.match(owner, /import purify from '\.\/dompurify\/dompurify\.js';/);
  const replacement = config.resolve.alias[specifier];
  assert.equal(
    replacement,
    path.resolve(__dirname, "../node_modules/dompurify/dist/purify.es.mjs"),
  );
  const upstream = fs.readFileSync(replacement, "utf8");
  assert.match(upstream, /DOMPurify\.version = ["']3\.4\.16["']/);
});
