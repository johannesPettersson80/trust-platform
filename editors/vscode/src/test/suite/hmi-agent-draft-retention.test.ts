import * as assert from "assert";
import { createHash, randomUUID } from "crypto";
import * as fs from "fs/promises";
import * as os from "os";
import * as path from "path";
import type { DocumentSnapshot } from "../../hmiBuilder/agent/documents";
import { DRAFT_LIMITS } from "../../hmiBuilder/agent/draftRecords";
import { DraftStore } from "../../hmiBuilder/agent/drafts";

// Specification 43, "Repairs — 7 October 2026", item 1: draft storage stays bounded and never
// locks itself because of old drafts; open drafts are never removed without telling the user.

const snapshot = (file: string, text: string): DocumentSnapshot => ({
  file,
  uri: `file:///fixture/${file}`,
  exists: true,
  text,
  revision: `disk:${file}`,
  version: null,
  dirty: false,
});
const digest = (text: string): string => createHash("sha256").update(text).digest("hex");
const mustNotRun = async (): Promise<never> => {
  throw new Error("the original request must not run again");
};

suite("HMI agent draft storage bounds", function () {
  this.timeout(120000);
  let directory: string;
  setup(async () => {
    directory = await fs.mkdtemp(path.join(os.tmpdir(), "trust-agent-retention-"));
  });
  teardown(async () => fs.rm(directory, { recursive: true, force: true }));
  const record = (id: string): string => path.join(directory, `${id}.json`);
  const records = async (): Promise<string[]> => (await fs.readdir(directory)).filter((name) => name.endsWith(".json"));
  const storageBytes = async (): Promise<number> => {
    let total = 0;
    for (const name of await records()) total += (await fs.stat(path.join(directory, name))).size;
    return total;
  };
  const exists = (file: string): Promise<boolean> =>
    fs.stat(file).then(
      () => true,
      () => false,
    );

  test("edit receipts never copy the pages: a hundred edits keep one copy and a bounded receipt list", async () => {
    const store = new DraftStore(directory);
    const file = "hmi/pages/edited.toml";
    const page = (edit: number): string => `edit ${edit}\n`.padEnd(20 * 1024, "#");
    const base = Array.from({ length: 10 }, (_, index) =>
      snapshot(`hmi/pages/base-${index}.toml`, "b".repeat(20 * 1024)),
    );
    let draft = await store.create("workspace", base, { [file]: page(0) });
    const created = (await fs.stat(record(draft.id))).size;
    for (let edit = 1; edit <= 100; edit++) {
      draft = await store.edit(draft.id, draft.revision, `edit-${edit}`, `edit-${edit}`, async () => ({
        [file]: page(edit),
      }));
    }
    const size = (await fs.stat(record(draft.id))).size;
    assert.ok(size < created * 1.5, `the record grew from ${created} to ${size} bytes over 100 edits`);
    const stored = JSON.parse(await fs.readFile(record(draft.id), "utf8")) as { requests: Record<string, unknown> };
    assert.ok(
      Object.keys(stored.requests).length <= DRAFT_LIMITS.maxReceipts,
      `${Object.keys(stored.requests).length} receipts kept`,
    );
    // Edit k commits revision k + 1. The newest retry returns the current draft whole.
    assert.deepStrictEqual(await store.edit(draft.id, "100", "edit-100", "edit-100", mustNotRun), draft);
    // A superseded but retained receipt names its revision and hashes, without page bodies.
    const earlier = await store.edit(draft.id, "90", "edit-90", "edit-90", mustNotRun);
    assert.strictEqual(earlier.revision, "91");
    assert.strictEqual(earlier.superseded, true);
    assert.deepStrictEqual(earlier.files, {});
    assert.deepStrictEqual(earlier.contentHashes, { [file]: digest(page(90)) });
    // A receipt past the bound is answered as a changed revision, never applied a second time.
    await assert.rejects(
      store.edit(draft.id, "1", "edit-1", "edit-1", mustNotRun),
      /^Error: CONFLICT: draft revision changed/,
    );
    assert.strictEqual((await store.get(draft.id)).files[file], page(100));
  });

  test("many apply and discard cycles stay within the product bounds and new drafts are still accepted", async () => {
    let clock = 0;
    const store = new DraftStore(directory, { now: () => ++clock });
    const ids: string[] = [];
    for (let cycle = 0; cycle < 80; cycle++) {
      const draft = await store.create("workspace", [snapshot("hmi/pages/base.toml", "base page")], {
        "hmi/pages/proposal.toml": `cycle ${cycle}`,
      });
      const edited = await store.edit(draft.id, draft.revision, `edit-${cycle}`, `edit-${cycle}`, async () => ({
        "hmi/pages/proposal.toml": `edited ${cycle}`,
      }));
      if (cycle % 2) await store.discard(draft.id, { workspaceId: "workspace", revision: edited.revision });
      else
        await store.apply(draft.id, edited.revision, `apply-${cycle}`, "same", async () => ({ applied: true, cycle }));
      ids.push(draft.id);
      const count = (await records()).length;
      assert.ok(count <= DRAFT_LIMITS.maxOpen + DRAFT_LIMITS.maxFinished, `${count} records after cycle ${cycle}`);
      assert.ok((await storageBytes()) <= DRAFT_LIMITS.maxBytes);
    }
    const accepted = await store.create("workspace", [], { "hmi/pages/next.toml": "next" });
    assert.strictEqual((await store.get(accepted.id)).files["hmi/pages/next.toml"], "next");
    assert.deepStrictEqual(
      (await store.list("workspace")).map((entry) => entry.draftId),
      [accepted.id],
    );
    assert.ok((await records()).length <= DRAFT_LIMITS.maxFinished + 1, "finished records beyond the bound left");
    await assert.rejects(store.get(ids[0]), /EXPIRED_DRAFT/, "the oldest finished record left first");
    assert.deepStrictEqual(
      await store.apply(ids[78], "2", "apply-78", "same", mustNotRun),
      { applied: true, cycle: 78 },
      "a recent apply retry still returns its original outcome",
    );
    const applied = JSON.parse(await fs.readFile(record(ids[78]), "utf8")) as {
      draft: { files: object; base: unknown[] };
      requests: object;
    };
    assert.deepStrictEqual(
      [applied.draft.files, applied.draft.base, applied.requests],
      [{}, [], {}],
      "an applied draft keeps receipts only",
    );
  });

  test("finished records left by the earlier format leave oldest first instead of locking storage", async () => {
    const maxBytes = 300 * 1024;
    const store = new DraftStore(directory, { maxBytes, retentionMs: Number.POSITIVE_INFINITY });
    await fs.mkdir(directory, { recursive: true });
    const legacy: string[] = [];
    for (let index = 0; index < 6; index++) {
      const id = randomUUID(),
        body = "x".repeat(20 * 1024),
        file = "hmi/pages/old.toml";
      const draft = {
        id,
        workspaceId: "workspace",
        revision: "2",
        state: "applied",
        files: { [file]: body },
        base: [snapshot(file, body)],
        findings: [],
      };
      // Format 1 kept the whole draft in every receipt and applied drafts with their pages.
      await fs.writeFile(
        record(id),
        JSON.stringify({
          format: 1,
          draft,
          requests: { edit: { fingerprint: "f", outcome: { ...draft, state: "draft" } } },
          applications: { apply: { fingerprint: "f", status: "committed", outcome: { applied: true } } },
        }),
      );
      const time = new Date(Date.UTC(2026, 9, 1, 8, index));
      await fs.utimes(record(id), time, time);
      legacy.push(id);
    }
    assert.ok((await storageBytes()) > maxBytes, "fixture: earlier finished records alone exceed the bound");
    const draft = await store.create("workspace", [snapshot("hmi/pages/base.toml", "y".repeat(10 * 1024))], {
      "hmi/pages/new.toml": "new",
    });
    assert.strictEqual((await store.get(draft.id)).files["hmi/pages/new.toml"], "new");
    assert.ok((await storageBytes()) <= maxBytes, `${await storageBytes()} bytes stored`);
    const kept: boolean[] = [];
    for (const id of legacy) kept.push(await exists(record(id)));
    const firstKept = kept.indexOf(true);
    assert.ok(firstKept > 0, "at least the oldest earlier record left");
    assert.ok(kept.slice(firstKept).every(Boolean), `only the oldest left: ${JSON.stringify(kept)}`);
  });

  test("open drafts are never removed: a full bound names them, and a discard always makes room", async () => {
    const store = new DraftStore(directory, { maxBytes: 150 * 1024 });
    const base = [snapshot("hmi/pages/base.toml", "z".repeat(60 * 1024))];
    const reviewed = await store.create("workspace", base, { "hmi/pages/reviewed.toml": "under review" });
    await store.review(reviewed.id, reviewed.revision, []);
    const other = await store.create("workspace", base, { "hmi/pages/other.toml": "other" });
    await assert.rejects(store.create("workspace", base, { "hmi/pages/third.toml": "third" }), (error: unknown) => {
      assert.ok(error instanceof Error);
      assert.match(error.message, /^LIMIT_EXCEEDED: draft storage would exceed/);
      for (const id of [reviewed.id, other.id])
        assert.ok(error.message.includes(id), `the refusal names ${id}: ${error.message}`);
      return true;
    });
    assert.strictEqual((await store.get(reviewed.id)).state, "reviewed", "the reviewed draft is untouched");
    assert.strictEqual((await store.get(other.id)).files["hmi/pages/other.toml"], "other");
    await store.discard(other.id, { workspaceId: "workspace", revision: other.revision });
    const third = await store.create("workspace", base, { "hmi/pages/third.toml": "third" });
    assert.deepStrictEqual(
      (await store.list("workspace")).map((entry) => entry.draftId).sort(),
      [reviewed.id, third.id].sort(),
    );
  });

  test("the open-draft quota names every open draft instead of removing one", async () => {
    const store = new DraftStore(directory);
    const ids: string[] = [];
    for (let index = 0; index < DRAFT_LIMITS.maxOpen; index++)
      ids.push((await store.create("workspace", [], { [`hmi/pages/p${index}.toml`]: "page" })).id);
    await assert.rejects(store.create("workspace", [], { "hmi/pages/extra.toml": "extra" }), (error: unknown) => {
      assert.ok(error instanceof Error);
      assert.match(error.message, /^LIMIT_EXCEEDED: 5 open proposals already exist/);
      for (const id of ids) assert.ok(error.message.includes(id), `the refusal names ${id}`);
      return true;
    });
    for (const id of ids) assert.strictEqual((await store.get(id)).id, id);
    await store.discard(ids[0]);
    await store.create("workspace", [], { "hmi/pages/extra.toml": "extra" });
  });

  test("finished records expire after the retention period and open drafts do not", async () => {
    let now = Date.UTC(2026, 9, 7);
    const store = new DraftStore(directory, { now: () => now });
    const applied = await store.create("workspace", [], { "hmi/pages/applied.toml": "applied" });
    await store.apply(applied.id, applied.revision, "apply", "same", async () => ({ applied: true }));
    const discarded = await store.create("workspace", [], { "hmi/pages/discarded.toml": "discarded" });
    await store.discard(discarded.id);
    const open = await store.create("workspace", [], { "hmi/pages/open.toml": "open" });
    now += DRAFT_LIMITS.retentionMs;
    await store.create("workspace", [], { "hmi/pages/later.toml": "later" });
    assert.strictEqual(await exists(record(applied.id)), false);
    assert.strictEqual(await exists(record(discarded.id)), false);
    await assert.rejects(store.get(applied.id), /EXPIRED_DRAFT/);
    assert.strictEqual((await store.get(open.id)).files["hmi/pages/open.toml"], "open");
  });

  test("an unreadable record no longer blocks discovery or creation and is kept for inspection", async () => {
    const store = new DraftStore(directory);
    await fs.mkdir(directory, { recursive: true });
    const unreadable = randomUUID();
    await fs.writeFile(record(unreadable), '{"format":1,');
    const crashed = path.join(directory, `${randomUUID()}.json.${randomUUID()}.tmp`);
    await fs.writeFile(crashed, "partial write");
    const draft = await store.create("workspace", [], { "hmi/pages/fresh.toml": "fresh" });
    assert.deepStrictEqual(
      (await store.list("workspace")).map((entry) => entry.draftId),
      [draft.id],
    );
    assert.strictEqual(await exists(record(unreadable)), true, "retained for inspection");
    await assert.rejects(store.get(unreadable), /INVALID_ARGUMENT.*corrupt/i);
    assert.strictEqual(await exists(crashed), false, "a crashed write's temporary file is removed");
  });
});
