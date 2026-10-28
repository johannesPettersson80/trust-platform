import type { ControllerOffer, LoopDefinition } from "../generated/OpenResult";
import { controllerInstanceLabel } from "../controllerGroupPresentation";
import type { StudioPost } from "./threePageOperation";

/** One selector names both the declared ST target and its compatible configuration. */
export class ThreePageController {
  choice = "automatic";
  private explicit = false;
  private recommended: string | undefined;
  /** The configuration of the automatic candidate that manual settings edit, kept apart from the host's recommendation. */
  private candidate: string | undefined;
  /** The families the engine offers the loop, in its order, each with the label the engine gives it (`controllerOffer`, C2, C4). */
  private families: ControllerOffer["families"] = [];
  /** The family the engine names as the default until the engineer chooses one (`controllerOffer.defaultFamily`, B4.2). */
  private defaultFamily: string | undefined;
  /** The manual-settings symbols the engine names per family (`controllerOffer.familySettings`, C2); undefined when the loop has no offer. */
  private familySettings: Record<string, readonly string[]> | undefined;
  /** Receives the tuning methods the engine gives the loop and their titles (`methods` and `methodTitles`, C4), so that the Tune page offers exactly those. */
  methodsChanged: ((methods: readonly string[], titles: Readonly<Record<string, string>>) => void) | undefined;
  private readonly select: HTMLSelectElement;
  /** The family the selector shows: the engineer's choice, else the engine's latest candidate's, else the open answer's default (B4.2). It is shown only; no request sends it. */
  get selectedFamily(): string { return this.explicit ? this.choice : this.candidate ?? this.defaultFamily ?? "automatic"; }
  /**
   * The family the engineer chose, which is the only family a request sends (B4.2): none until the engineer, or the study's record, chose one. The
   * engine takes the structure from the study's inputs when a request names none, and the family the selector shows for the manual method.
   */
  get chosenFamily(): string | undefined { return this.explicit ? this.choice : undefined; }
  constructor(private readonly root: HTMLElement, private readonly loop: LoopDefinition, private readonly post: StudioPost) {
    this.select = root.querySelector<HTMLSelectElement>("[data-studio-controller]")!;
    this.select.addEventListener("change", () => {
      this.choice = this.select.selectedOptions[0]?.dataset.family ?? "automatic";
      this.explicit = this.choice !== "automatic";
      this.post("control-studio.engineer.invalidate", { family: this.choice });
    });
  }
  receive(payload: Record<string, unknown>): void {
    // The open answer's controller (C2, C4, B4.2) is the page's only source of the families, their labels and their settings.
    if ("controllerOffer" in payload) this.adopt(payload.controllerOffer as ControllerOffer | null);
    if (Array.isArray(payload.methods)) this.methodsChanged?.(payload.methods.filter((method): method is string => typeof method === "string"), titlesOf(payload.methodTitles));
    // The selector shows the family of the engine's latest candidate until the engineer chooses one (B4.2): the engine names the family.
    if (payload.status === "candidate" && typeof payload.family === "string" && !this.explicit) this.candidate = payload.family;
    this.renderOptions();
  }
  /** Shows the controller the study recorded (A8.6): a declared family, or Automatic. The options follow once the families are known. */
  restoreChoice(family: string | undefined): void {
    this.choice = family ?? "automatic";
    this.explicit = this.choice !== "automatic";
    this.renderOptions();
  }
  /** The manual-settings symbols the engine names for a family, or undefined when it names none for this loop. */
  symbolsOf(family: string): readonly string[] | undefined {
    return this.familySettings?.[family];
  }
  /** Takes the controller the engine offers the loop: its families, the family it recommends and the one it shows by default, and the settings each takes. */
  private adopt(offer: ControllerOffer | null): void {
    this.families = offer?.families ?? [];
    this.familySettings = offer?.familySettings;
    this.recommended = offer?.recommended ?? undefined;
    this.defaultFamily = offer?.defaultFamily ?? undefined;
  }
  private renderOptions(): void {
    if (!this.families.length) return;
    const options: HTMLOptionElement[] = [];
    const instanceLabel = controllerInstanceLabel({ instance: this.loop.controller.instance, group: this.loop.controller.group ?? undefined });
    if (this.choice === "automatic") {
      const automatic = new Option(`${instanceLabel} · Choose automatically`, this.loop.controller.instance);
      automatic.dataset.family = "automatic"; options.push(automatic);
    }
    for (const { family, label } of this.families) {
      const option = new Option(`${instanceLabel} · ${label}${family === this.recommended ? " — recommended" : ""}`, `${this.loop.controller.instance}::${family}`);
      option.dataset.family = family; options.push(option);
    }
    // The same select lists the other compiled PID instances and the placeholder that ThreePageBindings adds (its options carry
    // data-binding-loop-id, its placeholder data-binding-placeholder). They are the bindings' to change, so they stay in place.
    const previous = this.select.value;
    const placeholders = Array.from(this.select.querySelectorAll<HTMLOptionElement>("[data-binding-placeholder]"));
    const alternatives = Array.from(this.select.querySelectorAll<HTMLOptionElement>("[data-binding-loop-id]"));
    this.select.replaceChildren(...placeholders, ...options, ...alternatives);
    const kept = [...placeholders, ...alternatives].some(option => option.value === previous);
    const selected = options.find(option => option.dataset.family === this.selectedFamily) ?? options.find(option => option.dataset.family === "automatic");
    if (kept) this.select.value = previous;
    else if (selected) this.select.value = selected.value;
  }
}

/** The engine's titles of the tuning methods, read as a record of strings: a value that is not a string is ignored. */
function titlesOf(value: unknown): Record<string, string> {
  const titles: Record<string, string> = {};
  if (value && typeof value === "object" && !Array.isArray(value)) {
    for (const [method, title] of Object.entries(value)) if (typeof title === "string") titles[method] = title;
  }
  return titles;
}
