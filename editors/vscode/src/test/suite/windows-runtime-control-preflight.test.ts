import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";

import {
  launchControlPreparationError,
  prepareLaunchControl,
} from "../../debug/launchControl";
import { classifyRuntimeStartFailure } from "../../networkCanvas/runtimeFailures";
import { migrateWindowsRuntimeControlProject } from "../../windowsRuntimeControlMigration";
import { prepareLocalSimulatorProject } from "../../localSimulatorPreparation";
import { validityLine } from "../../compileGate";
import { selectedRuntime, SIMULATOR_RUNTIME_ID } from "../../trustHomeModel";

const GENERATED_TOKEN = "preflight-generated-token-1234567890";

function controlSource(
  endpoint = "tcp://127.0.0.1:9902",
  authLine = "",
): string {
  return [
    "[bundle]",
    "version = 1",
    "",
    "[runtime.control]",
    `endpoint = "${endpoint}"`,
    ...(authLine ? [authLine] : []),
    'mode = "production"',
    "",
  ].join("\n");
}

function withProject(
  source: string,
  run: (root: string, runtimeToml: string) => void,
): void {
  const root = fs.mkdtempSync(
    path.join(os.tmpdir(), "trust-control-preflight-"),
  );
  const runtimeToml = path.join(root, "runtime.toml");
  try {
    fs.writeFileSync(runtimeToml, source, { mode: 0o640 });
    run(root, runtimeToml);
  } finally {
    try {
      fs.chmodSync(runtimeToml, 0o640);
    } catch {
      // The test may intentionally remove or replace the fixture.
    }
    fs.rmSync(root, { recursive: true, force: true });
  }
}

async function withProjectAsync(
  source: string,
  run: (root: string, runtimeToml: string) => Promise<void>,
): Promise<void> {
  const root = fs.mkdtempSync(
    path.join(os.tmpdir(), "trust-control-preflight-"),
  );
  const runtimeToml = path.join(root, "runtime.toml");
  try {
    fs.writeFileSync(runtimeToml, source, { mode: 0o640 });
    await run(root, runtimeToml);
  } finally {
    try {
      fs.chmodSync(runtimeToml, 0o640);
    } catch {
      // The test may intentionally remove or replace the fixture.
    }
    fs.rmSync(root, { recursive: true, force: true });
  }
}

suite("Windows runtime control launch preflight", () => {
  test("a stale runtime auth diagnostic cannot disable or bypass one-click Start preparation", async () => {
    await withProjectAsync(controlSource(), async (root, runtimeToml) => {
      const collection = vscode.languages.createDiagnosticCollection(
        "trust-stale-runtime-auth-test",
      );
      try {
        collection.set(vscode.Uri.file(runtimeToml), [
          new vscode.Diagnostic(
            new vscode.Range(0, 0, 0, 1),
            "runtime.control.auth_token required for tcp endpoint",
            vscode.DiagnosticSeverity.Error,
          ),
        ]);
        assert.ok(
          validityLine().configErrors >= 1,
          "the fixture must reproduce a preexisting runtime.toml error",
        );

        const selected = selectedRuntime({
          snapshot: {
            runtimeMode: "simulate",
            runtimeState: "stopped",
            endpoint: "",
            endpointConfigured: false,
            endpointReachable: false,
            starting: false,
          },
          remotes: [],
          managed: [],
          selectedId: SIMULATOR_RUNTIME_ID,
        });
        assert.strictEqual(selected.primary.action, "start");
        assert.strictEqual(selected.primary.enabled, true);

        let validatorCalls = 0;
        const preparation = await prepareLocalSimulatorProject(root, {
          platform: "win32",
          tokenFactory: () => GENERATED_TOKEN,
          validateProject: async () => {
            validatorCalls += 1;
            assert.ok(
              fs
                .readFileSync(runtimeToml, "utf8")
                .includes(`auth_token = "${GENERATED_TOKEN}"`),
              "Start must migrate runtime.toml before its compile check",
            );
            return {
              version: 1,
              ok: true,
              status: "ok",
              errors: 0,
              warnings: 0,
              issues: [],
            };
          },
        });
        assert.strictEqual(preparation.ok, true);
        assert.strictEqual(validatorCalls, 1);

        const home = fs.readFileSync(
          path.join(
            path.resolve(__dirname, "../../.."),
            "src",
            "trustHomeView.ts",
          ),
          "utf8",
        );
        const runAction = home.slice(
          home.indexOf("private async runAction()"),
          home.indexOf("private async runManagedAction("),
        );
        assert.doesNotMatch(
          runAction,
          /primaryActionGateReason|withPrimaryActionGate|compileGateReason|validityLine\(\)/,
          "Run must not pre-gate Start using cached diagnostics or an earlier Compile result",
        );
      } finally {
        collection.dispose();
      }
    });
  });

  test("source errors are checked by Start without requiring a separate Compile click", async () => {
    await withProjectAsync(controlSource(), async (root) => {
      let validatorCalls = 0;
      const preparation = await prepareLocalSimulatorProject(root, {
        platform: "win32",
        tokenFactory: () => GENERATED_TOKEN,
        validateProject: async () => {
          validatorCalls += 1;
          return {
            version: 1,
            ok: false,
            status: "failed",
            errors: 1,
            warnings: 0,
            issues: [
              {
                severity: "error",
                code: "compile",
                file: path.join(root, "src", "program.st"),
                message: "test source error",
              },
            ],
          };
        },
      });
      assert.strictEqual(validatorCalls, 1);
      assert.strictEqual(
        preparation.ok,
        false,
        "source errors must stop launch without becoming runtime.toml recovery",
      );
      assert.ok(
        !preparation.ok && "validationRejected" in preparation,
        "source errors use the typed non-launch validation outcome",
      );

      const home = fs.readFileSync(
        path.join(
          path.resolve(__dirname, "../../.."),
          "src",
          "trustHomeView.ts",
        ),
        "utf8",
      );
      const runAction = home.slice(
        home.indexOf("private async runAction()"),
        home.indexOf("private async runManagedAction("),
      );
      assert.ok(
        runAction.includes('"validationRejected" in startResult') &&
          runAction.includes("this.setCompileState(compile)") &&
          runAction.includes("return;"),
        "Start itself must render only the Compile/Problems failure and stop before launch recovery",
      );
    });
  });

  test("lifecycle-owned Start cannot be vetoed by a stale editor diagnostic after validation", () => {
    const startCommand = fs.readFileSync(
      path.join(
        path.resolve(__dirname, "../../.."),
        "src",
        "debug",
        "startCommand.ts",
      ),
      "utf8",
    );
    assert.ok(
      startCommand.includes("const lifecycleOwnedStart = Boolean(") &&
        startCommand.includes("!lifecycleOwnedStart &&") &&
        startCommand.includes("diagnostics.some(") &&
        startCommand.includes("if (!(await validateConfiguration(programUri)))"),
      "sidebar Start must trust its fresh project validation while direct F5 keeps file validation",
    );
  });

  test("Run preparation migrates before the project validator", async () => {
    await withProjectAsync(controlSource(), async (root, runtimeToml) => {
      let validatorCalls = 0;
      const result = await prepareLocalSimulatorProject(root, {
        platform: "win32",
        tokenFactory: () => GENERATED_TOKEN,
        validateProject: async () => {
          validatorCalls += 1;
          assert.ok(
            fs
              .readFileSync(runtimeToml, "utf8")
              .includes(`auth_token = "${GENERATED_TOKEN}"`),
            "the validator must never observe the legacy tokenless file",
          );
          return {
            version: 1,
            ok: true,
            status: "ok",
            errors: 0,
            warnings: 0,
            issues: [],
          };
        },
      });

      assert.strictEqual(result.ok, true);
      assert.strictEqual(validatorCalls, 1);
    });
  });

  test("unsafe control configs fail before project validation", async () => {
    const fixtures = [
      {
        source: controlSource(),
        readOnly: true,
      },
      {
        source:
          controlSource() +
          '[runtime.control]\nendpoint = "tcp://127.0.0.1:9903"\n',
        readOnly: false,
      },
      {
        source: controlSource("tcp://192.168.50.42:9902"),
        readOnly: false,
      },
    ];
    for (const fixture of fixtures) {
      await withProjectAsync(fixture.source, async (root, runtimeToml) => {
        if (fixture.readOnly) {
          fs.chmodSync(runtimeToml, 0o440);
        }
        let validatorCalls = 0;
        const result = await prepareLocalSimulatorProject(root, {
          platform: "win32",
          tokenFactory: () => GENERATED_TOKEN,
          validateProject: async () => {
            validatorCalls += 1;
            return undefined;
          },
        });
        assert.strictEqual(result.ok, false);
        assert.ok(!result.ok && "failure" in result);
        if (result.ok || !("failure" in result)) {
          return;
        }
        assert.strictEqual(result.failure.kind, "configuration");
        assert.strictEqual(validatorCalls, 0);
      });
    }
  });

  test("project validator maps runtime.toml schema errors to configuration recovery", async () => {
    await withProjectAsync(controlSource(), async (root) => {
      const result = await prepareLocalSimulatorProject(root, {
        platform: "win32",
        tokenFactory: () => GENERATED_TOKEN,
        validateProject: async () => ({
          version: 1,
          ok: false,
          status: "failed",
          errors: 1,
          warnings: 0,
          issues: [
            {
              severity: "error",
              code: "config.runtime",
              file: path.join(root, "runtime.toml"),
              message: "private raw parser detail",
            },
          ],
        }),
      });
      assert.strictEqual(result.ok, false);
      if (result.ok) {
        return;
      }
      assert.ok("failure" in result);
      if (!("failure" in result)) {
        return;
      }
      assert.strictEqual(result.failure.kind, "configuration");
      assert.strictEqual(
        result.failure.message,
        "Runtime configuration could not be loaded. Open runtime.toml and fix the reported setting.",
      );
      assert.strictEqual(
        result.failure.message.includes("private raw parser detail"),
        false,
      );
    });
  });

  test("writable missing and placeholder tokens migrate to a valid launch configuration", () => {
    for (const authLine of ["", 'auth_token = "some-secret-value"']) {
      withProject(
        controlSource("tcp://127.0.0.1:9902", authLine),
        (root, runtimeToml) => {
          let tokenCalls = 0;
          const result = migrateWindowsRuntimeControlProject(
            root,
            "win32",
            () => {
              tokenCalls += 1;
              return GENERATED_TOKEN;
            },
          );
          const persisted = fs.readFileSync(runtimeToml, "utf8");

          assert.strictEqual(result.changed, true);
          assert.strictEqual(result.failure, undefined);
          assert.strictEqual(tokenCalls, 1);
          assert.ok(persisted.includes(`auth_token = "${GENERATED_TOKEN}"`));
          assert.ok(!persisted.includes("some-secret-value"));

          const folder = {
            uri: vscode.Uri.file(root),
            name: "preflight",
            index: 0,
          } as vscode.WorkspaceFolder;
          const preparation = prepareLaunchControl(
            {
              type: "structured-text",
              request: "launch",
              name: "truST Simulator",
              runtimeRoot: root,
            },
            folder,
            false,
            "win32",
          );
          assert.strictEqual(preparation.failure, undefined);
        },
      );
    }
  });

  test("an explicit same-computer launch endpoint receives the session token automatically", () => {
    withProject(
      controlSource(
        "tcp://127.0.0.1:9902",
        'auth_token = "already-secured-test-token-1234567890"',
      ),
      (root) => {
        const folder = {
          uri: vscode.Uri.file(root),
          name: "preflight",
          index: 0,
        } as vscode.WorkspaceFolder;
        const config: vscode.DebugConfiguration = {
          type: "structured-text",
          request: "launch",
          name: "truST Simulator",
          runtimeRoot: root,
          controlEndpoint: "tcp://127.42.0.5:23456",
        };

        const preparation = prepareLaunchControl(
          config,
          folder,
          false,
          "win32",
        );

        assert.strictEqual(preparation.failure, undefined);
        assert.strictEqual(config.controlEndpoint, "tcp://127.42.0.5:23456");
        assert.match(
          String(config.controlAuthToken ?? ""),
          /^[a-f0-9]{36}$/,
          "Run must inject a strong per-workspace token before DAP launch",
        );
      },
    );
  });

  test("an explicit launch token is preserved and a non-local tokenless endpoint fails before DAP", () => {
    withProject(
      controlSource(
        "tcp://127.0.0.1:9902",
        'auth_token = "already-secured-test-token-1234567890"',
      ),
      (root) => {
        const folder = {
          uri: vscode.Uri.file(root),
          name: "preflight",
          index: 0,
        } as vscode.WorkspaceFolder;
        const explicitToken = "explicit-session-token-1234567890";
        const configured: vscode.DebugConfiguration = {
          type: "structured-text",
          request: "launch",
          name: "truST Simulator",
          runtimeRoot: root,
          controlEndpoint: "tcp://localhost:23456",
          controlAuthToken: explicitToken,
        };
        const configuredPreparation = prepareLaunchControl(
          configured,
          folder,
          false,
          "win32",
        );
        assert.strictEqual(configuredPreparation.failure, undefined);
        assert.strictEqual(configured.controlAuthToken, explicitToken);

        const unsafe: vscode.DebugConfiguration = {
          type: "structured-text",
          request: "launch",
          name: "truST Simulator",
          runtimeRoot: root,
          controlEndpoint: "tcp://192.168.50.42:23456",
        };
        const unsafePreparation = prepareLaunchControl(
          unsafe,
          folder,
          false,
          "win32",
        );
        assert.strictEqual(
          unsafePreparation.failure?.code,
          "runtime_control_auth_requires_manual_configuration",
        );
        assert.strictEqual(unsafe.controlAuthToken, undefined);
      },
    );
  });

  test("read-only, malformed, and non-loopback tokenless TCP configs stop before token generation", () => {
    const fixtures = [
      {
        name: "read-only",
        source: controlSource(),
        code: "runtime_control_toml_not_writable",
        makeReadOnly: true,
      },
      {
        name: "malformed duplicate control table",
        source:
          controlSource() +
          '[runtime.control]\nendpoint = "tcp://127.0.0.1:9903"\n',
        code: "runtime_control_toml_malformed",
        makeReadOnly: false,
      },
      {
        name: "non-loopback missing token",
        source: controlSource("tcp://192.168.50.42:9902"),
        code: "runtime_control_auth_requires_manual_configuration",
        makeReadOnly: false,
      },
    ] as const;

    for (const fixture of fixtures) {
      withProject(fixture.source, (root, runtimeToml) => {
        if (fixture.makeReadOnly) {
          fs.chmodSync(runtimeToml, 0o440);
        }
        let tokenCalls = 0;
        const result = migrateWindowsRuntimeControlProject(
          root,
          "win32",
          () => {
            tokenCalls += 1;
            return GENERATED_TOKEN;
          },
        );

        assert.strictEqual(result.changed, false, fixture.name);
        assert.strictEqual(result.failure?.kind, "configuration", fixture.name);
        assert.strictEqual(result.failure?.code, fixture.code, fixture.name);
        assert.strictEqual(
          tokenCalls,
          0,
          `${fixture.name} must fail before generating a token`,
        );
        assert.strictEqual(
          fs.readFileSync(runtimeToml, "utf8"),
          fixture.source,
        );
        assert.doesNotMatch(
          result.failure?.message ?? "",
          /tcp:\/\/|192\.168|127\.0\.0\.1|auth_token\s*=|some-secret-value|generated-token/i,
          `${fixture.name} failure must not expose endpoint or credential material`,
        );
        assert.deepStrictEqual(
          fs
            .readdirSync(root)
            .filter((entry) => entry.includes(".trust-migrate-")),
          [],
        );
      });
    }
  });

  test("typed preflight failure reaches configuration recovery before DAP launch", () => {
    withProject(controlSource("tcp://192.168.50.42:9902"), (root) => {
      const folder = {
        uri: vscode.Uri.file(root),
        name: "preflight",
        index: 0,
      } as vscode.WorkspaceFolder;
      const preparation = prepareLaunchControl(
        {
          type: "structured-text",
          request: "launch",
          name: "truST Simulator",
          runtimeRoot: root,
        },
        folder,
        false,
        "win32",
      );
      assert.ok(preparation.failure);

      const classified = classifyRuntimeStartFailure(
        launchControlPreparationError(preparation.failure!),
      );
      assert.strictEqual(classified.kind, "configuration");
      assert.strictEqual(classified.message, preparation.failure?.message);
      assert.strictEqual(classified.detail, undefined);

      const extensionRoot = path.resolve(__dirname, "../../..");
      const startCommand = fs.readFileSync(
        path.join(extensionRoot, "src", "debug", "startCommand.ts"),
        "utf8",
      );
      const prepareIndex = startCommand.indexOf(
        "const launchControl = prepareLaunchControl(",
      );
      const diagnosticsIndex = startCommand.indexOf(
        "vscode.languages.getDiagnostics(programUri)",
      );
      const validationIndex = startCommand.indexOf(
        "validateConfiguration(programUri)",
      );
      assert.ok(
        prepareIndex >= 0 &&
          diagnosticsIndex > prepareIndex &&
          validationIndex > prepareIndex &&
          startCommand.includes(
            "throw launchControlPreparationError(launchControl.failure)",
          ) &&
          startCommand.indexOf("throw launchControlPreparationError") <
            startCommand.indexOf("vscode.debug.startDebugging"),
        "direct Start/F5 must migrate and preflight runtime.toml before diagnostics, validation, or DAP launch",
      );
      assert.ok(
        startCommand.indexOf("launchControlEndpointError(") > validationIndex,
        "endpoint bind probing remains after source validation",
      );

      const home = fs.readFileSync(
        path.join(extensionRoot, "src", "trustHomeView.ts"),
        "utf8",
      );
      const homeFailures = fs.readFileSync(
        path.join(extensionRoot, "src", "trustHomeFailures.ts"),
        "utf8",
      );
      assert.ok(
        homeFailures.includes('failure.kind === "configuration"') &&
          homeFailures.includes("return [OPEN_RUNTIME_TOML_ACTION]"),
        "typed configuration failures must offer exactly Open runtime.toml from Run",
      );
      const runAction = home.slice(
        home.indexOf("private async runAction()"),
        home.indexOf("private async runManagedAction("),
      );
      const lifecycle = fs.readFileSync(
        path.join(extensionRoot, "src", "runtimeLifecycle.ts"),
        "utf8",
      );
      const coordinator = fs.readFileSync(
        path.join(extensionRoot, "src", "localSimulatorStartCoordinator.ts"),
        "utf8",
      );
      const startLocal = lifecycle.slice(
        lifecycle.indexOf("async startLocalSimulator("),
        lifecycle.indexOf("async connectRemote("),
      );
      assert.ok(
        runAction.indexOf("runtimeLifecycleService.startLocalSimulator(") >=
          0 &&
          runAction.indexOf("runtimeLifecycleService.startLocalSimulator(") <
            runAction.indexOf("CHECK_PROGRAM_COMMAND") &&
          runAction.includes(
            "CHECK_PROGRAM_COMMAND, { silent: true, projectRoot }",
          ) &&
          startLocal.indexOf('this.operations.begin("local_start"') >= 0 &&
          startLocal.indexOf('this.operations.begin("local_start"') <
            startLocal.indexOf("coordinateLocalSimulatorStart({") &&
          coordinator.includes("prepareLocalSimulatorProject("),
        "Run must claim Starting, enter migration/preflight, and keep its internal Compile presentation silent before validation observes runtime.toml",
      );
    });
  });
});
