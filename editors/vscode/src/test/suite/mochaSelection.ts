import * as fs from "fs";
import * as path from "path";

type EvidenceTest = {
  title?: string;
  duration?: number;
  fullTitle?: () => string;
};

type EvidenceRunner = {
  on(event: "pass", listener: (test: EvidenceTest) => void): unknown;
  on(
    event: "fail",
    listener: (test: EvidenceTest, error: unknown) => void
  ): unknown;
  on(event: "end", listener: () => void): unknown;
};

type EvidenceResult = {
  title: string;
  full_title: string;
  status: "passed" | "failed";
  duration_ms: number;
  error?: string;
};

export function configuredMochaGrep(
  environment: Readonly<Record<string, string | undefined>>
): RegExp | undefined {
  const configured = environment.TRUST_VSCODE_TEST_GREP?.trim();
  return configured ? new RegExp(configured) : undefined;
}

export function attachConfiguredMochaEvidence(
  runner: EvidenceRunner,
  environment: Readonly<Record<string, string | undefined>>
): boolean {
  const configuredPath = environment.TRUST_VSCODE_TEST_EVIDENCE?.trim();
  if (!configuredPath) {
    return false;
  }
  const sourceCommit = environment.TRUST_VSCODE_TEST_SOURCE_COMMIT?.trim();
  if (!sourceCommit) {
    throw new Error(
      "TRUST_VSCODE_TEST_EVIDENCE requires TRUST_VSCODE_TEST_SOURCE_COMMIT"
    );
  }

  const results: EvidenceResult[] = [];
  runner.on("pass", (test) => {
    results.push(evidenceResult(test, "passed"));
  });
  runner.on("fail", (test, error) => {
    results.push({
      ...evidenceResult(test, "failed"),
      error: error instanceof Error ? error.message : String(error),
    });
  });
  runner.on("end", () => {
    const evidencePath = path.resolve(configuredPath);
    fs.mkdirSync(path.dirname(evidencePath), { recursive: true });
    const temporaryPath = `${evidencePath}.tmp-${process.pid}-${Date.now()}`;
    const passedCount = results.filter(
      (result) => result.status === "passed"
    ).length;
    const failedCount = results.length - passedCount;
    fs.writeFileSync(
      temporaryPath,
      `${JSON.stringify(
        {
          schema_version: 1,
          source_commit: sourceCommit,
          selector: environment.TRUST_VSCODE_TEST_GREP?.trim() ?? null,
          test_count: results.length,
          passed_count: passedCount,
          failed_count: failedCount,
          results,
        },
        null,
        2
      )}\n`,
      "utf8"
    );
    fs.renameSync(temporaryPath, evidencePath);
  });
  return true;
}

function evidenceResult(
  test: EvidenceTest,
  status: "passed" | "failed"
): EvidenceResult {
  return {
    title: test.title ?? "",
    full_title: test.fullTitle?.() ?? test.title ?? "",
    status,
    duration_ms:
      typeof test.duration === "number"
        ? Math.max(0, Math.round(test.duration))
        : 0,
  };
}
