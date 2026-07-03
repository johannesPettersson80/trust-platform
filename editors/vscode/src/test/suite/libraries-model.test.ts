import * as assert from "assert";

import {
  collectSymbolSummaries,
  classifyGitPin,
  formatDependencySpec,
  parseDependencyEntries,
  parsePackageVersion,
  posixPath,
  removeDependency,
  upsertDependency,
} from "../../librariesModel";

suite("Libraries model", () => {
  test("adds dependencies without rewriting existing project manifest shape", () => {
    const manifest = `include_paths = ["src"]\n`;
    const updated = upsertDependency(manifest, "OSCAT", {
      path: "libraries/oscat",
      version: "0.1.0",
    });
    assert.strictEqual(
      updated,
      `include_paths = ["src"]\n\n[dependencies]\nOSCAT = { path = "libraries/oscat", version = "0.1.0" }\n`
    );
  });

  test("updates and removes dependency entries in place", () => {
    const manifest = `[dependencies]\nOSCAT = { path = "libraries/oscat", version = "0.0.1" }\nOther = { path = "vendor/other" }\n`;
    const updated = upsertDependency(manifest, "OSCAT", {
      path: "libraries/oscat",
      version: "0.1.0",
    });
    assert.ok(updated.includes('OSCAT = { path = "libraries/oscat", version = "0.1.0" }'));
    assert.ok(updated.includes('Other = { path = "vendor/other" }'));

    const removed = removeDependency(updated, "OSCAT");
    assert.ok(!removed.includes("OSCAT ="));
    assert.ok(removed.includes('Other = { path = "vendor/other" }'));
  });

  test("parses path and git dependencies", () => {
    const manifest = `[dependencies]\nOSCAT = { path = "libraries/oscat", version = "0.1.0" }\nVendorLib = { git = "file:///tmp/vendor", tag = "v1", version = "1.0.0" }\n`;
    const entries = parseDependencyEntries(manifest);
    assert.deepStrictEqual(entries[0], {
      name: "OSCAT",
      source: "local",
      path: "libraries/oscat",
      version: "0.1.0",
    });
    assert.deepStrictEqual(entries[1], {
      name: "VendorLib",
      source: "git",
      git: "file:///tmp/vendor",
      tag: "v1",
      version: "1.0.0",
    });
  });

  test("formats git dependency with exactly one pin selector", () => {
    assert.strictEqual(
      formatDependencySpec({
        git: "file:///tmp/vendor",
        branch: "main",
        version: "1.0.0",
      }),
      `{ git = "file:///tmp/vendor", branch = "main", version = "1.0.0" }`
    );
    assert.throws(
      () => formatDependencySpec({ git: "file:///tmp/vendor", tag: "v1", rev: "abc" }),
      /only one/
    );
  });

  test("classifies git pins and normalizes paths", () => {
    assert.deepStrictEqual(classifyGitPin("v1.2.3"), { tag: "v1.2.3" });
    assert.deepStrictEqual(classifyGitPin("feature/main"), { branch: "feature/main" });
    assert.deepStrictEqual(classifyGitPin("0123456789abcdef"), { rev: "0123456789abcdef" });
    assert.strictEqual(posixPath("libraries\\oscat"), "libraries/oscat");
  });

  test("reads package version and groups library symbols", () => {
    assert.strictEqual(parsePackageVersion(`[package]\nversion = "0.1.0"\n`), "0.1.0");
    const symbols = collectSymbolSummaries([
      {
        file: "lib.st",
        text: `TYPE AxisState : INT; END_TYPE\nFUNCTION SCALE : REAL\nEND_FUNCTION\nFUNCTION_BLOCK Averager\nEND_FUNCTION_BLOCK\n`,
      },
    ]);
    assert.deepStrictEqual(
      symbols.map((symbol) => [symbol.kind, symbol.name]),
      [
        ["function", "SCALE"],
        ["function_block", "Averager"],
        ["type", "AxisState"],
      ]
    );
    assert.deepStrictEqual(
      symbols.map((symbol) => symbol.declaration),
      ["FUNCTION SCALE : REAL", "FUNCTION_BLOCK Averager", "TYPE AxisState : INT; END_TYPE"]
    );
  });
});
