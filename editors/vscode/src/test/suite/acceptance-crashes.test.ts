import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import { crashDumps, withCrashDumps } from "../../acceptance/crashes";

// Specification 22, "Acceptance journeys on the packaged extension": VS Code's crash reporter writes a session's crash
// dumps to `<folder>/<session>/crashes/`, and a failed session's result names them (checklist W50: VS Code ended with
// SIGTRAP after a passing journey and left nothing to read).
suite("Acceptance runner crash dumps (specification 22)", () => {
  let out: string;

  setup(() => {
    out = fs.mkdtempSync(path.join(os.tmpdir(), "acceptance-crashes-"));
  });

  teardown(() => {
    fs.rmSync(out, { recursive: true, force: true });
  });

  test("every dump in the crash reporter's folders is named from the output folder, in order", () => {
    // Crashpad keeps a dump in completed/ or pending/, beside files of its own. Catches a dump missed in a nested folder
    // and another file taken for a dump.
    const crashes = path.join(out, "hmi-equipment", "crashes");
    for (const file of ["completed/b.dmp", "completed/a.dmp", "pending/c.dmp", "settings.dat", "attachments/a.txt"]) {
      fs.mkdirSync(path.dirname(path.join(crashes, file)), { recursive: true });
      fs.writeFileSync(path.join(crashes, file), "");
    }
    assert.deepStrictEqual(crashDumps(crashes, out), [
      "hmi-equipment/crashes/completed/a.dmp",
      "hmi-equipment/crashes/completed/b.dmp",
      "hmi-equipment/crashes/pending/c.dmp",
    ]);
  });

  test("a session whose VS Code wrote no crash folder has no dumps", () => {
    // The usual session crashes nothing. Catches an error raised for the missing folder.
    assert.deepStrictEqual(crashDumps(path.join(out, "first-program", "crashes"), out), []);
  });

  test("a failed session names its dumps after its failure, and only when there are any", () => {
    // Catches a failure whose dumps are left out, and one that names dumps it does not have.
    const failure = "Test run terminated with signal SIGTRAP";
    assert.strictEqual(
      withCrashDumps(failure, ["hmi-equipment/crashes/completed/a.dmp"]),
      "Test run terminated with signal SIGTRAP; crash dumps: hmi-equipment/crashes/completed/a.dmp",
    );
    assert.strictEqual(withCrashDumps(failure, []), failure);
  });
});
