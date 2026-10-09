import type { ParameterBinding } from "./parameterBinding";
import { isSavedFiniteSearch, type SavedFiniteSearch } from "./finiteSearchPersistence";

export interface SearchConnection { source: "simulation" | "physical"; request: (operation: string, params: Record<string, unknown>) => Promise<unknown> }

export class FiniteSearchSession {
  private saved: SavedFiniteSearch | undefined;
  constructor(private readonly connect: () => Promise<SearchConnection>, private readonly draft?: () => Promise<Record<string, unknown>>) {}

  async selectedParameterValues(binding: ParameterBinding): Promise<Record<string, number>> {
    const saved = this.saved;
    const selected = saved?.selectedIndex;
    if (!saved || selected === undefined) throw new Error("Select a qualifying search trial first.");
    const trial = saved.result.trials.find(item => item.index === selected);
    if (!trial || trial.status !== "qualified") throw new Error("The selected trial did not qualify.");
    const axes = saved.request.grid;
    const initial = saved.request.initial_values;
    if (!Array.isArray(axes) || !record(initial)) throw new Error("The search lacks complete parameter declarations.");
    const values: Record<string, number> = {};
    for (const parameter of binding.parameters) {
      const axis = axes.find(value => record(value) && value.symbol === parameter.symbol);
      const value = Object.prototype.hasOwnProperty.call(trial.parameters, parameter.symbol) ? trial.parameters[parameter.symbol] : initial[parameter.symbol];
      if (value === undefined) throw new Error(`The search is missing a fixed or tuned value for ${parameter.symbol}; complete the trial settings first.`);
      if ((axis && (!record(axis) || axis.unit !== parameter.unit)) || typeof value !== "number" || !Number.isFinite(value) ||
          value < parameter.minimum || value > parameter.maximum) throw new Error(`The selected ${parameter.symbol} value or unit is incompatible with this binding.`);
      values[parameter.symbol] = value;
    }
    const connection = await this.connect();
    if (connection.source !== "simulation") throw new Error("The simulation source changed; review the result before transferring parameters.");
    const readiness = await connection.request("control.search.status", {});
    if (this.saved !== saved || !record(readiness) || readiness.ready !== true || readiness.project_revision !== saved.result.project_revision) {
      throw new Error("The loaded ST or simulated plant source changed since this search.");
    }
    return values;
  }

  snapshot(): SavedFiniteSearch | undefined { return this.saved ? JSON.parse(JSON.stringify(this.saved)) : undefined; }
  restore(saved: SavedFiniteSearch | undefined): void {
    if (saved && !isSavedFiniteSearch(saved)) throw new Error("Saved search evidence is invalid.");
    const restored = saved ? JSON.parse(JSON.stringify(saved)) : undefined;
    this.dispose();
    this.saved = restored;
  }
  dispose(): void { this.saved = undefined; }
}
function record(value: unknown): value is Record<string, unknown> { return !!value && typeof value === "object" && !Array.isArray(value); }
