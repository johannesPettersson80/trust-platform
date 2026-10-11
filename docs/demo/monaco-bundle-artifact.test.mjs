import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";

const bundlePath = path.resolve(
  process.cwd(),
  "docs/demo/assets/ide-monaco.20260215.js",
);
const bundle = fs.readFileSync(bundlePath, "utf8");

test("demo monaco bundle contains references controller", () => {
  assert.match(bundle, /editor\.contrib\.referencesController/);
});

test("demo monaco bundle contains rename controller", () => {
  assert.match(bundle, /editor\.contrib\.renameController/);
});

test("demo monaco bundle contains rename action id", () => {
  assert.match(bundle, /editor\.action\.rename/);
});

test("runtime Monaco bundle keeps reference and rename contributions", () => {
  const runtimeBundle = fs.readFileSync(
    path.resolve(process.cwd(), "crates/trust-runtime/src/web/ui/assets/ide-monaco.20260215.js"),
    "utf8",
  );
  assert.match(runtimeBundle, /editor\.contrib\.referencesController/);
  assert.match(runtimeBundle, /editor\.contrib\.renameController/);
  assert.match(runtimeBundle, /editor\.action\.rename/);
});

test("runtime Monaco ships the patched sanitizer, not the older vendored copy", () => {
  const runtimeBundle = fs.readFileSync(
    path.resolve(process.cwd(), "crates/trust-runtime/src/web/ui/assets/ide-monaco.20260215.js"),
    "utf8",
  );
  assert.match(runtimeBundle, /\.version\s*=\s*["']3\.4\.16["']/);
  assert.doesNotMatch(runtimeBundle, /\.version\s*=\s*["']3\.4\.15["']/);
  assert.ok(runtimeBundle.includes("a node selected for removal could not be safely returned"));
});
