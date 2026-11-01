// The "Try it yourself" experiments of the control-pid-commissioning walkthrough: on the unmodified example, Start and
// Identify plant (Automatic), then two requirement sets with PI and the Automatic method: settling in 6 s (24 s validation),
// where the design rule leaves only the Faster response, and settling in 30 s (60 s validation), where Automatic takes the
// More robust one. Each is tuned and validated, and the journey records what Tune and Validate show; the README quotes these
// results.
import { assertNoFailedStep, open, Session, step } from "../session";
import { identify, openStudio, requirements, startSimulator, tuneAndValidate } from "../studio";

suite("Try it yourself: other settling times", () => {
  let s: Session;
  suiteSetup(async () => { s = await open(process.env.E2E_SESSION || "pid-try"); });

  test("tunes and validates for 6 s and for 30 s", async () => {
    let studio = "";
    await step(s, "running", () => startSimulator(s));
    await step(s, "identified", async () => {
      studio = await openStudio(s);
      return identify(s, studio);
    });
    await step(s, "six-seconds", () => tuneAndValidate(s, studio, requirements("6 s", "24 s")));
    await step(s, "thirty-seconds", () => tuneAndValidate(s, studio, requirements("30 s", "60 s")));
    assertNoFailedStep(s);
  });
});
