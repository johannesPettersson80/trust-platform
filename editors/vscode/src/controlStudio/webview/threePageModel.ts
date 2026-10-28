import type { IdentifiedModelPresentation } from "../generated/ManualIdentificationAnswer";

/** The engine's equation split for display: the gain, the denominator and the delay of the exponent (`equationFraction`, B3.8). */
type EquationFraction = NonNullable<IdentifiedModelPresentation["equationFraction"]>;

/**
 * The identification result's presentation (specification 35, B3.8): its model type, its equation as a stacked fraction, one line of
 * key numbers and the presentation's warnings. The type, the parts of the fraction and the key numbers are the engine's words
 * (`modelType`, `equationFraction` and `keyNumbers` of the presentation); the page lays them out and composes none of them.
 */
export function renderModelPresentation(root: HTMLElement, presentation: IdentifiedModelPresentation): void {
  const type = root.querySelector<HTMLElement>("[data-studio-model-type]")!;
  const equation = root.querySelector<HTMLElement>("[data-studio-model-equation]")!;
  type.textContent = presentation.modelType ?? "";
  const fraction = presentation.equationFraction;
  if (fraction) equation.replaceChildren(...transferEquation(root.ownerDocument, fraction));
  else equation.textContent = presentation.equation;
  setKeyNumbers(root, presentation.keyNumbers ?? "");
  setWarnings(root, presentation.warnings);
  root.querySelector<HTMLElement>("[data-studio-model-result]")!.hidden = false;
}

/** G(s) = fraction · e^(−θs): the fraction is written as a stacked numerator over denominator. */
function transferEquation(doc: Document, fraction: EquationFraction): Node[] {
  const stacked = doc.createElement("span"); stacked.className = "studio-math-fraction";
  const top = doc.createElement("span"); top.className = "studio-math-num"; top.textContent = fraction.numerator;
  const bottom = doc.createElement("span"); bottom.className = "studio-math-den"; bottom.textContent = fraction.denominator;
  stacked.append(top, bottom);
  const nodes: Node[] = [doc.createTextNode("G(s) = "), stacked];
  if (fraction.delay) {
    const exponent = doc.createElement("sup"); exponent.textContent = `−${fraction.delay}`;
    nodes.push(doc.createTextNode(" · e"), exponent);
  }
  return nodes;
}

function setKeyNumbers(root: HTMLElement, text: string): void {
  const line = root.querySelector<HTMLElement>("[data-studio-model-numbers]");
  if (!line) return;
  line.textContent = text;
  line.hidden = !text;
}

function setWarnings(root: HTMLElement, warnings: readonly string[]): void {
  const list = root.querySelector<HTMLElement>("[data-studio-model-warnings]");
  if (!list) return;
  list.replaceChildren(...warnings.map(warning => {
    const item = root.ownerDocument.createElement("li"); item.textContent = warning; return item;
  }));
  list.hidden = warnings.length === 0;
}
