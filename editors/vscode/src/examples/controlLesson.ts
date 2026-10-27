// The types of a control lesson's metadata and how the gallery presents it (specification 25 §3.1; specification 38).
// The lesson contract itself is checked by the language server (`trust-lsp.examplesCatalog`); this module only shapes
// what a valid lesson shows.

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

export interface ControlLessonDetail {
  readonly objective: string;
  readonly description: string;
  readonly equipment: string;
  readonly prerequisites: ReadonlyArray<string>;
  readonly expectedObservation: string;
  readonly preview: ReadonlyArray<ControlLessonPresentationSection>;
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

/**
 * Projects canonical lesson metadata into the detail-card contract. The
 * catalog description is supplied by the catalog loader so it is not copied
 * into a second syllabus; preview paragraphs come from this lesson's own
 * objective, plant, experiment, and learning metadata.
 */
export function controlLessonDetail(
  lesson: ControlLessonMetadata,
  description: string
): ControlLessonDetail {
  if (!("schemaVersion" in lesson)) {
    return {
      objective: `${lesson.principalTechnique} control using ${lesson.equipment}.`,
      description,
      equipment: lesson.equipment,
      prerequisites: lesson.prerequisites,
      expectedObservation: "This legacy lesson is unavailable as a runnable project in the current gallery.",
      preview: [],
    };
  }
  const objective = "objective" in lesson ? lesson.objective : lesson.physicalPurpose;
  const expectedObservation = "learning" in lesson
    ? lesson.learning.validation
    : lesson.baseline.limitation;
  const paragraphs: Array<ControlLessonPresentationSection> = [
    { title: "Objective", paragraphs: [objective] },
    { title: "Plant and signals", paragraphs: [lesson.physicalPurpose, `${lesson.signals.map((signal) => `${signal.name} (${signal.role}, ${signal.unit})`).join(", ")}.`] },
    { title: "Expected observation", paragraphs: [expectedObservation] },
  ];
  if ("learning" in lesson) {
    paragraphs.push(
      { title: "What to try", paragraphs: [lesson.learning.practicalPreparation.join(" "), lesson.learning.change] },
      { title: "How to decide", paragraphs: [lesson.learning.comparison, lesson.learning.explanation] },
    );
  } else {
    paragraphs.push({ title: "How to decide", paragraphs: [lesson.baseline.limitation, lesson.change.decision] });
  }
  return {
    objective,
    description,
    equipment: lesson.equipment,
    prerequisites: lesson.prerequisites,
    expectedObservation,
    preview: paragraphs.filter((section) => section.paragraphs.every((paragraph) => paragraph.trim().length > 0)),
  };
}
