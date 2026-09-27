import type { ControlLoopDefinition } from "./loopDefinition";
import type { GeneralModel } from "./generalModel";
import { engineerVerificationPlan, type EngineerRequirements } from "./engineerVerificationPlan";

/** Family behavior is exercised by actual ST, with frozen independent controller states. */
export function engineerControllerScenarios(loop: ControlLoopDefinition, model: GeneralModel, task: EngineerRequirements,
  values: Readonly<Record<string, number>>) {
  const cases: Array<{ id: string } & ReturnType<typeof engineerVerificationPlan>> = [];
  if (loop.controller.onOff && loop.signals.setpoint) {
    const switching = loop.controller.onOff, step = loop.controller.taskPeriodMs;
    const dwell = Math.max(values[switching.minimumOnTimeSymbol], values[switching.minimumOffTimeSymbol], step / 1000);
    const quarter = Math.ceil(Math.max(task.durationSeconds / 4, 2 * dwell) * 1000 / step) * step;
    const durationSeconds = 4 * quarter / 1000;
    const plan = engineerVerificationPlan(loop, model, { ...task, durationSeconds,
      settlingTimeSeconds: Math.min(task.settlingTimeSeconds, durationSeconds / 2) }, values);
    const reference = loop.signals.setpoint;
    const low = Math.max(reference.range.minimum, loop.signals.processVariable.range.minimum);
    const high = Math.min(reference.range.maximum, loop.signals.processVariable.range.maximum);
    const width = values[switching.hysteresisSymbol] * (switching.bandConvention === "full-band" ? 0.5 : 1);
    if (high - low <= 2 * width) throw new Error("The reference range cannot exercise both switching thresholds; review scenario requirements.");
    cases.push({ id: "switching-cycle", ...plan, request: { ...plan.request,
      initial_values: { ...plan.request.initial_values, [reference.symbol]: high },
      events: [1, 2, 3].map(index => ({ at_ms: quarter * index, symbol: reference.symbol, value: index % 2 ? low : high })),
    }, requirements: plan.requirements.filter(requirement => !["settling", "error"].includes(requirement.kind)) });
  }
  const state = loop.controller.stateFeedback;
  if (state?.family === "LQG" && state.initialization && state.estimatorStateSymbols) {
    const plan = engineerVerificationPlan(loop, model, task, values);
    const initial_values = { ...plan.request.initial_values };
    for (const [index, symbol] of state.estimatorStateSymbols.entries()) {
      const member = state.initialization.states.find(state => state.symbol === symbol)!;
      const original = initial_values[symbol];
      const modelState = model.kind === "transfer-function" ? undefined : model.states[index];
      const span = modelState ? modelState.operatingRange.maximum - modelState.operatingRange.minimum : member.maximum - member.minimum;
      const delta = Math.min(span * 0.01, (member.maximum - member.minimum) * 0.1);
      const proposed = original + delta <= member.maximum ? original + delta : original - delta;
      const value = member.iec_type === "REAL" ? Math.fround(proposed) : proposed;
      if (!Number.isFinite(value) || value < member.minimum || value > member.maximum || value === original) throw new Error("Declare an observable estimator initialization perturbation within the state bounds.");
      initial_values[symbol] = value;
    }
    cases.push({ id: "estimator-recovery", ...plan, request: { ...plan.request, initial_values } });
  }
  if (state && state.family !== "LQG" && model.kind !== "transfer-function" && model.states.length > 0) {
    const configuredState = loop.channels?.initialState ?? Array<number>(model.states.length).fill(0);
    if (configuredState.length !== model.states.length) throw new Error("State-feedback recovery needs one configured initial value per model state.");
    const perturbedState = model.states.map((stateModel, index) => {
      const span = stateModel.operatingRange.maximum - stateModel.operatingRange.minimum;
      if (!(span > 0) || !Number.isFinite(span)) throw new Error("State-feedback recovery requires finite nonzero model-state ranges.");
      const delta = span * 0.01;
      const initial = configuredState[index];
      const direction = initial + delta <= stateModel.operatingRange.maximum && initial + delta >= stateModel.operatingRange.minimum ? 1
        : initial - delta >= stateModel.operatingRange.minimum && initial - delta <= stateModel.operatingRange.maximum ? -1 : 0;
      if (direction === 0) throw new Error("State-feedback recovery cannot perturb a state inside its declared range.");
      return initial + direction * delta;
    });
    cases.push({ id: "state-recovery", ...engineerVerificationPlan(loop, model, task, values, perturbedState) });
  }
  return cases;
}
