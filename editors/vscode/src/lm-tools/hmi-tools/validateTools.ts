import { getTrustConfiguration } from "../../configuration";

// Responsibility: focused LM tools module with a single concern.
import * as path from "path";
import * as vscode from "vscode";
import { requestRuntimeControl } from "../runtimeControl";
import { LspToolBase } from "../lspTools";
import * as shared from "../shared";

const {
  asRecord,
  buildHmiLockEntries,
  coerceInt,
  collectHmiDiagnosticsForFiles,
  ensureWorkspaceUri,
  errorCodeMatches,
  errorResult,
  evidenceRunId,
  extractErrorCode,
  extractLayoutBindingRefs,
  generateHmiCandidates,
  hashContent,
  HMI_CANDIDATE_STRATEGIES,
  hmiDescriptorFileFromPointer,
  hmiSeverityRank,
  layoutDescriptorPages,
  normalizeErrorCode,
  normalizeEvidenceRunId,
  normalizeHmiBindingsCatalog,
  normalizeScenario,
  normalizeSnapshotViewports,
  normalizeStringList,
  normalizeTraceIds,
  parseHmiSchemaPayload,
  parseHmiValuesPayload,
  parseQuotedArrayFromToml,
  parseWritePolicyFromConfigToml,
  pruneEvidenceRuns,
  readHmiLayoutSnapshot,
  renderIntentToml,
  renderSnapshotSvg,
  resolveWorkspaceFolder,
  sleepWithCancellation,
  stableComponent,
  stableJsonString,
  textResult,
  uriFromFilePath,
  writeUtf8File,
} = shared;

type InvocationOptions<T> = shared.InvocationOptions<T>;
type HmiBindingsParams = shared.HmiBindingsParams;
type HmiGetLayoutParams = shared.HmiGetLayoutParams;
type HmiApplyPatchParams = shared.HmiApplyPatchParams;
type HmiPlanIntentParams = shared.HmiPlanIntentParams;
type HmiValidateParams = shared.HmiValidateParams;
type HmiTraceCaptureParams = shared.HmiTraceCaptureParams;
type HmiGenerateCandidatesParams = shared.HmiGenerateCandidatesParams;
type HmiPreviewSnapshotParams = shared.HmiPreviewSnapshotParams;
type HmiRunJourneyParams = shared.HmiRunJourneyParams;
type HmiExplainWidgetParams = shared.HmiExplainWidgetParams;
type HmiInitParams = shared.HmiInitParams;
type HmiValidationCheck = shared.HmiValidationCheck;
type HmiSchemaResult = shared.HmiSchemaResult;
type HmiCandidate = shared.HmiCandidate;
type HmiJourneyAction = shared.HmiJourneyAction;
type SnapshotViewport = shared.SnapshotViewport;

export class STHmiValidateTool extends LspToolBase {
  async invoke(
    options: InvocationOptions<HmiValidateParams>,
    token: vscode.CancellationToken,
  ): Promise<unknown> {
    if (token.isCancellationRequested) {
      return textResult("Cancelled.");
    }
    const layout = await readHmiLayoutSnapshot(options.input.rootPath, token);
    if (layout.error) {
      if (layout.error === "Cancelled.") {
        return textResult("Cancelled.");
      }
      return errorResult(layout.error);
    }
    const snapshot = layout.snapshot;
    if (!snapshot) {
      return errorResult("Unable to resolve HMI workspace.");
    }
    if (!snapshot.exists) {
      return errorResult("hmi/ directory does not exist.");
    }

    const dryRun = options.input.dry_run === true;
    const prune = options.input.prune === true;
    const retainRuns = Number.isInteger(options.input.retain_runs)
      ? Math.max(1, Number(options.input.retain_runs))
      : 10;
    const checks: HmiValidationCheck[] = [];
    const layoutRefs = extractLayoutBindingRefs(snapshot.pages);
    if (layoutRefs.length === 0) {
      checks.push({
        code: "HMI_VALIDATE_LAYOUT_NO_BINDS",
        severity: "warning",
        message: "No bind/source entries were found in hmi page files.",
      });
    }

    const writePolicy = parseWritePolicyFromConfigToml(snapshot.config?.content);
    if (writePolicy.enabled && writePolicy.allow.length === 0) {
      checks.push({
        code: "HMI_VALIDATE_WRITE_ALLOWLIST_EMPTY",
        severity: "error",
        file: "hmi/_config.toml",
        message:
          "[write].enabled is true but no allowlist entries were found in hmi/_config.toml.",
      });
    }
    for (const target of writePolicy.allow) {
      if (!target.startsWith("resource/")) {
        checks.push({
          code: "HMI_VALIDATE_WRITE_ALLOW_NON_CANONICAL",
          severity: "warning",
          file: "hmi/_config.toml",
          message: `Write allowlist target '${target}' is not canonical (expected resource/... identifier).`,
        });
      }
    }

    const pollMs = getTrustConfiguration(
      vscode.Uri.file(snapshot.rootPath)
    ).get<number>("hmi.pollIntervalMs", 500);
    if (pollMs < 50) {
      checks.push({
        code: "HMI_VALIDATE_POLL_INTERVAL_TOO_LOW",
        severity: "warning",
        message: `Configured poll interval (${pollMs}ms) is below the recommended lower bound (50ms).`,
      });
    } else if (pollMs > 1000) {
      checks.push({
        code: "HMI_VALIDATE_POLL_INTERVAL_TOO_HIGH",
        severity: "warning",
        message: `Configured poll interval (${pollMs}ms) exceeds the recommended upper bound (1000ms).`,
      });
    }

    let catalog = normalizeHmiBindingsCatalog({});
    let catalogAvailable = false;
    const bindingsRequest = await this.request(
      "workspace/executeCommand",
      {
        command: "trust-lsp.hmiBindings",
        arguments: [{ root_uri: vscode.Uri.file(snapshot.rootPath).toString() }],
      },
      token,
    );
    if ("error" in bindingsRequest) {
      checks.push({
        code: "HMI_VALIDATE_BINDINGS_UNAVAILABLE",
        severity: "warning",
        message: `Unable to load binding catalog from trust-lsp.hmiBindings: ${bindingsRequest.error}`,
      });
    } else {
      const payload =
        bindingsRequest.response && typeof bindingsRequest.response === "object"
          ? (bindingsRequest.response as Record<string, unknown>)
          : {};
      if (payload.ok === false) {
        checks.push({
          code: "HMI_VALIDATE_BINDINGS_UNAVAILABLE",
          severity: "warning",
          message: `trust-lsp.hmiBindings failed: ${String(payload.error ?? "unknown error")}`,
        });
      } else {
        catalog = normalizeHmiBindingsCatalog(payload);
        catalogAvailable = true;
      }
    }

    const lock = buildHmiLockEntries(layoutRefs, catalog);
    for (const unknownPath of lock.unknownPaths) {
      checks.push({
        code: "HMI_VALIDATE_UNKNOWN_BIND_PATH",
        severity: catalogAvailable ? "error" : "warning",
        message: `Binding path '${unknownPath}' is not present in the current binding catalog.`,
      });
    }

    const diagnosticChecks = await collectHmiDiagnosticsForFiles(
      snapshot.rootPath,
      snapshot.files,
      token,
    );
    checks.push(...diagnosticChecks);
    checks.sort((left, right) => {
      const severity = hmiSeverityRank(left.severity) - hmiSeverityRank(right.severity);
      if (severity !== 0) {
        return severity;
      }
      if ((left.file ?? "") !== (right.file ?? "")) {
        return (left.file ?? "").localeCompare(right.file ?? "");
      }
      if (left.code !== right.code) {
        return left.code.localeCompare(right.code);
      }
      return left.message.localeCompare(right.message);
    });

    const errors = checks.filter((check) => check.severity === "error").length;
    const warnings = checks.filter((check) => check.severity === "warning").length;
    const infos = checks.filter((check) => check.severity === "info").length;
    const ok = errors === 0;

    const lockDocument = {
      version: 1,
      widgets: lock.entries,
    };
    const lockContent = `${stableJsonString(lockDocument)}\n`;
    const generatedAt = new Date();
    const validationDocument = {
      version: 1,
      generated_at: generatedAt.toISOString(),
      ok,
      root_path: snapshot.rootPath,
      hmi_path: snapshot.hmiPath,
      counts: {
        errors,
        warnings,
        infos,
      },
      checks,
    };
    const journeysDocument = {
      version: 1,
      generated_at: generatedAt.toISOString(),
      journeys: [],
      note: "No journey scenarios executed in validate-only run.",
    };

    const hmiRoot = vscode.Uri.file(snapshot.hmiPath);
    const lockUri = vscode.Uri.joinPath(hmiRoot, "_lock.json");
    let evidencePath: string | null = null;
    let prunedRuns: string[] = [];
    if (!dryRun) {
      await vscode.workspace.fs.createDirectory(hmiRoot);
      await writeUtf8File(lockUri, lockContent);

      const runId = evidenceRunId(generatedAt);
      const evidenceRoot = vscode.Uri.joinPath(hmiRoot, "_evidence");
      const runRoot = vscode.Uri.joinPath(evidenceRoot, runId);
      await vscode.workspace.fs.createDirectory(runRoot);
      await writeUtf8File(
        vscode.Uri.joinPath(runRoot, "validation.json"),
        `${stableJsonString(validationDocument)}\n`,
      );
      await writeUtf8File(
        vscode.Uri.joinPath(runRoot, "journeys.json"),
        `${stableJsonString(journeysDocument)}\n`,
      );
      evidencePath = path.posix.join("hmi", "_evidence", runId);
      if (prune) {
        prunedRuns = await pruneEvidenceRuns(hmiRoot, retainRuns);
      }
    }

    return textResult(
      JSON.stringify(
        {
          ok,
          dry_run: dryRun,
          prune,
          retain_runs: retainRuns,
          rootPath: snapshot.rootPath,
          lockPath: path.posix.join("hmi", "_lock.json"),
          evidencePath,
          prunedRuns,
          checks,
          counts: { errors, warnings, infos },
          widgetCount: lock.entries.length,
        },
        null,
        2,
      ),
    );
  }
}
