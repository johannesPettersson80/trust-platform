import type { LoopDefinition, SideSignal } from "../generated/OpenResult";
import type { MappingAnswer, MappingOption } from "../generated/SetupDiscoverResult";
import type { RecordingSignalSelection } from "../generated/InputsResult";
import { displayEngineeringUnit } from "../unitDisplay";
import { OPENED_OFFERS_EVENT, openedOffers } from "./openedOffers";

/** The post that sends a message to the engine route. */
type Post = (kind: string, payload?: Record<string, unknown>) => void;

/** A binding target's signal as the page lists it: its identity, its symbol and its unit. */
interface BindingSignalView {
  id: string;
  symbol: string;
  unit: string;
}

/** A loop of the project as the binding answer lists it: the signals of its two sides (B1.2). */
interface BindingTargetView {
  inputs: BindingSignalView[];
  outputs: BindingSignalView[];
}

/** One channel of a recording as the engine lists it: its name, its unit and, for an older file, the words it is shown with. */
interface CatalogChannel {
  id: string;
  unit: string;
  label?: string;
}

/**
 * The channels of a recording and the selection it names (the opened recording's catalog, B3.6). The engine gives each selector its
 * channels: `inputs` for the plant inputs and `outputs` for the measured outputs, filtered by the loop's sides.
 */
interface RecordingSignalCatalog {
  channels: CatalogChannel[];
  inputs: CatalogChannel[];
  outputs: CatalogChannel[];
  selection: { inputs: string[]; outputs: string[] };
}

/** The value of a controller choice in the controller selector: its loop and its family. */
function choiceValue(choice: { loopId: string; family: string }): string {
  return `${choice.loopId}::${choice.family}`;
}

/**
 * The binding selectors (B1.2, B3.6). The engine answers which loop a plant mapping names, the controller choices it offers and whether a
 * recording's selection changed; the page shows that answer and sends the loop choice it names. The selectors list the signals that the
 * engine's binding answer and the open answer give, and nothing here decides a match.
 */
export class ThreePageBindings {
  private targets: readonly BindingTargetView[] = [];
  private mapping: MappingAnswer | undefined;
  private pending: string | undefined;
  private invalid = false;
  private problem = "";
  private recording: RecordingSignalCatalog | undefined;
  private recordingStale = false;
  private readonly controllerSelect: HTMLSelectElement;
  private readonly inputs: HTMLSelectElement;
  private readonly outputs: HTMLSelectElement;
  constructor(private readonly root: HTMLElement, private readonly loop: LoopDefinition,
    private readonly post: Post,
    private readonly notice: (page: "model" | "tune", message: string) => void,
    private readonly refreshGate: (valid: boolean, reason?: string) => void = () => undefined,
    private readonly recordingChanged: (selection: RecordingSignalSelection) => void = () => undefined) {
    this.controllerSelect = root.querySelector<HTMLSelectElement>("[data-studio-controller]")!;
    this.inputs = root.querySelector<HTMLSelectElement>("[data-studio-inputs]")!;
    this.outputs = root.querySelector<HTMLSelectElement>("[data-studio-outputs]")!;
    this.controllerSelect.addEventListener("change", event => {
      const option = this.controllerSelect.selectedOptions[0];
      const choice = this.mapping?.options.find(item => choiceValue(item) === option?.value);
      if (!choice) return;
      event.stopImmediatePropagation();
      this.choose(choice.selection, choice.family, "tune");
    }, true);
    this.inputs.addEventListener("change", () => this.signalsChanged());
    this.outputs.addEventListener("change", () => this.signalsChanged());
    this.controllerSelect.addEventListener("focus", () => { if (!this.targets.length) this.post("control-studio.binding.list"); });
    // The live sides come with the open answer, which may arrive after this page is built (B3.6).
    root.ownerDocument.defaultView?.addEventListener(OPENED_OFFERS_EVENT, () => { if (!this.recording && openedOffers().liveSides) this.replaceLiveSides(); });
  }
  get canRun(): boolean { return !this.pending && !this.invalid; }
  /** Why the mapping cannot run yet, as the notice shows it; undefined when it can. */
  get blockReason(): string | undefined { return this.canRun ? undefined : this.problem || "Wait for the selected controller binding."; }
  get recordingSelection(): RecordingSignalSelection | undefined {
    return this.recording ? { inputs: [...this.selectedValues(this.inputs)], outputs: [...this.selectedValues(this.outputs)] } : undefined;
  }
  useLiveSignals(): void {
    if (!this.recording) return;
    this.recording = undefined; this.recordingStale = true; this.invalid = false;
    this.replaceLiveSides();
    this.synchronize();
  }
  explain(): void { this.notice("model", this.problem || "Wait for the selected controller binding."); }
  /**
   * Reopening shows the recording selection the study keeps (B3.6, A8.6): the channels it names are the selectors' options, selected.
   * The study keeps only their names, so a channel the loop does not name shows its name without a unit.
   */
  restoreRecording(selection: RecordingSignalSelection): void {
    const names = [...new Set([...selection.inputs, ...selection.outputs])];
    const channel = (id: string) => ({ id, unit: this.unitOf(id) });
    const catalog: RecordingSignalCatalog = { channels: names.map(channel), inputs: selection.inputs.map(channel), outputs: selection.outputs.map(channel),
      selection: { inputs: [...selection.inputs], outputs: [...selection.outputs] } };
    this.recording = catalog;
    this.recordingStale = false;
    this.fillRecording(catalog);
    this.synchronize();
  }
  receive(payload: Record<string, unknown>): void {
    if (payload.recordingCatalog && typeof payload.recordingCatalog === "object") {
      const catalog = payload.recordingCatalog as RecordingSignalCatalog;
      this.recording = catalog; this.invalid = false; this.pending = undefined;
      // A checked file keeps the selection it names: that selection is the study's, not a draft (`checked`, B3.6); the engine's other
      // answer for a kept file, `recording-needed`, leaves the results hidden until the file is checked.
      this.recordingStale = payload.status !== "checked";
      this.fillRecording(catalog);
    }
    if (payload.status === "bindings" && Array.isArray(payload.targets)) {
      // The targets list the signals each loop offers; the controller's families and settings come from the open answer (threePageController.ts).
      // The first answer selects the live sides; a later one keeps the engineer's selection and only adds the signals it lists.
      const first = this.targets.length === 0;
      this.targets = payload.targets as BindingTargetView[];
      if (!this.recording) {
        this.populate(this.inputs, "inputs", first ? this.liveSymbols("inputs") : this.selectedValues(this.inputs));
        this.populate(this.outputs, "outputs", first ? this.liveSymbols("outputs") : this.selectedValues(this.outputs));
      }
      if (payload.mapping && typeof payload.mapping === "object") this.applyMapping(payload.mapping as MappingAnswer);
    } else if (payload.status === "failed") {
      const selecting = Boolean(this.pending); this.pending = undefined;
      if (selecting || !this.targets.length && payload.page) this.notice(payload.page === "tune" ? "tune" : "model", String(payload.reason ?? "The controller binding is unavailable."));
    }
    this.synchronize();
  }
  synchronize(): void {
    this.refreshGate(this.canRun, this.blockReason);
    const current = this.controllerSelect.value;
    for (const option of Array.from(this.controllerSelect.querySelectorAll("[data-binding-loop-id]"))) option.remove();
    for (const choice of this.mapping?.options ?? []) {
      const option = new Option(choice.label, choiceValue(choice));
      option.dataset.bindingLoopId = choice.loopId; option.dataset.family = choice.family; this.controllerSelect.append(option);
    }
    const ambiguous = this.mapping?.outcome === "ambiguous";
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
  /** The selectors' options for a recording: its channels, each selected when the recording's selection names it. */
  private fillRecording(catalog: RecordingSignalCatalog): void {
    // Each selector offers the channels the engine gives its side (B3.6): an input is never a measured output, and an extra output the loop
    // does not bind stays selectable. A legacy file's channels are its applied input and its measured output.
    for (const [select, kind] of [[this.inputs, "inputs"], [this.outputs, "outputs"]] as const) {
      select.replaceChildren(...catalog[kind].map(channel => new Option(channelLabel(channel), channel.id, false, catalog.selection[kind].includes(channel.id))));
    }
  }
  /** The live sides the engine answered on open (B3.6), each offered by its selector and selected. */
  private replaceLiveSides(): void {
    for (const [select, kind] of [[this.inputs, "inputs"], [this.outputs, "outputs"]] as const) {
      const chosen = this.liveSides(kind);
      select.replaceChildren(...chosen.map(signal => new Option(`${signal.symbol} (${displayEngineeringUnit(signal.unit)})`, signal.symbol, true, true)));
      this.populate(select, kind, chosen.map(signal => signal.symbol));
    }
  }
  /** The plant signals of one live side, as the open answer names them (B3.6). */
  private liveSides(kind: "inputs" | "outputs"): readonly SideSignal[] {
    return openedOffers().liveSides?.[kind] ?? [];
  }
  private liveSymbols(kind: "inputs" | "outputs"): string[] {
    return this.liveSides(kind).map(signal => signal.symbol);
  }

  /** The unit of a signal that a recording names, from the live sides the open answer gives, or "" when none offers it. */
  private unitOf(name: string): string {
    const offered = [...this.liveSides("inputs"), ...this.liveSides("outputs")];
    return offered.find(signal => signal.symbol === name)?.unit ?? "";
  }
  private populate(select: HTMLSelectElement, kind: "inputs" | "outputs", chosen: readonly string[]): void {
    const options = new Map<string, HTMLOptionElement>();
    for (const option of Array.from(select.options)) options.set(option.value, option);
    for (const signal of this.targets.flatMap(target => [...target[kind]])) {
      options.set(signal.symbol, new Option(`${signal.symbol} (${displayEngineeringUnit(signal.unit)})`, signal.symbol, false, chosen.includes(signal.symbol)));
    }
    select.replaceChildren(...options.values());
  }
  /**
   * The selectors changed: the engine answers the mapping that they now name (B1.2, B3.6). While a recording is shown, the request also names
   * the selection that the recording holds (its catalog's), so that the engine can say whether the selectors changed it; the selectors' own
   * values would never differ from themselves.
   */
  private signalsChanged(): void {
    const mapping: Record<string, unknown> = { loopId: this.loop.id, inputs: this.selectedValues(this.inputs), outputs: this.selectedValues(this.outputs) };
    if (this.recording) mapping.recordingSelection = { inputs: [...this.recording.selection.inputs], outputs: [...this.recording.selection.outputs] };
    this.post("control-studio.binding.list", { mapping });
  }
  /**
   * The engine's answer to the mapping (B1.2, B3.6): a recording's change refits it; a loop the engine names is chosen; a mapping the
   * engine says blocks the work shows the engine's sentence and invalidates the model; the loop the panel shows needs nothing. The
   * page reads `blocksRun`, `changed` and `selection`. It routes an answer whose outcome is `recorded` to the recording's refit and
   * offers the Controller selector's placeholder for an `ambiguous` one (`synchronize`); it matches no loop itself.
   */
  private applyMapping(answer: MappingAnswer): void {
    if (answer.outcome === "recorded") {
      const selection = this.recordingSelection;
      if (!answer.changed || !selection) return;
      this.recordingStale = true;
      this.synchronize();
      this.recordingChanged(selection);
      return;
    }
    this.mapping = answer;
    this.invalid = answer.blocksRun === true;
    this.pending = undefined;
    // The sentence this page showed for the mapping before this answer: it is cleared once the mapping is no problem, and any other
    // message of the model page stays, as when the engine's first answer offers the choices of a loop that has no problem (B3.6).
    const shown = this.problem;
    this.problem = answer.sentence;
    if (this.invalid) {
      this.notice("model", answer.sentence);
      this.post("control-studio.engineer.invalidate", { bindingInvalid: true,
        bindingInputs: this.selectedValues(this.inputs), bindingOutputs: this.selectedValues(this.outputs) });
    } else if (answer.selection) this.choose(answer.selection, undefined, "model");
    else { this.problem = ""; if (shown) this.notice("model", ""); }
    this.synchronize();
  }
  private selectedValues(select: HTMLSelectElement): readonly string[] {
    return Array.from(select.selectedOptions).map(option => option.value);
  }
  /** Sends the loop the engine named for a choice (B1.3): its selection, and the family the engineer chose with it. */
  private choose(selection: MappingOption["selection"], family: string | undefined, page: "model" | "tune"): void {
    this.pending = `${selection.loopId}::${family ?? ""}`;
    this.post("control-studio.binding.select", { selection: { ...selection }, ...(family ? { family } : {}), page });
    this.synchronize();
  }
}

/** A channel's option text: the words the engine gives it (an older file's applied input and measured output) or its name, then its unit. */
function channelLabel(channel: CatalogChannel): string {
  const name = channel.label ?? channel.id;
  return channel.unit ? `${name} (${displayEngineeringUnit(channel.unit)})` : name;
}
