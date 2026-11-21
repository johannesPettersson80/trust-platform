import * as fs from "fs";
import * as vscode from "vscode";
import * as nodePath from "path";

// Persist the intended destination across the folder-opening window reload.
export const FOCUS_MAIN_KEY = "trust.newProject.focusMain";
export const OPEN_LESSON_README_KEY = "trust.examples.openLessonReadme";
/** Where an earlier version kept the structured intent; an intent found there moves to the file. */
export const PROJECT_LANDING_KEY = "trust.projectLanding";
/** The pending intent's one stored representation, in the extension's global storage (specification 25 §3.2). */
export const PROJECT_LANDING_FILE = "project-landing.json";

export interface ProjectLandingIntent {
  intentId: string;
  projectPath: string;
  kind: "starter" | "lesson";
  filePath: string;
  exampleId?: string;
}

export type ProjectLandingResult =
  | { kind: "none" | "waiting" | "opened" }
  | { kind: "cleared"; message: string }
  | { kind: "failed"; message: string; retryable: true };

let landingInFlight: Promise<ProjectLandingResult> | undefined;

/** The pending intent, read from its file as any window last wrote it. The intent is not kept in `globalState`: VS Code
 * sends a window its own earlier write back about 200 ms after it, so a value written in between disappears in that
 * window for a moment, and is lost for good when a further write follows (VS Code 1.147, checklist W49). */
export function readProjectLanding(context: vscode.ExtensionContext): ProjectLandingIntent | undefined {
  let text: string;
  try {
    text = fs.readFileSync(projectLandingFile(context), "utf8");
  } catch {
    return undefined;
  }
  try {
    return JSON.parse(text) as ProjectLandingIntent;
  } catch {
    return undefined;
  }
}

/** Replaces the pending intent for every window at once, or removes it: a reader sees the earlier file or this one. */
function writeProjectLanding(context: vscode.ExtensionContext, intent: ProjectLandingIntent | undefined): void {
  const file = projectLandingFile(context);
  if (!intent) {
    fs.rmSync(file, { force: true });
    return;
  }
  fs.mkdirSync(nodePath.dirname(file), { recursive: true });
  const written = `${file}.${process.pid}-${Date.now()}.tmp`;
  fs.writeFileSync(written, JSON.stringify(intent), "utf8");
  fs.renameSync(written, file);
}

function projectLandingFile(context: vscode.ExtensionContext): string {
  return nodePath.join(context.globalStorageUri.fsPath, PROJECT_LANDING_FILE);
}

/** Clears a key an earlier version used, without a write when it holds nothing. */
async function clearEarlierKey(context: vscode.ExtensionContext, key: string): Promise<void> {
  if (context.globalState.get(key) !== undefined) await context.globalState.update(key, undefined);
}

/** Consume the one structured landing intent only in its destination workspace. */
export function consumeProjectLanding(context: vscode.ExtensionContext): Promise<ProjectLandingResult> {
  if (landingInFlight) return landingInFlight;
  landingInFlight = consumeProjectLandingOnce(context).finally(() => { landingInFlight = undefined; });
  return landingInFlight;
}

async function consumeProjectLandingOnce(context: vscode.ExtensionContext): Promise<ProjectLandingResult> {
  // An intent an earlier version left in globalState moves to the file, unless this version has written one since.
  const earlier = context.globalState.get<ProjectLandingIntent>(PROJECT_LANDING_KEY);
  if (earlier) {
    if (!readProjectLanding(context)) writeProjectLanding(context, earlier);
    await context.globalState.update(PROJECT_LANDING_KEY, undefined);
  }
  const storedIntent = readProjectLanding(context);
  let intent = storedIntent;
  let legacyKey: string | undefined;
  if (!intent) {
    const legacy = await matchingLegacyLanding(context);
    if (readProjectLanding(context)) return { kind: "waiting" };
    if (legacy.kind === "ambiguous") {
      await context.globalState.update(FOCUS_MAIN_KEY, undefined);
      await context.globalState.update(OPEN_LESSON_README_KEY, undefined);
      return { kind: "cleared", message: "More than one pending landing matches this workspace. Start the project or copy the lesson again to choose a destination." };
    }
    if (!legacy.intent) return { kind: "none" };
    intent = legacy.intent;
    legacyKey = legacy.key;
  }
  const uri = vscode.Uri.file(intent.filePath);
  const projectUri = vscode.Uri.file(intent.projectPath);
  const projectWorkspace = vscode.workspace.getWorkspaceFolder(projectUri);
  if (!projectWorkspace) return { kind: "waiting" };
  const relative = nodePath.relative(projectUri.fsPath, uri.fsPath);
  if (relative === "" || relative === ".." || relative.startsWith(`..${nodePath.sep}`) || nodePath.isAbsolute(relative)) {
    return { kind: "waiting" };
  }
  const fileWorkspace = vscode.workspace.getWorkspaceFolder(uri);
  if (!fileWorkspace || fileWorkspace.uri.toString() !== projectWorkspace.uri.toString()) {
    return { kind: "waiting" };
  }
  try {
    await vscode.workspace.fs.stat(uri);
  } catch (error) {
    const current = readProjectLanding(context);
    if (current?.intentId !== storedIntent?.intentId) return { kind: "waiting" };
    if ((error as { code?: string }).code !== "FileNotFound") {
      return {
        kind: "failed",
        retryable: true,
        message: `Could not inspect the pending ${intent.kind} landing file ${intent.filePath}. Retry the activation.`,
      };
    }
    if (storedIntent) writeProjectLanding(context, undefined);
    if (legacyKey) await context.globalState.update(legacyKey, undefined);
    return { kind: "cleared", message: `The pending ${intent.kind} landing file ${intent.filePath} is missing. Recreate the project or copy the example again.` };
  }
  const latestBeforeOpen = readProjectLanding(context);
  if (latestBeforeOpen?.intentId !== storedIntent?.intentId) return { kind: "waiting" };
  try {
    if (intent.kind === "lesson") {
      await vscode.commands.executeCommand("markdown.showPreview", uri);
    } else {
      const document = await vscode.workspace.openTextDocument(uri);
      await vscode.window.showTextDocument(document, { preview: false });
    }
  } catch {
    const current = readProjectLanding(context);
    if (current?.intentId !== storedIntent?.intentId) return { kind: "waiting" };
    return {
      kind: "failed",
      retryable: true,
      message: `Could not open the pending ${intent.kind} landing file ${intent.filePath}. Retry the activation to open it.`,
    };
  }
  const current = readProjectLanding(context);
  if (storedIntent && current?.intentId === intent.intentId) writeProjectLanding(context, undefined);
  if (legacyKey) await context.globalState.update(legacyKey, undefined);
  return { kind: "opened" };
}

async function matchingLegacyLanding(context: vscode.ExtensionContext): Promise<
  { intent?: ProjectLandingIntent; key?: string; kind: "one" | "none" | "ambiguous" }
> {
  const candidates: Array<{ intent: ProjectLandingIntent; key: string }> = [];
  const focus = context.globalState.get<string>(FOCUS_MAIN_KEY);
  if (focus) candidates.push({
    key: FOCUS_MAIN_KEY,
    intent: { intentId: `legacy-focus-${focus}`, projectPath: nodePath.dirname(nodePath.dirname(focus)), kind: "starter", filePath: focus },
  });
  const readme = context.globalState.get<string>(OPEN_LESSON_README_KEY);
  if (readme) candidates.push({
    key: OPEN_LESSON_README_KEY,
    intent: { intentId: `legacy-readme-${readme}`, projectPath: nodePath.dirname(readme), kind: "lesson", filePath: readme, exampleId: "legacy-control-lesson" },
  });
  const matching = candidates.filter(({ intent }) => Boolean(vscode.workspace.getWorkspaceFolder(vscode.Uri.file(intent.projectPath))));
  if (matching.length > 1) return { kind: "ambiguous" };
  if (matching.length === 0) return { kind: "none" };
  return { kind: "one", intent: matching[0].intent, key: matching[0].key };
}

/** Keep starter activation as the exclusive landing owner. The project is the one named, or else the parent of
 * the starter's `src/`. */
export async function prepareStarterProjectHandoff(
  context: vscode.ExtensionContext,
  mainPath: string,
  projectPath: string = nodePath.dirname(nodePath.dirname(mainPath))
): Promise<void> {
  writeProjectLanding(context, {
    intentId: `${Date.now()}-${Math.random().toString(36).slice(2)}`,
    projectPath,
    kind: "starter",
    filePath: mainPath,
  });
  await clearEarlierKey(context, PROJECT_LANDING_KEY);
  await clearEarlierKey(context, FOCUS_MAIN_KEY);
  await clearEarlierKey(context, OPEN_LESSON_README_KEY);
}

export async function prepareLessonProjectHandoff(
  context: vscode.ExtensionContext,
  projectPath: string,
  readmePath: string,
  exampleId: string
): Promise<void> {
  writeProjectLanding(context, {
    intentId: `${Date.now()}-${Math.random().toString(36).slice(2)}`,
    projectPath,
    kind: "lesson",
    filePath: readmePath,
    exampleId,
  });
  await clearEarlierKey(context, PROJECT_LANDING_KEY);
  await clearEarlierKey(context, FOCUS_MAIN_KEY);
  await clearEarlierKey(context, OPEN_LESSON_README_KEY);
}
