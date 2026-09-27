import type { IdentifiedModelPresentation } from "../identification";

/** View-only channel selection; preserves the full immutable fit for every output. */
export class IdentificationOutputSelection {
  private readonly label: HTMLLabelElement;
  private readonly picker: HTMLSelectElement;
  private selected = "";
  private activePlot = "";
  private evidence: IdentifiedModelPresentation | undefined;

  constructor(private readonly changed: () => void) {
    this.label = document.createElement("label");
    this.label.dataset.identificationOutputControl = "true";
    this.label.hidden = true;
    this.label.append("Output ");
    this.picker = document.createElement("select");
    this.picker.dataset.identificationOutput = "true";
    this.picker.setAttribute("aria-label", "Identified model output");
    this.label.append(this.picker);
    document.querySelector("[data-control-plot-tabs]")?.after(this.label);
    this.picker.addEventListener("change", () => {
      this.selected = this.picker.value;
      this.changed();
      if (this.evidence) window.dispatchEvent(new CustomEvent("trust-control-studio-identification-output-selected", { detail: this.select(this.evidence) }));
    });
    window.addEventListener("trust-control-studio-plot-shown", event => {
      this.activePlot = String((event as CustomEvent).detail?.id ?? "");
      this.visibility();
    });
  }

  select(evidence: IdentifiedModelPresentation): IdentifiedModelPresentation {
    this.evidence = evidence;
    const outputs = evidence.outputFits ?? [];
    if (!outputs.some(output => output.id === this.selected)) this.selected = evidence.outputId ?? outputs[0]?.id ?? "";
    if (JSON.stringify([...this.picker.options].map(option => option.value)) !== JSON.stringify(outputs.map(output => output.id))) {
      this.picker.replaceChildren(...outputs.map(output => {
        const option = document.createElement("option");
        option.value = output.id; option.textContent = `${output.id} (${output.unit})`; return option;
      }));
    }
    this.picker.value = this.selected;
    this.visibility();
    const output = outputs.find(output => output.id === this.selected);
    return output ? { ...evidence, outputId: output.id, units: { ...evidence.units, output: output.unit },
      predictedResponse: output.points, validityRange: output.validityRange,
      validation: { normalizedError: output.normalizedError, residualLagOneCorrelation: output.residualLagOneCorrelation } } : evidence;
  }

  private visibility(): void {
    this.label.hidden = (this.evidence?.outputFits?.length ?? 0) < 2
      || !["fit", "residuals", "validation-data", "poles-zeros"].includes(this.activePlot);
  }
}
