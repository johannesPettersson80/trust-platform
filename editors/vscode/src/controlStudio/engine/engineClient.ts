import type * as vscode from "vscode";
import type { EngineProcess } from "./engineProcess";
import type { ApplicationAnswer } from "../generated/ApplicationAnswer";
import type { ApplicationRequest } from "../generated/ApplicationRequest";
import type { CancelResult } from "../generated/CancelResult";
import type { ExportRequest } from "../generated/ExportRequest";
import type { ExportResult } from "../generated/ExportResult";
import type { Failure } from "../generated/Failure";
import type { IdentifyRequest } from "../generated/IdentifyRequest";
import type { IdentifyResult } from "../generated/IdentifyResult";
import type { ManualIdentificationAnswer } from "../generated/ManualIdentificationAnswer";
import type { IdentifyReview } from "../generated/IdentifyReview";
import type { InputsRequest } from "../generated/InputsRequest";
import type { InputsResult } from "../generated/InputsResult";
import type { KeepRequest } from "../generated/KeepRequest";
import type { KeepReviewAnswer } from "../generated/KeepReviewAnswer";
import type { MonitorRequest } from "../generated/MonitorRequest";
import type { MonitorResult } from "../generated/MonitorResult";
import type { OpenRequest } from "../generated/OpenRequest";
import type { OpenResult } from "../generated/OpenResult";
import type { OperationRequest } from "../generated/OperationRequest";
import type { ParameterWorkAnswer } from "../generated/ParameterWorkAnswer";
import type { ParameterWorkRequest } from "../generated/ParameterWorkRequest";
import type { OperationStarted } from "../generated/OperationStarted";
import type { OperationUpdate } from "../generated/OperationUpdate";
import type { PageRequest } from "../generated/PageRequest";
import type { PreviewRequest } from "../generated/PreviewRequest";
import type { ResetResult } from "../generated/ResetResult";
import type { RunRequest } from "../generated/RunRequest";
import type { ProjectLoopsRequest } from "../generated/ProjectLoopsRequest";
import type { ProjectLoopsResult } from "../generated/ProjectLoopsResult";
import type { SetupDiscoverRequest } from "../generated/SetupDiscoverRequest";
import type { SetupDiscoverResult } from "../generated/SetupDiscoverResult";
import type { SetupSaveRequest } from "../generated/SetupSaveRequest";
import type { SetupSaveResult } from "../generated/SetupSaveResult";
import type { TuneRequest } from "../generated/TuneRequest";
import type { TuneResult } from "../generated/TuneResult";
import type { ValidateRequest } from "../generated/ValidateRequest";
import type { AnalyzeAnswer } from "../generated/AnalyzeAnswer";
import type { AnalyzeRequest } from "../generated/AnalyzeRequest";
import type { ChartExportAnswer } from "../generated/ChartExportAnswer";
import type { ChartExportRequest } from "../generated/ChartExportRequest";
import type { PlantMonitorRequest } from "../generated/PlantMonitorRequest";
import type { PlantMonitorResult } from "../generated/PlantMonitorResult";
import type { SimulatorStartAnswer } from "../generated/SimulatorStartAnswer";
import type { SimulatorStartOutcome } from "../generated/SimulatorStartOutcome";
import type { SimulatorStartReply } from "../generated/SimulatorStartReply";
import type { SimulatorStartRequest } from "../generated/SimulatorStartRequest";

/** A method's answer: its `result`, or the `failure` that refuses it (specification 35, A11). */
export type EngineAnswer<T> = { result: T } | { failure: Failure };

/** The answer of a method with no result to show (`controlStudio/page`). */
export type EmptyResult = Record<string, never>;

/** What `controlStudio/identify` answers: the answer of the action it was asked for. */
export type IdentifyAnswer =
  | OperationStarted
  | ResetResult
  | IdentifyReview
  | IdentifyResult
  | ManualIdentificationAnswer
  | CancelResult;

/** The engine's methods (specification 35, §0.1a). Each sends its request and returns the engine's answer. */
export interface ControlStudioEngine {
  open(request: OpenRequest): Promise<EngineAnswer<OpenResult>>;
  status(request: OperationRequest): Promise<EngineAnswer<OperationUpdate>>;
  cancel(request: OperationRequest): Promise<EngineAnswer<OperationUpdate>>;
  identify(request: IdentifyRequest): Promise<EngineAnswer<IdentifyAnswer>>;
  /** The analysis the Tune page shows is recorded with the page (B9.9); it is absent when none is shown. */
  page(request: PageRequest): Promise<EngineAnswer<EmptyResult>>;
  inputs(request: InputsRequest): Promise<EngineAnswer<InputsResult>>;
  run(request: RunRequest): Promise<EngineAnswer<OperationStarted>>;
  tune(request: TuneRequest): Promise<EngineAnswer<OperationStarted>>;
  preview(request: PreviewRequest): Promise<EngineAnswer<TuneResult>>;
  validate(request: ValidateRequest): Promise<EngineAnswer<OperationStarted>>;
  apply(request: ApplicationRequest): Promise<EngineAnswer<ApplicationAnswer>>;
  restore(request: ApplicationRequest): Promise<EngineAnswer<ApplicationAnswer>>;
  /** The answer is the Keep review, or the started operation when the request names the review's identity. */
  keep(request: KeepRequest): Promise<EngineAnswer<KeepReviewAnswer | OperationStarted>>;
  /** The parameter writes of a declared analysis (C12): the review, its answer, or the started Apply or Restore. */
  parameters(request: ParameterWorkRequest): Promise<EngineAnswer<ParameterWorkAnswer | OperationStarted>>;
  /** The loop's sample and, when it changed, the target's label (B2, B9.2). The engine decides availability and staleness. */
  monitor(request: MonitorRequest): Promise<EngineAnswer<MonitorResult>>;
  /** The plant summary of a plant handoff (B1.2, specification 47, §20.4): unavailable with its reason, or the plant's status. */
  plantMonitor(request: PlantMonitorRequest): Promise<EngineAnswer<PlantMonitorResult>>;
  /** Decides a Start simulator request (A2.8): the extension starts the simulator, joins the start in flight, or shows the reply. */
  simulatorStart(request: SimulatorStartRequest): Promise<EngineAnswer<SimulatorStartAnswer>>;
  /** Reports the outcome of the simulator start the extension made, and answers the reply the page shows (A2.8). */
  simulatorStartResult(request: SimulatorStartOutcome): Promise<EngineAnswer<SimulatorStartReply>>;
  /** The chart image export (B10.5): the file name, the save filter and the bytes the extension writes. */
  chartExport(request: ChartExportRequest): Promise<EngineAnswer<ChartExportAnswer>>;
  setupDiscover(request: SetupDiscoverRequest): Promise<EngineAnswer<SetupDiscoverResult>>;
  setupSave(request: SetupSaveRequest): Promise<EngineAnswer<SetupSaveResult>>;
  /** The loop files of the workspace folders, decided by the engine (A8.3, B1.2): each file, the declaring folder and an equipment's loops. */
  projectLoops(request: ProjectLoopsRequest): Promise<EngineAnswer<ProjectLoopsResult>>;
  /** Without `tool` the answer lists the tools that apply; with it, a quick calculation, or the started operation (B9.9). */
  analyze(request: AnalyzeRequest): Promise<EngineAnswer<AnalyzeAnswer>>;
  /** The loop's results for the engineer to save (B10.5): the text of the JSON or the CSV export, and the file name it suggests. */
  exportLoop(request: ExportRequest): Promise<EngineAnswer<ExportResult>>;
  /**
   * Runs `listener` for each `controlStudio/update` of the operation `operationId`, and nothing after its final update. The engine
   * sends updates as notifications that nothing keeps for a later listener, so a final update sent before this subscription stood
   * would never reach it: unless `catchUp` is false, the operation's state is read once the subscription stands
   * (`controlStudio/status`; the engine records a final state before it sends it), and the listener hears one final update, whichever
   * source brings it first. A reopened panel follows the operations `controlStudio/open` listed without it, since an operation that
   * has ended shows nothing there (A8.6).
   */
  onUpdate(
    operationId: string,
    listener: (update: OperationUpdate) => void,
    options?: { catchUp?: boolean }
  ): vscode.Disposable;
}

const METHOD = {
  open: "controlStudio/open",
  status: "controlStudio/status",
  cancel: "controlStudio/cancel",
  identify: "controlStudio/identify",
  page: "controlStudio/page",
  inputs: "controlStudio/inputs",
  run: "controlStudio/run",
  tune: "controlStudio/tune",
  preview: "controlStudio/preview",
  validate: "controlStudio/validate",
  apply: "controlStudio/apply",
  restore: "controlStudio/restore",
  keep: "controlStudio/keep",
  setupDiscover: "controlStudio/setupDiscover",
  setupSave: "controlStudio/setupSave",
  projectLoops: "controlStudio/projectLoops",
  analyze: "controlStudio/analyze",
  parameters: "controlStudio/parameters",
  monitor: "controlStudio/monitor",
  plantMonitor: "controlStudio/plantMonitor",
  simulatorStart: "controlStudio/simulatorStart",
  simulatorStartResult: "controlStudio/simulatorStartResult",
  chartExport: "controlStudio/chartExport",
  exportLoop: "controlStudio/export",
} as const;

const UPDATE_NOTIFICATION = "controlStudio/update";

/** The typed methods of the engine, over the engine process of the window. */
export function createEngineClient(engine: EngineProcess): ControlStudioEngine {
  const call = async <T>(method: string, request: unknown): Promise<EngineAnswer<T>> => {
    const connection = await engine.connection();
    return connection.sendRequest<EngineAnswer<T>>(method, request);
  };
  return {
    open: (request) => call<OpenResult>(METHOD.open, request),
    status: (request) => call<OperationUpdate>(METHOD.status, request),
    cancel: (request) => call<OperationUpdate>(METHOD.cancel, request),
    identify: (request) => call<IdentifyAnswer>(METHOD.identify, request),
    page: (request) => call<EmptyResult>(METHOD.page, request),
    inputs: (request) => call<InputsResult>(METHOD.inputs, request),
    run: (request) => call<OperationStarted>(METHOD.run, request),
    tune: (request) => call<OperationStarted>(METHOD.tune, request),
    preview: (request) => call<TuneResult>(METHOD.preview, request),
    validate: (request) => call<OperationStarted>(METHOD.validate, request),
    apply: (request) => call<ApplicationAnswer>(METHOD.apply, request),
    restore: (request) => call<ApplicationAnswer>(METHOD.restore, request),
    keep: (request) => call<KeepReviewAnswer | OperationStarted>(METHOD.keep, request),
    setupDiscover: (request) => call<SetupDiscoverResult>(METHOD.setupDiscover, request),
    setupSave: (request) => call<SetupSaveResult>(METHOD.setupSave, request),
    projectLoops: (request) => call<ProjectLoopsResult>(METHOD.projectLoops, request),
    analyze: (request) => call<AnalyzeAnswer>(METHOD.analyze, request),
    parameters: (request) => call<ParameterWorkAnswer | OperationStarted>(METHOD.parameters, request),
    monitor: (request) => call<MonitorResult>(METHOD.monitor, request),
    plantMonitor: (request) => call<PlantMonitorResult>(METHOD.plantMonitor, request),
    simulatorStart: (request) => call<SimulatorStartAnswer>(METHOD.simulatorStart, request),
    simulatorStartResult: (request) => call<SimulatorStartReply>(METHOD.simulatorStartResult, request),
    chartExport: (request) => call<ChartExportAnswer>(METHOD.chartExport, request),
    exportLoop: (request) => call<ExportResult>(METHOD.exportLoop, request),
    onUpdate: (operationId, listener, options) => {
      let ended = false;
      let disposed = false;
      const deliver = (update: OperationUpdate): void => {
        if (disposed || ended) return;
        if (update.state !== "running") ended = true;
        listener(update);
      };
      const subscription = engine.onNotification(UPDATE_NOTIFICATION, (params) => {
        const update = params as OperationUpdate;
        if (update.operationId === operationId) deliver(update);
      });
      if (options?.catchUp !== false) {
        void call<OperationUpdate>(METHOD.status, { operationId }).then(reply => {
          if ("result" in reply && reply.result.state !== "running") deliver(reply.result);
        }, () => undefined);
      }
      return {
        dispose: () => {
          disposed = true;
          subscription.dispose();
        },
      };
    },
  };
}
