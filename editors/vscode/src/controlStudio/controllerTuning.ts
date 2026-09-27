import { MainWorkflowError } from "./mainWorkflowFailure";
import { manualControllerCandidate } from "./manualControllerCandidate";
import { poleMatchedPid } from "./pidTaskDesign";
import type { ProjectControllerDesign } from "./projectController";
import { designOnOffController, type OnOffControllerDesign } from "./onOffController";
import { designStateController, stateControllerPoles, type StateControllerDesign, type StateControllerFamily } from "./stateControllerDesign";
import type { ControlLoopDefinition } from "./loopDefinition";
import type { GeneralModel } from "./generalModel";
import type { EngineerGainChoice } from "./engineerGainChoice";
import type { EngineerRequirements } from "./engineerVerificationPlan";
import { proportionalParameter } from "./engineerVerificationPlan";
import { controllerAdapter, tuneTypedController, type ControllerFamily } from "./controllerAdapters";
import { recommendAutomaticController, type AutomaticControllerAttempt } from "./automaticControllerRecommendation";
import { decodePidLaw, proportionalLoopAnalysis } from "./controllerLaw";
import { designSimpleProportionalController, type SimpleProportionalDesign } from "./simpleControllerDesign";
import { designManualProportionalCandidate, type ManualProportionalDesign } from "./manualProportionalDesign";
import { designTypedProportionalCandidate, type PidPiDesign } from "./pidPiDesign";
import { selectedModelAnalysis } from "./selectedModelAnalysis";
import { calculateDeclaredRouth } from "./routhHurwitz";
import type { StateDesignPreferences } from "./stateDesignPreferences";

export type ControllerDesign = SimpleProportionalDesign | ManualProportionalDesign | PidPiDesign | StateControllerDesign | OnOffControllerDesign | ProjectControllerDesign;
export interface ControllerTuningResult {
  family: ControllerFamily;
  design: ControllerDesign;
  values: Record<string, number>;
  reason: string;
  attempts?: readonly AutomaticControllerAttempt[];
  stability?: ReturnType<typeof calculateDeclaredRouth>;
  gainChoice?: EngineerGainChoice;
}

/** Family-specific calculation stays behind the same workflow operation. */
export function tuneControllerAdapter(loop: ControlLoopDefinition, model: GeneralModel, task: EngineerRequirements,
  options: { family: string; applicationMode: "auto" | "manual"; gainMode?: unknown; proportionalGain?: unknown; pinnedValues?: Readonly<Record<string, number>>; manualValues?: Readonly<Record<string, number>>; stateDesign?: StateDesignPreferences },
  retainedGain?: EngineerGainChoice): ControllerTuningResult {
  const adapter = controllerAdapter(loop);
  if (options.manualValues !== undefined) return manualControllerCandidate(loop, model, options.family, options.manualValues, options.pinnedValues);
  if (loop.channels?.inputs.some(input => input.actuator.kind === "on-off") && !loop.controller.onOff && !loop.controller.project) {
    throw new MainWorkflowError("ON/OFF-only actuation needs a declared switching ST adapter with hysteresis and dwell; continuous-gain tuning cannot install that structure.", { cause: "controller-capability", nextAction: "choose-controller" });
  }
  if (loop.controller.onOff) {
    if (options.family !== "automatic" && options.family !== "ON/OFF") throw new Error("The bound switching ST controller supports ON/OFF settings only.");
    const initialDesign = designOnOffController(loop, model, task);
    const declared = [loop.controller.onOff.hysteresisSymbol, loop.controller.onOff.minimumOnTimeSymbol, loop.controller.onOff.minimumOffTimeSymbol];
    const pins = options.pinnedValues ?? {};
    for (const symbol of Object.keys(pins)) if (!declared.includes(symbol)) throw new Error(`Pinned switching setting ${symbol} is not declared by the ON/OFF adapter.`);
    const parameters = { ...initialDesign.candidate.parameters, ...Object.fromEntries(declared.filter(symbol => pins[symbol] !== undefined).map(symbol => [symbol, pins[symbol]])) };
    const values = adapter.materialize({ ...initialDesign.candidate, parameters }, {});
    const design = { ...initialDesign, candidate: { ...initialDesign.candidate, parameters: values } };
    return { family: "ON/OFF", design, values,
      reason: "ON/OFF settings respect the declared process band and equipment dwell. Verify actual ST before applying." };
  }
  if (loop.controller.stateFeedback) {
    if (options.family !== "automatic" && !adapter.families.includes(options.family as ControllerFamily)) throw new Error("The selected controller family does not match the bound ST coefficient adapter.");
    const family = options.family === "automatic" ? adapter.families.includes(loop.controller.stateFeedback.family)
      ? loop.controller.stateFeedback.family : adapter.families[0] : options.family;
    const initialDesign = designStateController(loop, model, task.settlingTimeSeconds, family as StateControllerFamily, options.stateDesign);
    const pins = options.pinnedValues ?? {};
    const gainSymbols = loop.controller.stateFeedback.gainSymbols.flat();
    const mappedSymbols = [...gainSymbols, ...loop.controller.stateFeedback.referenceGainSymbols.flat(), ...(loop.controller.stateFeedback.observerGainSymbols?.flat() ?? [])];
    for (const symbol of Object.keys(pins)) if (!mappedSymbols.includes(symbol)) throw new Error(`Pinned coefficient ${symbol} is not declared by the state-controller adapter.`);
    const pinned = Object.fromEntries(mappedSymbols.filter(symbol => pins[symbol] !== undefined).map(symbol => [symbol, pins[symbol]]));
    const design = Object.keys(pinned).length === 0 ? initialDesign : (() => {
      for (const [symbol, value] of Object.entries(pinned)) {
        const parameter = loop.parameterBinding?.parameters.find(item => item.symbol === symbol);
        if (!parameter || !Number.isFinite(value) || value < parameter.minimum || value > parameter.maximum) throw new Error(`Pinned coefficient ${symbol} is outside its declared bounds.`);
      }
      const parameters = { ...initialDesign.candidate.parameters, ...pinned };
      const realized = adapter.materialize({ ...initialDesign.candidate, parameters }, {});
      const poles = stateControllerPoles(loop, model, realized);
      const gainModified = gainSymbols.some(symbol => pinned[symbol] !== undefined);
      return { ...initialDesign, candidate: { ...initialDesign.candidate, parameters: realized }, closedLoopPoles: poles,
        ...(initialDesign.horizon && gainModified ? { horizon: { ...initialDesign.horizon, policy: "pinned-fixed-gain" as const } } : {}) };
    })();
    return { family: design.candidate.structure, design, values: adapter.materialize(design.candidate, {}),
      reason: "State-controller coefficients calculated using the existing design engine. Verify the actual ST; Auto requires its separate state-transfer binding." };
  }
  // Choosing the sole available P adapter automatically does not clear an explicit gain choice.
  if (options.family === "automatic" && options.gainMode === "manual" && adapter.families.length === 1 && adapter.families[0] === "P") {
    options = { ...options, family: "P" };
  }
  if (options.family === "automatic") return recommendAutomaticController(loop, model, task, options.applicationMode);
  if (!adapter.families.includes(options.family as ControllerFamily)) throw new Error("Choose a controller adapter compatible with the actual ST law.");
  if (options.family === "PI" || options.family === "PD" || options.family === "PID") {
    return { family: options.family, ...tuneTypedController(loop, model, options.family, options.applicationMode, task, options.pinnedValues),
      reason: `${options.family} candidate ready. Verify the actual ST and initialization before applying.` };
  }
  const pid = decodePidLaw(loop.controller.law, loop.parameterBinding);
  const analysisLoop = proportionalDesignLoop(loop);
  const parameter = proportionalParameter(analysisLoop);
  const requirements = { referenceChange: task.referenceChange, constantInputDisturbance: task.constantInputDisturbance ?? 0,
    maximumSteadyError: task.maximumError - task.settlingBand, settlingBand: task.settlingBand,
    settlingTimeSeconds: task.settlingTimeSeconds,
    proportionalGainBounds: { minimum: parameter.minimum, maximum: parameter.maximum } };
  const gainMode = options.gainMode ?? "automatic";
  if (gainMode !== "automatic" && gainMode !== "manual") throw new Error("Choose Automatic or Manual gain selection.");
  const requested = options.proportionalGain;
  if (gainMode === "manual" && (typeof requested !== "number" || !Number.isFinite(requested))) throw new Error("Enter a finite manual proportional gain.");
  const realized = parameter.iec_type === "REAL" ? Math.fround(requested as number) : requested as number;
  const design = (() => {
    if (gainMode === "manual") {
      try { return designManualProportionalCandidate(model, realized, requirements); }
      catch { return designTypedProportionalCandidate(model, realized); }
    }
    try { return designSimpleProportionalController(model, requirements); }
    catch (error) {
      // A different formula cannot remove an explicitly established gain limit.
      if (error instanceof Error && /gain bounds|proportional gain is outside/i.test(error.message)) throw error;
      return poleMatchedPid(model, "P", task, loop.controller.taskPeriodMs);
    }
  })();
  const pinned = options.pinnedValues && Object.keys(options.pinnedValues).length
    ? applyProportionalPin(loop, design.candidate.proportionalGain, options.pinnedValues)
    : design.candidate.proportionalGain;
  const realizedDesign = pinned === design.candidate.proportionalGain ? design : withProportionalGain(design, pinned);
  const connected = loop.controller.law ? proportionalLoopAnalysis(analysisLoop, model, realizedDesign.candidate.proportionalGain)
    : selectedModelAnalysis(loop.analysis!, model);
  const stability = calculateDeclaredRouth(connected, realizedDesign.candidate.proportionalGain);
  if (gainMode === "automatic" && stability.polynomial.stable !== true) throw new Error("The connected stability calculation did not establish a stable candidate.");
  const typed = pid ? designTypedProportionalCandidate(model, realizedDesign.candidate.proportionalGain) : realizedDesign;
  const gainChoice: EngineerGainChoice = gainMode === "manual" ? { mode: "manual", pin: { parameterSymbol: parameter.symbol,
    unit: parameter.unit, loopRevision: loop.revision, modelRevision: model.revision, value: realized } }
    : { mode: "automatic", ...(retainedGain?.pin ? { pin: retainedGain.pin } : {}) };
  return { family: "P", design: typed, values: adapter.materialize(typed.candidate, {}), stability, gainChoice,
    reason: gainMode === "manual" ? "Manual exploratory gain retained. Verify the actual ST before applying."
      : "Candidate ready. Verify the actual ST before applying." };
}

function applyProportionalPin(loop: ControlLoopDefinition, proposed: number, pins: Readonly<Record<string, number>>): number {
  const parameter = proportionalParameter(proportionalDesignLoop(loop));
  const entries = Object.entries(pins);
  if (entries.some(([symbol]) => symbol !== parameter.symbol)) throw new Error("Pinned settings contain a parameter outside the selected P adapter.");
  const value = pins[parameter.symbol];
  if (value === undefined || !Number.isFinite(value) || value < parameter.minimum || value > parameter.maximum
    || parameter.iec_type === "REAL" && Math.fround(value) !== value) throw new Error("Pinned proportional gain is outside its declared representable bounds.");
  return value === proposed ? proposed : value;
}

function withProportionalGain(
  design: SimpleProportionalDesign | ManualProportionalDesign | PidPiDesign,
  proportionalGain: number,
): SimpleProportionalDesign | ManualProportionalDesign | PidPiDesign {
  const retained = structuredClone(design);
  retained.candidate.proportionalGain = proportionalGain;
  return retained;
}
export function proportionalDesignLoop(loop: ControlLoopDefinition): ControlLoopDefinition {
  const pid = decodePidLaw(loop.controller.law, loop.parameterBinding);
  return pid ? { ...loop, controller: { ...loop.controller, law: { kind: "proportional", gainSymbol: pid.proportionalGainSymbol } } } : loop;
}
