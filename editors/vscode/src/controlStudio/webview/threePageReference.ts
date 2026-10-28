import type { LoopDefinition } from "../generated/OpenResult";
import { displayEngineeringUnit } from "../chartContract";
import { OPENED_OFFERS_EVENT, openedOffers } from "./openedOffers";

/** Only missing-reference identification exposes this additional signal selector. */
export class ThreePageReference {
  private readonly row?: HTMLLabelElement;
  private readonly select?: HTMLSelectElement;
  private readonly method: HTMLSelectElement;
  constructor(root: HTMLElement, loop: LoopDefinition) {
    this.method = root.querySelector<HTMLSelectElement>("[data-engineer-acquisition-method]")!;
    if (loop.signals.setpoint) return;
    this.row = document.createElement("label"); this.row.className = "studio-field"; this.row.textContent = "Setpoint";
    this.select = document.createElement("select"); this.select.dataset.identifySetpoint = "";
    this.fill();
    // An open answer that arrives after this editor is built refills the choices (the engine's offers, B3.6).
    root.ownerDocument.defaultView?.addEventListener(OPENED_OFFERS_EVENT, () => this.fill());
    this.row.append(this.select);
    root.querySelector("[data-studio-outputs]")!.closest("label")!.after(this.row);
    this.method.addEventListener("change", () => this.synchronize());
    this.select.addEventListener("change", () => this.method.dispatchEvent(new Event("change", { bubbles: true })));
    this.synchronize();
  }
  /**
   * The setpoint signal the engineer chose: the identify request sends its identity whatever the method, and the engine reads the loop from the
   * project (B3.6). The engine uses it for the setpoint step alone and runs another method as if none were named (B3.3).
   */
  get signalId(): string | undefined { return this.select?.value || undefined; }
  restore(id?: string): void { if (this.select && id) { this.fill(); this.select.value = id; } this.synchronize(); }
  /** The setpoints the engine offers a loop without a declared one (B3.6), which the open answer gives; the placeholder comes first. */
  private fill(): void {
    if (!this.select) return;
    const chosen = this.select.value;
    this.select.replaceChildren(new Option("Select setpoint", ""),
      ...(openedOffers().setpointChoices ?? []).map(choice => new Option(`${choice.symbol} (${displayEngineeringUnit(choice.unit)})`, choice.id)));
    this.select.value = chosen;
  }
  private synchronize(): void { if (this.row) this.row.hidden = this.method.value !== "setpoint_step"; }
}
