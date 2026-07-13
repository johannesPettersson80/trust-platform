import {
  assert,
  fs,
  path,
  workspaceRoot,
} from "./ux-shell-contract-fixtures";

suite("Phase 0 — packaged runtime tools (v5 shell)", () => {
  test("release VSIX bundles trust-runtime beside trust-lsp and trust-debug", () => {
    const releaseWorkflow = fs.readFileSync(
      path.join(workspaceRoot(), ".github", "workflows", "release.yml"),
      "utf8"
    );
    for (const binary of ["trust-lsp", "trust-debug", "trust-runtime"]) {
      assert.ok(
        releaseWorkflow.includes(`cp target/\${{ matrix.target }}/release/${binary} editors/vscode/bin/`),
        `Unix VSIX packaging must copy ${binary} into editors/vscode/bin`
      );
      assert.ok(
        releaseWorkflow.includes(`cp target/\${{ matrix.target }}/release/${binary}.exe editors/vscode/bin/`),
        `Windows VSIX packaging must copy ${binary}.exe into editors/vscode/bin`
      );
    }
  });
});
