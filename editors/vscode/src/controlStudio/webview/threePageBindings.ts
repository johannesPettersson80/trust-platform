import { controllerInstanceLabel } from "../controllerGroupPresentation";
import type { ControlLoopDefinition } from "../loopDefinition";
import type { ControlStudioBindingTargetSummary } from "../hostBindingSession";
import type { StudioPost } from "./threePageOperation";
import type { ThreePageController } from "./threePageController";
import { loopSignal } from "../loopChannels";
import { displayEngineeringUnit } from "../unitDisplay";
import type { RecordingSignalCatalog, RecordingSignalSelection } from "../openedRecordingChannels";

/** Selectors use exact configured/compiled identities supplied by the scoped host. */
export class ThreePageBindings {
  private targets: readonly ControlStudioBindingTargetSummary[] = [];
  private pending: string | undefined;
  private invalid = false;
  private problem = "";
  private recording: RecordingSignalCatalog | undefined;
  private recordingStale = false;
  private readonly controllerSelect: HTMLSelectElement;
  private readonly inputs: HTMLSelectElement;
  private readonly outputs: HTMLSelectElement;
  constructor(private readonly root: HTMLElement, private readonly loop: ControlLoopDefinition,
    private readonly controller: ThreePageController, private readonly post: StudioPost,
    private readonly notice: (page: "model" | "tune", message: string) => void,
    private readonly refreshGate: (valid: boolean, reason?: string) => void = () => undefined,
    private readonly recordingChanged: (selection: RecordingSignalSelection) => void = () => undefined) {
    this.controllerSelect = root.querySelector<HTMLSelectElement>("[data-studio-controller]")!;
    this.inputs = root.querySelector<HTMLSelectElement>("[data-studio-inputs]")!;
    this.outputs = root.querySelector<HTMLSelectElement>("[data-studio-outputs]")!;
    this.controllerSelect.addEventListener("change", event => {
      const option = this.controllerSelect.selectedOptions[0];
      if (!option?.dataset.bindingLoopId) return;
      event.stopImmediatePropagation();
      const target = this.targets.find(item => item.loopId === option.dataset.bindingLoopId);
      if (target) this.choose(target, option.dataset.family, "tune");
    }, true);
    this.inputs.addEventListener("change", () => this.signalsChanged());
    this.outputs.addEventListener("change", () => this.signalsChanged());
    this.controllerSelect.addEventListener("focus", () => { if (!this.targets.length) this.post("control-studio.binding.list"); });
  }
  get canRun(): boolean { return !this.pending && !this.invalid; }
  /** Why the mapping cannot run yet, as the notice shows it; undefined when it can. */
  get blockReason(): string | undefined { return this.canRun ? undefined : this.problem || "Wait for the selected controller binding."; }
  get recordingSelection(): RecordingSignalSelection | undefined {
    return this.recording ? { inputs: this.selectedValues(this.inputs), outputs: this.selectedValues(this.outputs) } : undefined;
  }
  useLiveSignals(): void {
    if (!this.recording) return;
    this.recording = undefined; this.recordingStale = true; this.invalid = false;
    const signals = (kind: "inputs" | "outputs") => this.loop.channels?.[kind].map(channel => loopSignal(this.loop, channel.signalId))
      ?? [kind === "inputs" ? this.loop.signals.manipulatedVariable : this.loop.signals.processVariable];
    for (const [select, kind] of [[this.inputs, "inputs"], [this.outputs, "outputs"]] as const) {
      const chosen = signals(kind);
      select.replaceChildren(...chosen.map(signal => new Option(`${signal.symbol} (${displayEngineeringUnit(signal.unit)})`, signal.symbol, true, true)));
      this.populate(select, kind, chosen.map(signal => signal.symbol));
    }
    this.synchronize();
  }
  explain(): void { this.notice("model", this.problem || "Wait for the selected controller binding."); }
  receive(payload: Record<string, unknown>): void {
    if (payload.recordingCatalog && typeof payload.recordingCatalog === "object") {
      const catalog = payload.recordingCatalog as RecordingSignalCatalog;
      this.recording = catalog; this.invalid = false; this.pending = undefined;
      this.recordingStale = payload.status !== "model-ready" && !(payload.status === "restored" && payload.model);
      for (const [select, kind] of [[this.inputs, "inputs"], [this.outputs, "outputs"]] as const) {
        const options = catalog.kind === "legacy" ? catalog.channels.filter(channel => channel.id === (kind === "inputs" ? "appliedMv" : "pv")) : catalog.channels;
        select.replaceChildren(...options.map(channel => new Option(
          `${catalog.kind === "legacy" ? channel.id === "appliedMv" ? "Applied input" : "Measured output" : channel.id} (${displayEngineeringUnit(channel.unit)})`,
          channel.id, false, catalog.selection[kind].includes(channel.id))));
      }
    }
    if (payload.status === "model-ready") this.recordingStale = false;
    if (payload.status === "bindings" && Array.isArray(payload.targets)) {
      this.targets = payload.targets as ControlStudioBindingTargetSummary[];
      const target = this.targets.find(item => item.loopId === this.loop.id);
      if (target?.controllerCompatibility.kind === "accepted") this.controller.receive({ availableFamilies: target.controllerCompatibility.families });
      const selectedInputs = this.loop.channels?.inputs.map(item => loopSignal(this.loop, item.signalId).symbol) ?? [this.loop.signals.manipulatedVariable.symbol];
      const selectedOutputs = this.loop.channels?.outputs.map(item => loopSignal(this.loop, item.signalId).symbol) ?? [this.loop.signals.processVariable.symbol];
      if (!this.recording) { this.populate(this.inputs, "inputs", selectedInputs); this.populate(this.outputs, "outputs", selectedOutputs); }
    } else if (payload.status === "failed") {
      const selecting = Boolean(this.pending); this.pending = undefined;
      if (selecting || !this.targets.length && payload.page) this.notice(payload.page === "tune" ? "tune" : "model", String(payload.reason ?? "The controller binding is unavailable."));
    }
    this.synchronize();
  }
  synchronize(): void {
    this.refreshGate(this.canRun, this.blockReason);
    const current = this.controllerSelect.value;
    const matches = this.mappingMatches();
    const currentTarget = matches.find(item => item.loopId === this.loop.id);
    for (const option of Array.from(this.controllerSelect.querySelectorAll("[data-binding-loop-id]"))) option.remove();
    for (const target of matches) {
      if (target.loopId === this.loop.id || target.controllerCompatibility.kind !== "accepted") continue;
      for (const family of target.controllerCompatibility.families) {
        const option = new Option(`${controllerInstanceLabel(target.controller)} · ${family}`, `${target.loopId}::${family}`);
        option.dataset.bindingLoopId = target.loopId; option.dataset.family = family; this.controllerSelect.append(option);
      }
    }
    const ambiguous = !currentTarget && matches.length > 1;
    const placeholder = this.controllerSelect.querySelector<HTMLOptionElement>("[data-binding-placeholder]");
    if (ambiguous && !placeholder) {
      const option = new Option("Select controller", "", true, true);
      option.dataset.bindingPlaceholder = "true";
      this.controllerSelect.prepend(option);
    } else if (!ambiguous && placeholder) placeholder.remove();
    if (this.pending) this.controllerSelect.value = this.pending;
    else if (ambiguous) this.controllerSelect.value = "";
    else if (Array.from(this.controllerSelect.options).some(option => option.value === current)) this.controllerSelect.value = current;
    if (!this.canRun) for (const button of Array.from(this.root.querySelectorAll<HTMLButtonElement>("[data-engineer-find],[data-engineer-tune],[data-engineer-verify],[data-identify-import]"))) button.disabled = true;
    if (this.pending) this.controllerSelect.disabled = true;
    this.root.dataset.bindingDraft = String(this.invalid || this.recordingStale);
  }
  private populate(select: HTMLSelectElement, kind: "inputs" | "outputs", chosen: readonly string[]): void {
    const options = new Map<string, HTMLOptionElement>();
    for (const option of Array.from(select.options)) options.set(option.value, option);
    for (const signal of this.targets.flatMap(target => [...target[kind]])) {
      options.set(signal.symbol, new Option(`${signal.symbol} (${displayEngineeringUnit(signal.unit)})`, signal.symbol, false, chosen.includes(signal.symbol)));
    }
    select.replaceChildren(...options.values());
  }
  private signalsChanged(): void {
    if (this.recording) {
      if (JSON.stringify(this.recordingSelection) === JSON.stringify(this.recording.selection)) return;
      this.recordingStale = true;
      this.synchronize();
      this.recordingChanged(this.recordingSelection!);
      return;
    }
    const matches = this.mappingMatches();
    const target = matches.find(item => item.loopId === this.loop.id) ?? (matches.length === 1 ? matches[0] : undefined);
    this.invalid = !target;
    this.pending = undefined;
    this.problem = matches.length > 1 ? "Select a controller for this plant mapping on Tune." : "Choose the complete input and output mapping of a configured loop.";
    if (!target) {
      this.notice("model", this.problem);
      this.post("control-studio.engineer.invalidate", { bindingInvalid: true,
        bindingInputs: this.selectedValues(this.inputs), bindingOutputs: this.selectedValues(this.outputs) });
    }
    else if (target.loopId !== this.loop.id) this.choose(target, undefined, "model");
    else { this.problem = ""; this.notice("model", ""); }
    this.synchronize();
  }
  private mappingMatches(): readonly ControlStudioBindingTargetSummary[] {
    const inputs = this.selectedValues(this.inputs);
    const outputs = this.selectedValues(this.outputs);
    return this.targets.filter(target => same(inputs, target.inputs.map(signal => signal.symbol))
      && same(outputs, target.outputs.map(signal => signal.symbol)));
  }
  private selectedValues(select: HTMLSelectElement): readonly string[] {
    return Array.from(select.selectedOptions).map(option => option.value);
  }
  private choose(target: ControlStudioBindingTargetSummary, family: string | undefined, page: "model" | "tune"): void {
    this.pending = `${target.loopId}::${family}`;
    this.post("control-studio.binding.select", { selection: { loopId: target.loopId, controllerInstance: target.controller.instance,
      inputIds: target.inputs.map(signal => signal.id), outputIds: target.outputs.map(signal => signal.id) }, ...(family ? { family } : {}), page });
    this.synchronize();
  }
}
function same(left: readonly string[], right: readonly string[]): boolean { return left.length === right.length && left.every(item => right.includes(item)); }
