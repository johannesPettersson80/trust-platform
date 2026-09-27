import type { ControlLoopDefinition } from "../loopDefinition";
import { displayEngineeringUnit } from "../chartContract";

/** Only missing-reference identification exposes this additional signal selector. */
export class ThreePageReference {
  private readonly row?: HTMLLabelElement;
  private readonly select?: HTMLSelectElement;
  private readonly method: HTMLSelectElement;
  constructor(root: HTMLElement, private readonly loop: ControlLoopDefinition) {
    this.method = root.querySelector<HTMLSelectElement>("[data-engineer-acquisition-method]")!;
    if (loop.signals.setpoint) return;
    this.row = document.createElement("label"); this.row.className = "studio-field"; this.row.textContent = "Setpoint";
    this.select = document.createElement("select"); this.select.dataset.identifySetpoint = "";
    this.select.append(new Option("Select setpoint", ""));
    for (const signal of loop.signals.additional ?? []) {
      if (signal.role === "SP" && signal.unit === loop.signals.processVariable.unit) {
        this.select.append(new Option(`${signal.symbol} (${displayEngineeringUnit(signal.unit)})`, signal.id));
      }
    }
    this.row.append(this.select);
    root.querySelector("[data-studio-outputs]")!.closest("label")!.after(this.row);
    this.method.addEventListener("change", () => this.synchronize());
    this.select.addEventListener("change", () => this.method.dispatchEvent(new Event("change", { bubbles: true })));
    this.synchronize();
  }
  get signalId(): string | undefined { return this.method.value === "setpoint_step" ? this.select?.value || undefined : undefined; }
  get acquisitionLoop(): ControlLoopDefinition {
    const signal = this.loop.signals.additional?.find(item => item.id === this.signalId);
    return signal ? { ...this.loop, signals: { ...this.loop.signals, setpoint: signal } } : this.loop;
  }
  restore(id?: string): void { if (this.select && id) this.select.value = id; this.synchronize(); }
  private synchronize(): void { if (this.row) this.row.hidden = this.method.value !== "setpoint_step"; }
}
