const fs = require("fs");
const path = require("path");

const extensionRoot = path.resolve(__dirname, "..");
const repositoryRoot = path.resolve(extensionRoot, "..", "..");
const sourceRoot = process.env.TRUST_EXAMPLES_SOURCE_ROOT
  ? path.resolve(process.env.TRUST_EXAMPLES_SOURCE_ROOT)
  : path.join(repositoryRoot, "examples");
const catalogPath = path.join(sourceRoot, "catalog.json");
const outputRoot = process.env.TRUST_EXAMPLES_OUTPUT_ROOT
  ? path.resolve(process.env.TRUST_EXAMPLES_OUTPUT_ROOT)
  : path.join(extensionRoot, "media", "examples");

const catalogText = fs.readFileSync(catalogPath, "utf8");
const entries = JSON.parse(catalogText);
if (!Array.isArray(entries)) {
  throw new Error("examples/catalog.json must contain an array");
}

// The catalog's packaging check (specification 25 §3.1) is trust-lsp's: its test
// `the_repository_catalog_passes_the_packaging_check` checks this catalog in every `cargo test`, so CI
// and the release refuse a broken catalog before a package is built. This script only copies.

// media/examples is package output. Maintainers edit only examples/ so the
// repository, docs, tests, and extension cannot acquire competing copies.
fs.rmSync(outputRoot, { recursive: true, force: true });
fs.mkdirSync(outputRoot, { recursive: true });
for (const entry of entries) {
  fs.cpSync(
    path.join(sourceRoot, entry.path),
    path.join(outputRoot, entry.path),
    { recursive: true, errorOnExist: true }
  );
}
fs.writeFileSync(path.join(outputRoot, "manifest.json"), catalogText);

console.log(`Bundled ${entries.length} examples from examples/catalog.json`);
