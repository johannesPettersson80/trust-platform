import * as vscode from "vscode";
import { appendLessonMarkup, LESSON_MARKUP_STYLES } from "./lessonMarkupView";

export function renderExamplesGalleryHtml(panel: vscode.WebviewPanel, context: vscode.ExtensionContext): string {
    const nonce = nonceValue();
    const themeUri = panel.webview.asWebviewUri(
      vscode.Uri.joinPath(context.extensionUri, "src", "webview", "theme.css")
    );
    const csp = `default-src 'none'; style-src ${panel.webview.cspSource} 'unsafe-inline'; script-src 'nonce-${nonce}';`;
    return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <meta http-equiv="Content-Security-Policy" content="${csp}" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>Start from example</title>
  <link rel="stylesheet" href="${themeUri}" />
  <style>
    ${LESSON_MARKUP_STYLES}
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
    input, select {
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
      flex: 1 1 220px;
      min-width: 0;
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
    .lesson-technical {
      border-top: 1px solid var(--trust-border);
      margin-top: 14px;
      padding-top: 10px;
    }
    .lesson-technical summary {
      color: var(--trust-text);
      cursor: pointer;
      font-size: 12px;
      font-weight: 650;
    }
    .lesson-technical[open] summary {
      margin-bottom: 8px;
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
      <p class="meta" id="resultCount" role="status" aria-live="polite"></p>
    </section>
    <section class="grid" id="grid" aria-label="Examples" hidden></section>
  </main>
  <script nonce="${nonce}">
    const appendLessonMarkup = ${appendLessonMarkup.toString()};
    const vscode = acquireVsCodeApi();
    let examples = [];
    let courses = [];
    let topics = [];
    let completedLessonIds = [];
    let view = { kind: "landing" };
    let hardwareFilter = "all";
    let tagFilter = "all";
    let topicFilter = "all";
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
      if (!example.lesson && !example.lessonAvailability) { return; }
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
      for (const kind of ["hardware", "category"]) {
        const group = document.createElement("label");
        group.className = "filter-group";
        const label = document.createElement("span");
        label.className = "filter-label";
        label.textContent = kind === "hardware" ? "Hardware" : "Category";
        group.appendChild(label);
        const select = document.createElement("select");
        select.dataset.filterKind = kind;
        const values = kind === "hardware" ? ["all", "no-hardware", "hardware"] : allTags();
        for (const value of values) {
          const option = document.createElement("option");
          option.value = value;
          option.textContent = kind === "hardware" ? hardwareFilterLabel(value) : tagFilterLabel(value);
          select.appendChild(option);
        }
        select.value = kind === "hardware" ? hardwareFilter : tagFilter;
        select.addEventListener("change", () => {
          if (kind === "hardware") { hardwareFilter = select.value; }
          else { tagFilter = select.value; }
          render();
          chips.querySelector('select[data-filter-kind="' + kind + '"]').focus();
        });
        group.appendChild(select);
        chips.appendChild(group);
      }
    }

    function matches(example) {
      const text = (example.title + " " + example.description + " " + (example.tags || []).join(" ")).toLowerCase();
      const query = search.trim().toLowerCase();
      const tags = tagsFor(example);
      const hardwareOk = hardwareFilter === "all" || tags.includes(hardwareFilter);
      const tagOk = tagFilter === "all" || tags.includes(tagFilter);
      const topicOk = topicFilter === "all" || topicMatches(example, topicFilter);
      return hardwareOk && tagOk && topicOk && (!query || text.includes(query));
    }

    function topicMatches(example, topicId) {
      const tags = example.tags || [];
      if (topicId === "tutorials") { return tags.includes("control") || tags.includes("starter"); }
      if (topicId === "test-debug") { return tags.includes("diagnosis") || tags.includes("simulator"); }
      if (topicId === "hmi") { return tags.includes("hmi"); }
      if (topicId === "connectivity") { return tags.includes("fieldbus") || tags.includes("ads") || tags.includes("ethercat") || tags.includes("gpio"); }
      if (topicId === "runtime-cloud") { return tags.includes("networked") || tags.includes("raspberrypi"); }
      if (topicId === "visual-editors") { return tags.includes("visual") || tags.includes("statechart"); }
      if (topicId === "vendor") { return tags.includes("vendor") || tags.includes("beckhoff"); }
      if (topicId === "motion") { return tags.includes("motion") || tags.includes("library"); }
      if (topicId === "capstones") { return tags.includes("capstone"); }
      return false;
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
      const simulatorStarter = ["empty-simulator", "conveyor", "twincat-ads", "raspberry-pi", "hmi-starter", "plcopen-motion-single-axis"].includes(example.id);
      if (simulatorStarter) {
        const execution = document.createElement("span");
        execution.className = "badge ok";
        execution.textContent = "Runs in simulator";
        badges.appendChild(execution);
      }
      const hw = document.createElement("span");
      hw.className = "badge hardware " + (simulatorStarter || example.hardware === "none" ? "ok" : "requires");
      hw.textContent = simulatorStarter
        ? (example.id === "twincat-ads" ? "Hardware optional: TwinCAT"
          : example.id === "raspberry-pi" ? "Hardware optional: Raspberry Pi" : "No hardware needed")
        : example.hardwareLabel;
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
      appendLessonAvailability(card, example);

      const actions = document.createElement("div");
      actions.className = "actions";
      if (!example.lessonAvailability || lessonIsAvailable(example)) {
        actions.appendChild(actionButton("Create project", true, "useExample", example.id));
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
      const lessonTotal = courses.reduce((total, course) => total + course.lessonCount, 0);
      for (const label of [lessonTotal + " practical lessons", courses.length + " modules", "No hardware"]) {
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
        actions.appendChild(actionButton("Browse examples", true, "selectTopic", topic.id));
        actions.appendChild(actionButton("Read guide ↗", false, "openExamplesDocumentation", "read-guide:" + topic.id));
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
        if (example.lesson.retainedProjectPaths?.length) {
          cardActions.appendChild(actionButton("Open existing project", false, "openRetainedProject", example.id));
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
      const lessonTitle = example.title.replace(/^Control · /, "").replace(/^\\d+\\.\\s*/, "");
      setHeader("Control engineering / " + (module ? module.title : "Control engineering"),
        lessonTitle,
        [example.lesson.positionLabel, example.lesson.equipment].join(" · "));
      const descriptionText = example.lesson.detail ? example.lesson.detail.description : example.description;
      const filePreview = target => vscode.postMessage({type:"previewLessonFile", id:example.id, target});
      appendLessonMarkup(current, example.lesson.detail?.markup?.description, descriptionText, filePreview);
      if (!lessonIsAvailable(example)) appendLessonAvailability(current, example);
      const useActions = document.createElement("div");
      useActions.className = "actions";
      if (lessonIsAvailable(example)) {
        useActions.appendChild(actionButton("Create project", true, "useExample", example.id));
      }
      if (example.lesson.retainedProjectPaths?.length) {
        useActions.appendChild(actionButton("Open existing project", false, "openRetainedProject", example.id));
      }
      current.appendChild(useActions);
      const previewError = document.createElement("p");
      previewError.className = "lesson-preview-error";
      previewError.dataset.lessonPreviewError = "";
      previewError.setAttribute("role", "alert");
      previewError.hidden = true;
      current.appendChild(previewError);

      const technicalDisclosure = document.createElement("details");
      technicalDisclosure.className = "lesson-technical";
      const technicalSummary = document.createElement("summary");
      technicalSummary.textContent = "Lesson details and expected observation";
      technicalDisclosure.appendChild(technicalSummary);
      const details = document.createElement("dl");
      const objective = example.lesson.detail ? example.lesson.detail.objective : example.lesson.principalTechnique;
      const metadata = [
        ["Module", module ? module.title : "—"],
        ["Equipment", example.lesson.equipment],
        ["Technique", example.lesson.principalTechnique],
        ["Prerequisite", prerequisiteLabel(example)],
        ["Expected observation", example.lesson.detail ? example.lesson.detail.expectedObservation : "Not specified in this legacy lesson."],
        ["Progress", progressLabel(example)],
      ];
      if (objective !== (example.lesson.detail ? example.lesson.detail.description : example.description)) {
        metadata.splice(1, 0, ["Objective", objective]);
      }
      for (const pair of metadata) {
        const term = document.createElement("dt");
        term.textContent = pair[0];
        const value = document.createElement("dd");
        const markup = pair[0] === "Objective" ? example.lesson.detail?.markup?.objective
          : pair[0] === "Expected observation" ? example.lesson.detail?.markup?.expectedObservation : undefined;
        if (markup) appendLessonMarkup(value, markup, pair[1], filePreview);
        else value.textContent = pair[1];
        details.appendChild(term);
        details.appendChild(value);
      }
      technicalDisclosure.appendChild(details);
      current.appendChild(technicalDisclosure);
      content.appendChild(current);

      const preview = example.lesson.detail ? example.lesson.detail.preview.filter(stage => stage.title !== "Objective" && stage.title !== "Expected observation") : example.lesson.presentation;
      if (preview && preview.length) {
        const flow = document.createElement("section");
        flow.className = "card lesson-flow section";
        const heading = document.createElement("h2");
        heading.textContent = "Inside this lesson";
        flow.appendChild(heading);
        const stages = document.createElement("ol");
        for (const stage of preview) {
          const item = document.createElement("li");
          const title = document.createElement("h3");
          title.textContent = stage.title;
          item.appendChild(title);
          for (const [paragraphIndex, paragraphText] of stage.paragraphs.entries()) {
            if (paragraphText === descriptionText) continue;
            appendLessonMarkup(item, stage.markup?.[paragraphIndex], paragraphText, filePreview);
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
      document.getElementById("resultCount").textContent = visible.length + " of " + examples.length + " projects";
      if (!visible.length) {
        const empty = document.createElement("div");
        empty.className = "empty";
      const text = document.createElement("p");
        text.textContent = topicFilter === "telemetry" || topicFilter === "runbooks"
          ? "This topic is documentation only; no bundled project is available to copy."
          : "No examples match this search and filter.";
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
          searchInput.focus();
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
      if (!msg) { return; }
      if (msg.type === "filePreviewError" && view.kind === "lesson" && view.id === msg.id) {
        const error = document.querySelector("[data-lesson-preview-error]");
        if (error) { error.textContent = msg.message; error.hidden = false; }
        return;
      }
      if (msg.type === "topicSelection") {
        topicFilter = msg.topicId || "all";
        hardwareFilter = "all";
        tagFilter = "all";
        search = "";
        searchInput.value = "";
        return;
      }
      if (msg.type !== "state") { return; }
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

function nonceValue(): string {
  const chars =
    "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
  let nonce = "";
  for (let i = 0; i < 32; i += 1) {
    nonce += chars.charAt(Math.floor(Math.random() * chars.length));
  }
  return nonce;
}
