import { pidOwnMeasurement } from "../pidMeasurement";

/** Compiled facts that Control Studio setup reads (Specification 35 §17.1). Nothing here guesses from names. */

/** The compiled facts Control Studio setup reads (Specification 35 §17.1): symbols, instances and call links. */
export interface CompiledSymbol { symbol: string; type: string; address?: string }
export interface CompiledController { instance: string; type: string; tasks: readonly { name: string; intervalMs: number }[] }
export interface ControllerCallLink {
  instance: string; method: string; argument: string; record: string; output: string | null;
  fields: Readonly<Record<string, readonly string[]>>;
  /** The BOOL of an enclosing `IF NOT <flag> THEN`: the call runs only until its startup is done. */
  guardedBy?: string;
  /** The BOOL an `IF` tests to set the record's `Mode`, and the mode it chooses while TRUE. */
  mode?: { symbol: string; whenTrue: "Auto" | "Manual" };
  /** Variables the code assigns the call's output unchanged, such as a manual output that follows it. */
  copiesOfOutput?: readonly string[];
}
export interface CompiledCatalog {
  symbols: readonly CompiledSymbol[];
  controllers: readonly CompiledController[];
  links?: readonly ControllerCallLink[];
  /** Every symbol the program's code assigns, as absolute paths. */
  writes?: readonly string[];
}

/** What the compiled program already says about one library PID; every entry is optional evidence. */
export interface PidCandidate {
  instance: string;
  task?: { name: string; intervalMs: number };
  parameterRecord?: string;
  output?: string;
  setpoint?: string;
  processValue?: string;
  manualOutput?: string;
  /** The measurement the PID itself holds, the value its last Update received (`<instance>.StateValue.ProcessValue`). */
  ownMeasurement?: string;
  /** The Auto/Manual signal the code tests to choose the PID's mode. */
  mode?: { symbol: string; whenTrue: "Auto" | "Manual" };
  /** The flag the code tests in `IF NOT <flag>` around the PID's Initialize. */
  startupFlag?: string;
  /** The code copies the PID's output into its manual output, so Manual starts where Auto left off. */
  manualOutputFollowsOutput?: boolean;
}

export const PID_TYPE = "CONTROL.PID";
export const PID_PARAMETERS = ["ProportionalGain", "IntegralGain", "DerivativeGain", "ProportionalSetpointWeight", "DerivativeSetpointWeight",
  "Bias", "OutputMinimum", "OutputMaximum", "RateMinimum", "RateMaximum", "TrackingTime"] as const;

/** Every compiled library PID with the evidence its own calls provide. Nothing is inferred from names. */
export function pidCandidates(catalog: CompiledCatalog): PidCandidate[] {
  return catalog.controllers.filter(controller => controller.type.toUpperCase() === PID_TYPE).map(controller => {
    const calls = (catalog.links ?? []).filter(link => link.instance === controller.instance);
    const call = (method: string, argument: string) => calls.find(link =>
      link.method.toUpperCase() === method && link.argument.toUpperCase() === argument);
    const update = call("UPDATE", "INPUT");
    const only = (field: string) => { const sources = update?.fields[field]; return sources?.length === 1 ? sources[0] : undefined; };
    const candidate: PidCandidate = { instance: controller.instance };
    if (controller.tasks.length === 1) candidate.task = { ...controller.tasks[0] };
    const configure = call("CONFIGURE", "PARAMETERS"), record = configure?.record;
    if (record) candidate.parameterRecord = record;
    if (update?.output) candidate.output = update.output;
    for (const [key, field] of [["setpoint", "Setpoint"], ["processValue", "ProcessValue"], ["manualOutput", "ManualOutput"]] as const) {
      const source = only(field);
      if (source) candidate[key] = source;
    }
    const own = pidOwnMeasurement(controller.instance);
    if (catalog.symbols.some(entry => entry.symbol === own)) candidate.ownMeasurement = own;
    if (update?.mode) candidate.mode = { ...update.mode };
    const startup = calls.find(link => link.method.toUpperCase() === "INITIALIZE" && link.guardedBy)?.guardedBy;
    if (startup) candidate.startupFlag = startup;
    if (candidate.manualOutput && update?.copiesOfOutput?.includes(candidate.manualOutput)) candidate.manualOutputFollowsOutput = true;
    return candidate;
  });
}

/** Records that hold every numeric Control.PID parameter, so that one can be bound as the loop's parameter record. A PID's own
 * copy (`<instance>.ParametersValue`) is not one when the code configures that PID from a record of its own: Configure would
 * overwrite whatever Apply wrote there on the next scan. */
export function pidParameterRecords(catalog: CompiledCatalog): string[] {
  const internal = new Set((catalog.links ?? []).filter(link => link.method.toUpperCase() === "CONFIGURE"
    && link.record.toUpperCase() !== `${link.instance}.ParametersValue`.toUpperCase())
    .map(link => `${link.instance}.ParametersValue`.toUpperCase()));
  const fields = new Map<string, Set<string>>();
  for (const entry of catalog.symbols) {
    const cut = entry.symbol.lastIndexOf(".");
    if (cut <= 0 || (entry.type !== "REAL" && entry.type !== "LREAL")) continue;
    const record = entry.symbol.slice(0, cut), field = entry.symbol.slice(cut + 1);
    if ((PID_PARAMETERS as readonly string[]).includes(field)) (fields.get(record) ?? fields.set(record, new Set()).get(record)!).add(field);
  }
  return [...fields].filter(([record, present]) => present.size === PID_PARAMETERS.length && !internal.has(record.toUpperCase()))
    .map(([record]) => record).sort();
}

/** Pairs in which one PID's output (or call) is the only source of another PID's setpoint. */
export function cascadePairs(candidates: readonly PidCandidate[]): { outer: string; inner: string }[] {
  return candidates.flatMap(inner => candidates
    .filter(outer => outer !== inner && inner.setpoint !== undefined
      && (inner.setpoint === outer.output || inner.setpoint === outer.instance))
    .map(outer => ({ outer: outer.instance, inner: inner.instance })));
}


/** Reads the language server's `controlSymbols` payload; malformed entries are dropped, never repaired. */
export function decodeCompiledCatalog(value: unknown): CompiledCatalog | undefined {
  const entry = (item: unknown): item is Record<string, unknown> => !!item && typeof item === "object" && !Array.isArray(item);
  const text = (item: unknown): item is string => typeof item === "string" && item.length > 0;
  if (!entry(value) || !Array.isArray(value.symbols) || !Array.isArray(value.controllers)) return undefined;
  const symbols = value.symbols.filter(entry).filter(item => text(item.symbol) && text(item.type))
    .map(item => ({ symbol: item.symbol as string, type: item.type as string, ...(text(item.address) ? { address: item.address } : {}) }));
  const controllers = value.controllers.filter(entry).filter(item => text(item.instance) && text(item.type) && Array.isArray(item.tasks))
    .map(item => ({ instance: item.instance as string, type: item.type as string,
      tasks: (item.tasks as unknown[]).filter(entry).filter(task => text(task.name) && typeof task.intervalMs === "number")
        .map(task => ({ name: task.name as string, intervalMs: task.intervalMs as number })) }));
  const mode = (item: unknown) => entry(item) && text(item.symbol) && (item.whenTrue === "Auto" || item.whenTrue === "Manual")
    ? { symbol: item.symbol, whenTrue: item.whenTrue as "Auto" | "Manual" } : undefined;
  const links = (Array.isArray(value.links) ? value.links : []).filter(entry)
    .filter(item => text(item.instance) && text(item.method) && text(item.argument) && text(item.record)
      && (item.output === null || text(item.output)) && entry(item.fields))
    .map(item => {
      const link: ControllerCallLink = { instance: item.instance as string, method: item.method as string, argument: item.argument as string,
        record: item.record as string, output: item.output as string | null,
        fields: Object.fromEntries(Object.entries(item.fields as Record<string, unknown>)
          .filter((field): field is [string, string[]] => Array.isArray(field[1]) && field[1].every(text))) };
      const chosen = mode(item.mode);
      const copies = Array.isArray(item.copiesOfOutput) ? item.copiesOfOutput.filter(text) : [];
      return { ...link, ...(text(item.guardedBy) ? { guardedBy: item.guardedBy } : {}), ...(chosen ? { mode: chosen } : {}),
        ...(copies.length ? { copiesOfOutput: copies } : {}) };
    });
  const writes = [...new Set((Array.isArray(value.writes) ? value.writes : []).filter(text))].sort();
  return { symbols, controllers, links, writes };
}
