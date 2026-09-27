/** Keeps optional forms out of the navigation header without duplicating their controls. */
export function initializeEngineerInspectors(section: HTMLElement): void {
  const d = section.ownerDocument;
  const root = section.closest<HTMLElement>("[data-control-studio]")!;
  const options = section.querySelector<HTMLDetailsElement>("[data-engineer-options]")!;
  const results = section.querySelector<HTMLDetailsElement>("[data-engineer-results]")!;
  const titles: HTMLElement[] = [];
  for (const [details, name] of [[options, "settings"], [results, "details"]] as const) {
    const panel = d.createElement("section"); panel.className = "engineer-inspector"; panel.setAttribute("role", "region");
    const header = d.createElement("div"); header.className = "engineer-inspector-header";
    const title = d.createElement("strong"); titles.push(title);
    const close = d.createElement("button"); close.type = "button"; close.className = "trust-button";
    close.dataset.engineerInspectorClose = name; close.textContent = "Close"; close.setAttribute("aria-label", `Close ${name}`);
    close.addEventListener("click", () => { details.open = false; details.querySelector<HTMLElement>(":scope > summary")?.focus(); sync(); });
    header.append(title, close);
    const body = d.createElement("div"); body.className = "engineer-inspector-body";
    for (const node of Array.from(details.childNodes)) if (!(node instanceof d.defaultView!.HTMLElement && node.tagName === "SUMMARY")) body.append(node);
    panel.append(header, body); details.append(panel);
    details.addEventListener("toggle", () => {
      if (details.open) (details === options ? results : options).open = false;
      sync();
    });
  }
  const measure = () => {
    const header = root.querySelector<HTMLElement>(".header")?.getBoundingClientRect();
    const modes = root.querySelector<HTMLElement>(".modes")?.getBoundingClientRect();
    const footer = root.querySelector<HTMLElement>(".values")?.getBoundingClientRect();
    const top = root.dataset.manualWorkspace === "true" ? Math.max(header?.bottom ?? 0, modes?.bottom ?? 0) : header?.bottom ?? 0;
    root.style.setProperty("--engineer-inspector-top", `${top + 8}px`);
    root.style.setProperty("--engineer-inspector-bottom", `${(footer?.height ?? 60) + 8}px`);
  };
  function sync() {
    if (root.dataset.manualWorkspace === "true") { options.open = false; results.open = false; }
    root.dataset.inspectorOpen = options.open ? "settings" : results.open ? "details" : "none";
    const stage = section.dataset.selectedStage ?? "model";
    titles.forEach((title, index) => { title.textContent = `${stage[0].toUpperCase()}${stage.slice(1)} ${index ? "details" : "settings"}`; });
    measure();
  }
  const observer = new d.defaultView!.ResizeObserver(measure);
  for (const selector of [".header", ".modes", ".values"]) { const element = root.querySelector(selector); if (element) observer.observe(element); }
  new d.defaultView!.MutationObserver(sync).observe(root, { attributes: true, attributeFilter: ["data-manual-workspace"] });
  new d.defaultView!.MutationObserver(sync).observe(section, { attributes: true, attributeFilter: ["data-selected-stage"] });
  d.addEventListener("keydown", event => {
    if (event.key !== "Escape") return;
    const active = [options, results].find(details => details.open && details.contains(d.activeElement));
    if (active) { event.preventDefault(); active.open = false; active.querySelector<HTMLElement>(":scope > summary")?.focus(); sync(); }
  });
  root.dataset.manualView = "controls";
  const toggle = section.querySelector<HTMLButtonElement>("[data-engineer-manual-view]");
  toggle?.addEventListener("click", () => {
    root.dataset.manualView = root.dataset.manualView === "plot" ? "controls" : "plot";
    toggle.textContent = root.dataset.manualView === "plot" ? "Show controls" : "Show plot";
    measure();
  });
  sync();
}
