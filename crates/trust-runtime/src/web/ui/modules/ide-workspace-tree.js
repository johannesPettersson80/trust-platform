function flattenFiles(nodes, out = []) {
  for (const node of nodes) {
    if (node.kind === "file") {
      out.push(node.path);
    } else if (Array.isArray(node.children)) {
      flattenFiles(node.children, out);
    }
  }
  return out;
}

function nodeKindForPath(path, nodes = state.tree) {
  for (const node of nodes || []) {
    if (node.path === path) {
      return node.kind || null;
    }
    if (node.kind === "directory" && Array.isArray(node.children)) {
      const nested = nodeKindForPath(path, node.children);
      if (nested) {
        return nested;
      }
    }
  }
  return null;
}

function nodeMatchesFilter(node, filter) {
  if (!filter) {
    return true;
  }
  const name = String(node.name || "").toLowerCase();
  const path = String(node.path || "").toLowerCase();
  if (name.includes(filter) || path.includes(filter)) {
    return true;
  }
  return Array.isArray(node.children) && node.children.some((child) => nodeMatchesFilter(child, filter));
}

function selectPath(path) {
  state.selectedPath = path || null;
  renderFileTree();
}

function toggleDir(path) {
  if (state.expandedDirs.has(path)) {
    state.expandedDirs.delete(path);
  } else {
    state.expandedDirs.add(path);
  }
  renderFileTree();
}

function closeTreeContextMenu() {
  el.treeContextMenu.classList.add("ide-hidden");
  state.contextPath = null;
}

function openTreeContextMenu(path, x, y) {
  state.contextPath = path;
  selectPath(path);
  const writable = Boolean(state.writeEnabled);
  el.ctxNewFileBtn.disabled = !writable;
  el.ctxNewFolderBtn.disabled = !writable;
  el.ctxRenameBtn.disabled = !writable;
  el.ctxDeleteBtn.disabled = !writable;
  el.treeContextMenu.style.left = `${Math.max(8, Math.floor(x))}px`;
  el.treeContextMenu.style.top = `${Math.max(8, Math.floor(y))}px`;
  el.treeContextMenu.classList.remove("ide-hidden");
}

function renderTreeNode(node, depth) {
  if (!nodeMatchesFilter(node, state.fileFilter)) {
    return;
  }
  const row = document.createElement("button");
  row.type = "button";
  row.className = "ide-tree-row";
  row.setAttribute("role", "treeitem");
  row.style.paddingLeft = `${8 + depth * 14}px`;
  const isSelected = state.selectedPath === node.path || state.activePath === node.path;
  if (isSelected) {
    row.setAttribute("aria-current", "true");
  }

  const indent = document.createElement("span");
  indent.className = "ide-tree-indent";
  indent.textContent = "";
  row.appendChild(indent);

  const icon = document.createElement("span");
  icon.className = "ide-tree-icon";
  if (node.kind === "directory") {
    const expanded = state.expandedDirs.has(node.path) || state.fileFilter.length > 0;
    icon.classList.add(expanded ? "folder-open" : "folder-closed");
  } else {
    const ext = String(node.name || "").split(".").pop().toLowerCase();
    const iconMap = {st: "file-st", toml: "file-toml", md: "file-md", json: "file-json"};
    icon.classList.add(iconMap[ext] || "file-generic");
  }
  row.appendChild(icon);

  const label = document.createElement("span");
  label.textContent = node.name;
  row.appendChild(label);

  row.addEventListener("click", async () => {
    closeTreeContextMenu();
    selectPath(node.path);
    if (node.kind === "directory") {
      toggleDir(node.path);
    } else {
      await openFile(node.path);
    }
  });
  row.addEventListener("contextmenu", (event) => {
    event.preventDefault();
    openTreeContextMenu(node.path, event.clientX, event.clientY);
  });
  el.fileTree.appendChild(row);

  if (node.kind === "directory" && (state.expandedDirs.has(node.path) || state.fileFilter.length > 0)) {
    for (const child of node.children || []) {
      renderTreeNode(child, depth + 1);
    }
  }
}

function renderFileTree() {
  el.fileTree.innerHTML = "";
  if (state.tree.length === 0) {
    const empty = document.createElement("div");
    empty.className = "muted";
    empty.textContent = state.activeProject
      ? "No visible files in project root."
      : "No project selected. Use Open Folder.";
    el.fileTree.appendChild(empty);
    return;
  }
  for (const node of state.tree) {
    renderTreeNode(node, 0);
  }
}

function renderTabs() {
  if (state.splitEnabled) {
    renderPrimaryTabs();
    renderSecondaryTabs();
  } else {
    // Single-editor mode: render into the shared tab bar
    el.tabBar.innerHTML = "";
    for (const [path, tab] of state.openTabs.entries()) {
      el.tabBar.appendChild(createTabButton(path, tab, path === state.activePath, async () => {
        await switchTab(path);
      }));
    }
  }
}

function createTabButton(path, tab, isActive, onClick) {
  const button = document.createElement("button");
  button.type = "button";
  button.className = `ide-tab${isActive ? " active" : ""}`;
  button.setAttribute("aria-label", `Open tab ${path}`);
  if (tab.dirty) {
    const dot = document.createElement("span");
    dot.className = "dirty-dot";
    button.appendChild(dot);
  }
  const label = document.createElement("span");
  label.textContent = path;
  button.appendChild(label);
  button.addEventListener("click", onClick);
  return button;
}

function renderPrimaryTabs() {
  el.tabBarPrimary.innerHTML = "";
  for (const [path, tab] of state.openTabs.entries()) {
    el.tabBarPrimary.appendChild(createTabButton(path, tab, path === state.activePath, async () => {
      await switchTab(path);
    }));
  }
}

function renderSecondaryTabs() {
  el.tabBarSecondary.innerHTML = "";
  for (const path of state.secondaryOpenTabs) {
    const tab = state.openTabs.get(path);
    if (!tab) continue;
    el.tabBarSecondary.appendChild(createTabButton(path, tab, path === state.secondaryPath, async () => {
      openInSecondaryPane(path, tab.content);
      renderSecondaryTabs();
    }));
  }
}

function renderBreadcrumbs(path) {
  el.breadcrumbBar.innerHTML = "";
  const projectRoot = state.activeProject || "project";
  const rootLabel = projectRoot.split("/").filter(Boolean).pop() || projectRoot;
  if (!path) {
    el.breadcrumbBar.textContent = rootLabel;
    return;
  }
  const parts = String(path).split("/").filter(Boolean);
  const root = document.createElement("span");
  root.textContent = rootLabel;
  el.breadcrumbBar.appendChild(root);
  for (const [index, part] of parts.entries()) {
    const sep = document.createElement("span");
    sep.className = "sep";
    sep.textContent = "\u203A";
    el.breadcrumbBar.appendChild(sep);

    const item = document.createElement("span");
    item.textContent = part;
    if (index === parts.length - 1) {
      item.className = "current";
    }
    el.breadcrumbBar.appendChild(item);
  }
}

function markTabDirty(path, dirty) {
  const tab = state.openTabs.get(path);
  if (!tab) {
    return;
  }
  const nextDirty = !!dirty;
  if (tab.dirty === nextDirty) {
    return;
  }
  tab.dirty = nextDirty;
  renderTabs();
  if (nextDirty && typeof setProjectValidationPending === "function") {
    setProjectValidationPending();
  }
  document.dispatchEvent(new CustomEvent("ide-tab-dirty-change", {
    detail: {
      path,
      dirty: nextDirty,
    },
  }));
}

function updateDraftInfo() {
  const dirtyTabs = [...state.openTabs.values()].filter((tab) => tab.dirty).length;
  if (typeof ideSetCodeTabDirty === "function") {
    ideSetCodeTabDirty(dirtyTabs > 0);
  }
  if (dirtyTabs === 0) {
    el.draftInfo.textContent = "Draft sync idle";
    return;
  }
  el.draftInfo.textContent = `${dirtyTabs} unsynced draft(s)`;
}


function applyProjectSelection(selection) {
  const active = selection?.active_project ? String(selection.active_project) : "";
  const startup = selection?.startup_project ? String(selection.startup_project) : "";
  state.activeProject = active || null;
  state.startupProject = startup || null;

  if (typeof updateIdeTitleWithProject === "function") {
    updateIdeTitleWithProject();
    if (typeof setProjectValidationPending === "function") {
      setProjectValidationPending("Not validated yet.");
    }
  } else {
    const projectPath = state.activeProject || state.startupProject || "";
    const projectName = projectPath ? projectPath.split("/").filter(Boolean).pop() || projectPath : "";
    el.ideTitle.textContent = projectName || "truST IDE";
  }
  el.statusProject.textContent = state.activeProject || "--";
  if (state.activeProject) {
    const shortName = state.activeProject.split("/").filter(Boolean).pop() || state.activeProject;
    el.scopeNote.textContent = shortName;
  } else {
    el.scopeNote.textContent = "No project";
  }

  document.dispatchEvent(new CustomEvent("ide-project-changed", {
    detail: {
      activeProject: state.activeProject,
      startupProject: state.startupProject,
    },
  }));
}

async function refreshProjectSelection() {
  const selection = await apiJson("/api/ide/project", {
    method: "GET",
    headers: apiHeaders(),
  });
  applyProjectSelection(selection || {});
  return selection;
}

function loadRecentProjects() {
  try {
    const raw = localStorage.getItem(RECENT_PROJECTS_KEY);
    return raw ? JSON.parse(raw) : [];
  } catch {
    return [];
  }
}

function saveRecentProject(path) {
  const recent = loadRecentProjects().filter((item) => item.path !== path);
  recent.unshift({path, ts: Date.now()});
  if (recent.length > MAX_RECENT_PROJECTS) recent.length = MAX_RECENT_PROJECTS;
  try {
    localStorage.setItem(RECENT_PROJECTS_KEY, JSON.stringify(recent));
  } catch {
    // quota exceeded
  }
}

function renderRecentProjects(onSelect) {
  const recent = loadRecentProjects();
  el.openProjectRecent.innerHTML = "";
  if (recent.length === 0) {
    const hint = document.createElement("div");
    hint.className = "muted";
    hint.style.padding = "6px 0";
    hint.textContent = "No recent projects. Enter a path above.";
    el.openProjectRecent.appendChild(hint);
    return;
  }
  state._recentItems = [];
  for (const item of recent) {
    const row = document.createElement("button");
    row.type = "button";
    row.className = "ide-recent-item";
    row.innerHTML = `<svg viewBox="0 0 16 16"><path d="M2 13V4a1 1 0 0 1 1-1h3.5l2 2H13a1 1 0 0 1 1 1v6a1 1 0 0 1-1 1H3a1 1 0 0 1-1-1z"/></svg>`;
    const label = document.createElement("span");
    label.textContent = item.path;
    row.appendChild(label);
    const ts = document.createElement("span");
    ts.className = "recent-ts";
    ts.textContent = item.ts ? new Date(item.ts).toLocaleDateString() : "";
    row.appendChild(ts);
    row.addEventListener("click", () => onSelect(item.path));
    el.openProjectRecent.appendChild(row);
    state._recentItems.push(row);
  }
}

function openProjectPanel() {
  state._recentSelectedIndex = -1;
  el.openProjectInput.value = state.activeProject || state.startupProject || "";
  renderRecentProjects((path) => {
    closeOpenProjectPanel();
    doOpenProject(path);
  });
  hideBrowseListing();
  el.openProjectPanel.classList.add("open");
  el.openProjectInput.focus();
  el.openProjectInput.select();
}

function closeOpenProjectPanel() {
  el.openProjectPanel.classList.remove("open");
  state._recentSelectedIndex = -1;
  hideBrowseListing();
}

// ── New Project Flow ─────────────────────────────────────

function updateNewProjectPreview() {
  const preview = el.newProjectPreview;
  if (!preview) return;
  const name = String(el.newProjectName?.value || "").trim();
  const location = String(el.newProjectLocation?.value || "").trim();
  if (!name || !location) {
    preview.textContent = "Will create: --";
    return;
  }
  const normalizedLocation = location.replace(/[\\/]+$/, "");
  preview.textContent = `Will create: ${normalizedLocation}/${name}`;
}

function openNewProjectModal(locationOverride) {
  el.newProjectName.value = "";
  const fallbackLocation = state.activeProject
    ? state.activeProject.split("/").slice(0, -1).join("/")
    : "";
  el.newProjectLocation.value = String(locationOverride || "").trim() || fallbackLocation;
  el.newProjectTemplate.value = "empty";
  updateNewProjectPreview();
  el.newProjectModal.classList.add("open");
  el.newProjectName.focus();
}

function closeNewProjectModal() {
  el.newProjectModal.classList.remove("open");
}

async function newProjectFlow() {
  openNewProjectModal();
}

async function submitNewProject() {
  const name = el.newProjectName.value.trim();
  const location = el.newProjectLocation.value.trim();
  const template = el.newProjectTemplate.value || "empty";
  if (!name) {
    setStatus("Project name is required.");
    el.newProjectName.focus();
    return;
  }
  if (!location) {
    setStatus("Project location is required.");
    el.newProjectLocation.focus();
    return;
  }
  closeNewProjectModal();
  setStatus(`Creating project "${name}"...`);
  const selection = await apiJson("/api/ide/project/create", {
    method: "POST",
    headers: apiHeaders(),
    body: JSON.stringify({name, location, template}),
  });
  applyProjectSelection(selection || {});
  saveRecentProject(state.activeProject || `${location}/${name}`);
  state.tree = [];
  state.files = [];
  state.openTabs.clear();
  state.secondaryOpenTabs.clear();
  state.activePath = null;
  state.selectedPath = null;
  state.secondaryPath = null;
  state.references = [];
  state.searchHits = [];
  showWelcomeScreen();
  renderFileTree();
  renderTabs();
  renderBreadcrumbs(null);
  renderReferences([]);
  renderSearchHits([]);
  if (state.editorView) {
    state.suppressEditorChange = true;
    state.editorView.setValue("");
    state.suppressEditorChange = false;
    applyMonacoMarkers([], activeModel());
  }
  updateDraftInfo();
  setStatus(`Created project: ${state.activeProject || name}`);
  if (typeof showIdeToast === "function") {
    showIdeToast(`Project "${name}" created`, "success");
  }
  await bootstrapFiles();
}

async function doOpenProject(pathStr) {
  const path = String(pathStr || "").trim();
  if (!path) return;

  // US-2.2: Prompt to save unsaved changes before switching projects
  const dirtyCount = [...state.openTabs.values()].filter((t) => t.dirty).length;
  if (dirtyCount > 0) {
    const save = await ideConfirm("Unsaved Changes", `Save ${dirtyCount} unsaved file(s) before switching projects?`);
    if (save) {
      await flushDirtyTabs();
    }
  }

  // US-2.2: Warn if the folder has no .st files
  try {
    const browseResult = await apiJson(`/api/ide/browse?path=${encodeURIComponent(path)}`, {
      method: "GET",
      headers: apiHeaders(),
      timeoutMs: 2000,
    });
    const entries = Array.isArray(browseResult.entries) ? browseResult.entries : [];
    const hasSt = entries.some((e) =>
      e.kind === "file" && e.name.toLowerCase().endsWith(".st")
    );
    if (!hasSt) {
      const proceed = await ideConfirm("No ST files", "No .st files found in this folder. Open anyway?");
      if (!proceed) return;
    }
  } catch {
    // Ignore browse errors and proceed with open
  }

  const selection = await apiJson("/api/ide/project/open", {
    method: "POST",
    headers: apiHeaders(),
    body: JSON.stringify({path}),
  });
  applyProjectSelection(selection || {});
  saveRecentProject(state.activeProject || path);

  state.tree = [];
  state.files = [];
  state.openTabs.clear();
  state.secondaryOpenTabs.clear();
  state.activePath = null;
  state.selectedPath = null;
  state.secondaryPath = null;
  state.references = [];
  state.searchHits = [];
  showWelcomeScreen();
  renderFileTree();
  renderTabs();
  renderBreadcrumbs(null);
  renderReferences([]);
  renderSearchHits([]);
  if (state.editorView) {
    state.suppressEditorChange = true;
    state.editorView.setValue("");
    state.suppressEditorChange = false;
    applyMonacoMarkers([], activeModel());
  }
  updateDraftInfo();
  setStatus(`Opened project: ${state.activeProject || path}`);
  await bootstrapFiles();
}

async function openProjectFlow() {
  openProjectPanel();
}

async function bootstrapFiles() {
  if (!state.activeProject) {
    state.tree = [];
    state.files = [];
    renderFileTree();
    renderBreadcrumbs(null);
    return;
  }
  let result;
  try {
    result = await apiJson("/api/ide/tree", {
      method: "GET",
      headers: apiHeaders(),
    });
  } catch (error) {
    const message = String(error?.message || error).toLowerCase();
    if (message.includes("project root unavailable")) {
      applyProjectSelection({active_project: null, startup_project: state.startupProject});
      state.tree = [];
      state.files = [];
      renderFileTree();
      renderBreadcrumbs(null);
      setStatus("No project selected. Use Open Folder.");
      return;
    }
    throw error;
  }
  state.tree = Array.isArray(result.tree) ? result.tree : [];
  state.files = flattenFiles(state.tree, []).sort((a, b) => a.localeCompare(b));
  renderFileTree();
  if (!state.activePath && state.files.length > 0) {
    await openFile(state.files[0]);
  } else if (!state.activePath) {
    renderBreadcrumbs(null);
  }
}
