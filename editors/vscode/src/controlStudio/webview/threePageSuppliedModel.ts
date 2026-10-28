import type { StudioPost } from "./threePageOperation";

/** File reading has no model or runtime authority; the existing host session selects it. */
export class ThreePageSuppliedModel {
  private selection = 0;
  private reading = false;
  private readonly input: HTMLInputElement;
  private readonly method: HTMLSelectElement;
  constructor(private readonly root: HTMLElement, private readonly post: StudioPost,
    private readonly busy: () => boolean, private readonly notice: (message: string) => void) {
    this.method = root.querySelector<HTMLSelectElement>("[data-engineer-acquisition-method]")!;
    this.input = root.ownerDocument.createElement("input");
    this.input.type = "file"; this.input.accept = ".json,application/json"; this.input.hidden = true;
    this.input.dataset.studioModelFile = ""; this.input.setAttribute("aria-label", "Open plant model");
    root.append(this.input);
    this.method.addEventListener("change", () => { this.selection++; this.synchronize(); });
    this.input.addEventListener("change", () => { void this.read(); });
  }
  get selected(): boolean { return this.method.value === "supplied-model"; }
  open(): void {
    if (!this.selected || this.busy() || this.reading) return;
    this.input.value = ""; this.input.click();
  }
  /** The Identify button names the model's file while Supplied plant model is the method and nothing runs. */
  synchronize(): void {
    if (this.selected && !this.busy()) this.root.querySelector<HTMLButtonElement>("[data-engineer-find]")!.textContent = this.reading ? "Opening model…" : "Open model…";
  }
  private async read(): Promise<void> {
    const file = this.input.files?.[0], current = ++this.selection;
    if (!file || this.busy()) return;
    try {
      this.reading = true; this.synchronize();
      const json = await file.text();
      if (current !== this.selection || this.busy()) return;
      this.post("control-studio.general-model.import", { json });
    } catch { this.notice("The plant model file could not be read. Open it again."); }
    finally { this.reading = false; this.synchronize(); }
  }
}
