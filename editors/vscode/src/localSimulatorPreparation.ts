import type { CheckProgramResponse } from "./checkProgramModel";
import type { RuntimeLifecycleResult } from "./runtimeLifecycleModel";
import { migrateWindowsRuntimeControlProject } from "./windowsRuntimeControlMigration";

export interface LocalSimulatorPreparationDependencies {
  readonly validateProject: () => Promise<CheckProgramResponse | undefined>;
  readonly platform?: NodeJS.Platform;
  readonly tokenFactory?: () => string;
}

export type LocalSimulatorPreparationResult =
  | RuntimeLifecycleResult
  | {
      readonly ok: false;
      readonly validationRejected: CheckProgramResponse;
    };

export async function prepareLocalSimulatorProject(
  projectRoot: string | undefined,
  dependencies: LocalSimulatorPreparationDependencies
): Promise<LocalSimulatorPreparationResult> {
  const migration = migrateWindowsRuntimeControlProject(
    projectRoot,
    dependencies.platform ?? process.platform,
    dependencies.tokenFactory
  );
  if (migration.failure) {
    return {
      ok: false,
      failure: {
        kind: "configuration",
        message: migration.failure.message,
      },
    };
  }

  let validation: CheckProgramResponse | undefined;
  try {
    validation = await dependencies.validateProject();
  } catch {
    return {
      ok: false,
      failure: {
        kind: "failed_spawn",
        message:
          "Simulator project validation could not finish. Check the Structured Text Debugger output for details.",
      },
    };
  }
  if (hasRuntimeConfigurationError(validation)) {
    return {
      ok: false,
      failure: {
        kind: "configuration",
        message:
          "Runtime configuration could not be loaded. Open runtime.toml and fix the reported setting.",
      },
    };
  }
  if (validation && !validation.ok) {
    return {
      ok: false,
      validationRejected: validation,
    };
  }
  return { ok: true, message: "Simulator project is ready." };
}

function hasRuntimeConfigurationError(
  validation: CheckProgramResponse | undefined
): boolean {
  return Boolean(
    validation?.issues.some((issue) => {
      if (issue.severity.toLowerCase() !== "error") {
        return false;
      }
      const code = (issue.code ?? "").toLowerCase();
      const file = (issue.file ?? "").replace(/\\/g, "/").toLowerCase();
      return code === "config.runtime" || file.endsWith("/runtime.toml") || file === "runtime.toml";
    })
  );
}
