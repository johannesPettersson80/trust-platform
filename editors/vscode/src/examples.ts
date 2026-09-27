import * as vscode from "vscode";

import {
  compactExampleDescription,
  controlLessonPositionLabel,
  controlLessonProgressState,
  controlModuleId,
  controlModulesFromEntries,
  hardwareBadge,
  isExampleCopyable,
  type ExampleEntry,
  type GalleryView,
} from "./examples/model";
import { copyExample } from "./examples/copy";
import {
  controlLessonPresentation,
  controlLessonDetail,
  retainControlLessonProgress,
} from "./examples/controlLesson";
import { loadExamples } from "./examples/load";
import { readControlLessonProjects } from "./examples/projectLocations";
import { openRetainedControlLessonProject } from "./examples/projectReopening";
import { renderExamplesGalleryHtml } from "./examples/galleryWebview";
import { EXAMPLE_TOPICS, exampleDocumentationUrl, exampleTopicAction } from "./examples/topics";
import { BUNDLED_EXAMPLE_SCHEME, BundledExamplePreviews } from "./examples/bundledFilePreview";
import { renderLessonMarkup } from "./examples/lessonMarkup";

// S-25 — "Start from example": a browsable gallery of bundled starters with hardware badges. Choosing a
// card copies an editable working project and opens it (focus Main.st). The user never hand-edits TOML
// to start; a "No hardware" starter is immediately runnable in the Simulator.

export const START_FROM_EXAMPLE_COMMAND = "trust.examples.start";

const COMPLETED_CONTROL_LESSONS_KEY = "trust.examples.completedControlLessons";
let examplesPanel: ExamplesGalleryPanel | undefined;

export function registerExamples(context: vscode.ExtensionContext): void {
  const previews = new BundledExamplePreviews(vscode.Uri.joinPath(context.extensionUri, "media", "examples"));
  context.subscriptions.push(
    vscode.workspace.registerTextDocumentContentProvider(BUNDLED_EXAMPLE_SCHEME, previews),
    vscode.commands.registerCommand(START_FROM_EXAMPLE_COMMAND, () =>
      openExamplesGallery(context, previews)
    )
  );
}

async function openExamplesGallery(context: vscode.ExtensionContext, previews: BundledExamplePreviews): Promise<void> {
  let entries: ExampleEntry[];
  try {
    entries = await loadExamples(context);
  } catch (error) {
    void vscode.window.showErrorMessage(
      `Could not load the examples list: ${String(error)}`
    );
    return;
  }

  if (!examplesPanel) {
    examplesPanel = new ExamplesGalleryPanel(context, previews);
  }
  examplesPanel.reveal(entries);
}

class ExamplesGalleryPanel {
  private readonly panel: vscode.WebviewPanel;
  private entries: ExampleEntry[] = [];
  private view: GalleryView = { kind: "landing" };
  private visibleFileTargets = new Set<string>();
  private previewGeneration = 0;

  constructor(private readonly context: vscode.ExtensionContext, private readonly previews: BundledExamplePreviews) {
    this.panel = vscode.window.createWebviewPanel(
      "trust.examples",
      "Start from example",
      vscode.ViewColumn.Beside,
      {
        enableScripts: true,
        retainContextWhenHidden: true,
        localResourceRoots: [context.extensionUri],
      }
    );
    this.panel.webview.html = this.html();
    this.panel.onDidDispose(() => {
      this.previewGeneration += 1;
      examplesPanel = undefined;
    });
    this.panel.webview.onDidReceiveMessage((message) => {
      void this.onMessage(message);
    });
  }

  reveal(entries: ExampleEntry[]): void {
    this.entries = entries;
    this.view = { kind: "landing" };
    this.panel.reveal(vscode.ViewColumn.Beside);
    void this.postState();
  }

  private async onMessage(message: unknown): Promise<void> {
    if (!message || typeof message !== "object") {
      return;
    }
    const msg = message as { type?: string; id?: string; target?: unknown };
    switch (msg.type) {
      case "previewLessonFile": {
        if (this.view.kind !== "lesson" || this.view.id !== msg.id || typeof msg.target !== "string" ||
            !this.visibleFileTargets.has(msg.target)) return;
        const entry = this.entries.find(candidate => candidate.id === msg.id);
        if (!entry?.lesson) return;
        const generation = ++this.previewGeneration;
        const stillCurrent = () => this.previewGeneration === generation && this.panel.active && this.view.kind === "lesson" && this.view.id === entry.id;
        try {
          if (!stillCurrent()) return;
          // A source preview must not replace the gallery's own preview tab.
          await vscode.commands.executeCommand("workbench.action.keepEditor");
          if (!stillCurrent()) return;
          const uri = await this.previews.prepare(entry, msg.target, [...this.visibleFileTargets]);
          if (!stillCurrent()) return;
          const document = await vscode.workspace.openTextDocument(uri);
          if (!stillCurrent()) return;
          await vscode.window.showTextDocument(document, { preview: true, viewColumn: this.panel.viewColumn });
        } catch (error) {
          if (stillCurrent()) await this.panel.webview.postMessage({ type: "filePreviewError", id: entry.id,
            message: `Could not preview ${msg.target}: ${error instanceof Error ? error.message : String(error)}` });
        }
        return;
      }
      case "ready":
        await this.postState();
        return;
      case "openControlCurriculum":
        this.view = { kind: "control" };
        await this.postState();
        return;
      case "openControlCourse":
        if (controlModulesFromEntries(this.entries).some((module) => module.id === msg.id)) {
          this.view = { kind: "course", id: msg.id as string };
          await this.postState();
        }
        return;
      case "openControlLesson":
        if (this.controlLessons().some((entry) => entry.id === msg.id)) {
          this.view = { kind: "lesson", id: msg.id as string };
          await this.postState();
        }
        return;
      case "openRetainedProject": {
        const entry = this.entries.find((candidate) => candidate.id === msg.id);
        if (!entry?.lesson) return;
        const result = await openRetainedControlLessonProject(
          this.context,
          entry.id,
          readControlLessonProjects(this.context.globalState),
        );
        if (result.kind === "missing") {
          const choice = await vscode.window.showWarningMessage(
            `Saved project ${result.projectPath} is missing. Locate moved folder or cancel.`,
            "Locate moved folder",
            "Cancel",
          );
          if (choice === "Locate moved folder") {
            await vscode.commands.executeCommand("workbench.action.files.openFolder");
          }
        }
        return;
      }
      case "browseExamples":
        this.view = { kind: "browse" };
        await this.postState();
        return;
      case "selectTopic":
        if (EXAMPLE_TOPICS.some((topic) => topic.id === msg.id)) {
          this.view = { kind: "browse" };
          await this.panel.webview.postMessage({ type: "topicSelection", topicId: msg.id });
          await this.postState();
        }
        return;
      case "navigateBack":
        this.view = this.parentView();
        await this.postState();
        return;
      case "openExamplesDocumentation":
        {
          const action = exampleTopicAction(String(msg.id ?? ""));
          const topicId = action.type === "openExamplesDocumentation"
            ? action.topicId
            : String(msg.id ?? "");
          await vscode.env.openExternal(vscode.Uri.parse(exampleDocumentationUrl(topicId)));
        return;
        }
      case "useExample": {
        const entry = this.entries.find((candidate) => candidate.id === msg.id);
        if (entry && isExampleCopyable(entry)) {
          const copied = await copyExample(this.context, entry);
          if (copied && entry.lesson) {
            // Retained signal: a project folder now exists for this lesson. This is real, but
            // it is not evidence of a finished Monitor -> Test run — see controlLessonProgressState
            // (W-05), which is what turns this id list into the honest per-lesson label.
            const created = new Set(this.completedLessonIds());
            created.add(entry.id);
            await this.context.globalState.update(
              COMPLETED_CONTROL_LESSONS_KEY,
              Array.from(created).sort()
            );
            await this.postState();
          }
        }
        return;
      }
    }
  }

  private controlLessons(): ExampleEntry[] {
    return this.entries
      .filter((entry) => entry.lesson)
      .sort((left, right) =>
        (left.lesson?.sequence ?? 0) - (right.lesson?.sequence ?? 0)
      );
  }

  private completedLessonIds(): string[] {
    return retainControlLessonProgress(
      this.context.globalState.get<string[]>(COMPLETED_CONTROL_LESSONS_KEY, []),
      this.controlLessons().map((entry) => entry.id)
    );
  }

  private parentView(): GalleryView {
    if (this.view.kind === "lesson") {
      const lessonId = this.view.id;
      const lesson = this.entries.find((entry) => entry.id === lessonId);
      const moduleId = lesson?.lesson ? controlModuleId(lesson.path) : undefined;
      return moduleId ? { kind: "course", id: moduleId } : { kind: "control" };
    }
    if (this.view.kind === "course") {
      return { kind: "control" };
    }
    return { kind: "landing" };
  }

  private async postState(): Promise<void> {
    this.previewGeneration += 1;
    const controlLessonCount = this.controlLessons().length;
    const createdProjectIds = this.completedLessonIds();
    const retainedProjects = readControlLessonProjects(this.context.globalState);
    this.visibleFileTargets.clear();
    await this.panel.webview.postMessage({
      type: "state",
      view: this.view,
      courses: controlModulesFromEntries(this.entries),
      topics: EXAMPLE_TOPICS.map(({ id, title, description }) => ({ id, title, description })),
      completedLessonIds: createdProjectIds,
      examples: this.entries.map((entry) => ({
        id: entry.id,
        title: entry.title,
        description: compactExampleDescription(entry.description),
        hardware: entry.hardware,
        hardwareLabel: hardwareBadge(entry.hardware),
        tags: entry.tags ?? [],
        lesson: entry.lesson
          ? {
              ...entry.lesson,
              module: controlModuleId(entry.path),
              positionLabel: controlLessonPositionLabel(
                entry.lesson.sequence,
                controlLessonCount
              ),
              presentation: controlLessonPresentation(entry.lesson),
              detail: this.lessonDetail(entry),
              progressState: controlLessonProgressState(entry.id, createdProjectIds),
              retainedProjectPaths: retainedProjects
                .filter((record) => record.lessonId === entry.id)
                .map((record) => record.projectPath),
            }
          : undefined,
        lessonAvailability: entry.lessonAvailability
          ? {
              kind: entry.lessonAvailability.kind,
              reason: entry.lessonAvailability.kind === "unavailable"
                ? entry.lessonAvailability.reason
                : undefined,
            }
          : undefined,
      })),
    });
  }

  private lessonDetail(entry: ExampleEntry) {
    if (!entry.lesson) return undefined;
    const detail = controlLessonDetail(entry.lesson, entry.description);
    if (this.view.kind !== "lesson" || this.view.id !== entry.id) return detail;
    const markup = (text: string) => {
      const result = renderLessonMarkup(text);
      for (const file of result.files) this.visibleFileTargets.add(file);
      return result.html;
    };
    return { ...detail, markup: { description: markup(detail.description), objective: markup(detail.objective),
      expectedObservation: markup(detail.expectedObservation) },
      preview: detail.preview.map(section => ({ ...section, markup: section.paragraphs.map(markup) })) };
  }

  private html(): string {
    return renderExamplesGalleryHtml(this.panel, this.context);
  }
}
