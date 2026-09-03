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
import { copyExample, openPendingControlLessonReadme } from "./examples/copy";
import {
  controlLessonPresentation,
  retainControlLessonProgress,
} from "./examples/controlLesson";
import { loadExamples } from "./examples/load";
import { EXAMPLE_TOPICS, exampleDocumentationUrl } from "./examples/topics";

// S-25 — "Start from example": a browsable gallery of bundled starters with hardware badges. Choosing a
// card copies an editable working project and opens it (focus Main.st). The user never hand-edits TOML
// to start; a "No hardware" starter is immediately runnable in the Simulator.

export const START_FROM_EXAMPLE_COMMAND = "trust.examples.start";

const COMPLETED_CONTROL_LESSONS_KEY = "trust.examples.completedControlLessons";
let examplesPanel: ExamplesGalleryPanel | undefined;

export function registerExamples(context: vscode.ExtensionContext): void {
  context.subscriptions.push(
    vscode.commands.registerCommand(START_FROM_EXAMPLE_COMMAND, () =>
      openExamplesGallery(context)
    )
  );
  // After "Create project" reloads the window into the new lesson folder (W-06), land the
  // learner on the lesson text instead of an empty Explorer.
  void openPendingControlLessonReadme(context);
}

async function openExamplesGallery(context: vscode.ExtensionContext): Promise<void> {
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
    examplesPanel = new ExamplesGalleryPanel(context);
  }
  examplesPanel.reveal(entries);
}

class ExamplesGalleryPanel {
  private readonly panel: vscode.WebviewPanel;
  private entries: ExampleEntry[] = [];
  private view: GalleryView = { kind: "landing" };

  constructor(private readonly context: vscode.ExtensionContext) {
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
    const msg = message as { type?: string; id?: string };
    switch (msg.type) {
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
      case "browseExamples":
        this.view = { kind: "browse" };
        await this.postState();
        return;
      case "navigateBack":
        this.view = this.parentView();
        await this.postState();
        return;
      case "openExamplesDocumentation":
        await vscode.env.openExternal(
          vscode.Uri.parse(exampleDocumentationUrl(msg.id))
        );
        return;
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
    const controlLessonCount = this.controlLessons().length;
    const createdProjectIds = this.completedLessonIds();
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
              progressState: controlLessonProgressState(entry.id, createdProjectIds),
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

  private html(): string {
    const nonce = nonceValue();
    const themeUri = this.panel.webview.asWebviewUri(
      vscode.Uri.joinPath(this.context.extensionUri, "src", "webview", "theme.css")
    );
    const csp = `default-src 'none'; style-src ${this.panel.webview.cspSource} 'unsafe-inline'; script-src 'nonce-${nonce}';`;
    return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <meta http-equiv="Content-Security-Policy" content="${csp}" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>Start from example</title>
  <link rel="stylesheet" href="${themeUri}" />
  <style>
    * { box-sizing: border-box; }
    [hidden] { display: none !important; }
    body {
      margin: 0;
      font-family: var(--vscode-font-family);
      font-size: var(--vscode-font-size);
    }
    .shell {
      max-width: 980px;
      margin: 0 auto;
      padding: 18px;
    }
    header {
      border-bottom: 1px solid var(--trust-border);
      margin-bottom: 14px;
      padding-bottom: 12px;
    }
    .crumb {
      color: var(--trust-text-muted);
      font-size: 11px;
      font-weight: 650;
      margin-bottom: 5px;
    }
    h1 {
      color: var(--trust-text);
      font-size: 18px;
      line-height: 1.2;
      margin: 0;
    }
    .lead {
      color: var(--trust-text-muted);
      font-size: 12px;
      line-height: 1.45;
      margin: 6px 0 0;
      max-width: 660px;
    }
    .toolbar {
      align-items: center;
      display: flex;
      flex-wrap: wrap;
      gap: 8px;
      margin: 0 0 14px;
    }
    .search {
      min-width: 220px;
      flex: 1 1 260px;
    }
    input {
      background: var(--trust-input-bg);
      border: 1px solid var(--trust-input-border);
      border-radius: var(--trust-radius-sm);
      color: var(--trust-text);
      font: inherit;
      padding: 7px 9px;
      width: 100%;
    }
    .chips {
      display: flex;
      flex-wrap: wrap;
      gap: 6px;
    }
    .filter-group {
      align-items: center;
      display: flex;
      flex-wrap: wrap;
      gap: 6px;
    }
    .filter-label {
      color: var(--trust-text-muted);
      font-size: 11px;
      font-weight: 650;
      margin-right: 2px;
      text-transform: uppercase;
    }
    .chip {
      border: 1px solid var(--trust-border);
      border-radius: var(--trust-pill);
      background: transparent;
      color: var(--trust-text);
      cursor: pointer;
      font: inherit;
      font-size: 12px;
      padding: 5px 9px;
    }
    .chip[aria-pressed="true"] {
      background: var(--trust-selected-bg);
      border-color: var(--trust-accent);
    }
    .grid {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(230px, min(100%, 340px)));
      justify-content: start;
      gap: 10px;
    }
    .card {
      background: var(--trust-surface);
      border: 1px solid var(--trust-border);
      border-radius: var(--trust-radius);
      display: flex;
      flex-direction: column;
      gap: 9px;
      min-height: 178px;
      padding: 12px;
    }
    .card h2 {
      color: var(--trust-text);
      font-size: 14px;
      line-height: 1.25;
      margin: 0;
    }
    .card p {
      color: var(--trust-text-muted);
      font-size: 12px;
      line-height: 1.45;
      margin: 0;
    }
    .badges {
      display: flex;
      flex-wrap: wrap;
      gap: 5px;
    }
    .badge {
      border: 1px solid var(--trust-border);
      border-radius: var(--trust-pill);
      color: var(--trust-text-muted);
      font-size: 10px;
      font-weight: 700;
      padding: 2px 7px;
    }
    .badge.hardware {
      text-transform: uppercase;
    }
    .badge.ok {
      border-color: color-mix(in srgb, var(--trust-ok) 55%, var(--trust-border));
      color: var(--trust-ok);
    }
    .badge.requires {
      border-color: color-mix(in srgb, var(--trust-warn) 62%, var(--trust-border));
      color: var(--trust-warn);
    }
    .actions {
      margin-top: auto;
    }
    .empty {
      border: 1px solid var(--trust-border);
      border-radius: var(--trust-radius);
      color: var(--trust-text-muted);
      padding: 18px;
      text-align: center;
    }
    .empty p {
      margin: 0 0 10px;
    }
    .header-row,
    .section-heading,
    .progress-row,
    .lesson-navigation {
      align-items: center;
      display: flex;
      gap: 10px;
      justify-content: space-between;
    }
    .back-button {
      margin-bottom: 10px;
    }
    .section {
      margin-bottom: 22px;
    }
    .section-heading {
      margin-bottom: 9px;
    }
    .section-heading h2 {
      color: var(--trust-text);
      font-size: 14px;
      margin: 0;
    }
    .section-heading p,
    .meta {
      color: var(--trust-text-muted);
      font-size: 11px;
      margin: 0;
    }
    .path-card {
      border-left: 3px solid var(--trust-accent);
      min-height: 0;
    }
    .course-grid {
      display: grid;
      gap: 10px;
      grid-template-columns: repeat(auto-fit, minmax(250px, 1fr));
    }
    .course-card {
      min-height: 190px;
    }
    .progress-row {
      margin: 0 0 14px;
    }
    progress {
      accent-color: var(--trust-accent);
      height: 8px;
      max-width: 300px;
      width: 100%;
    }
    .lesson-list {
      display: grid;
      gap: 8px;
    }
    .lesson-card {
      min-height: 0;
    }
    .lesson-card[aria-current="step"] {
      border-color: var(--trust-accent);
    }
    .lesson-detail {
      max-width: 720px;
    }
    .lesson-detail dl {
      display: grid;
      gap: 8px 14px;
      grid-template-columns: max-content 1fr;
      margin: 6px 0 18px;
    }
    .lesson-detail dt {
      color: var(--trust-text-muted);
      font-weight: 650;
    }
    .lesson-detail dd {
      color: var(--trust-text);
      margin: 0;
    }
    .lesson-flow {
      max-width: 720px;
    }
    .lesson-flow ol {
      display: grid;
      gap: 12px;
      margin: 0;
      padding-left: 24px;
    }
    .lesson-flow li {
      padding-left: 4px;
    }
    .lesson-flow h3 {
      color: var(--trust-text);
      font-size: 13px;
      margin: 0 0 4px;
    }
    .lesson-flow p {
      color: var(--trust-text-muted);
      font-size: 12px;
      line-height: 1.5;
      margin: 3px 0;
    }
    .actions {
      align-items: center;
      display: flex;
      flex-wrap: wrap;
      gap: 8px;
    }
    @media (max-width: 520px) {
      .shell { padding: 12px; }
      .grid, .course-grid { grid-template-columns: 1fr; }
      .progress-row, .lesson-navigation { align-items: stretch; flex-direction: column; }
      progress { max-width: none; }
      .lesson-detail dl { grid-template-columns: 1fr; gap: 3px; }
      .lesson-detail dd { margin-bottom: 8px; }
    }
  </style>
</head>
<body>
  <main class="shell">
    <header>
      <button class="trust-button trust-button--secondary back-button" id="back" type="button" hidden>Back</button>
      <div class="crumb" id="crumb">truST</div>
      <h1 id="title">Start from example</h1>
      <p class="lead" id="lead">Choose a runnable starter or follow an ordered learning path.</p>
    </header>
    <section id="content"></section>
    <section class="toolbar" id="toolbar" aria-label="Example filters" hidden>
      <label class="search" id="searchWrap" aria-label="Search examples">
        <input id="search" placeholder="Search examples" />
      </label>
      <div class="chips" id="chips"></div>
    </section>
    <section class="grid" id="grid" aria-label="Examples" hidden></section>
  </main>
  <script nonce="${nonce}">
    const vscode = acquireVsCodeApi();
    let examples = [];
    let courses = [];
    let topics = [];
    let completedLessonIds = [];
    let view = { kind: "landing" };
    let hardwareFilter = "all";
    let tagFilter = "all";
    let search = "";

    const back = document.getElementById("back");
    const crumb = document.getElementById("crumb");
    const title = document.getElementById("title");
    const lead = document.getElementById("lead");
    const content = document.getElementById("content");
    const toolbar = document.getElementById("toolbar");
    const grid = document.getElementById("grid");
    const chips = document.getElementById("chips");
    const searchInput = document.getElementById("search");
    const searchWrap = document.getElementById("searchWrap");

    function post(type, id) {
      vscode.postMessage(id ? { type, id } : { type });
    }

    function actionButton(label, primary, type, id) {
      const button = document.createElement("button");
      button.className = "trust-button " + (primary ? "trust-button--primary" : "trust-button--secondary");
      button.type = "button";
      button.textContent = label;
      button.dataset.actionType = type;
      if (id) { button.dataset.actionId = id; }
      button.addEventListener("click", () => post(type, id));
      return button;
    }

    function lessonIsAvailable(example) {
      return example.lessonAvailability && example.lessonAvailability.kind === "available";
    }

    // Honest per-lesson progress (W-05): a project folder is real, retained evidence that it
    // exists, but it is not evidence of a finished Monitor -> Test run, so it is never "Completed".
    function progressLabel(example) {
      return example.lesson && example.lesson.progressState === "project_created"
        ? "Project created"
        : "Not started";
    }

    function moduleFor(example) {
      return example.lesson ? courses.find((course) => course.id === example.lesson.module) : undefined;
    }

    function appendLessonAvailability(parent, example) {
      if (!example.lesson) { return; }
      const status = document.createElement("p");
      status.className = "meta";
      if (lessonIsAvailable(example)) {
        status.textContent = "Ready to create";
      } else {
        const reason = example.lessonAvailability && example.lessonAvailability.reason;
        status.textContent = "Not yet available" + (reason ? ": " + reason : ".");
      }
      parent.appendChild(status);
    }

    function sectionHeading(name, description) {
      const wrapper = document.createElement("div");
      wrapper.className = "section-heading";
      const heading = document.createElement("h2");
      heading.textContent = name;
      wrapper.appendChild(heading);
      if (description) {
        const detail = document.createElement("p");
        detail.textContent = description;
        wrapper.appendChild(detail);
      }
      return wrapper;
    }

    function resetSurface() {
      content.innerHTML = "";
      grid.innerHTML = "";
      content.hidden = false;
      toolbar.hidden = true;
      grid.hidden = true;
      back.hidden = view.kind === "landing";
    }

    function setHeader(crumbText, titleText, leadText) {
      crumb.textContent = crumbText;
      title.textContent = titleText;
      lead.textContent = leadText;
    }

    function tagsFor(example) {
      return [example.hardware === "none" ? "no-hardware" : "hardware"].concat(example.tags || []);
    }

    function allTags() {
      const filters = new Set(["all"]);
      for (const example of examples) {
        for (const tag of example.tags || []) {
          filters.add(tag);
        }
      }
      return Array.from(filters);
    }

    function hardwareFilterLabel(value) {
      if (value === "all") { return "All"; }
      if (value === "no-hardware") { return "No hardware"; }
      if (value === "hardware") { return "Needs hardware"; }
      return value;
    }

    const TAG_LABELS = {
      ads: "ADS",
      ethercat: "EtherCAT",
      gpio: "GPIO",
      hmi: "HMI",
      plcopen: "PLCopen",
      raspberrypi: "Raspberry Pi",
      twincat: "TwinCAT",
    };

    function titleCaseTag(value) {
      return value
        .split("-")
        .filter(Boolean)
        .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
        .join(" ");
    }

    function tagFilterLabel(value) {
      if (value === "all") { return "All categories"; }
      return TAG_LABELS[value] || titleCaseTag(value);
    }

    function renderFilters() {
      searchWrap.hidden = false;
      chips.innerHTML = "";
      const hardwareGroup = document.createElement("div");
      hardwareGroup.className = "filter-group";
      hardwareGroup.setAttribute("aria-label", "Hardware filter");
      const hardwareLabel = document.createElement("span");
      hardwareLabel.className = "filter-label";
      hardwareLabel.textContent = "Hardware";
      hardwareGroup.appendChild(hardwareLabel);
      for (const value of ["all", "no-hardware", "hardware"]) {
        const button = document.createElement("button");
        button.className = "chip";
        button.type = "button";
        button.textContent = hardwareFilterLabel(value);
        button.dataset.filterKind = "hardware";
        button.dataset.filterValue = value;
        button.setAttribute("aria-pressed", value === hardwareFilter ? "true" : "false");
        button.addEventListener("click", () => {
          hardwareFilter = value;
          render();
        });
        hardwareGroup.appendChild(button);
      }
      chips.appendChild(hardwareGroup);

      const categoryGroup = document.createElement("div");
      categoryGroup.className = "filter-group";
      categoryGroup.setAttribute("aria-label", "Category filter");
      const categoryLabel = document.createElement("span");
      categoryLabel.className = "filter-label";
      categoryLabel.textContent = "Category";
      categoryGroup.appendChild(categoryLabel);
      for (const value of allTags()) {
        const button = document.createElement("button");
        button.className = "chip";
        button.type = "button";
        button.textContent = tagFilterLabel(value);
        button.dataset.filterKind = "category";
        button.dataset.filterValue = value;
        button.setAttribute("aria-pressed", value === tagFilter ? "true" : "false");
        button.addEventListener("click", () => {
          tagFilter = value;
          render();
        });
        categoryGroup.appendChild(button);
      }
      chips.appendChild(categoryGroup);
    }

    function matches(example) {
      const text = (example.title + " " + example.description + " " + (example.tags || []).join(" ")).toLowerCase();
      const query = search.trim().toLowerCase();
      const tags = tagsFor(example);
      const hardwareOk = hardwareFilter === "all" || tags.includes(hardwareFilter);
      const tagOk = tagFilter === "all" || tags.includes(tagFilter);
      return hardwareOk && tagOk && (!query || text.includes(query));
    }

    function createExampleCard(example, compact) {
      const card = document.createElement("article");
      card.className = "card" + (compact ? " lesson-card" : "");
      card.dataset.exampleId = example.id;

      const heading = document.createElement("h2");
      heading.textContent = example.title;
      card.appendChild(heading);

      const badges = document.createElement("div");
      badges.className = "badges";
      const hw = document.createElement("span");
      hw.className = "badge hardware " + (example.hardware === "none" ? "ok" : "requires");
      hw.textContent = example.hardwareLabel;
      badges.appendChild(hw);
      for (const tag of example.tags || []) {
        const badge = document.createElement("span");
        badge.className = "badge";
        badge.textContent = tagFilterLabel(tag);
        badges.appendChild(badge);
      }
      card.appendChild(badges);

      const description = document.createElement("p");
      description.textContent = example.description;
      card.appendChild(description);

      const actions = document.createElement("div");
      actions.className = "actions";
      if (!example.lesson || lessonIsAvailable(example)) {
        actions.appendChild(actionButton(example.lesson ? "Create project" : "Use this example", true, "useExample", example.id));
      }
      card.appendChild(actions);
      return card;
    }

    function renderLanding() {
      resetSurface();
      setHeader("truST", "Start from example", "Choose a runnable starter or follow an ordered learning path.");

      const learning = document.createElement("section");
      learning.className = "section";
      learning.appendChild(sectionHeading("Learning paths", "Structured courses keep related examples in a useful order."));
      const paths = document.createElement("div");
      paths.className = "course-grid";

      const control = document.createElement("article");
      control.className = "card path-card";
      const controlTitle = document.createElement("h2");
      controlTitle.textContent = "Control engineering";
      control.appendChild(controlTitle);
      const controlMeta = document.createElement("div");
      controlMeta.className = "badges";
      for (const label of ["84 practical lessons", courses.length + " modules", "No hardware"]) {
        const badge = document.createElement("span");
        badge.className = "badge";
        badge.textContent = label;
        controlMeta.appendChild(badge);
      }
      control.appendChild(controlMeta);
      const controlDescription = document.createElement("p");
      controlDescription.textContent = "Progress lesson by lesson through all " + courses.length + " modules, from feedback fundamentals to modern control.";
      control.appendChild(controlDescription);
      const controlActions = document.createElement("div");
      controlActions.className = "actions";
      controlActions.appendChild(actionButton("Open learning path", true, "openControlCurriculum"));
      control.appendChild(controlActions);
      paths.appendChild(control);

      learning.appendChild(paths);
      content.appendChild(learning);

      const topicSection = document.createElement("section");
      topicSection.className = "section";
      topicSection.appendChild(sectionHeading("Explore by topic", "Every maintained example and tutorial is indexed in these focused guides."));
      const topicGrid = document.createElement("div");
      topicGrid.className = "course-grid";
      for (const topic of topics) {
        const card = document.createElement("article");
        card.className = "card";
        const heading = document.createElement("h2");
        heading.textContent = topic.title;
        card.appendChild(heading);
        const description = document.createElement("p");
        description.textContent = topic.description;
        card.appendChild(description);
        const actions = document.createElement("div");
        actions.className = "actions";
        actions.appendChild(actionButton("Open " + topic.title, false, "openExamplesDocumentation", topic.id));
        card.appendChild(actions);
        topicGrid.appendChild(card);
      }
      topicSection.appendChild(topicGrid);
      const allActions = document.createElement("div");
      allActions.className = "actions";
      allActions.style.marginTop = "12px";
      allActions.appendChild(actionButton("All examples and tutorials", false, "openExamplesDocumentation", "all"));
      topicSection.appendChild(allActions);
      content.appendChild(topicSection);

      const starters = examples.filter((example) => !(example.tags || []).includes("control"));
      const starterSection = document.createElement("section");
      starterSection.className = "section";
      starterSection.appendChild(sectionHeading("Starter projects", "Small projects that can be copied and run immediately."));
      const starterGrid = document.createElement("div");
      starterGrid.className = "grid";
      for (const example of starters) {
        starterGrid.appendChild(createExampleCard(example, false));
      }
      starterSection.appendChild(starterGrid);
      const browseActions = document.createElement("div");
      browseActions.className = "actions";
      browseActions.style.marginTop = "12px";
      browseActions.appendChild(actionButton("Browse all bundled projects", false, "browseExamples"));
      starterSection.appendChild(browseActions);
      content.appendChild(starterSection);
    }

    function controlLessons() {
      return examples.filter((example) => example.lesson).sort((left, right) => left.lesson.sequence - right.lesson.sequence);
    }

    function renderProgress(parent, lessons) {
      const created = lessons.filter((lesson) => completedLessonIds.includes(lesson.id)).length;
      const row = document.createElement("div");
      row.className = "progress-row";
      const label = document.createElement("span");
      label.className = "meta";
      label.textContent = created + " of " + lessons.length + " projects created";
      const progress = document.createElement("progress");
      progress.max = lessons.length;
      progress.value = created;
      progress.setAttribute("aria-label", "Course progress");
      row.appendChild(label);
      row.appendChild(progress);
      parent.appendChild(row);
      return created;
    }

    function renderControlCurriculum() {
      resetSurface();
      setHeader("Examples / Learning paths", "Control engineering", "One practical progression: observe a limitation, add one technique, and validate the result.");
      const lessons = controlLessons();
      const completed = renderProgress(content, lessons);
      const actions = document.createElement("div");
      actions.className = "actions section";
      const next = lessons.find((lesson) => !completedLessonIds.includes(lesson.id)) || lessons[0];
      if (next) {
        actions.appendChild(actionButton(completed === lessons.length ? "Review course" : "Continue course", true, "openControlLesson", next.id));
      }
      content.appendChild(actions);

      const courseGrid = document.createElement("div");
      courseGrid.className = "course-grid";
      for (const course of courses) {
        const courseLessons = lessons.filter((lesson) => lesson.lesson.module === course.id);
        const done = courseLessons.filter((lesson) => completedLessonIds.includes(lesson.id)).length;
        const card = document.createElement("article");
        card.className = "card course-card";
        const heading = document.createElement("h2");
        heading.textContent = course.title;
        card.appendChild(heading);
        const meta = document.createElement("p");
        meta.className = "meta";
        meta.textContent = course.range + " · " + courseLessons.length + " lessons";
        card.appendChild(meta);
        const status = document.createElement("p");
        status.textContent = done + " of " + courseLessons.length + " projects created";
        card.appendChild(status);
        const cardActions = document.createElement("div");
        cardActions.className = "actions";
        cardActions.appendChild(actionButton("Open module", true, "openControlCourse", course.id));
        card.appendChild(cardActions);
        courseGrid.appendChild(card);
      }
      content.appendChild(courseGrid);
    }

    function prerequisiteLabel(example) {
      const prerequisiteId = example.lesson.prerequisites[example.lesson.prerequisites.length - 1];
      if (!prerequisiteId) { return "No prerequisite"; }
      const prerequisite = examples.find((candidate) => candidate.id === prerequisiteId);
      return prerequisite ? prerequisite.lesson.positionLabel + " · " + prerequisite.title : prerequisiteId;
    }

    function renderControlCourse(courseId) {
      resetSurface();
      const course = courses.find((candidate) => candidate.id === courseId);
      const lessons = controlLessons().filter((lesson) => lesson.lesson.module === courseId);
      if (!course || !lessons.length) {
        renderControlCurriculum();
        return;
      }
      setHeader("Control engineering / " + course.range, course.title,
        course.lessonCount + (course.lessonCount === 1 ? " lesson in this module." : " lessons in this module."));
      renderProgress(content, lessons);
      const list = document.createElement("div");
      list.className = "lesson-list";
      for (const example of lessons) {
        const card = document.createElement("article");
        card.className = "card lesson-card";
        card.dataset.exampleId = example.id;
        const heading = document.createElement("h2");
        heading.textContent = example.lesson.positionLabel + " · " + example.title.replace(/^Control · /, "");
        card.appendChild(heading);
        const meta = document.createElement("p");
        meta.className = "meta";
        meta.textContent = example.lesson.equipment;
        card.appendChild(meta);
        const technique = document.createElement("p");
        technique.textContent = example.lesson.principalTechnique;
        card.appendChild(technique);
        const prerequisite = document.createElement("p");
        prerequisite.textContent = "Prerequisite: " + prerequisiteLabel(example);
        card.appendChild(prerequisite);
        appendLessonAvailability(card, example);
        const cardActions = document.createElement("div");
        cardActions.className = "actions";
        cardActions.appendChild(actionButton("View lesson", false, "openControlLesson", example.id));
        if (lessonIsAvailable(example)) {
          cardActions.appendChild(actionButton("Create project", true, "useExample", example.id));
        }
        if (example.lesson.progressState === "project_created") {
          const done = document.createElement("span");
          done.className = "badge ok";
          done.textContent = "Project created";
          cardActions.appendChild(done);
        }
        card.appendChild(cardActions);
        list.appendChild(card);
      }
      content.appendChild(list);
    }

    function renderControlLesson(lessonId) {
      resetSurface();
      const lessons = controlLessons();
      const index = lessons.findIndex((candidate) => candidate.id === lessonId);
      if (index < 0) {
        renderControlCurriculum();
        return;
      }
      const example = lessons[index];
      const current = document.createElement("article");
      current.className = "card lesson-detail";
      current.dataset.exampleId = example.id;
      current.setAttribute("aria-current", "step");
      const module = moduleFor(example);
      setHeader("Control engineering / " + (module ? module.title : "Control engineering"),
        example.lesson.positionLabel, example.title.replace(/^Control · /, ""));
      const description = document.createElement("p");
      description.textContent = example.description;
      current.appendChild(description);
      const details = document.createElement("dl");
      for (const pair of [
        ["Module", module ? module.title : "—"],
        ["Equipment", example.lesson.equipment],
        ["Technique", example.lesson.principalTechnique],
        ["Prerequisite", prerequisiteLabel(example)],
        ["Progress", progressLabel(example)],
      ]) {
        const term = document.createElement("dt");
        term.textContent = pair[0];
        const value = document.createElement("dd");
        value.textContent = pair[1];
        details.appendChild(term);
        details.appendChild(value);
      }
      current.appendChild(details);
      appendLessonAvailability(current, example);
      const useActions = document.createElement("div");
      useActions.className = "actions";
      if (lessonIsAvailable(example)) {
        useActions.appendChild(actionButton("Create project", true, "useExample", example.id));
      }
      current.appendChild(useActions);
      content.appendChild(current);

      if (example.lesson.presentation && example.lesson.presentation.length) {
        const flow = document.createElement("section");
        flow.className = "card lesson-flow section";
        const heading = document.createElement("h2");
        heading.textContent = "Lesson outline";
        flow.appendChild(heading);
        const stages = document.createElement("ol");
        for (const stage of example.lesson.presentation) {
          const item = document.createElement("li");
          const title = document.createElement("h3");
          title.textContent = stage.title;
          item.appendChild(title);
          for (const paragraphText of stage.paragraphs) {
            const paragraph = document.createElement("p");
            paragraph.textContent = paragraphText;
            item.appendChild(paragraph);
          }
          stages.appendChild(item);
        }
        flow.appendChild(stages);
        content.appendChild(flow);
      }

      const navigation = document.createElement("nav");
      navigation.className = "lesson-navigation section";
      navigation.setAttribute("aria-label", "Lesson navigation");
      if (index > 0) {
        navigation.appendChild(actionButton("Previous lesson", false, "openControlLesson", lessons[index - 1].id));
      } else {
        navigation.appendChild(document.createElement("span"));
      }
      if (index + 1 < lessons.length) {
        navigation.appendChild(actionButton("Next lesson", false, "openControlLesson", lessons[index + 1].id));
      }
      content.appendChild(navigation);
    }

    function renderBrowse() {
      resetSurface();
      setHeader("Examples", "Browse all bundled projects", "Search all copyable projects by hardware requirement or category.");
      content.hidden = true;
      toolbar.hidden = false;
      grid.hidden = false;
      renderFilters();
      grid.innerHTML = "";
      const visible = examples.filter(matches);
      if (!visible.length) {
        const empty = document.createElement("div");
        empty.className = "empty";
        const text = document.createElement("p");
        text.textContent = "No examples match this search and filter.";
        empty.appendChild(text);
        const reset = document.createElement("button");
        reset.className = "trust-button trust-button--secondary";
        reset.type = "button";
        reset.textContent = "Clear search and filters";
        reset.addEventListener("click", () => {
          search = "";
          hardwareFilter = "all";
          tagFilter = "all";
          searchInput.value = "";
          render();
        });
        empty.appendChild(reset);
        grid.appendChild(empty);
        return;
      }
      for (const example of visible) {
        grid.appendChild(createExampleCard(example, false));
      }
    }

    function render() {
      if (view.kind === "control") { renderControlCurriculum(); return; }
      if (view.kind === "course") { renderControlCourse(view.id); return; }
      if (view.kind === "lesson") { renderControlLesson(view.id); return; }
      if (view.kind === "browse") { renderBrowse(); return; }
      renderLanding();
    }

    searchInput.addEventListener("input", () => {
      search = searchInput.value || "";
      render();
    });
    back.addEventListener("click", () => post("navigateBack"));

    window.addEventListener("message", (event) => {
      const msg = event.data;
      if (!msg || msg.type !== "state") { return; }
      examples = Array.isArray(msg.examples) ? msg.examples : [];
      courses = Array.isArray(msg.courses) ? msg.courses : [];
      topics = Array.isArray(msg.topics) ? msg.topics : [];
      completedLessonIds = Array.isArray(msg.completedLessonIds) ? msg.completedLessonIds : [];
      view = msg.view && typeof msg.view.kind === "string" ? msg.view : { kind: "landing" };
      render();
    });

    vscode.postMessage({ type: "ready" });
  </script>
</body>
</html>`;
  }
}

function nonceValue(): string {
  const chars =
    "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
  let nonce = "";
  for (let i = 0; i < 32; i += 1) {
    nonce += chars.charAt(Math.floor(Math.random() * chars.length));
  }
  return nonce;
}
