/**
 * The Analyze… entry on the Tune row (Specification 35 B9.9): one short list of the analysis tools that exist for the
 * current model, and a Close control that returns the page to its tuning result.
 */
export interface AnalysisChooserOptions {
  readonly labels: Readonly<Record<string, string>>;
  /** The tools that can run now; Analyze… is shown only while there is at least one. */
  readonly tools: () => readonly string[];
  readonly choose: (tool: string) => void;
  readonly close: () => void;
}

export class ThreePageAnalysisChooser {
  private readonly button: HTMLButtonElement;
  private readonly list: HTMLElement;
  private readonly closeButton: HTMLButtonElement;
  constructor(root: HTMLElement, private readonly options: AnalysisChooserOptions) {
    this.button = root.querySelector<HTMLButtonElement>("[data-studio-analyze]")!;
    this.list = root.querySelector<HTMLElement>("[data-studio-analyze-list]")!;
    this.closeButton = root.querySelector<HTMLButtonElement>("[data-studio-analysis-close]")!;
    this.button.addEventListener("click", () => { if (this.list.hidden) this.show(); else this.hide(); });
    this.closeButton.addEventListener("click", () => { this.options.close(); this.button.focus(); });
  }
  /** The tool list is shown while the chooser is open; a chosen tool is shown in place of the tuning result. */
  synchronize(toolActive: boolean): void {
    this.button.hidden = this.options.tools().length === 0;
    this.closeButton.hidden = !toolActive;
    if (this.button.hidden) this.hide();
  }
  get open(): boolean { return !this.list.hidden; }
  private show(): void {
    const choices = this.options.tools().map(tool => {
      const choice = this.list.ownerDocument.createElement("button");
      choice.type = "button"; choice.className = "trust-button"; choice.dataset.studioAnalysisTool = tool;
      choice.textContent = this.options.labels[tool] ?? tool;
      choice.addEventListener("click", () => { this.hide(); this.options.choose(tool); });
      return choice;
    });
    this.list.replaceChildren(...choices);
    this.list.hidden = false; this.button.setAttribute("aria-expanded", "true");
    choices[0]?.focus();
  }
  private hide(): void {
    this.list.hidden = true; this.button.setAttribute("aria-expanded", "false");
  }
}
