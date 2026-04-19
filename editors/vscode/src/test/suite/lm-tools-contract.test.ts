import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";

type PackageJsonContract = {
  activationEvents?: string[];
  contributes?: { languageModelTools?: Array<{ name?: string }> };
};

function sortedUnique(values: string[]): string[] {
  return [...new Set(values)].sort((left, right) => left.localeCompare(right));
}

function declaredToolNamesFromPackageJson(packageJson: PackageJsonContract): string[] {
  return sortedUnique(
    (packageJson.contributes?.languageModelTools ?? [])
      .map((tool) => tool.name)
      .filter((name): name is string => typeof name === "string" && name.length > 0),
  );
}

function activationToolNamesFromPackageJson(packageJson: PackageJsonContract): string[] {
  return sortedUnique(
    (packageJson.activationEvents ?? [])
      .filter((event) => event.startsWith("onLanguageModelTool:"))
      .map((event) => event.replace("onLanguageModelTool:", "")),
  );
}

function registeredToolNamesFromSource(lmToolsSource: string): string[] {
  return sortedUnique(
    Array.from(lmToolsSource.matchAll(/lm\.registerTool\(\s*"([^"]+)"/g)).map(
      (match) => match[1],
    ),
  );
}

suite("Language model tool contract (VS Code)", () => {
  function loadContract(): {
    declaredToolNames: string[];
    activationToolNames: string[];
    registeredToolNames: string[];
    lmToolsSource: string;
  } {
    assert.ok(
      vscode.workspace.workspaceFolders?.length !== undefined,
      "Expected VS Code workspace services to be available during extension tests.",
    );
    const extensionRoot = path.resolve(__dirname, "..", "..", "..");
    const packageJson = JSON.parse(
      fs.readFileSync(
        path.join(extensionRoot, "package.json"),
        "utf8",
      ),
    ) as PackageJsonContract;
    const lmToolsSource = fs.readFileSync(
      path.join(extensionRoot, "src", "lm-tools.ts"),
      "utf8",
    );
    return {
      declaredToolNames: declaredToolNamesFromPackageJson(packageJson),
      activationToolNames: activationToolNamesFromPackageJson(packageJson),
      registeredToolNames: registeredToolNamesFromSource(lmToolsSource),
      lmToolsSource,
    };
  }

  test("manifest declarations match activation events and registered tool names", () => {
    const {
      declaredToolNames,
      activationToolNames,
      registeredToolNames,
    } = loadContract();

    assert.ok(
      declaredToolNames.length > 0,
      "Expected language model tools to be declared in package.json.",
    );
    assert.deepStrictEqual(
      activationToolNames,
      declaredToolNames,
      "Language model tool activation events must match declared manifest tools.",
    );
    assert.deepStrictEqual(
      registeredToolNames,
      declaredToolNames,
      "Language model tool registrations in src/lm-tools.ts must match package.json declarations.",
    );
  });

  test("synthetic registration drift is detected by the contract check", () => {
    const { declaredToolNames, lmToolsSource } = loadContract();
    const driftedSource = lmToolsSource.replace(
      'lm.registerTool("trust_get_hover"',
      'lm.registerTool("trust_get_hover_drifted"',
    );

    assert.notStrictEqual(
      driftedSource,
      lmToolsSource,
      "Expected the synthetic mismatch fixture to modify the LM tools source.",
    );
    assert.notDeepStrictEqual(
      registeredToolNamesFromSource(driftedSource),
      declaredToolNames,
      "Synthetic LM tool drift should be detected by the contract matcher.",
    );
  });
});
