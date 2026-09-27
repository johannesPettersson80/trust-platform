import { decodePidLaw, materializePidGainRecord } from "./controllerLaw";
import { saturationWindupEvidence, type ControlLoopDefinition } from "./loopDefinition";
import type { RunRequirement } from "./runValidation";
import type { SaturationWindupAssessment } from "./saturationWindup";
import type { RuntimeVerificationRequest } from "./verificationClient";

export interface AntiWindupVerificationScenario {
  readonly stepMs: number;
  readonly durationMs: number;
  readonly seed: number;
  readonly wallBudgetMs: number;
  readonly initialValues: Readonly<Record<string, number>>;
  readonly events: readonly { atMs: number; symbol: string; value: number }[];
}

export interface AntiWindupVerificationPlan {
  readonly baseline: RuntimeVerificationRequest;
  readonly candidate: RuntimeVerificationRequest;
  readonly requirements: readonly RunRequirement[];
}

/** Builds the two actual-ST requests from one frozen simulation scenario. */
export function buildAntiWindupVerificationPlan(input: {
  readonly loop: ControlLoopDefinition;
  readonly baselineValues: Readonly<Record<string, number>>;
  readonly trackingTimeSeconds: number;
  readonly scenario?: AntiWindupVerificationScenario;
  readonly saturationWindup: SaturationWindupAssessment;
}): AntiWindupVerificationPlan {
  if (input.saturationWindup.status !== "qualified") {
    throw new Error("A qualified saturation-return recording is required before comparing anti-windup.");
  }
  if (!Number.isFinite(input.trackingTimeSeconds) || input.trackingTimeSeconds <= 0) {
    throw new Error("Enter a finite positive tracking time before verification.");
  }
  const law = decodePidLaw(input.loop.controller.law, input.loop.parameterBinding);
  if (!law?.antiWindup || !input.loop.parameterBinding) {
    throw new Error("A typed back-calculation anti-windup adapter is required.");
  }
  const evidence = saturationWindupEvidence(input.loop);
  if (!evidence) throw new Error("Declared requested-output and integral-state bindings are required.");
  const requested = input.loop.signals.additional?.find(binding => binding.id === evidence.requestedOutputId);
  const integral = input.loop.signals.additional?.find(binding => binding.id === evidence.integralStateId);
  if (!requested || !integral) throw new Error("Declared saturation evidence bindings are unavailable.");
  const setpoint = input.loop.signals.setpoint;
  if (!setpoint) throw new Error("A declared setpoint binding is required for the saturation-return scenario.");
  const baselineGain = input.baselineValues[law.antiWindup.trackingGainSymbol];
  if (baselineGain !== 0) throw new Error("The matched baseline must have zero back-calculation gain.");
  const candidateValues = materializePidGainRecord(input.loop, {
    proportionalGain: input.baselineValues[law.proportionalGainSymbol],
    integralGain: input.baselineValues[law.integralGainSymbol],
    derivativeGain: input.baselineValues[law.derivativeGainSymbol],
    backCalculationGain: 1 / input.trackingTimeSeconds,
  }, input.baselineValues);
  const scenario = freezeScenario(input.scenario ?? declaredScenario(input.loop));
  const observations = [setpoint, input.loop.signals.processVariable, input.loop.signals.manipulatedVariable,
    requested, integral].map(binding => ({ symbol: binding.symbol, unit: binding.unit }));
  const shared = {
    step_ms: scenario.stepMs,
    duration_ms: scenario.durationMs,
    seed: scenario.seed,
    max_total_cycles: scenario.durationMs / scenario.stepMs + 1,
    wall_budget_ms: scenario.wallBudgetMs,
    initial_values: scenario.initialValues,
    events: scenario.events,
    observations,
  } as const;
  const overrides = (values: Readonly<Record<string, number>>) => input.loop.parameterBinding!.parameters.map(parameter => {
    const value = values[parameter.symbol];
    if (!Number.isFinite(value)) throw new Error(`The baseline parameter ${parameter.symbol} is unavailable.`);
    return { ...parameter, value };
  });
  const requirements: readonly RunRequirement[] = [{
    id: "applied-output-bounds", kind: "bounds", channel: input.loop.signals.manipulatedVariable.symbol,
    window: { start: 0, end: scenario.durationMs / 1000 },
    minimum: input.loop.limits.outputMinimum, maximum: input.loop.limits.outputMaximum,
  }];
  return {
    baseline: { ...shared, parameter_overrides: overrides(input.baselineValues) },
    candidate: { ...shared, parameter_overrides: overrides(candidateValues) },
    requirements,
  };
}

function declaredScenario(loop: ControlLoopDefinition): AntiWindupVerificationScenario {
  const scenario = loop.controller.antiWindupVerification;
  if (!scenario) throw new Error("Declare an immutable anti-windup verification scenario before running the automatic comparison.");
  return {
    stepMs: scenario.stepMs,
    durationMs: scenario.durationMs,
    seed: scenario.seed,
    wallBudgetMs: scenario.wallBudgetMs,
    initialValues: Object.fromEntries(scenario.initialValues.map(value => [value.symbol, value.value])),
    events: scenario.events.map(event => ({ ...event })),
  };
}

function freezeScenario(value: AntiWindupVerificationScenario): {
  stepMs: number; durationMs: number; seed: number; wallBudgetMs: number;
  initialValues: Readonly<Record<string, number>>; events: readonly { at_ms: number; symbol: string; value: number }[];
} {
  if (!Number.isSafeInteger(value.wallBudgetMs) || value.wallBudgetMs <= 0 || value.wallBudgetMs > 10_000) {
    throw new Error("The per-run wall budget must be a finite integer from 1 to 10000 ms.");
  }
  if (!Number.isSafeInteger(value.stepMs) || value.stepMs <= 0 || !Number.isSafeInteger(value.durationMs) ||
      value.durationMs <= 0 || value.durationMs % value.stepMs !== 0 || !Number.isSafeInteger(value.seed) || value.seed < 0) {
    throw new Error("Anti-windup verification needs finite scan-aligned timing and a deterministic seed.");
  }
  const initialValues = Object.freeze({ ...value.initialValues });
  if (Object.keys(initialValues).length === 0 || !Object.values(initialValues).every(Number.isFinite)) {
    throw new Error("Anti-windup verification needs explicit finite initial values.");
  }
  const events = value.events.map(event => ({ at_ms: event.atMs, symbol: event.symbol, value: event.value }));
  if (!events.every(event => Number.isSafeInteger(event.at_ms) && event.at_ms >= 0 && event.at_ms <= value.durationMs &&
      event.at_ms % value.stepMs === 0 && event.symbol.length > 0 && Number.isFinite(event.value))) {
    throw new Error("Anti-windup verification events must be finite and scan-aligned.");
  }
  return { stepMs: value.stepMs, durationMs: value.durationMs, seed: value.seed, wallBudgetMs: value.wallBudgetMs,
    initialValues, events: Object.freeze(events) };
}
