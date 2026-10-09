/**
 * Sets the page's target label (specification 35, B9.2). The label element is the one threePageTemplate.ts renders. The label
 * is set as text, never as markup. Returns false when the page has no label element or the message carries no text.
 */
export function applyStudioTargetLabel(
  root: { querySelector(selector: string): { textContent: string | null; hidden?: boolean } | null },
  message: { readonly label?: unknown }
): boolean {
  const element = root.querySelector("[data-studio-target]");
  if (!element || typeof message.label !== "string") return false;
  element.textContent = message.label;
  element.hidden = message.label === "";
  return true;
}
