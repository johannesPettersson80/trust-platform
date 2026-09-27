/** One status node: ordinary history is inspectable; actionable notices stay in view. */
export function presentEngineerNotice(section: HTMLElement, payload: Record<string, unknown>): void {
  const status = String(payload.status ?? "start");
  const unresolved = (payload.commissioning as { recoveryConfirmed?: boolean } | undefined)?.recoveryConfirmed === false;
  const attention = unresolved || ["running", "busy", "commissioning", "review", "failed", "verification-failed", "unavailable", "recovery-required"].includes(status)
    || Boolean(payload.failure) || payload.cancellationUnconfirmed === true || payload.commissioningCheckpoint === true;
  const node = section.querySelector<HTMLElement>("[data-engineer-status]")!;
  const results = section.querySelector<HTMLElement>("[data-engineer-results]")!;
  node.classList.toggle("engineer-notice", attention);
  node.setAttribute("role", attention && (unresolved || ["failed", "verification-failed", "recovery-required"].includes(status)) ? "alert" : "status");
  if (attention) section.append(node);
  else { const evidence = results.querySelector("[data-engineer-evidence]"); (evidence?.parentElement ?? results).insertBefore(node, evidence); }
}

export function showEngineerInputError(section: HTMLElement, message: string): void {
  const node = section.querySelector<HTMLElement>("[data-engineer-status]")!;
  node.textContent = message;
  node.classList.add("engineer-notice");
  node.setAttribute("role", "alert");
  section.append(node);
}
