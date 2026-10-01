import { cascadePairs, PID_TYPE, pidCandidates, pidParameterRecords, type CompiledCatalog, type CompiledSymbol, type PidCandidate } from "./compiledCatalog";

export interface SetupFormModel {
  structure: "pid" | "cascade" | "group" | "identify";
  pids: PidCandidate[];
  cascade?: { outer: string; inner: string };
  /** REAL/LREAL signals with an I/O address: Studio's identification writes the manual output through the process image. */
  addressedReal: string[];
  /** Signals a loop can use as a controller output or setpoint, addressed or not (Specification 35 §17.1): every REAL/LREAL
   * except the library PIDs' own parameter records and internal states. */
  signals?: string[];
  /** Signals a simulated plant can write as a measurement (Specification 33 §13.2): inputs, and program variables
   * without an address that the program only reads. */
  measurements?: string[];
  booleans: string[];
  /** Model files in the project an LQG estimator can implement: state space, one input, one output (Specification 35 §17.1). */
  estimatorModels?: { file: string; states: number }[];
  /** BOOLs without an I/O address: the controller transfer switches only such an Auto/Manual signal. */
  modes?: string[];
  records: string[];
  /** Program and function block instances other than library PIDs that run in one cyclic task: switching logic lives in one of them. */
  instances: string[];
  /** Every REAL/LREAL symbol: Studio writes controller settings directly, so they need no I/O address. */
  reals: string[];
  /** Symbols the program assigns and PID instances it initializes itself. Validation seeds the program's state; code that
   * writes a seeded value, for example in a startup branch, needs the flag that marks its startup as done. */
  writes: string[];
  initializes: string[];
  /** What each PID's own calls say, for the controller the user chooses; `prefill` is the only PID's, preselected. */
  prefills: PidPrefill[];
  prefill?: PidPrefill;
}
export interface PidPrefill {
  instance: string; parameterRecord?: string; manipulatedVariable?: string; setpoint?: string; processValue?: string; manualOutput?: string;
  /** A manual output the controller's calls use that has no address: identification writes it through the process image. */
  unaddressed?: Partial<Record<"manualOutput", string>>;
  /** A measurement the controller's calls use that the program computes, which a simulated measurement could not reach. */
  computed?: Partial<Record<"processValue", string>>;
}

/** What the setup form offers: the structure the compiled program shows and only choices a loop can use. */
export function setupFormModel(catalog: CompiledCatalog, estimatorModels?: { file: string; states: number }[]): SetupFormModel {
  const pids = pidCandidates(catalog), pairs = cascadePairs(pids), records = pidParameterRecords(catalog);
  const real = (entry: CompiledSymbol) => entry.type === "REAL" || entry.type === "LREAL";
  const addressedReal = catalog.symbols.filter(entry => real(entry) && entry.address).map(entry => entry.symbol).sort();
  const booleans = catalog.symbols.filter(entry => entry.type === "BOOL").map(entry => entry.symbol).sort();
  const modes = catalog.symbols.filter(entry => entry.type === "BOOL" && !entry.address).map(entry => entry.symbol).sort();
  const reals = catalog.symbols.filter(real).map(entry => entry.symbol).sort();
  const written = new Set((catalog.writes ?? []).map(symbol => symbol.toUpperCase()));
  const internal = (symbol: string) => [...records, ...pids.map(pid => pid.instance)].some(owner => symbol.startsWith(`${owner}.`));
  const signals = catalog.symbols.filter(entry => real(entry) && !internal(entry.symbol)).map(entry => entry.symbol).sort();
  const measurements = catalog.symbols.filter(entry => real(entry) && !internal(entry.symbol)
    && (entry.address ? entry.address.toUpperCase().startsWith("%I") : !written.has(entry.symbol.toUpperCase()))).map(entry => entry.symbol).sort();
  const instances = catalog.controllers.filter(controller => controller.type.toUpperCase() !== PID_TYPE && controller.tasks.length === 1
    && controller.tasks[0].intervalMs > 0).map(controller => controller.instance).sort();
  const structure = pairs.length ? "cascade" : pids.length > 1 ? "group" : pids.length === 1 ? "pid" : "identify";
  const writes = [...catalog.writes ?? []];
  const initializes = pids.filter(pid => (catalog.links ?? []).some(link => link.instance === pid.instance
    && link.method.toUpperCase() === "INITIALIZE")).map(pid => pid.instance).sort();
  const prefills = pids.map(pid => {
    const prefill: PidPrefill = { instance: pid.instance };
    if (pid.parameterRecord && records.includes(pid.parameterRecord)) prefill.parameterRecord = pid.parameterRecord;
    if (pid.output && signals.includes(pid.output)) prefill.manipulatedVariable = pid.output;
    if (pid.setpoint && signals.includes(pid.setpoint)) prefill.setpoint = pid.setpoint;
    if (pid.processValue && measurements.includes(pid.processValue)) prefill.processValue = pid.processValue;
    else if (pid.processValue && signals.includes(pid.processValue)) prefill.computed = { processValue: pid.processValue };
    if (pid.manualOutput && addressedReal.includes(pid.manualOutput)) prefill.manualOutput = pid.manualOutput;
    else if (pid.manualOutput && signals.includes(pid.manualOutput)) prefill.unaddressed = { manualOutput: pid.manualOutput };
    return prefill;
  });
  const model: SetupFormModel = { structure, pids, addressedReal, signals, measurements, booleans, modes, records, instances, reals, writes, initializes, prefills,
    ...(estimatorModels?.length ? { estimatorModels } : {}) };
  if (pairs.length) model.cascade = pairs[0];
  if (structure === "pid") model.prefill = prefills[0];
  return model;
}

