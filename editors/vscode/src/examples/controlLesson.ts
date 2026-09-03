export type ControlSignalRole = "SP" | "PV" | "MV" | "disturbance" | "state" | "quality";
export type ControlWorkflowStage = "Monitor" | "Test" | "Identify" | "Tune" | "Validate";
export type ControlEvidenceClass = "simulation" | "physical" | "recorded" | "model";
export type ControlPublicationState =
  | "not_started"
  | "scripted"
  | "captured"
  | "edited"
  | "reviewed"
  | "unlisted"
  | "published";

export interface ControlLessonSummary {
  readonly sequence: number;
  readonly level: string;
  readonly equipment: string;
  readonly principalTechnique: string;
  readonly prerequisites: ReadonlyArray<string>;
}

export interface ControlLessonSignal {
  readonly role: ControlSignalRole;
  readonly name: string;
  readonly unit: string;
  readonly minimum: number;
  readonly maximum: number;
  readonly resolution: number;
  readonly binding: string;
}

export interface ControlLessonPlantParameter {
  readonly name: string;
  readonly value: number;
  readonly unit: string;
}

export interface ControlLessonRange {
  readonly signal: string;
  readonly minimum: number;
  readonly maximum: number;
  readonly unit: string;
}

export interface ControlLessonPlant {
  readonly modelId: string;
  readonly executable: string;
  readonly parameters: ReadonlyArray<ControlLessonPlantParameter>;
  readonly sampleTimeSeconds: number;
  readonly initialState: Readonly<Record<string, number>>;
  readonly seed: number;
  readonly operatingPoint: Readonly<Record<string, number>>;
  readonly validityRange: ReadonlyArray<ControlLessonRange>;
  readonly simplifications: ReadonlyArray<string>;
}

export type ControlLessonSetting =
  | string
  | number
  | boolean
  | ReadonlyArray<string | number | boolean>
  | Readonly<Record<string, string | number | boolean>>;

export interface ControlLessonBaseline {
  readonly executableId: string;
  readonly settings: Readonly<Record<string, ControlLessonSetting>>;
  readonly limitation: string;
}

export interface ControlLessonChange {
  readonly executableId: string;
  readonly settings: Readonly<Record<string, ControlLessonSetting>>;
  readonly decision: string;
}

export interface ControlLessonExperiment {
  readonly kind: "observation" | "setpoint_step" | "load_step" | "mv_step" | "mv_pulse" | "relay" | "prbs" | "multisine";
  readonly signal: string;
  readonly baseline: number;
  readonly amplitude: number;
  readonly durationSeconds: number;
  readonly settlingSeconds: number;
  readonly disturbances: ReadonlyArray<string>;
  readonly seed: number;
  readonly restorationValue: number;
}

export interface ControlLessonAcceptance {
  readonly metric: string;
  readonly signal: string;
  readonly window: string;
  readonly comparator: "<" | "<=" | "==" | ">=" | ">";
  readonly limit: number;
  readonly unit: string;
  readonly tolerance: number;
  readonly evidenceClass: ControlEvidenceClass;
}

export interface ControlLessonSafety {
  readonly hardLimits: ReadonlyArray<ControlLessonRange>;
  readonly abortPredicates: ReadonlyArray<string>;
  readonly safeMv: number;
  readonly restorationMode: string;
  readonly physicalApprovalRole: string;
  readonly simulationOnlyRestrictions: ReadonlyArray<string>;
}

export interface ControlLessonProof {
  readonly specification: string;
  readonly nativeTest: string;
  readonly nativeCommand: string;
  readonly projectPath: string;
  readonly runArtifactFields: ReadonlyArray<string>;
  readonly renderedJourneyId: string;
}

export interface ControlLessonVideo {
  readonly viewerOutcome: string;
  readonly requiredVisibleEvidence: ReadonlyArray<string>;
  readonly durationClass: string;
  readonly scriptPath: string;
  readonly captionPath: string;
  readonly thumbnailPath: string;
  readonly publicationState: ControlPublicationState;
}

/**
 * The teaching sequence is data, not an implied README convention. Keeping it
 * ordered here lets the gallery and future documentation present theory before
 * asking a learner to operate the plant.
 */
export interface ControlLessonLearning {
  readonly context: string;
  readonly reason: string;
  readonly theory: ReadonlyArray<string>;
  readonly workedPrediction: string;
  readonly practicalPreparation: ReadonlyArray<string>;
  readonly baseline: string;
  readonly change: string;
  readonly comparison: string;
  readonly validation: string;
  readonly explanation: string;
  readonly transfer: string;
}

export interface ControlLessonV3 extends ControlLessonSummary {
  readonly schemaVersion: 3;
  readonly id: string;
  readonly title: string;
  readonly physicalPurpose: string;
  readonly signals: ReadonlyArray<ControlLessonSignal>;
  readonly plant: ControlLessonPlant;
  readonly baseline: ControlLessonBaseline;
  readonly change: ControlLessonChange;
  readonly experiment: ControlLessonExperiment;
  readonly acceptance: ReadonlyArray<ControlLessonAcceptance>;
  readonly safety: ControlLessonSafety;
  readonly workflowStages: ReadonlyArray<ControlWorkflowStage>;
  readonly proof: ControlLessonProof;
  readonly video: ControlLessonVideo;
}

export interface ControlLessonV4 extends Omit<ControlLessonV3, "schemaVersion"> {
  readonly schemaVersion: 4;
  readonly objective: string;
  readonly nextLesson: string | null;
  readonly learning: ControlLessonLearning;
}

export type ControlLesson = ControlLessonV3 | ControlLessonV4;
export type ControlLessonMetadata = ControlLessonSummary | ControlLesson;

export interface ControlLessonPresentationSection {
  readonly title: string;
  readonly paragraphs: ReadonlyArray<string>;
}

export type ControlLessonAvailability =
  | { readonly kind: "available"; readonly lesson: ControlLesson }
  | { readonly kind: "unavailable"; readonly reason: string; readonly lesson?: ControlLessonSummary };

export type ControlCurriculumValidation =
  | { readonly kind: "accepted" }
  | { readonly kind: "rejected"; readonly id: string; readonly reason: "duplicate_sequence"; readonly sequence: number }
  | { readonly kind: "rejected"; readonly id: string; readonly reason: "sequence_gap"; readonly expected: number; readonly actual: number }
  | { readonly kind: "rejected"; readonly id: string; readonly reason: "unknown_prerequisite" | "forward_prerequisite"; readonly prerequisite: string };

const SIGNAL_ROLES = new Set<ControlSignalRole>(["SP", "PV", "MV", "disturbance", "state", "quality"]);
const WORKFLOW_STAGES = new Set<ControlWorkflowStage>(["Monitor", "Test", "Identify", "Tune", "Validate"]);
const EXPERIMENT_KINDS = new Set([
  "observation",
  "setpoint_step",
  "load_step",
  "mv_step",
  "mv_pulse",
  "relay",
  "prbs",
  "multisine",
]);
const COMPARATORS = new Set(["<", "<=", "==", ">=", ">"]);
const EVIDENCE_CLASSES = new Set<ControlEvidenceClass>(["simulation", "physical", "recorded", "model"]);
const PUBLICATION_STATES = new Set<ControlPublicationState>([
  "not_started",
  "scripted",
  "captured",
  "edited",
  "reviewed",
  "unlisted",
  "published",
]);
// Deliberately small and explicit. Add an engineering unit only with the first
// lesson that uses it, so misspellings cannot silently become distinct units.
export const CONTROL_UNITS = new Set([
  "1", "1/s", "1/s2", "%", "pp", "pp/pp", "pp*s", "%/s", "dB", "A", "V", "W", "kW", "kW/degC", "kJ/degC", "Hz", "rpm",
  "rad", "rad/s", "rad/s2", "rad*s", "deg", "deg/s", "degC", "degC/s", "K",
  "Pa", "Pa/s", "kPa", "bar", "g", "g*s", "t/h", "t/h/%", "m", "m2", "m2/s", "cm", "mm", "m/s", "cm/s", "mm/s", "m/s2", "%/m", "%/mm", "%/(mm/s)", "rpm/%", "mm/%", "mm/(%*s^2)", "mm/(A*s^2)", "cm/(%*s)", "m2*sqrt(m)/s", "m/(m3/s)", "degC/load-unit",
  "L", "L/s", "L/min", "cm/L", "m3", "m3/s", "m3/h", "m*s", "(m3/s)/m", "(m3/s)/(m*s)", "kg", "kg m2", "kg/s", "kg/s/%", "(kg/s)^2", "mol/s", "%/bar", "%/(bar*s)", "%/(degC*s)",
  "N", "kN", "N/m", "N s/m", "N m", "N m/rad", "N m/(rad*s)", "N m s", "N m s/rad", "rad/(N m)", "m/(kN*s^2)", "s", "ms", "degC*s", "degC^2*s", "degC*s^2", "degC/(%*s)", "bar*s", "degC/%", "%/degC", "%*s/degC", "%/rpm", "degC/(L/min)", "L/min/%", "bar/%", "kPa/%", "Pa/%",
]);

export function parseControlLessonContract(raw: unknown, manifestId: string): ControlLessonMetadata {
  const lesson = objectValue(raw, manifestId, "metadata");
  const summary = parseSummary(lesson, manifestId);
  const version = lesson.schemaVersion;
  if (version === 2) {
    return summary;
  }
  if (version !== 3 && version !== 4) {
    fail(manifestId, "unsupported schemaVersion");
  }

  for (const field of ["id", "title", "physicalPurpose"] as const) {
    stringValue(lesson[field], manifestId, field);
  }
  if (lesson.id !== manifestId) {
    fail(manifestId, "id does not match manifest");
  }

  const signals = parseSignals(lesson.signals, manifestId);
  parsePlant(lesson.plant, signals, manifestId);
  parseExecutableChange(lesson.baseline, manifestId, "baseline", "limitation");
  parseExecutableChange(lesson.change, manifestId, "change", "decision");
  parseExperiment(lesson.experiment, signals, manifestId);
  parseAcceptance(lesson.acceptance, signals, manifestId);
  parseSafety(lesson.safety, signals, manifestId);
  parseWorkflowStages(lesson.workflowStages, manifestId);
  parseProof(lesson.proof, manifestId);
  parseVideo(lesson.video, manifestId);

  if (version === 4) {
    stringValue(lesson.objective, manifestId, "objective");
    nullableString(lesson.nextLesson, manifestId, "nextLesson");
    parseLearning(lesson.learning, manifestId);
  }

  return lesson as unknown as ControlLesson;
}

/**
 * Keeps an incomplete lesson discoverable without presenting it as runnable.
 * The gallery needs the legacy summary for course order, while creation is
 * reserved for lessons whose complete engineering contract is valid.
 */
export function evaluateControlLessonAvailability(
  raw: unknown,
  manifestId: string
): ControlLessonAvailability {
  try {
    const lesson = parseControlLessonContract(raw, manifestId);
    if (!("schemaVersion" in lesson)) {
      return {
        kind: "unavailable",
        reason: "Requires the complete version 3 engineering contract.",
        lesson,
      };
    }
    return { kind: "available", lesson };
  } catch (error) {
    const detail = error instanceof Error ? error.message : String(error);
    const prefix = `control lesson '${manifestId}' `;
    const reason = detail.startsWith(prefix) ? detail.slice(prefix.length) : detail;
    let lesson: ControlLessonSummary | undefined;
    try {
      lesson = parseSummary(objectValue(raw, manifestId, "metadata"), manifestId);
    } catch {
      // A broken summary cannot be positioned in the curriculum, but the
      // original contract failure still explains why creation is unavailable.
    }
    return {
      kind: "unavailable",
      reason: reason === "missing object 'plant'"
        ? "Missing the executable plant contract."
        : sentence(reason),
      lesson,
    };
  }
}

export function validateControlCurriculum(
  lessons: ReadonlyArray<{ readonly id: string; readonly sequence: number; readonly prerequisites: ReadonlyArray<string> }>
): ControlCurriculumValidation {
  const byId = new Map(lessons.map((lesson) => [lesson.id, lesson]));
  const seenSequences = new Set<number>();
  for (const [index, lesson] of lessons.entries()) {
    if (seenSequences.has(lesson.sequence)) {
      return { kind: "rejected", id: lesson.id, reason: "duplicate_sequence", sequence: lesson.sequence };
    }
    seenSequences.add(lesson.sequence);
    const expected = index + 1;
    if (lesson.sequence !== expected) {
      return { kind: "rejected", id: lesson.id, reason: "sequence_gap", expected, actual: lesson.sequence };
    }
    for (const prerequisite of lesson.prerequisites) {
      const dependency = byId.get(prerequisite);
      if (!dependency) {
        return { kind: "rejected", id: lesson.id, reason: "unknown_prerequisite", prerequisite };
      }
      if (dependency.sequence >= lesson.sequence) {
        return { kind: "rejected", id: lesson.id, reason: "forward_prerequisite", prerequisite };
      }
    }
  }
  return { kind: "accepted" };
}

/**
 * Progress belongs to a stable lesson id, not to its current availability.
 * A temporarily invalid lesson therefore remains completed while it stays in
 * the curriculum; only ids that no longer exist are discarded.
 */
export function retainControlLessonProgress(
  completedIds: ReadonlyArray<string>,
  knownLessonIds: ReadonlyArray<string>
): string[] {
  const known = new Set(knownLessonIds);
  return completedIds.filter((id) => known.has(id));
}

/**
 * The nine canonical learner-facing README sections, in their mandatory order
 * (Specification 38 §2.1). This is structure, not content: the lesson's own
 * README is the one place its teaching prose lives, so no paragraph text is
 * duplicated here.
 */
const CANONICAL_LESSON_SECTIONS: ReadonlyArray<string> = [
  "The problem",
  "What we need",
  "The new idea",
  "Prediction",
  "Implementation",
  "Experiment in Control Studio",
  "Evidence and decision",
  "Conclusion",
  "Reason for the next lesson",
];

/**
 * Returns the single product-owned outline used by every rendered view
 * (W-03): the nine canonical README sections above, followed by the lesson's
 * real, executable Control Studio workflow (Specification 38 §2.1 restricts
 * every lesson to `workflowStages`, normally exactly Monitor -> Test) rather
 * than the retired eleven-stage Context/Reason/Theory/.../Transfer sequence.
 */
export function controlLessonPresentation(
  lesson: ControlLessonMetadata
): ControlLessonPresentationSection[] {
  if (!("schemaVersion" in lesson)) return [];
  const sections: ControlLessonPresentationSection[] = CANONICAL_LESSON_SECTIONS.map(
    (title) => ({ title, paragraphs: [] })
  );
  if (!lesson.workflowStages.length) {
    return sections;
  }
  return [
    ...sections,
    { title: "Control Studio workflow", paragraphs: [lesson.workflowStages.join(" -> ")] },
  ];
}

function parseSummary(lesson: Record<string, unknown>, id: string): ControlLessonSummary {
  for (const field of ["level", "equipment", "principalTechnique"] as const) {
    stringValue(lesson[field], id, field);
  }
  const sequence = finiteNumber(lesson.sequence, id, "sequence");
  if (!Number.isInteger(sequence) || sequence < 1) {
    fail(id, "has invalid sequence");
  }
  const prerequisites = stringArray(lesson.prerequisites, id, "prerequisites");
  return {
    sequence,
    level: lesson.level as string,
    equipment: lesson.equipment as string,
    principalTechnique: lesson.principalTechnique as string,
    prerequisites,
  };
}

function parseSignals(value: unknown, id: string): ControlLessonSignal[] {
  if (!Array.isArray(value) || value.length === 0) {
    fail(id, "has invalid signals");
  }
  const seenRoles = new Set<string>();
  const seenBindings = new Set<string>();
  const signals = value.map((item, index) => {
    const signal = objectValue(item, id, `signals[${index}]`);
    const role = stringValue(signal.role, id, `signals[${index}].role`);
    if (!SIGNAL_ROLES.has(role as ControlSignalRole)) {
      fail(id, `signal has unsupported role '${role}'`);
    }
    const name = stringValue(signal.name, id, `signals[${index}].name`);
    const unit = controlUnit(signal.unit, id, `signal '${role}'`);
    const binding = stringValue(signal.binding, id, `signals[${index}].binding`);
    const minimum = finiteNumber(signal.minimum, id, `signals[${index}].minimum`);
    const maximum = finiteNumber(signal.maximum, id, `signals[${index}].maximum`);
    const resolution = finiteNumber(signal.resolution, id, `signals[${index}].resolution`);
    if (minimum >= maximum) fail(id, `signal '${role}' has invalid range`);
    if (resolution <= 0) fail(id, `signal '${role}' has invalid resolution`);
    if (seenBindings.has(binding)) fail(id, `signals have duplicate binding '${binding}'`);
    seenBindings.add(binding);
    seenRoles.add(role);
    return { role: role as ControlSignalRole, name, unit, minimum, maximum, resolution, binding };
  });
  // Open-loop identification and plant-classification lessons have no
  // setpoint. PV and MV remain mandatory; closed-loop lesson tests require SP.
  for (const required of ["PV", "MV"]) {
    if (!seenRoles.has(required)) fail(id, `signals missing required role '${required}'`);
  }
  return signals;
}

function parsePlant(value: unknown, signals: ControlLessonSignal[], id: string): void {
  const plant = objectValue(value, id, "plant");
  stringValue(plant.modelId, id, "plant.modelId", "plant missing string 'modelId'");
  stringValue(plant.executable, id, "plant.executable");
  const sampleTime = finiteNumber(plant.sampleTimeSeconds, id, "plant.sampleTimeSeconds");
  if (sampleTime <= 0) fail(id, "plant has invalid sampleTimeSeconds");
  finiteRecord(plant.initialState, id, "plant.initialState");
  const seed = finiteNumber(plant.seed, id, "plant.seed");
  if (!Number.isInteger(seed)) fail(id, "plant has invalid seed");
  const operatingPoint = finiteRecord(plant.operatingPoint, id, "plant.operatingPoint");
  const knownSignals = new Set(signals.flatMap((signal) => [signal.role, signal.name]));
  for (const signal of Object.keys(operatingPoint)) {
    if (!knownSignals.has(signal)) fail(id, `plant operating point names unknown signal '${signal}'`);
  }
  if (!Array.isArray(plant.validityRange) || plant.validityRange.length === 0) {
    fail(id, "plant has invalid validityRange");
  }
  for (const item of plant.validityRange) {
    const range = objectValue(item, id, "plant validity range");
    const signal = stringValue(range.signal, id, "plant validity range signal");
    if (!knownSignals.has(signal)) fail(id, `plant validity range names unknown signal '${signal}'`);
    const minimum = finiteNumber(range.minimum, id, `plant validity range '${signal}' minimum`);
    const maximum = finiteNumber(range.maximum, id, `plant validity range '${signal}' maximum`);
    if (minimum >= maximum) fail(id, `plant validity range '${signal}' is reversed`);
    controlUnit(range.unit, id, `plant validity range '${signal}'`);
  }
  stringArray(plant.simplifications, id, "plant.simplifications");
  if (!Array.isArray(plant.parameters) || plant.parameters.length === 0) {
    fail(id, "plant has invalid parameters");
  }
  for (const item of plant.parameters) {
    const parameter = objectValue(item, id, "plant parameter");
    const name = stringValue(parameter.name, id, "plant parameter name");
    controlUnit(parameter.unit, id, `plant parameter '${name}'`);
    if (typeof parameter.value !== "number" || !Number.isFinite(parameter.value)) {
      fail(id, `plant parameter '${name}' has nonfinite value`);
    }
  }
}

function parseExecutableChange(value: unknown, id: string, field: string, explanation: string): void {
  const change = objectValue(value, id, field);
  stringValue(change.executableId, id, `${field}.executableId`);
  objectValue(change.settings, id, `${field}.settings`);
  stringValue(change[explanation], id, `${field}.${explanation}`);
  assertFiniteTree(change.settings, id, `${field}.settings`);
}

function parseExperiment(value: unknown, signals: ControlLessonSignal[], id: string): void {
  const experiment = objectValue(value, id, "experiment");
  const kind = stringValue(experiment.kind, id, "experiment.kind");
  if (!EXPERIMENT_KINDS.has(kind)) fail(id, `experiment has unsupported kind '${kind}'`);
  const signal = stringValue(experiment.signal, id, "experiment.signal");
  const knownSignals = new Set(signals.flatMap((candidate) => [candidate.role, candidate.name]));
  if (!knownSignals.has(signal)) fail(id, `experiment names unknown signal '${signal}'`);
  for (const field of ["baseline", "amplitude", "durationSeconds", "settlingSeconds", "seed", "restorationValue"]) {
    finiteNumber(experiment[field], id, `experiment.${field}`);
  }
  if ((experiment.durationSeconds as number) <= 0) fail(id, "experiment has invalid durationSeconds");
  if ((experiment.settlingSeconds as number) < 0 ||
      (experiment.settlingSeconds as number) > (experiment.durationSeconds as number)) {
    fail(id, "experiment has invalid settlingSeconds");
  }
  stringArray(experiment.disturbances, id, "experiment.disturbances");
}

function parseAcceptance(value: unknown, signals: ControlLessonSignal[], id: string): void {
  if (!Array.isArray(value) || value.length === 0) fail(id, "has invalid acceptance");
  const knownSignals = new Set(signals.flatMap((signal) => [signal.role, signal.name]));
  for (const item of value) {
    const acceptance = objectValue(item, id, "acceptance metric");
    const metric = stringValue(acceptance.metric, id, "acceptance.metric");
    const signal = stringValue(acceptance.signal, id, `acceptance metric '${metric}' signal`);
    if (!knownSignals.has(signal)) fail(id, `acceptance metric '${metric}' names unknown signal '${signal}'`);
    stringValue(acceptance.window, id, `acceptance metric '${metric}' window`);
    const comparator = stringValue(acceptance.comparator, id, `acceptance metric '${metric}' comparator`);
    if (!COMPARATORS.has(comparator)) fail(id, `acceptance metric '${metric}' has unsupported comparator '${comparator}'`);
    if (typeof acceptance.limit !== "number" || !Number.isFinite(acceptance.limit)) {
      fail(id, `acceptance metric '${metric}' has nonfinite limit`);
    }
    const tolerance = finiteNumber(acceptance.tolerance, id, `acceptance metric '${metric}' tolerance`);
    if (tolerance < 0) fail(id, `acceptance metric '${metric}' has invalid tolerance`);
    controlUnit(acceptance.unit, id, `acceptance metric '${metric}'`);
    const evidenceClass = stringValue(acceptance.evidenceClass, id, `acceptance metric '${metric}' evidenceClass`);
    if (!EVIDENCE_CLASSES.has(evidenceClass as ControlEvidenceClass)) {
      fail(id, `acceptance metric '${metric}' has unsupported evidenceClass '${evidenceClass}'`);
    }
  }
}

function parseSafety(value: unknown, signals: ControlLessonSignal[], id: string): void {
  const safety = objectValue(value, id, "safety");
  const knownSignals = new Set(signals.flatMap((signal) => [signal.role, signal.name]));
  if (!Array.isArray(safety.hardLimits) || safety.hardLimits.length === 0) fail(id, "safety has invalid hardLimits");
  for (const item of safety.hardLimits) {
    const limit = objectValue(item, id, "safety limit");
    const signal = stringValue(limit.signal, id, "safety limit signal");
    if (!knownSignals.has(signal)) fail(id, `safety limit names unknown signal '${signal}'`);
    const minimum = finiteNumber(limit.minimum, id, `safety limit '${signal}' minimum`);
    const maximum = finiteNumber(limit.maximum, id, `safety limit '${signal}' maximum`);
    if (minimum >= maximum) fail(id, `safety limit '${signal}' has invalid range`);
    controlUnit(limit.unit, id, `safety limit '${signal}'`);
  }
  if (stringArray(safety.abortPredicates, id, "safety.abortPredicates").length === 0) {
    fail(id, "safety has no abortPredicates");
  }
  finiteNumber(safety.safeMv, id, "safety.safeMv");
  stringValue(safety.restorationMode, id, "safety.restorationMode");
  stringValue(safety.physicalApprovalRole, id, "safety.physicalApprovalRole");
  stringArray(safety.simulationOnlyRestrictions, id, "safety.simulationOnlyRestrictions");
}

function parseWorkflowStages(value: unknown, id: string): void {
  const stages = stringArray(value, id, "workflowStages");
  const seen = new Set<string>();
  let previous = -1;
  for (const stage of stages) {
    if (!WORKFLOW_STAGES.has(stage as ControlWorkflowStage)) {
      fail(id, `workflowStages contains unsupported stage '${stage}'`);
    }
    if (seen.has(stage)) fail(id, `workflowStages contains duplicate stage '${stage}'`);
    const order = ["Monitor", "Test", "Identify", "Tune", "Validate"].indexOf(stage);
    if (order <= previous) fail(id, "workflowStages are not in execution order");
    previous = order;
    seen.add(stage);
  }
}

function parseProof(value: unknown, id: string): void {
  const proof = objectValue(value, id, "proof");
  for (const field of ["specification", "nativeTest", "nativeCommand", "projectPath", "renderedJourneyId"]) {
    stringValue(proof[field], id, `proof.${field}`);
  }
  const nativeTest = proof.nativeTest as string;
  if (!(proof.nativeCommand as string).includes(nativeTest)) {
    fail(id, `proof nativeCommand does not name nativeTest '${nativeTest}'`);
  }
  const commandArguments = (proof.nativeCommand as string).split(/\s+/).filter(Boolean);
  if (!commandArguments.includes(nativeTest)) {
    fail(id, `proof nativeCommand does not name nativeTest '${nativeTest}' as an exact argument`);
  }
  if (stringArray(proof.runArtifactFields, id, "proof.runArtifactFields").length === 0) {
    fail(id, "proof has no runArtifactFields");
  }
}

function parseVideo(value: unknown, id: string): void {
  const video = objectValue(value, id, "video");
  for (const field of ["viewerOutcome", "durationClass", "scriptPath", "captionPath", "thumbnailPath"]) {
    stringValue(video[field], id, `video.${field}`);
  }
  if (stringArray(video.requiredVisibleEvidence, id, "video.requiredVisibleEvidence").length === 0) {
    fail(id, "video has no requiredVisibleEvidence");
  }
  const state = stringValue(video.publicationState, id, "video.publicationState");
  if (!PUBLICATION_STATES.has(state as ControlPublicationState)) {
    fail(id, `video has unsupported publicationState '${state}'`);
  }
}

function parseLearning(value: unknown, id: string): void {
  const learning = objectValue(value, id, "learning");
  for (const field of [
    "context", "reason", "workedPrediction", "baseline", "change", "comparison",
    "validation", "explanation", "transfer",
  ] as const) {
    stringValue(learning[field], id, `learning.${field}`);
  }
  for (const field of ["theory", "practicalPreparation"] as const) {
    if (stringArray(learning[field], id, `learning.${field}`).length === 0) {
      fail(id, `has empty learning.${field}`);
    }
  }
}

function objectValue(value: unknown, id: string, field: string): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    if (field === "metadata") fail(id, "metadata is not an object");
    fail(id, `missing object '${field}'`);
  }
  return value as Record<string, unknown>;
}

function sentence(value: string): string {
  const normalized = value.trim().replace(/[.]+$/, "");
  return normalized
    ? `${normalized.charAt(0).toUpperCase()}${normalized.slice(1)}.`
    : "The lesson contract is invalid.";
}

function stringValue(value: unknown, id: string, field: string, message?: string): string {
  if (typeof value !== "string" || !value.trim()) fail(id, message ?? `missing string '${field}'`);
  return value as string;
}

function nullableString(value: unknown, id: string, field: string): string | null {
  if (value === null) return null;
  return stringValue(value, id, field);
}

function controlUnit(value: unknown, id: string, owner: string): string {
  const unit = stringValue(value, id, `${owner} unit`);
  if (!CONTROL_UNITS.has(unit)) {
    fail(id, `${owner} has unsupported unit '${unit}'`);
  }
  return unit;
}

function stringArray(value: unknown, id: string, field: string): string[] {
  if (!Array.isArray(value) || !value.every((item) => typeof item === "string" && item.trim())) {
    fail(id, `has invalid ${field}`);
  }
  return value as string[];
}

function finiteNumber(value: unknown, id: string, field: string): number {
  if (typeof value !== "number" || !Number.isFinite(value)) fail(id, `has invalid ${field}`);
  return value as number;
}

function finiteRecord(value: unknown, id: string, field: string): Record<string, number> {
  const record = objectValue(value, id, field);
  if (Object.keys(record).length === 0) fail(id, `${field} is empty`);
  for (const [key, item] of Object.entries(record)) {
    if (typeof item !== "number" || !Number.isFinite(item)) {
      fail(id, `${field}.${key} is not finite`);
    }
  }
  return record as Record<string, number>;
}

function assertFiniteTree(value: unknown, id: string, field: string): void {
  if (typeof value === "number" && !Number.isFinite(value)) fail(id, `${field} contains nonfinite value`);
  if (Array.isArray(value)) value.forEach((item, index) => assertFiniteTree(item, id, `${field}[${index}]`));
  else if (value && typeof value === "object") {
    for (const [key, item] of Object.entries(value)) assertFiniteTree(item, id, `${field}.${key}`);
  }
}

function fail(id: string, reason: string): never {
  throw new Error(`control lesson '${id}' ${reason}`);
}
