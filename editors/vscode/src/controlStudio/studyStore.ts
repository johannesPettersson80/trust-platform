import * as fs from "fs";
import * as path from "path";
import { createHash, randomUUID } from "crypto";
import type * as vscode from "vscode";
import { STUDY_STORAGE_LIMIT_BYTES } from "./controlStudioLimits";
import type { ControlLoopDefinition } from "./loopDefinition";
import { controlStudioRequestsForFolder } from "./projectLoop";
import { carriedOverDraft } from "./studyCarryOver";
import {
  externalizeRecordings, inlineRecordings, isRecord, recordingIdFor, recordingReferences, type CompactRecordedRun,
} from "./recordingFile";
import { shareCopy } from "./recordedEvidence";

/**
 * The project study of one loop (Specification 35, A8.2 to A8.4 and B10.1). A study is one versioned document,
 * `control-studio/<loop id>/study.json`, plus one file per recording under `recordings/`. Earlier studies of the
 * same loop stay readable under `history/`. Writes are atomic and serialised per loop; a study over the storage
 * limit is refused before anything is written.
 */

/** The sections of a study document (A8.1). Recordings are not sections; each lives in its own file. */
export interface StudyState {
  readonly draft?: unknown;
  readonly history?: unknown;
  readonly page?: unknown;
  readonly mimo?: unknown;
}
export type StudySection = keyof StudyState;
const SECTIONS: readonly StudySection[] = ["draft", "history", "page", "mimo"];

/** The loop identity a study is keyed by (A8.2): the project, the loop id and the loop definition's binding revision. */
export interface StudyIdentity { readonly projectId: string; readonly id: string; readonly revision: string }

export interface StudyStoreOptions { readonly storageLimitBytes?: number }

const SCHEMA_VERSION = 1;
const STUDY_FILE = "study.json";
const RECORDINGS = "recordings";
const HISTORY = "history";

interface OpenStudy {
  readonly directory: string;
  readonly identity: StudyIdentity;
  readonly limit: number;
  state: StudyState;
  queue: Promise<void>;
}
const openStudies = new Map<string, OpenStudy>();

/** A folder name for a loop id. Letters, digits, '.', '-' and '_' are kept; anything else becomes '_', and a short hash is added when that changes the id. */
export function studyFolderName(loopId: string): string {
  const safe = loopId.replace(/[^A-Za-z0-9._-]/g, "_");
  if (safe === loopId && safe.length > 0 && !/^\.+$/.test(safe)) return safe;
  const hash = createHash("sha256").update(loopId, "utf8").digest("hex").slice(0, 8);
  const stem = safe.replace(/^\.+/, "").slice(0, 48);
  return `${stem || "loop"}-${hash}`;
}

/** The folder that holds one loop's study in a project: `control-studio/<loop id>/`. */
export function studyDirectory(projectRoot: string, loopId: string): string {
  return path.join(projectRoot, "control-studio", studyFolderName(loopId));
}

/** Writes a file through a temporary file in the same folder, so a failed write leaves the previous file whole. */
export async function writeFileAtomic(file: string, text: string): Promise<void> {
  const directory = path.dirname(file);
  await fs.promises.mkdir(directory, { recursive: true });
  const temporary = path.join(directory, `.${path.basename(file)}.${randomUUID()}.tmp`);
  try {
    const handle = await fs.promises.open(temporary, "wx");
    try {
      await handle.writeFile(text, "utf8");
      await handle.sync();
    } finally {
      await handle.close();
    }
    await fs.promises.rename(temporary, file);
  } catch (error) {
    await fs.promises.rm(temporary, { force: true }).catch(() => undefined);
    throw error;
  }
}

/** Finds the project folder whose loop definition declares this loop, preferring the exact binding revision. */
export async function projectRootHoldingLoop(loop: ControlLoopDefinition,
  folders: readonly vscode.WorkspaceFolder[]): Promise<string | undefined> {
  let sameLoop: string | undefined;
  for (const folder of folders) {
    const resolutions = await controlStudioRequestsForFolder(folder, loop.runtimeId);
    for (const resolution of resolutions) {
      if (resolution.kind !== "accepted") continue;
      const declared = resolution.request.loopDefinition;
      if (declared.id !== loop.id || declared.projectId !== loop.projectId) continue;
      if (declared.revision === loop.revision) return folder.uri.fsPath;
      sameLoop = sameLoop ?? folder.uri.fsPath;
    }
  }
  return sameLoop;
}

/**
 * Reads the loop's project study and makes it the open study. A study of an earlier binding revision moves to
 * `history/`, and the new working study keeps only the requirements and limits whose signals still exist (A8.2).
 */
export async function openProjectStudy(projectRoot: string, loop: ControlLoopDefinition,
  options: StudyStoreOptions = {}): Promise<StudyState> {
  const identity = identityOf(loop);
  const directory = studyDirectory(projectRoot, loop.id);
  const limit = options.storageLimitBytes ?? STUDY_STORAGE_LIMIT_BYTES;
  // Reopening reads the study as last saved (A8.6): a write the closing panel queued, such as its final draft, finishes first.
  await openStudies.get(keyOf(identity))?.queue.catch(() => undefined);
  openStudies.delete(keyOf(identity));
  const stored = await readDocument(directory, identity);
  let state: StudyState = stored?.state ?? {};
  if (stored && stored.bindingRevision !== identity.revision) {
    await archiveWorkingStudy(directory, stored.bindingRevision);
    state = carriedOverState(stored.state, loop);
    await writeDocument(directory, directory, identity, state, limit);
  }
  openStudies.set(keyOf(identity), { directory, identity, limit, state, queue: Promise.resolve() });
  return state;
}

/** The open study's sections, as last loaded or saved. Empty when the study is not open for this revision. */
export function projectStudyState(identity: StudyIdentity): StudyState {
  const entry = openStudies.get(keyOf(identity));
  return entry && entry.identity.revision === identity.revision ? entry.state : {};
}

/** Replaces one section of the open study and writes the study document atomically, in order with other writes. */
export function updateProjectStudy(identity: StudyIdentity, section: StudySection, value: unknown): Promise<void> {
  const entry = openEntry(identity);
  const previous = entry.state;
  const next = withSection(previous, section, value === undefined ? undefined : shareCopy(value));
  entry.state = next;
  // The write runs in a later turn, so the change that queued it and the write each hold the extension host briefly (A10.7).
  const writing = entry.queue.catch(() => undefined).then(nextTurn)
    .then(() => writeDocument(entry.directory, entry.directory, entry.identity, next, entry.limit));
  entry.queue = writing;
  return writing.catch(error => {
    if (entry.state === next) entry.state = previous;
    throw error;
  });
}

/**
 * Starts a new working study (A8.4 Reset). The previous working study moves to `history/` first, so nothing is
 * lost if the new study cannot be written.
 */
export function startNewWorkingStudy(identity: StudyIdentity, initial: StudyState): Promise<void> {
  const entry = openEntry(identity);
  const next = copySections(initial);
  const writing = entry.queue.catch(() => undefined).then(async () => {
    const archived = await archiveWorkingStudy(entry.directory, entry.identity.revision);
    if (archived) entry.state = {};
    await writeDocument(entry.directory, entry.directory, entry.identity, next, entry.limit);
    entry.state = next;
  });
  entry.queue = writing;
  return writing;
}

/** Keeps a study of an earlier binding revision as history, without making it the working study (migration). */
export function archiveStudyState(identity: StudyIdentity, revision: string, state: StudyState): Promise<void> {
  const entry = openEntry(identity);
  const writing = entry.queue.catch(() => undefined).then(async () => {
    const folder = await uniqueFolder(path.join(entry.directory, HISTORY), studyFolderName(revision));
    await writeDocument(entry.directory, folder, { ...entry.identity, revision }, copySections(state), entry.limit);
  });
  entry.queue = writing;
  return writing;
}

/** A storage port over one section of the open study, for code that reads and writes whole values. */
export function projectStudyStorage(identity: StudyIdentity, section: StudySection):
  { get(key: string): unknown; update(key: string, value: unknown): Promise<void> } {
  return {
    get: () => projectStudyState(identity)[section],
    update: (_key, value) => updateProjectStudy(identity, section, value),
  };
}

function identityOf(loop: ControlLoopDefinition): StudyIdentity {
  return { projectId: loop.projectId, id: loop.id, revision: loop.revision };
}

function keyOf(identity: StudyIdentity): string {
  return `${identity.projectId}\u0000${identity.id}`;
}

function openEntry(identity: StudyIdentity): OpenStudy {
  const entry = openStudies.get(keyOf(identity));
  if (!entry) throw new Error("This loop's project study is not open. Open Control Studio from its project folder so the study can be stored.");
  if (entry.identity.revision !== identity.revision) {
    throw new Error("The loop definition changed after Control Studio opened. Reopen Control Studio before saving.");
  }
  return entry;
}

function nextTurn(): Promise<void> {
  return new Promise(resolve => setImmediate(resolve));
}

function withSection(state: StudyState, section: StudySection, value: unknown): StudyState {
  const next: Record<string, unknown> = { ...state, [section]: value };
  if (value === undefined) delete next[section];
  return next as StudyState;
}

function copySections(state: StudyState): StudyState {
  return Object.fromEntries(SECTIONS.flatMap(section => state[section] === undefined ? []
    : [[section, shareCopy(state[section])]])) as StudyState;
}

function carriedOverState(state: StudyState, loop: ControlLoopDefinition): StudyState {
  const draft = carriedOverDraft(state.draft, loop);
  return draft ? { draft } : {};
}

interface StoredDocument { readonly bindingRevision: string; readonly state: StudyState }

async function readDocument(directory: string, identity: StudyIdentity): Promise<StoredDocument | undefined> {
  const file = path.join(directory, STUDY_FILE);
  const text = await readOptional(file);
  if (text === undefined) return undefined;
  const document = parseStored(text, file);
  const header = isRecord(document) ? document.loop : undefined;
  const stored = isRecord(document) ? document.state : undefined;
  const bindingRevision = isRecord(header) ? header.bindingRevision : undefined;
  if (!isRecord(document) || document.schemaVersion !== SCHEMA_VERSION || !isRecord(header) || !isRecord(stored)
      || header.id !== identity.id || header.projectId !== identity.projectId || typeof bindingRevision !== "string") {
    throw new Error(`${file} is not a Control Studio study for this loop. It was left in place.`);
  }
  const runs = new Map<string, CompactRecordedRun>();
  for (const id of new Set(recordingReferences(stored))) runs.set(id, await readRecording(directory, id));
  const load = (id: string): CompactRecordedRun => {
    const run = runs.get(id);
    if (!run) throw new Error(`The study refers to recording ${id}, which is missing. The study was left in place.`);
    return run;
  };
  const state: Record<string, unknown> = {};
  for (const section of SECTIONS) {
    if (stored[section] !== undefined) state[section] = inlineRecordings(stored[section], load);
  }
  return { bindingRevision, state: state as StudyState };
}

async function readRecording(directory: string, id: string): Promise<CompactRecordedRun> {
  const file = recordingFile(directory, id);
  const text = await readOptional(file);
  const document = text === undefined ? undefined : parseStored(text, file);
  const run = isRecord(document) ? document.run : undefined;
  if (!isRecord(document) || document.schemaVersion !== SCHEMA_VERSION || !isRecord(run) || recordingIdFor(run) !== id) {
    throw new Error(`The recording file ${path.basename(file)} is missing or does not match its identity. The study was left in place.`);
  }
  return run;
}

/**
 * Writes a study document and its new recording files into `directory`. Recordings already on disk are never
 * rewritten. The size of the whole loop folder, history included, is checked before anything is written.
 */
async function writeDocument(loopDirectory: string, directory: string, identity: StudyIdentity, state: StudyState,
  limit: number): Promise<void> {
  const runs = new Map<string, CompactRecordedRun>();
  const sections: Record<string, unknown> = {};
  for (const section of SECTIONS) {
    if (state[section] !== undefined) {
      sections[section] = externalizeRecordings(state[section], (id, run) => { runs.set(id, run); });
    }
  }
  const document = `${JSON.stringify({
    schemaVersion: SCHEMA_VERSION,
    loop: { id: identity.id, projectId: identity.projectId, bindingRevision: identity.revision },
    state: sections,
  }, null, 2)}\n`;
  const incoming = new Map<string, string>();
  for (const [id, run] of runs) {
    if (!(await exists(recordingFile(directory, id)))) incoming.set(id, `${JSON.stringify({ schemaVersion: SCHEMA_VERSION, run })}\n`);
  }
  const documentFile = path.join(directory, STUDY_FILE);
  const kept = await bytesIn(loopDirectory) - await sizeOf(documentFile);
  const total = kept + Buffer.byteLength(document, "utf8")
    + [...incoming.values()].reduce((sum, text) => sum + Buffer.byteLength(text, "utf8"), 0);
  if (total > limit) {
    throw new Error(`The study is ${total} bytes (${mebibytes(total)} MiB), above the limit of ${limit} bytes (${mebibytes(limit)} MiB) for one loop's study storage. Nothing was written.`);
  }
  for (const [id, text] of incoming) await writeFileAtomic(recordingFile(directory, id), text);
  await writeFileAtomic(documentFile, document);
}

/** Moves the working study into `history/<revision>/`. Returns false when there was no working study to move. */
async function archiveWorkingStudy(loopDirectory: string, revision: string): Promise<boolean> {
  const study = path.join(loopDirectory, STUDY_FILE);
  if (!(await exists(study))) return false;
  const folder = await uniqueFolder(path.join(loopDirectory, HISTORY), studyFolderName(revision));
  await fs.promises.mkdir(folder, { recursive: true });
  const recordings = path.join(loopDirectory, RECORDINGS);
  const movedRecordings = await exists(recordings);
  if (movedRecordings) await fs.promises.rename(recordings, path.join(folder, RECORDINGS));
  try {
    await fs.promises.rename(study, path.join(folder, STUDY_FILE));
  } catch (error) {
    if (movedRecordings) await fs.promises.rename(path.join(folder, RECORDINGS), recordings).catch(() => undefined);
    throw error;
  }
  return true;
}

async function uniqueFolder(parent: string, name: string): Promise<string> {
  for (let index = 1; ; index += 1) {
    const candidate = path.join(parent, index === 1 ? name : `${name}-${index}`);
    if (!(await exists(candidate))) return candidate;
  }
}

function recordingFile(directory: string, id: string): string {
  return path.join(directory, RECORDINGS, `${id}.json`);
}

function parseStored(text: string, file: string): unknown {
  try {
    return JSON.parse(text);
  } catch {
    throw new Error(`${file} is not valid JSON. It was left in place.`);
  }
}

async function readOptional(file: string): Promise<string | undefined> {
  try {
    return await fs.promises.readFile(file, "utf8");
  } catch (error) {
    if (error instanceof Error && "code" in error && error.code === "ENOENT") return undefined;
    throw error;
  }
}

async function exists(file: string): Promise<boolean> {
  return fs.promises.access(file).then(() => true, () => false);
}

async function sizeOf(file: string): Promise<number> {
  return fs.promises.stat(file).then(stat => stat.size, () => 0);
}

async function bytesIn(directory: string): Promise<number> {
  let entries: fs.Dirent[];
  try {
    entries = await fs.promises.readdir(directory, { withFileTypes: true });
  } catch {
    return 0;
  }
  let total = 0;
  for (const entry of entries) {
    const file = path.join(directory, entry.name);
    total += entry.isDirectory() ? await bytesIn(file) : await sizeOf(file);
  }
  return total;
}

function mebibytes(bytes: number): string {
  return (bytes / (1024 * 1024)).toFixed(1);
}
