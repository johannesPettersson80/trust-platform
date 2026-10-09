import { createHash } from "crypto";
import * as assert from "assert";
import * as fs from "fs/promises";
import * as os from "os";
import * as path from "path";
import { DraftStore } from "../../hmiBuilder/agent/drafts";

suite("HMI agent draft persistence", () => {
  let directory: string;
  let store: DraftStore;
  setup(async () => {
    directory = await fs.mkdtemp(path.join(os.tmpdir(), "trust-agent-drafts-"));
    store = new DraftStore(directory);
  });
  teardown(async () => fs.rm(directory, { recursive: true, force: true }));
  test("draft survives restart with stable identity and isolated returned objects", async () => {
    const draft = await store.create("workspace", [], { "hmi/pages/a.toml": "original" });
    assert.ok(draft.id.length > 10, "opaque draft id");
    draft.files["hmi/pages/a.toml"] = "external mutation";
    const restored = await new DraftStore(directory).get(draft.id);
    assert.strictEqual(restored.files["hmi/pages/a.toml"], "original");
    assert.strictEqual(restored.revision, draft.revision);
  });
  test("quota exhaustion preserves all existing drafts", async () => {
    const ids: string[] = [];
    for (let i = 0; i < 5; i++) ids.push((await store.create("workspace", [], {})).id);
    await assert.rejects(store.create("workspace", [], {}), /LIMIT_EXCEEDED/);
    assert.strictEqual(new Set(ids).size, 5);
    for (const id of ids) assert.strictEqual((await store.get(id)).id, id);
  });
  test("edit persists idempotent outcome and refuses key reuse with another request", async () => {
    const first = await store.create("workspace", [], { page: "before" });
    let calls = 0;
    const update = async () => {
      calls++;
      return { page: "after" };
    };
    const second = await store.edit(first.id, first.revision, "request", "same", update);
    assert.strictEqual(second.files.page, "after");
    assert.notStrictEqual(second.revision, first.revision);
    const replay = await new DraftStore(directory).edit(first.id, first.revision, "request", "same", update);
    assert.deepStrictEqual(replay, second);
    assert.strictEqual(calls, 1);
    await assert.rejects(store.edit(first.id, first.revision, "request", "different", update), /CONFLICT/);
  });
  test("only one concurrent edit of a revision commits", async () => {
    const first = await store.create("workspace", [], {});
    const outcomes = await Promise.allSettled(
      [1, 2].map((i) => store.edit(first.id, first.revision, "key" + i, "body" + i, async () => ({ page: String(i) }))),
    );
    assert.strictEqual(outcomes.filter((o) => o.status === "fulfilled").length, 1);
    assert.strictEqual(outcomes.filter((o) => o.status === "rejected").length, 1);
  });
  test("failed edit leaves prior draft and review intact; successful edit invalidates review", async () => {
    const first = await store.create("workspace", [], { page: "before" });
    const reviewed = await store.review(first.id, first.revision, []);
    assert.strictEqual(reviewed.state, "reviewed");
    await assert.rejects(
      store.edit(first.id, reviewed.revision, "bad", "bad", async () => {
        throw new Error("invalid operation");
      }),
      /invalid operation/,
    );
    assert.deepStrictEqual(await store.get(first.id), reviewed);
    const edited = await store.edit(first.id, reviewed.revision, "good", "good", async () => ({ page: "after" }));
    assert.strictEqual(edited.state, "draft");
    assert.deepStrictEqual(edited.findings, []);
  });
  test("discard removes only the draft, and an unknown id cannot escape storage", async () => {
    const first = await store.create("workspace", [], {});
    await store.discard(first.id);
    await assert.rejects(store.get(first.id), /EXPIRED_DRAFT/);
    await assert.rejects(store.get("../outside"), /INVALID_ARGUMENT/);
  });
  test("separate store instances cannot both edit the same persisted revision", async () => {
    const first = await store.create("workspace", [], {});
    const other = new DraftStore(directory);
    const results = await Promise.allSettled(
      [store, other].map((owner, index) =>
        owner.edit(first.id, first.revision, "writer" + index, "input" + index, async () => {
          await new Promise((resolve) => setTimeout(resolve, 20));
          return { page: String(index) };
        }),
      ),
    );
    assert.strictEqual(results.filter((result) => result.status === "fulfilled").length, 1);
  });
  test("application receipt survives restart and prevents repeated source application", async () => {
    const first = await store.create("workspace", [], {});
    let commits = 0;
    const commit = async () => ({ applied: true, sequence: ++commits });
    const outcome = await store.apply(first.id, first.revision, "apply-1", "same", commit);
    const replay = await new DraftStore(directory).apply(first.id, first.revision, "apply-1", "same", commit);
    assert.deepStrictEqual(replay, outcome);
    assert.strictEqual(commits, 1);
    assert.strictEqual((await store.get(first.id)).state, "applied");
    await assert.rejects(store.apply(first.id, first.revision, "apply-1", "different", commit), /CONFLICT/);
  });
  test("creation replay survives restart without rebuilding or reviving a discarded proposal", async () => {
    let builds = 0;
    const build = async () => {
      builds++;
      return { base: [], files: { page: "proposal" }, data: { model: "original" } };
    };
    type CreateOnce = (
      workspaceId: string,
      key: string,
      fingerprint: string,
      build: () => Promise<{ base: never[]; files: { page: string }; data: { model: string } }>,
    ) => Promise<{ draft: { id: string; revision: string }; data: { model: string } }>;
    const create = (owner: DraftStore, key: string, fingerprint: string) => {
      const method = (owner as DraftStore & { createOnce?: CreateOnce }).createOnce;
      assert.ok(method, "draft creation needs a persisted request receipt");
      return method.call(owner, "workspace", key, fingerprint, build);
    };
    const first = await create(store, "create-1", "same");
    const replay = await create(new DraftStore(directory), "create-1", "same");
    assert.deepStrictEqual(replay, first);
    assert.strictEqual(builds, 1);
    await assert.rejects(create(store, "create-1", "different"), /CONFLICT/);
    await store.discard(first.draft.id);
    await assert.rejects(create(new DraftStore(directory), "create-1", "same"), /EXPIRED_DRAFT/);
    assert.strictEqual(builds, 1, "discarded proposals cannot be recreated by replaying old requests");
  });
  test("an applied draft cannot be moved back into reviewed state", async () => {
    const first = await store.create("workspace", [], {});
    await store.apply(first.id, first.revision, "apply", "same", async () => ({ applied: true }));
    await assert.rejects(store.review(first.id, first.revision, []), /CONFLICT/);
    assert.strictEqual((await store.get(first.id)).state, "applied");
  });
  test("discard checks the expected revision inside its mutation lock", async () => {
    const first = await store.create("workspace", [], { page: "before" });
    const edited = await store.edit(first.id, first.revision, "edit", "changed", async () => ({ page: "after" }));
    await assert.rejects(
      Reflect.apply(store.discard, store, [first.id, { workspaceId: "workspace", revision: first.revision }]),
      /CONFLICT/,
    );
    assert.strictEqual((await store.get(first.id)).files.page, "after");
    await Reflect.apply(store.discard, store, [first.id, { workspaceId: "workspace", revision: edited.revision }]);
    await Reflect.apply(store.discard, store, [first.id, { workspaceId: "workspace", revision: edited.revision }]);
    await assert.rejects(store.get(first.id), /EXPIRED_DRAFT/);
  });

  test("restart discovery lists only recoverable proposals in the selected workspace", async () => {
    const first = await store.create("workspace", [], { "hmi/pages/keep.toml": "private proposal body" });
    const gone = await store.create("workspace", [], { page: "discard me" });
    const applied = await store.create("workspace", [], { page: "applied" });
    await store.create("other-workspace", [], { page: "other" });
    await store.discard(gone.id);
    await store.apply(applied.id, applied.revision, "apply", "same", async () => ({ applied: true }));
    type Listing = (workspaceId: string) => Promise<unknown[]>;
    const restarted = new DraftStore(directory) as DraftStore & { list?: Listing };
    assert.ok(restarted.list, "persisted proposals must be discoverable after restart");
    const proposals = await restarted.list("workspace");
    assert.deepStrictEqual(proposals, [
      { draftId: first.id, revision: first.revision, state: "draft", files: ["hmi/pages/keep.toml"] },
    ]);
    assert.ok(!JSON.stringify(proposals).includes("private proposal body"));
  });

  test("file hashes survive restart, change with edits and detect damaged proposal contents", async () => {
    const file = "hmi/pages/hashed.toml";
    const first = await store.create("workspace", [], { [file]: "before" });
    const digest = (text: string) => createHash("sha256").update(text).digest("hex");
    const hashes = (draft: unknown) => (draft as { contentHashes?: Record<string, string> }).contentHashes;
    assert.deepStrictEqual(hashes(first), { [file]: digest("before") }, "drafts need verifiable content hashes");
    const edited = await store.edit(first.id, first.revision, "edit", "after", async () => ({ [file]: "after" }));
    assert.deepStrictEqual(hashes(edited), { [file]: digest("after") });
    assert.deepStrictEqual(hashes(await new DraftStore(directory).get(first.id)), hashes(edited));
    const recordFile = path.join(directory, `${first.id}.json`);
    const record = JSON.parse(await fs.readFile(recordFile, "utf8"));
    record.draft.files[file] = "damaged body";
    await fs.writeFile(recordFile, JSON.stringify(record));
    await assert.rejects(new DraftStore(directory).get(first.id), /INVALID_ARGUMENT.*corrupt/i);
  });
  test("truncated storage reports a typed corrupt-record error", async () => {
    const first = await store.create("workspace", [], {});
    await fs.writeFile(path.join(directory, `${first.id}.json`), '{"format":1,');
    await assert.rejects(store.get(first.id), /INVALID_ARGUMENT.*corrupt/i);
  });

  test("read-only apply preflight does not consume a receipt and sees the locked current review", async () => {
    const first = await store.create("workspace", [], { page: "body" });
    await store.review(first.id, first.revision, [], { asset: "current" });
    let blocked = true,
      commits = 0;
    const prepare = async (draft: { reviewedAssets?: object }) => {
      assert.deepStrictEqual(draft.reviewedAssets, { asset: "current" });
      if (blocked) throw new Error("CONFLICT: review needs correction");
    };
    const apply = () =>
      Reflect.apply(store.apply, store, [
        first.id,
        first.revision,
        "apply",
        "same",
        async () => ({ applied: true, count: ++commits }),
        prepare,
      ]);
    await assert.rejects(apply(), /review needs correction/);
    assert.strictEqual(commits, 0);
    blocked = false;
    assert.deepStrictEqual(await apply(), { applied: true, count: 1 });
    assert.deepStrictEqual(await apply(), { applied: true, count: 1 });
    assert.strictEqual(commits, 1);
  });
  test("uncertain source application cannot be bypassed with another key or draft edit", async () => {
    const first = await store.create("workspace", [], { page: "body" });
    await assert.rejects(
      store.apply(first.id, first.revision, "first", "same", async () => {
        throw new Error("editor disconnected");
      }),
      /^Error: CONFLICT:.*editor disconnected/,
    );
    let called = false;
    await assert.rejects(
      new DraftStore(directory).apply(first.id, first.revision, "second", "same", async () => {
        called = true;
        return { applied: true };
      }),
      /CONFLICT.*inspection/,
    );
    assert.strictEqual(called, false);
    await assert.rejects(
      store.edit(first.id, first.revision, "edit", "edit", async () => ({ page: "other" })),
      /CONFLICT.*inspection/,
    );
    await assert.rejects(store.review(first.id, first.revision, []), /CONFLICT.*inspection/);
  });

  test("cancellation during proposal construction prevents its private commit", async () => {
    const cancellation = new AbortController();
    await assert.rejects(
      Reflect.apply(store.createOnce, store, [
        "workspace",
        "cancel-create",
        "same",
        async () => {
          cancellation.abort();
          return { base: [], files: { page: "never committed" }, data: {} };
        },
        cancellation.signal,
      ]),
      /CANCELLED/,
    );
    assert.deepStrictEqual(await store.list("workspace"), []);
  });
  test("cancellation during a draft edit preserves its previous revision", async () => {
    const first = await store.create("workspace", [], { page: "before" });
    const cancellation = new AbortController();
    await assert.rejects(
      Reflect.apply(store.edit, store, [
        first.id,
        first.revision,
        "cancel-edit",
        "same",
        async () => {
          cancellation.abort();
          return { page: "after" };
        },
        cancellation.signal,
      ]),
      /CANCELLED/,
    );
    assert.deepStrictEqual(await store.get(first.id), first);
  });
  test("cancelled review cannot certify a proposal", async () => {
    const first = await store.create("workspace", [], { page: "body" });
    const cancellation = new AbortController();
    cancellation.abort();
    await assert.rejects(
      Reflect.apply(store.review, store, [first.id, first.revision, [], {}, cancellation.signal]),
      /CANCELLED/,
    );
    assert.strictEqual((await store.get(first.id)).state, "draft");
  });
  test("cancelled discard retains the proposal", async () => {
    const first = await store.create("workspace", [], { page: "body" });
    const cancellation = new AbortController();
    cancellation.abort();
    await assert.rejects(
      Reflect.apply(store.discard, store, [
        first.id,
        { workspaceId: "workspace", revision: first.revision },
        cancellation.signal,
      ]),
      /CANCELLED/,
    );
    assert.deepStrictEqual(await store.get(first.id), first);
  });

  test("edit detail receipts survive later edits and restart without recomputing metadata", async () => {
    const first = await store.create("workspace", [], { page: "before" });
    type Detailed = (
      id: string,
      revision: string,
      key: string,
      fingerprint: string,
      update: (draft: unknown) => Promise<{ files: Record<string, string>; data: unknown }>,
    ) => Promise<{ draft: { revision: string }; data: unknown }>;
    const owner = store as DraftStore & { editDetailed?: Detailed };
    assert.ok(owner.editDetailed, "edit needs a durable detailed receipt");
    let calls = 0;
    const update = async () => {
      calls++;
      return { files: { page: "after" }, data: { changes: [{ id: "unit", properties: ["at"] }] } };
    };
    const firstEdit = await owner.editDetailed(first.id, first.revision, "edit-detail", "same", update);
    await store.edit(first.id, firstEdit.draft.revision, "later", "different", async () => ({ page: "later" }));
    const restarted = new DraftStore(directory) as DraftStore & { editDetailed: Detailed };
    const replay = await restarted.editDetailed(first.id, first.revision, "edit-detail", "same", update);
    // Spec 43 repairs of 7 October 2026: the receipt keeps its data, revision and hashes; the
    // superseded revision's pages are not kept.
    assert.deepStrictEqual(replay.data, firstEdit.data);
    assert.strictEqual(calls, 1);
    assert.strictEqual(replay.draft.revision, firstEdit.draft.revision);
    const receipt = replay.draft as { revision: string; contentHashes?: object; files?: object; superseded?: boolean };
    assert.deepStrictEqual(
      receipt.contentHashes,
      (firstEdit.draft as { revision: string; contentHashes?: object }).contentHashes,
    );
    assert.strictEqual(receipt.superseded, true);
    assert.deepStrictEqual(receipt.files, {});
    assert.strictEqual((await store.get(first.id)).files.page, "later");
  });
});
