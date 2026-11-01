// The slower-heater "Try it yourself" experiment of the control-pid-commissioning walkthrough: the runner sets the simulated
// heater's time_constant_s from 4.0 to 8.0 in its copy; the journey starts it, identifies the plant (Automatic), and tunes and
// validates PI with the walkthrough's own requirements (12 s, 24 s validation). It records what Identify, Tune and Validate
// show; the README quotes these results, in which the candidate meets every requirement.
import { assertNoFailedStep, open, Session, step } from "../session";
import { expectCandidatePasses, identify, openStudio, requirements, startSimulator, tuneAndValidate } from "../studio";

suite("Try it yourself: a slower heater", () => {
  let s: Session;
  suiteSetup(async () => { s = await open(process.env.E2E_SESSION || "pid-slow"); });

  test("identifies the slower heater, then tunes and validates for 12 s", async () => {
    let studio = "";
    await step(s, "running", () => startSimulator(s));
    await step(s, "identified", async () => {
      studio = await openStudio(s);
      return identify(s, studio);
    });
    await step(s, "twelve-seconds", async () => {
      const result = await tuneAndValidate(s, studio, requirements("12 s", "24 s"));
      expectCandidatePasses(result.rows);
      return result;
    });
    assertNoFailedStep(s);
  });
});
