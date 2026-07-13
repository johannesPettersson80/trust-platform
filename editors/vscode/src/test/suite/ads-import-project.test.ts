import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";

import {
  buildOfflineAdsImportArgs,
  listExistingAdsSnapshotPaths,
} from "../../networkCanvas/adsBrowseContract";
import { ensureAdsRuntimeEnabled } from "../../networkCanvas/offlineComm";
import { enableRuntimeAdsToml } from "../../networkCanvas/runtimeAdsToml";

const ADS_KEYS = [
  "enabled",
  "config_path",
  "worker_tick_interval_ms",
] as const;

function runtimeAdsLines(source: string): string[] {
  const lines = source.split(/\r\n|\n|\r/);
  const start = lines.findIndex((line) => /^\s*\[runtime\.ads\]/.test(line));
  assert.ok(start >= 0, "expected [runtime.ads]");
  let end = lines.length;
  let multiline = false;
  for (let index = start + 1; index < lines.length; index += 1) {
    const structural = !multiline;
    const tripleQuotes = lines[index].match(/"""|'''/g)?.length ?? 0;
    if (tripleQuotes % 2 === 1) {
      multiline = !multiline;
    }
    if (
      structural &&
      /^\s*\[\[?[^\]]+\]\]?\s*(?:#.*)?$/.test(lines[index])
    ) {
      end = index;
      break;
    }
  }
  return lines.slice(start + 1, end);
}

function assignmentValues(source: string, key: string): string[] {
  return runtimeAdsLines(source)
    .map((line) =>
      new RegExp(`^\\s*${key}\\s*=\\s*([^#]*?)\\s*(?:#.*)?$`).exec(line),
    )
    .filter((match): match is RegExpExecArray => match !== null)
    .map((match) => match[1]);
}

function existingSnapshotArgs(args: readonly string[]): string[] {
  const result: string[] = [];
  for (let index = 0; index < args.length; index += 1) {
    if (args[index] === "--existing-snapshot") {
      result.push(args[index + 1]);
    }
  }
  return result;
}

suite("ADS import project artifacts", () => {
  test("updates the whole existing runtime.ads table once and preserves other TOML", () => {
    const before = [
      "# project comment",
      "[bundle]",
      "version = 1",
      "",
      "[runtime.ads] # ADS settings",
      "# keep this ADS comment",
      'note = """',
      "[not.a.real.table]",
      "text inside the multiline value",
      '"""',
      "enabled = false # first enabled comment",
      "enabled = false # duplicate enabled comment",
      'config_path = "old.toml"',
      'config_path = "duplicate.toml" # duplicate config comment',
      "worker_tick_interval_ms = 17",
      "worker_tick_interval_ms = 99 # duplicate tick comment",
      'custom_setting = "preserved"',
      "",
      "[runtime.control]",
      'endpoint = "tcp://127.0.0.1:9901"',
      "enabled = false # belongs to runtime.control",
      "",
    ].join("\n");

    const after = enableRuntimeAdsToml(before, "ads.toml");
    assert.deepStrictEqual(assignmentValues(after, "enabled"), ["true"]);
    assert.deepStrictEqual(assignmentValues(after, "config_path"), [
      '"ads.toml"',
    ]);
    assert.deepStrictEqual(
      assignmentValues(after, "worker_tick_interval_ms"),
      ["17"],
    );
    assert.ok(after.includes("# project comment"));
    assert.ok(after.includes("# keep this ADS comment"));
    assert.ok(after.includes("# duplicate enabled comment"));
    assert.ok(after.includes("# duplicate config comment"));
    assert.ok(after.includes("# duplicate tick comment"));
    assert.ok(after.includes("[not.a.real.table]\ntext inside the multiline value"));
    assert.ok(after.includes('[runtime.control]\nendpoint = "tcp://127.0.0.1:9901"'));
    assert.ok(after.includes("enabled = false # belongs to runtime.control"));
    assert.ok(after.includes('custom_setting = "preserved"'));
    assert.strictEqual(enableRuntimeAdsToml(after, "ads.toml"), after);
  });

  test("adds a missing table with CRLF and stays idempotent", () => {
    const before = "# windows project\r\n[bundle]\r\nversion = 1\r\n";
    const after = enableRuntimeAdsToml(before, "config/ads.toml");
    assert.ok(!/(^|[^\r])\n/.test(after), "must preserve CRLF line endings");
    for (const key of ADS_KEYS) {
      assert.strictEqual(assignmentValues(after, key).length, 1);
    }
    assert.deepStrictEqual(assignmentValues(after, "config_path"), [
      '"config/ads.toml"',
    ]);
    assert.ok(after.includes("# windows project\r\n[bundle]"));
    assert.strictEqual(enableRuntimeAdsToml(after, "config/ads.toml"), after);
  });

  test("ensureAdsRuntimeEnabled handles a missing table and missing file honestly", () => {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), "trust-ads-config-"));
    try {
      const missing = ensureAdsRuntimeEnabled(root);
      assert.strictEqual(missing.ok, false);
      fs.writeFileSync(
        path.join(root, "runtime.toml"),
        "[resource]\nname = \"Simulator\"\n",
        "utf8",
      );
      const first = ensureAdsRuntimeEnabled(root);
      assert.deepStrictEqual(
        { ok: first.ok, changed: first.ok ? first.changed : undefined },
        { ok: true, changed: true },
      );
      const content = fs.readFileSync(path.join(root, "runtime.toml"), "utf8");
      for (const key of ADS_KEYS) {
        assert.strictEqual(assignmentValues(content, key).length, 1);
      }
      assert.match(
        content,
        /\[runtime\.ads\][\s\S]*?^enabled\s*=\s*true\s*$/m,
        "packaged import proof must still recognize ADS as enabled",
      );
      const second = ensureAdsRuntimeEnabled(root);
      assert.deepStrictEqual(
        { ok: second.ok, changed: second.ok ? second.changed : undefined },
        { ok: true, changed: false },
      );
    } finally {
      fs.rmSync(root, { recursive: true, force: true });
    }
  });

  test("second connection passes every prior snapshot once in deterministic order", () => {
    const root = fs.mkdtempSync(
      path.join(os.tmpdir(), "trust ADS project with spaces "),
    );
    try {
      const snapshots = path.join(root, "ads", "snapshots");
      fs.mkdirSync(path.join(snapshots, "ignored.symbols.json"), {
        recursive: true,
      });
      for (const name of [
        "z_line.symbols.json",
        "a_line.symbols.json",
        "second_line.symbols.json",
        "ignore.json",
      ]) {
        fs.writeFileSync(path.join(snapshots, name), "{}", "utf8");
      }
      const existing = listExistingAdsSnapshotPaths(root, "second_line");
      assert.deepStrictEqual(existing, [
        path.join(snapshots, "a_line.symbols.json"),
        path.join(snapshots, "z_line.symbols.json"),
      ]);
      const args = buildOfflineAdsImportArgs(
        root,
        { host: "192.0.2.10", ams_port: 851 },
        "second_line",
        ["MAIN.Second"],
        existing,
      );
      assert.deepStrictEqual(existingSnapshotArgs(args), existing);
      assert.ok(existingSnapshotArgs(args)[0].includes("project with spaces"));
    } finally {
      fs.rmSync(root, { recursive: true, force: true });
    }
  });

  test("Windows snapshot arguments preserve spaces and exclude duplicate current state", () => {
    const first = "C:\\TwinCAT Projects\\Cell One\\ads\\snapshots\\a.symbols.json";
    const later = "C:\\TwinCAT Projects\\Cell One\\ads\\snapshots\\z.symbols.json";
    const current =
      "C:\\TwinCAT Projects\\Cell One\\ads\\snapshots\\Current.symbols.json";
    const args = buildOfflineAdsImportArgs(
      "C:\\TwinCAT Projects\\Cell One",
      { host: "127.0.0.1" },
      "current",
      ["MAIN.Value"],
      [later, current, first, first.toUpperCase()],
    );
    assert.deepStrictEqual(existingSnapshotArgs(args), [first, later]);
  });
});
