import type { ControlLoopDefinition } from "../loopDefinition";
import { controllerInstanceLabel } from "../controllerGroupPresentation";
import type { StudioPost } from "./threePageOperation";

/** One selector names both the declared ST target and its compatible configuration. */
export class ThreePageController {
  choice = "automatic";
  private explicit = false;
  private recommended: string | undefined;
  private families: string[] = [];
  private readonly select: HTMLSelectElement;
  get selectedFamily(): string { return this.explicit ? this.choice : this.recommended ?? "automatic"; }
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
    if (payload.status === "candidate" && typeof payload.family === "string") {
      if (!this.explicit) this.recommended = payload.family;
    } else if (typeof payload.controllerChoice === "string" && (payload.status === "restored" || !this.explicit || payload.controllerChoice === this.choice)) {
      this.choice = payload.controllerChoice;
      this.explicit = payload.controllerChoice !== "automatic";
      if (!this.explicit && typeof payload.family === "string") this.recommended = payload.family;
      if (this.explicit) this.recommended = undefined;
    }
    if (!this.families.length) return;
    const options: HTMLOptionElement[] = [];
    if (this.choice === "automatic") {
      const automatic = new Option(`${controllerInstanceLabel(this.loop.controller)} · Choose automatically`, this.loop.controller.instance);
      automatic.dataset.family = "automatic"; options.push(automatic);
    }
    for (const family of this.families) {
      const option = new Option(`${controllerInstanceLabel(this.loop.controller)} · ${family}${family === this.recommended ? " — recommended" : ""}`, `${this.loop.controller.instance}::${family}`);
      option.dataset.family = family; options.push(option);
    }
    this.select.replaceChildren(...options);
    const selected = options.find(option => option.dataset.family === this.selectedFamily) ?? options.find(option => option.dataset.family === "automatic");
    if (selected) this.select.value = selected.value;
  }
}
