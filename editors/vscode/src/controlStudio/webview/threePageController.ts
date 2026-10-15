import { familyLabel } from "../controllerFamilies";
import type { ControlLoopDefinition } from "../loopDefinition";
import { controllerInstanceLabel } from "../controllerGroupPresentation";
import type { StudioPost } from "./threePageOperation";

/** One selector names both the declared ST target and its compatible configuration. */
export class ThreePageController {
  choice = "automatic";
  private explicit = false;
  private recommended: string | undefined;
  /** The configuration of the automatic candidate that manual settings edit, kept apart from the host's recommendation. */
  private candidate: string | undefined;
  private families: string[] = [];
  /** The manual-settings symbols the engine names per family (`familySettings`, C2); undefined for a loop that names none. */
  private familySettings: Record<string, readonly string[]> | undefined;
  /** Receives the tuning methods the engine gives the loop (`setupDiscover`'s `methods`, C4), so that the Tune page offers exactly those. */
  methodsChanged: ((methods: readonly string[]) => void) | undefined;
  private readonly select: HTMLSelectElement;
  get selectedFamily(): string { return this.explicit ? this.choice : this.candidate ?? this.recommended ?? "automatic"; }
  constructor(private readonly root: HTMLElement, private readonly loop: ControlLoopDefinition, private readonly post: StudioPost) {
    this.select = root.querySelector<HTMLSelectElement>("[data-studio-controller]")!;
    this.select.addEventListener("change", () => {
      this.choice = this.select.selectedOptions[0]?.dataset.family ?? "automatic";
      this.explicit = this.choice !== "automatic";
      this.post("control-studio.engineer.invalidate", { family: this.choice });
    });
    if (loop.controller.law?.kind !== "pid") {
      for (const option of Array.from(root.querySelector<HTMLSelectElement>("[data-engineer-tuning-method]")!.options)) {
        if (option.value === "SIMC" || option.value === "AMIGO") option.remove();
      }
    }
  }
  receive(payload: Record<string, unknown>): void {
    if (Array.isArray(payload.availableFamilies)) this.families = payload.availableFamilies.filter((family): family is string => typeof family === "string");
    if ("familySettings" in payload) this.familySettings = payload.familySettings as Record<string, readonly string[]> | undefined;
    if (Array.isArray(payload.methods)) this.methodsChanged?.(payload.methods.filter((method): method is string => typeof method === "string"));
    if (payload.status === "candidate" && typeof payload.family === "string") {
      if (!this.explicit) this.recommended = this.candidate = payload.family;
    } else if (typeof payload.controllerChoice === "string" && (payload.status === "restored" || !this.explicit || payload.controllerChoice === this.choice)) {
      this.choice = payload.controllerChoice;
      this.explicit = payload.controllerChoice !== "automatic";
      if (!this.explicit && typeof payload.family === "string") this.recommended = this.candidate = payload.family;
    }
    // The host recommends from the model and requirements; an explicit choice never changes what is recommended.
    if ("recommendedFamily" in payload) this.recommended = typeof payload.recommendedFamily === "string" ? payload.recommendedFamily : undefined;
    this.renderOptions();
  }
  /** Shows the controller the study recorded (A8.6): a declared family, or Automatic. The options follow once the families are known. */
  restoreChoice(family: string | undefined): void {
    this.choice = family ?? "automatic";
    this.explicit = this.choice !== "automatic";
    this.renderOptions();
  }
  /** The manual-settings symbols the engine names for a family, or undefined when it names none for this loop (a parked loop). */
  symbolsOf(family: string): readonly string[] | undefined {
    return this.familySettings ? this.familySettings[family] ?? [] : undefined;
  }
  private renderOptions(): void {
    if (!this.families.length) return;
    const options: HTMLOptionElement[] = [];
    if (this.choice === "automatic") {
      const automatic = new Option(`${controllerInstanceLabel(this.loop.controller)} · Choose automatically`, this.loop.controller.instance);
      automatic.dataset.family = "automatic"; options.push(automatic);
    }
    for (const family of this.families) {
      const option = new Option(`${controllerInstanceLabel(this.loop.controller)} · ${familyLabel(family)}${family === this.recommended ? " — recommended" : ""}`, `${this.loop.controller.instance}::${family}`);
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
