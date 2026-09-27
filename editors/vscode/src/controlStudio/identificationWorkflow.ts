import { prepareIdentificationReview, qualifyIdentificationData,
  type AcceptedIdentificationModel, type ExcludedInterval, type IdentifiedModel,
  type IdentificationModelSelection, type IdentificationReview, type IdentificationReviewInput,
  type IdentificationSample, type QualifiedIdentificationData } from "./identification";
import { measureStepResponse, qualifyStepResponse, type StepResponseSettings } from "./stepResponseIdentification";
import { decodeIdentificationState, restoreIdentificationInput, saveIdentificationInput,
  type IdentificationPhase, type IdentificationRecording, type SavedIdentificationState } from "./identificationState";
import { decodeIdentificationFormDraft, type IdentificationFormDraft } from "./identificationFormDraft";
import { chooseSensorLagModel, type StructureChoice } from "./sensorLagComparison";

export type IdentificationCalculator = (input: IdentificationReviewInput) => Promise<IdentificationReview | undefined>;

/** Keeps the estimation and validation recordings separate until the user accepts the model. */
export class IdentificationWorkflow {
  constructor(private maximumNormalizedError = 0.15) {}
  private qualified?: IdentificationReviewInput;
  private estimation?: IdentificationReviewInput;
  private pending?: Extract<IdentificationReview, { stage: "acceptance_required" }>;
  private accepted?: AcceptedIdentificationModel;
  private current?: IdentificationReviewInput;
  private phase: IdentificationPhase = "qualified";
  private qualification?: QualifiedIdentificationData;
  private review?: IdentificationReview;
  private failure?: string;
  private form?: IdentificationFormDraft;
  private structureChoice?: StructureChoice;
  private generation = 0;

  get acceptedModel(): IdentifiedModel | undefined { return this.accepted?.model; }
  get nextFitRole(): "estimation" | "validation" { return this.estimation ? "validation" : "estimation"; }

  get currentState() { return structuredClone({ phase: this.phase, qualification: this.qualification, review: this.review, failure: this.failure }); }

  snapshot(): SavedIdentificationState | undefined {
    if (!this.current && !this.form) return undefined;
    return { schemaVersion: 1, maximumNormalizedError: this.maximumNormalizedError, phase: this.phase,
      ...(this.current ? { current: saveIdentificationInput(this.current) } : {}), ...(this.estimation ? { estimation: saveIdentificationInput(this.estimation) } : {}),
      ...(this.form ? { form: structuredClone(this.form) } : {}),
      ...(this.structureChoice ? { structureChoice: this.structureChoice } : {}) } as SavedIdentificationState;
  }

  /** Replay through the existing algorithms in an isolated state before replacing this session. */
  restore(value: unknown): void {
    const steps = this.replay(value);
    let step = steps.next();
    while (!step.done) {
      try { step.value.fit(); step = steps.next(); }
      catch (error) { step = steps.throw(error); }
    }
    this.adopt(step.value);
  }

  async restoreUsing(value: unknown, calculate: IdentificationCalculator): Promise<boolean> {
    const generation = this.generation;
    const steps = this.replay(value);
    let step = steps.next();
    while (!step.done) {
      try {
        if (!await step.value.fitUsing(calculate)) return false;
        step = steps.next();
      } catch (error) { step = steps.throw(error); }
      if (generation !== this.generation) return false;
    }
    if (generation !== this.generation) return false;
    this.adopt(step.value);
    return true;
  }

  /** One replay plan is shared by synchronous checks and interactive worker execution. */
  private *replay(value: unknown): Generator<IdentificationWorkflow, IdentificationWorkflow, void> {
    const saved = value === undefined ? undefined : decodeIdentificationState(value);
    if (value !== undefined && !saved) throw new Error("Saved identification inputs are malformed or incompatible.");
    const next = new IdentificationWorkflow(saved?.maximumNormalizedError ?? this.maximumNormalizedError);
    const qualify = (input: IdentificationReviewInput, selected = false) => {
      const result = next.qualify(input.estimation, input.excluded, input.source,
        input.requestedKind, input.estimationRecordingId, input.stepResponseSettings, input.recording, input.firstOrderStepResponseSettings);
      if (selected) next.awaitQualification();
      return result;
    };
    if (saved) {
      if (saved.phase === "editing") {
        next.current = saved.current ? restoreIdentificationInput(saved.current) : undefined;
        next.estimation = saved.estimation ? restoreIdentificationInput(saved.estimation) : undefined;
        next.form = saved.form ? structuredClone(saved.form) : undefined;
        next.qualified = undefined; next.pending = undefined; next.accepted = undefined; next.qualification = undefined; next.review = undefined;
        next.phase = "editing";
      }
      if (saved.phase !== "editing" && saved.estimation) {
        if (!qualify(restoreIdentificationInput(saved.estimation)).suitable) throw new Error("Saved estimation no longer qualifies.");
        yield next;
      }
      if (saved.phase !== "editing" && saved.form && !saved.current) next.editForm(saved.form);
      if (saved.phase !== "editing" && saved.current && saved.phase !== "preview") {
        const result = qualify(restoreIdentificationInput(saved.current), saved.phase === "selected");
        if (saved.phase !== "selected" && (saved.phase === "rejected" ? result.suitable : !result.suitable)) throw new Error("Saved identification qualification does not match its inputs.");
        if (["review", "accepted", "failed"].includes(saved.phase)) {
          try { yield next; } catch (error) { if (saved.phase !== "failed") throw error; }
        }
        if (saved.phase === "accepted") next.accept();
      }
      if (saved.form && saved.current) {
        next.form = structuredClone(saved.form);
        if (saved.phase === "editing") {
          next.qualified = undefined; next.qualification = undefined; next.pending = undefined; next.review = undefined; next.accepted = undefined;
          next.phase = "editing";
        }
      }
      if (saved.structureChoice) next.chooseStructure(saved.structureChoice);
      if (next.phase !== saved.phase) throw new Error("Saved identification review stage does not match its inputs.");
    }
    return next;
  }

  private adopt(next: IdentificationWorkflow): void {
    this.generation++;
    this.maximumNormalizedError = next.maximumNormalizedError;
    this.qualified = next.qualified; this.estimation = next.estimation; this.pending = next.pending; this.accepted = next.accepted;
      this.current = next.current; this.phase = next.phase; this.qualification = next.qualification; this.review = next.review; this.failure = next.failure;
    this.form = next.form;
    this.structureChoice = next.structureChoice;
  }

  chooseStructure(choice: StructureChoice): IdentificationReview {
    if (this.phase !== "preview" || !this.review) throw new Error("Calculate the comparison before choosing a provisional model.");
    const review = chooseSensorLagModel(this.review, choice);
    this.generation++;
    this.review = review; this.structureChoice = choice;
    return review;
  }

  qualify(
    samples: readonly IdentificationSample[],
    excluded: readonly ExcludedInterval[],
    source: "Simulation" | "Physical",
    requestedKind: IdentificationModelSelection,
    recordingId?: string,
    stepResponseSettings?: StepResponseSettings,
    recording?: IdentificationRecording,
    firstOrderStepResponseSettings?: StepResponseSettings
  ): QualifiedIdentificationData {
    this.generation++;
    this.form = undefined;
    this.structureChoice = undefined;
    const previous = this.estimation ?? this.current;
    const sameEstimation = previous && previous.estimationRecordingId === recordingId && previous.source === source
      && JSON.stringify(previous.estimation) === JSON.stringify(samples);
    const referenceSettings = requestedKind === "sopdt" && stepResponseSettings
      ? firstOrderStepResponseSettings ?? (sameEstimation ? previous.firstOrderStepResponseSettings ?? previous.stepResponseSettings : undefined) ?? stepResponseSettings
      : undefined;
    if (this.accepted || this.estimation?.requestedKind !== requestedKind
      || JSON.stringify(this.estimation?.stepResponseSettings) !== JSON.stringify(stepResponseSettings)) this.estimation = undefined;
    const immutableSamples = samples.map((sample) => ({ ...sample }));
    const immutableExcluded = excluded.map((interval) => ({ ...interval }));
    let qualification = qualifyIdentificationData(immutableSamples, immutableExcluded);
    if (stepResponseSettings) {
      const step = immutableExcluded.length || !["fopdt", "sopdt"].includes(requestedKind)
        ? { suitable: false, reason: "Bounded step analysis requires a first-order or two-lag model and the complete unexcluded recording." }
        : qualifyStepResponse(immutableSamples, stepResponseSettings);
      qualification = step.suitable
        ? { ...qualification, stepResponse: measureStepResponse(immutableSamples, stepResponseSettings) }
        : { ...qualification, suitable: false, suitableForDesign: false, reason: step.reason };
    }
    this.current = { estimation: immutableSamples, excluded: immutableExcluded, source, requestedKind,
        ...(referenceSettings ? { firstOrderStepResponseSettings: structuredClone(referenceSettings) } : {}),
        ...(recording ? { recording: structuredClone(recording) } : {}),
        maximumNormalizedError: this.maximumNormalizedError,
        ...(stepResponseSettings ? { stepResponseSettings: JSON.parse(JSON.stringify(stepResponseSettings)) as StepResponseSettings } : {}),
        ...(recordingId ? { estimationRecordingId: recordingId } : {}) };
    this.qualified = qualification.suitable ? this.current : undefined;
    this.qualification = qualification;
    this.review = undefined; this.failure = undefined;
    this.phase = qualification.suitable ? this.estimation ? "validation-qualified" : "qualified" : "rejected";
    this.pending = undefined;
    this.accepted = undefined;
    return qualification;
  }

  selectRecording(...input: Parameters<IdentificationWorkflow["qualify"]>): void {
    const pendingForm = this.form;
    this.qualify(...input);
    this.form = pendingForm;
    this.awaitQualification();
  }

  editForm(form: IdentificationFormDraft): void {
    const decoded = decodeIdentificationFormDraft(form);
    if (!decoded) throw new Error("Identification form draft is malformed.");
    this.generation++;
    const wasAccepted = Boolean(this.accepted);
    this.structureChoice = undefined;
    const preservedCurrent = this.current ?? this.qualified ?? this.estimation;
    this.pending = undefined; this.accepted = undefined; this.qualified = undefined; this.qualification = undefined;
    this.review = undefined; this.failure = undefined;
    if (wasAccepted) this.estimation = undefined;
    this.current = preservedCurrent;
    this.form = decoded;
    this.phase = "editing";
  }

  private awaitQualification(): void {
    this.phase = "selected"; this.qualified = undefined; this.qualification = undefined;
  }

  fit(): IdentificationReview {
    const input = this.fitInput();
    this.generation++;
    try { return this.completeFit(prepareIdentificationReview(input)); }
    catch (error) {
      this.failFit(error); throw error;
    }
  }

  async fitUsing(calculate: IdentificationCalculator): Promise<IdentificationReview | undefined> {
    const input = this.fitInput();
    const generation = ++this.generation;
    try {
      const review = await calculate(structuredClone(input));
      if (!review || generation !== this.generation) return undefined;
      return this.completeFit(review);
    } catch (error) {
      if (generation !== this.generation) return undefined;
      this.failFit(error); throw error;
    }
  }

  private failFit(error: unknown): void {
    this.structureChoice = undefined;
    this.phase = "failed"; this.review = undefined;
    this.failure = error instanceof Error ? error.message : String(error);
  }

  private fitInput(): IdentificationReviewInput {
    if (!this.qualified) throw new Error("Qualify a suitable recording before fitting a model.");
    if (!this.estimation) return this.qualified;
    return {
      estimation: this.estimation.estimation,
      validation: this.qualified.estimation,
      ...(this.estimation.estimationRecordingId ? { estimationRecordingId: this.estimation.estimationRecordingId } : {}),
      ...(this.qualified.estimationRecordingId ? { validationRecordingId: this.qualified.estimationRecordingId } : {}),
      excluded: this.estimation.excluded,
      source: this.estimation.source,
      requestedKind: this.qualified.requestedKind,
      stepResponseSettings: this.estimation.stepResponseSettings,
      firstOrderStepResponseSettings: this.estimation.firstOrderStepResponseSettings,
    };
  }

  private completeFit(review: IdentificationReview): IdentificationReview {
    if (!this.qualified) throw new Error("Qualify a suitable recording before fitting a model.");
    if (!this.estimation) {
      this.estimation = {
        ...this.qualified,
        estimation: review.estimation.map((sample) => ({ ...sample })),
        excluded: this.qualified.excluded.map((interval) => ({ ...interval })),
      };
      this.phase = "preview"; this.review = review;
      return review;
    }
    if (review.stage !== "acceptance_required") throw new Error("Independent validation is required before acceptance.");
    if (!review.validationQualification.suitable) {
      this.pending = undefined; this.accepted = undefined; this.review = review;
      this.phase = "failed";
      this.failure = review.validationQualification.reason ?? "Independent validation failed.";
      return review;
    }
    this.pending = review;
    this.phase = "review"; this.review = review;
    return review;
  }

  accept(): AcceptedIdentificationModel {
    if (!this.pending || !this.pending.validationQualification.suitable) throw new Error(this.failure ?? "Independent validation and model review are required before acceptance.");
    this.generation++;
    this.accepted = {
      model: this.pending.selected,
      fit: this.pending.fit,
      evidence: this.pending.result.source,
      review: this.pending,
    };
    this.phase = "accepted";
    return this.accepted;
  }
}
