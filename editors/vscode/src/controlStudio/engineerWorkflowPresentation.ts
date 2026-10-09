/** Product status copy has no dependency on model IDs, runtime IDs or revision hashes. */
export function modelReadyReason(model: { source: string; validation?: string }, supportsAlternatives: boolean | readonly string[]): string {
  const ready = model.validation === "unvalidated" ? "Provisional model ready for tuning." : model.source === "fitted" ? "Model found and validated." : "Model ready.";
  if (Array.isArray(supportsAlternatives)) return `${ready} ${supportsAlternatives.length ? `Choose and tune a compatible controller: ${supportsAlternatives.join(", ")}.` : "Declare a compatible ST controller adapter before tuning."}`;
  return supportsAlternatives
    ? `${ready} Recommend P control first; PI, PD and PID are available if the task needs them.`
    : `${ready} Tune the P controller for your requirements.`;
}
