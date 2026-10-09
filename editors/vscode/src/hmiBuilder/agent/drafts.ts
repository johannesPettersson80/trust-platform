import { reviewRecord, VisualReview } from "./visualReview";
import type { DesignIntent } from "./intent";
import { NoSourceChangeError } from "./applicationState";
import { randomUUID } from "crypto";
import type { Stats } from "fs";
import * as fs from "fs/promises";
import * as path from "path";
import type { DocumentSnapshot } from "./documents";
import {
  ApplicationReceipt,
  contentHashes,
  define,
  describeOpen,
  DRAFT_LIMITS,
  DraftLimits,
  evictionPlan,
  finishedRecord,
  isFinished,
  normalizedRecord,
  RecordEntry,
  StoredDraft,
  withEditReceipt,
} from "./draftRecords";

export interface Draft {
  id: string;
  workspaceId: string;
  revision: string;
  state: "draft" | "validated" | "reviewed" | "applied";
  intent?: DesignIntent;
  visualReview?: VisualReview;
  repairAttempts?: number;
  contentHashes?: Record<string, string>;
  reviewedAssets?: Record<string, string>;
  files: Record<string, string>;
  base: DocumentSnapshot[];
  findings: unknown[];
  programRevision?: string;
  /** Only on a replayed receipt of a revision that later edits superseded: its page bodies are
   * no longer kept, only its identity, revision and content hashes. */
  superseded?: true;
}
const copy = <T>(value: T): T => JSON.parse(JSON.stringify(value)) as T;
const own = <T>(record: Record<string, T>, key: string): T | undefined =>
  Object.prototype.hasOwnProperty.call(record, key) ? record[key] : undefined;
const mebibytes = (bytes: number): string => `${Number((bytes / (1024 * 1024)).toFixed(2))} MiB`;
function initialDraft(
  workspaceId: string,
  base: DocumentSnapshot[],
  files: Record<string, string>,
  programRevision?: string,
  intent?: DesignIntent,
): Draft {
  return copy({
    id: randomUUID(),
    workspaceId,
    revision: "1",
    state: "draft",
    files,
    intent,
    contentHashes: contentHashes(files),
    base,
    findings: [],
    ...(programRevision ? { programRevision } : {}),
  });
}
function checkCancellation(signal?: AbortSignal): void {
  if (signal?.aborted) throw new Error("CANCELLED: draft mutation cancelled before commit.");
}
const absent = (error: unknown): boolean => (error as NodeJS.ErrnoException).code === "ENOENT";

/** An applied draft keeps only its receipts; an operation that needs its pages refuses plainly. */
export function requireRetainedPages(draft: Draft): void {
  if (draft.state === "applied") {
    throw new Error(
      "EXPIRED_DRAFT: this proposal was applied and its private pages are no longer kept; inspect the source page or compose a new proposal.",
    );
  }
}

/** One store per connected workspace, owned by the extension host. Serialized mutations and
 * atomic replacement keep failure and recovery separate from source-file transactions.
 * Storage is bounded (specification 43, §6, repairs of 7 October 2026): edit receipts never
 * copy pages, applied and discarded drafts shrink to receipts, and finished records leave oldest
 * first after their retention period or when a count or byte bound needs room. Open drafts are
 * never removed automatically; when they alone fill a bound, the refusal names them. */
export class DraftStore {
  private pending: Promise<unknown> = Promise.resolve();
  private readonly limits: DraftLimits;
  constructor(
    readonly directory: string,
    limits: Partial<DraftLimits> = {},
  ) {
    this.limits = { ...DRAFT_LIMITS, ...limits };
  }

  private locked<T>(action: () => Promise<T>): Promise<T> {
    const next = this.pending.then(
      () => this.exclusive(action),
      () => this.exclusive(action),
    );
    this.pending = next.catch(() => undefined);
    return next;
  }
  private async exclusive<T>(action: () => Promise<T>): Promise<T> {
    await fs.mkdir(this.directory, { recursive: true, mode: 0o700 });
    const lock = path.join(this.directory, ".mutation-lock");
    const deadline = Date.now() + 5000;
    let owner: import("fs/promises").FileHandle | undefined;
    while (!owner) {
      try {
        owner = await fs.open(lock, "wx", 0o600);
      } catch (error) {
        if ((error as NodeJS.ErrnoException).code !== "EEXIST") throw error;
        try {
          const saved = JSON.parse(await fs.readFile(lock, "utf8")) as { pid: number };
          if (Number.isInteger(saved.pid) && saved.pid > 0) {
            try {
              process.kill(saved.pid, 0);
            } catch (probe) {
              if ((probe as NodeJS.ErrnoException).code === "ESRCH") {
                await fs.rm(lock, { force: true });
                continue;
              }
            }
          }
        } catch {
          /* A live owner may still be writing its lock. Never guess ownership. */
        }
        if (Date.now() >= deadline) throw new Error("CONFLICT: draft storage is busy or its owner needs recovery.");
        await new Promise((resolve) => setTimeout(resolve, 10));
      }
    }
    try {
      await owner.writeFile(JSON.stringify({ pid: process.pid }));
      return await action();
    } finally {
      await owner.close();
      await fs.rm(lock, { force: true });
    }
  }
  private file(id: string): string {
    if (!/^[0-9a-f]{8}-(?:[0-9a-f]{4}-){3}[0-9a-f]{12}$/.test(id))
      throw new Error("INVALID_ARGUMENT: invalid draft id.");
    return path.join(this.directory, `${id}.json`);
  }
  private async read(id: string): Promise<StoredDraft> {
    const file = this.file(id);
    let raw: string;
    try {
      raw = await fs.readFile(file, "utf8");
    } catch (error) {
      if (absent(error)) throw new Error("EXPIRED_DRAFT: draft is unavailable.");
      throw error;
    }
    const corrupt = () =>
      new Error(
        "INVALID_ARGUMENT: unsupported or corrupt draft record; retain it for inspection and recover from the source project.",
      );
    let parsed: unknown;
    try {
      parsed = JSON.parse(raw);
    } catch {
      throw corrupt();
    }
    const stored = normalizedRecord(parsed);
    if (
      !stored ||
      stored.draft.id !== id ||
      !stored.draft.files ||
      typeof stored.draft.files !== "object" ||
      Array.isArray(stored.draft.files) ||
      Object.values(stored.draft.files).some((text) => typeof text !== "string")
    )
      throw corrupt();
    const hashes = contentHashes(stored.draft.files);
    const saved = stored.draft.contentHashes;
    if (
      saved !== undefined &&
      (!saved ||
        typeof saved !== "object" ||
        Array.isArray(saved) ||
        Object.keys(saved).length !== Object.keys(hashes).length ||
        Object.entries(hashes).some(([name, hash]) => saved[name] !== hash))
    )
      throw corrupt();
    stored.draft.contentHashes = hashes;
    return stored;
  }
  private async records(): Promise<string[]> {
    await fs.mkdir(this.directory, { recursive: true, mode: 0o700 });
    return (await fs.readdir(this.directory)).filter((name) => /^[0-9a-f-]{36}\.json$/.test(name));
  }
  private async size(name: string): Promise<number> {
    try {
      return (await fs.stat(path.join(this.directory, name))).size;
    } catch (error) {
      if (absent(error)) return 0;
      throw error;
    }
  }
  /** Every record with its size and age. An unreadable record is listed without content: it is
   * counted, retained for inspection and never removed automatically. */
  private async scan(): Promise<RecordEntry[]> {
    const entries: RecordEntry[] = [];
    for (const name of (await this.records()).sort()) {
      const id = name.slice(0, -5),
        file = path.join(this.directory, name);
      let stat: Stats;
      try {
        stat = await fs.stat(file);
      } catch (error) {
        if (absent(error)) continue;
        throw error;
      }
      let stored: StoredDraft | undefined = undefined;
      try {
        stored = await this.read(id);
      } catch (error) {
        const message = error instanceof Error ? error.message : "";
        if (message.startsWith("EXPIRED_DRAFT:")) continue;
        if (!message.startsWith("INVALID_ARGUMENT:")) throw error;
      }
      entries.push({ id, file, size: stat.size, modified: stat.mtimeMs, ...(stored ? { stored } : {}) });
    }
    return entries;
  }
  /** Removes the finished records that `evictionPlan` selects and returns the remaining ones. */
  private async evict(entries: RecordEntry[], needBytes = 0, keep?: string): Promise<RecordEntry[]> {
    const plan = evictionPlan(entries, this.limits, needBytes, keep);
    for (const entry of plan) await fs.rm(entry.file, { force: true });
    const removed = new Set(plan.map((entry) => entry.id));
    return entries.filter((entry) => !removed.has(entry.id));
  }
  /** Inside the mutation lock no write is in progress, so a temporary file is a crashed write's. */
  private async removeTemporaries(): Promise<void> {
    for (const name of await fs.readdir(this.directory)) {
      if (name.endsWith(".tmp")) await fs.rm(path.join(this.directory, name), { force: true });
    }
  }
  /** Room for one more open draft: finished records past retention or beyond the count bound
   * leave first. Open drafts are never removed; a full quota names them so one can be discarded. */
  private async roomForDraft(entries: RecordEntry[]): Promise<void> {
    await this.removeTemporaries();
    const open = (await this.evict(entries)).flatMap((entry) =>
      entry.stored && !isFinished(entry.stored) ? [entry.stored] : [],
    );
    if (open.length >= this.limits.maxOpen) {
      throw new Error(
        `LIMIT_EXCEEDED: ${open.length} open proposals already exist: ${describeOpen(open)}. ` +
          'Discard one that is no longer needed (hmi_apply_draft with action "discard"); open proposals are never removed automatically.',
      );
    }
  }
  /** Atomic replacement. A write that would pass the byte bound first removes finished records,
   * oldest first, and is refused (naming the open drafts) only while open drafts still fill it.
   * `settle` writes (discard and application receipts) are never refused for space, so full
   * storage can always be cleaned up. */
  private async write(stored: StoredDraft, signal?: AbortSignal, settle = false): Promise<void> {
    checkCancellation(signal);
    const files = Object.entries(stored.draft.files);
    if (
      files.length > 32 ||
      files.some(([, text]) => typeof text !== "string" || Buffer.byteLength(text) > 1024 * 1024)
    ) {
      throw new Error("LIMIT_EXCEEDED: at most 32 files, each no larger than 1 MiB.");
    }
    const data = JSON.stringify(stored);
    const ownName = `${stored.draft.id}.json`;
    let bytes = Buffer.byteLength(data);
    for (const name of await this.records()) if (name !== ownName) bytes += await this.size(name);
    if (bytes > this.limits.maxBytes) {
      const remaining = await this.evict(await this.scan(), bytes - this.limits.maxBytes, stored.draft.id);
      const others = remaining.filter((entry) => entry.id !== stored.draft.id);
      bytes = Buffer.byteLength(data) + others.reduce((total, entry) => total + entry.size, 0);
      if (!settle && bytes > this.limits.maxBytes) {
        const open = others.flatMap((entry) => (entry.stored && !isFinished(entry.stored) ? [entry.stored] : []));
        const unreadable = others.filter((entry) => !entry.stored).map((entry) => entry.id);
        throw new Error(
          `LIMIT_EXCEEDED: draft storage would exceed ${mebibytes(this.limits.maxBytes)}; ` +
            `open proposals: ${describeOpen(open) || "none"}. ` +
            (unreadable.length ? `Unreadable records kept for inspection: ${unreadable.join(", ")}. ` : "") +
            "Discard a proposal that is no longer needed or submit a smaller one.",
        );
      }
    }
    const file = this.file(stored.draft.id);
    const temporary = `${file}.${randomUUID()}.tmp`;
    try {
      await fs.writeFile(temporary, data, { mode: 0o600, flag: "wx", signal });
      checkCancellation(signal);
      await fs.rename(temporary, file);
    } catch (error) {
      checkCancellation(signal);
      throw error;
    } finally {
      await fs.rm(temporary, { force: true });
    }
  }
  /** The draft a replayed receipt returns: the current revision whole, or for a revision that
   * later edits superseded, its identity, revision and content hashes without page bodies. */
  private receiptDraft(stored: StoredDraft, revision: string, hashes: Record<string, string>): Draft {
    if (revision === stored.draft.revision) return copy(stored.draft);
    const { id, workspaceId, intent, programRevision } = stored.draft;
    const superseded: Draft = {
      id,
      workspaceId,
      revision,
      state: "draft",
      intent,
      contentHashes: hashes,
      files: {},
      base: [],
      findings: [],
      ...(programRevision ? { programRevision } : {}),
      superseded: true,
    };
    return copy(superseded);
  }
  async create(
    workspaceId: string,
    base: DocumentSnapshot[],
    files: Record<string, string>,
    programRevision?: string,
    signal?: AbortSignal,
  ): Promise<Draft> {
    return this.locked(async () => {
      await this.roomForDraft(await this.scan());
      const draft = initialDraft(workspaceId, base, files, programRevision);
      await this.write({ format: 2, draft, requests: {} }, signal);
      return copy(draft);
    });
  }
  private active(stored: StoredDraft): void {
    if (stored.discarded) throw new Error("EXPIRED_DRAFT: this proposal was discarded.");
  }
  private noUncertainApplication(stored: StoredDraft): void {
    if (Object.values(stored.applications ?? {}).some((application) => application.status === "prepared"))
      throw new Error(
        "CONFLICT: prior application outcome needs inspection; edit, review and another key cannot bypass it.",
      );
  }
  async get(id: string): Promise<Draft> {
    const stored = await this.read(id);
    this.active(stored);
    return copy(stored.draft);
  }
  /** Discovery returns no source bodies or terminal proposals and survives host restarts. */
  async list(
    workspaceId: string,
  ): Promise<Array<{ draftId: string; revision: string; state: Draft["state"]; files: string[] }>> {
    return this.locked(async () =>
      (await this.scan())
        .flatMap(({ stored }) =>
          stored && !isFinished(stored) && stored.draft.workspaceId === workspaceId ? [stored.draft] : [],
        )
        .map((draft) => ({
          draftId: draft.id,
          revision: draft.revision,
          state: draft.state,
          files: Object.keys(draft.files).sort(),
        })),
    );
  }
  /** Persist one creation outcome. The callback runs only for a new request, never on retry. */
  async createOnce<T>(
    workspaceId: string,
    key: string,
    fingerprint: string,
    build: () => Promise<{
      base: DocumentSnapshot[];
      files: Record<string, string>;
      programRevision?: string;
      intent?: DesignIntent;
      data: T;
    }>,
    signal?: AbortSignal,
  ): Promise<{ draft: Draft; data: T }> {
    return this.locked(async () => {
      if (!key || key.length > 200 || !fingerprint)
        throw new Error("INVALID_ARGUMENT: creation key and fingerprint are required.");
      const entries = await this.scan();
      const prior = entries.find((entry) => entry.stored?.creation?.key === key)?.stored;
      const receipt = prior?.creation;
      if (prior && receipt) {
        if (prior.draft.workspaceId !== workspaceId)
          throw new Error("PROJECT_UNAVAILABLE: creation belongs to another workspace.");
        if (receipt.fingerprint !== fingerprint) throw new Error("CONFLICT: creation key reused with different input.");
        this.active(prior);
        if (prior.draft.state === "applied")
          throw new Error("EXPIRED_DRAFT: this proposal was already applied; its private pages are no longer kept.");
        const hashes = receipt.contentHashes;
        if (!hashes) throw new Error("EXPIRED_DRAFT: original creation receipt is unavailable.");
        return copy({ draft: this.receiptDraft(prior, "1", hashes), data: receipt.data }) as { draft: Draft; data: T };
      }
      await this.roomForDraft(entries);
      const prepared = await build();
      const draft = initialDraft(workspaceId, prepared.base, prepared.files, prepared.programRevision, prepared.intent);
      const outcome = { draft, data: prepared.data };
      // The creation receipt keeps the original data and hashes, not a second copy of the pages.
      const creation = {
        key,
        fingerprint,
        contentHashes: draft.contentHashes ?? {},
        ...(prepared.data === undefined ? {} : { data: copy(prepared.data) }),
      };
      await this.write({ format: 2, draft, requests: {}, creation }, signal);
      return copy(outcome);
    });
  }

  async edit(
    id: string,
    revision: string,
    key: string,
    fingerprint: string,
    update: (draft: Draft) => Promise<Record<string, string>>,
    signal?: AbortSignal,
  ): Promise<Draft> {
    return (
      await this.editDetailed(
        id,
        revision,
        key,
        fingerprint,
        async (draft) => ({ files: await update(draft), data: undefined }),
        signal,
      )
    ).draft;
  }
  async editDetailed<T>(
    id: string,
    revision: string,
    key: string,
    fingerprint: string,
    update: (draft: Draft) => Promise<{ files: Record<string, string>; data: T }>,
    signal?: AbortSignal,
  ): Promise<{ draft: Draft; data: T | undefined }> {
    return this.locked(async () => {
      if (!key || key.length > 200 || !fingerprint)
        throw new Error("INVALID_ARGUMENT: request key and fingerprint are required.");
      const stored = await this.read(id);
      this.active(stored);
      const previous = own(stored.requests, key);
      if (previous) {
        if (previous.fingerprint !== fingerprint)
          throw new Error("CONFLICT: request key already used with different input.");
        const data = previous.data === undefined ? undefined : (copy(previous.data) as T);
        return { draft: this.receiptDraft(stored, previous.revision, previous.contentHashes), data };
      }
      this.noUncertainApplication(stored);
      if (stored.draft.revision !== revision) throw new Error("CONFLICT: draft revision changed.");
      if (stored.draft.state === "applied")
        throw new Error("CONFLICT: create a new draft from the current source before editing again.");
      const { files, data } = await update(copy(stored.draft));
      const hashes = contentHashes(files);
      const draft: Draft = {
        ...stored.draft,
        files: copy(files),
        contentHashes: hashes,
        revision: String(Number(revision) + 1),
        state: "draft",
        findings: [],
        reviewedAssets: undefined,
        visualReview: undefined,
      };
      // The receipt names the committed revision and its hashes; it never copies the pages again.
      stored.requests = withEditReceipt(
        stored.requests,
        key,
        {
          fingerprint,
          revision: draft.revision,
          contentHashes: hashes,
          ...(data === undefined ? {} : { data: copy(data) }),
        },
        this.limits.maxReceipts,
      );
      stored.draft = draft;
      await this.write(stored, signal);
      return { draft: copy(draft), data: data === undefined ? undefined : copy(data) };
    });
  }
  /** Records the authoring service's review. The service must supply current validation first. */
  async review(
    id: string,
    revision: string,
    findings: unknown[],
    reviewedAssets: Record<string, string> = {},
    signal?: AbortSignal,
    visual?: Omit<VisualReview, "revision" | "attempt" | "customerDecisionRequired">,
  ): Promise<Draft> {
    return this.locked(async () => {
      const stored = await this.read(id);
      this.active(stored);
      if (stored.draft.state === "applied")
        throw new Error("CONFLICT: an applied proposal cannot become a new review.");
      this.noUncertainApplication(stored);
      if (stored.draft.revision !== revision) throw new Error("CONFLICT: draft revision changed.");
      const visualReview = visual
        ? reviewRecord(visual, revision, stored.draft.repairAttempts ?? 0)
        : stored.draft.visualReview;
      stored.draft = {
        ...stored.draft,
        state: "reviewed",
        findings: copy(findings),
        reviewedAssets: copy(reviewedAssets),
        visualReview,
        repairAttempts: visualReview?.attempt ?? stored.draft.repairAttempts,
      };
      await this.write(stored, signal);
      return copy(stored.draft);
    });
  }
  async discard(id: string, expected?: { workspaceId: string; revision: string }, signal?: AbortSignal): Promise<void> {
    return this.locked(async () => {
      let stored: StoredDraft;
      try {
        stored = await this.read(id);
      } catch (error) {
        if (!expected && error instanceof Error && error.message.startsWith("EXPIRED_DRAFT:")) return;
        throw error;
      }
      if (expected && stored.draft.workspaceId !== expected.workspaceId)
        throw new Error("PROJECT_UNAVAILABLE: draft belongs to another workspace.");
      if (expected && stored.draft.revision !== expected.revision)
        throw new Error("CONFLICT: draft changed before discard.");
      if (stored.discarded) return;
      // Retain only identity and the creation key so an old retry cannot revive the body.
      await this.write(finishedRecord(stored, this.limits.now(), "discarded"), signal, true);
    });
  }
  async apply<T>(
    id: string,
    revision: string,
    key: string,
    fingerprint: string,
    commit: (draft: Draft) => Promise<T>,
    preflight?: (draft: Draft) => Promise<void>,
  ): Promise<T> {
    return this.locked(async () => {
      if (!key || key.length > 200 || !fingerprint)
        throw new Error("INVALID_ARGUMENT: application key and fingerprint are required.");
      const stored = await this.read(id);
      this.active(stored);
      const applications = stored.applications ?? {};
      const previous = own(applications, key);
      if (previous) {
        if (previous.fingerprint !== fingerprint)
          throw new Error("CONFLICT: application key reused with different input.");
        if (previous.status !== "committed")
          throw new Error("CONFLICT: prior application outcome needs inspection; it will not be replayed.");
        return copy(previous.outcome) as T;
      }
      this.noUncertainApplication(stored);
      if (stored.draft.revision !== revision || stored.draft.state === "applied")
        throw new Error("CONFLICT: draft cannot be applied at this revision.");
      await preflight?.(copy(stored.draft));
      define<ApplicationReceipt>(applications, key, { fingerprint, status: "prepared" });
      stored.applications = applications;
      await this.write(stored, undefined, true);
      let outcome: T;
      try {
        outcome = await commit(copy(stored.draft));
      } catch (error) {
        if (error instanceof NoSourceChangeError) {
          delete applications[key];
          await this.write(stored, undefined, true);
          throw error;
        }
        throw new Error(
          `CONFLICT: source application outcome needs inspection: ${error instanceof Error ? error.message : String(error)}`,
        );
      }
      define<ApplicationReceipt>(applications, key, { fingerprint, status: "committed", outcome: copy(outcome) });
      // The source now holds the pages; the record keeps its receipts only.
      try {
        await this.write(finishedRecord(stored, this.limits.now(), "applied"), undefined, true);
      } catch {
        throw new Error(
          "CONFLICT: source application completed but its receipt could not be saved; inspect the source before continuing.",
        );
      }
      return outcome;
    });
  }
}
