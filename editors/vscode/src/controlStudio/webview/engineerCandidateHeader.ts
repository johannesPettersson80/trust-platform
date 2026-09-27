/** Shows engineer candidate state without confusing prediction, verification and adoption. */
export function initializeEngineerCandidateHeader(): void {
  let structure: string | undefined;
  let manual = false;
  window.addEventListener("trust-control-studio-engineer", event => {
    const payload = (event as CustomEvent).detail as Record<string, unknown>;
    const header = document.querySelector<HTMLElement>("[data-control-studio-candidate-revision]");
    if (!header) return;
    const design = payload.design as { status?: string; candidate?: { structure?: string } } | undefined;
    if (design?.candidate?.structure) { structure = design.candidate.structure; manual = design.status === "exploratory"; }
    const prefix = structure ? `${manual ? "Manual " : ""}${structure} · ` : "";
    const status = String(payload.status);
    if (["model-ready", "edited", "cancelled", "unavailable"].includes(status)) {
      structure = undefined; header.textContent = "None";
    } else if (status === "candidate") header.textContent = `${prefix}Unapplied`;
    else if (status === "verified") header.textContent = `${prefix}Verified, unapplied`;
    else if (status === "verification-failed") header.textContent = `${prefix}Verification failed`;
    else if (status === "running") header.textContent = "Working";
    else if (status === "failed") header.textContent = `${prefix}Not verified`;
    else if (status === "restored") { structure = undefined; header.textContent = "Historical"; }
    else if (status === "applied") header.textContent = "Parameters observed · Manual";
    else if (status === "commissioned") header.textContent = `${prefix}Auto observed`;
    header.title = header.textContent ?? "";
  });
}
